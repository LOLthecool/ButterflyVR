use bitvec::order::Lsb0;
use bitvec::vec::BitVec;
use boring::ssl::SslContextBuilder;
use boring::ssl::SslMethod;
use boring::ssl::SslVerifyMode;
use boring::ssl::SslVersion;
use bytes::{Bytes, BytesMut};
use quiche::Config;
use quiche::Connection;
use quiche::ConnectionId;
use quiche::RecvInfo;
use quiche::SendInfo;
use quiche::StreamIter;
use ring::rand::SecureRandom;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::{self};
use std::time::{Duration, Instant};

use crate::common::NetNodesConnectionId;

pub const MAX_DATAGRAM_SIZE: usize = 1350;
const PACKET_QUEUE_CAPACITY: usize = 1024;
pub const MAX_CLIENT_CONNECTIONS: usize = 256;

#[derive(Debug)]
pub enum ConnectionError {
    InvalidHandlerType,
    PeerNotFound,
    BufferFull,
    InvalidDatagramLength,
    InvalidDatagram,
    #[allow(dead_code)]
    QuicheError(quiche::Error),
    #[allow(dead_code)]
    GenericError(Box<dyn Debug + Send>),
}

impl<T: std::error::Error + Send + 'static> From<Box<T>> for ConnectionError {
    fn from(err: Box<T>) -> Self {
        Self::GenericError(err)
    }
}

impl From<quiche::Error> for ConnectionError {
    fn from(err: quiche::Error) -> Self {
        Self::QuicheError(err)
    }
}

#[derive(Debug)]
struct UDPListener {
    send: SyncSender<(Bytes, SendInfo)>,
    recv: Receiver<(Bytes, SocketAddr)>,
    bind_addr: SocketAddr,
    pacing_notifier: Receiver<()>,
}

impl UDPListener {
    fn new_client(bind_addr: SocketAddr, server_addr: SocketAddr) -> Self {
        let socket = Arc::new(UdpSocket::bind(bind_addr).unwrap());
        socket.connect(server_addr).unwrap();

        let (send_tx, send_rx) = sync_channel(PACKET_QUEUE_CAPACITY);
        let (recv_tx, recv_rx) = sync_channel(PACKET_QUEUE_CAPACITY);

        let (excessive_pacing_notifier_tx, excessive_pacing_notifier_rx) = sync_channel(1);

        let socket_ref = socket.clone();
        thread::spawn(move || {
            Self::sending_thread(&send_rx, &excessive_pacing_notifier_tx, &socket_ref);
        });

        let socket_ref = socket;
        thread::spawn(move || {
            Self::receiving_thread(&recv_tx, &socket_ref);
        });

        Self {
            send: send_tx,
            recv: recv_rx,
            bind_addr,
            pacing_notifier: excessive_pacing_notifier_rx,
        }
    }

    fn new_server(bind_addr: SocketAddr) -> Self {
        let socket = Arc::new(UdpSocket::bind(bind_addr).unwrap());

        let (send_tx, send_rx) = sync_channel(PACKET_QUEUE_CAPACITY);
        let (recv_tx, recv_rx) = sync_channel(PACKET_QUEUE_CAPACITY);

        let (excessive_pacing_notifier_tx, excessive_pacing_notifier_rx) = sync_channel(1);

        let socket_ref = socket.clone();
        thread::spawn(move || {
            Self::sending_thread(&send_rx, &excessive_pacing_notifier_tx, &socket_ref);
        });

        let socket_ref = socket;
        thread::spawn(move || {
            Self::receiving_thread(&recv_tx, &socket_ref);
        });

        Self {
            send: send_tx,
            recv: recv_rx,
            bind_addr,
            pacing_notifier: excessive_pacing_notifier_rx,
        }
    }

    fn receiving_thread(outgoing: &SyncSender<(Bytes, SocketAddr)>, socket: &Arc<UdpSocket>) {
        loop {
            let mut buffer = BytesMut::zeroed(MAX_DATAGRAM_SIZE);
            let (len, from) = socket.recv_from(&mut buffer).unwrap();
            buffer.truncate(len);
            let packet = buffer.freeze();
            outgoing.send((packet, from)).unwrap();
        }
    }

    fn sending_thread(
        incoming: &Receiver<(Bytes, SendInfo)>,
        excessive_pacing_notifier: &SyncSender<()>,
        socket: &Arc<UdpSocket>,
    ) {
        // generally we don't want to be queuing packets to send across multiple ticks
        // better to just send less data per frame in the priority accumulator
        const MAX_PACING_DELAY: Duration = Duration::from_millis(17);

        let mut delayed_packets: VecDeque<(Bytes, SendInfo)> =
            VecDeque::with_capacity(PACKET_QUEUE_CAPACITY);

        loop {
            let now = Instant::now();

            let (packet, info) = incoming.recv().unwrap();

            if info.at <= now {
                socket.send_to(&packet, info.to).unwrap();
            } else if delayed_packets.len() > PACKET_QUEUE_CAPACITY
                || info.at.saturating_duration_since(now) > MAX_PACING_DELAY
            {
                // only check for excessive pacing on a per tick basis
                // so we dont care beyond a single event getting through
                let _ = excessive_pacing_notifier.try_send(());
            } else {
                delayed_packets.push_back((packet, info));
            }

            while delayed_packets.front().is_some_and(|x| x.1.at <= now) {
                let (packet, info) = delayed_packets.pop_front().unwrap();
                socket.send_to(&packet, info.to).unwrap();
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerState {
    AwaitingConnection,
    Connected,
    Disconnected,
}

struct PeerConnection {
    id: ConnectionId<'static>,
    conn: Connection,
    state: PeerState,
    peer_addr: SocketAddr,
}

impl Debug for PeerConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerConnection")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("peer_addr", &self.peer_addr)
            .finish_non_exhaustive()
    }
}

type ServerState = (
    HashMap<NetNodesConnectionId, PeerConnection>,
    HashMap<SocketAddr, BlockedConnection>,
    Arc<Mutex<HashMap<[u8; 8], [u8; 32]>>>,
);

#[derive(Debug)]
enum HandlerType {
    Server(ServerState),
    Client(Box<PeerConnection>),
}

#[derive(Debug)]
struct BlockedConnection {
    block_count: u64,
    block_expiry: Instant,
}

#[derive(Debug)]
pub struct ConnectionHandler {
    handler: HandlerType,
    listener: UDPListener,
}

impl ConnectionHandler {
    pub fn update(&mut self) -> Result<(), ConnectionError> {
        match self.handler {
            HandlerType::Server(ref mut data) => {
                Self::update_server(data, &mut self.listener)?;
            }

            HandlerType::Client(ref mut data) => {
                Self::update_client(data, &mut self.listener)?;
            }
        }
        Ok(())
    }

    fn update_server(
        data: &mut ServerState,
        listener: &UDPListener,
    ) -> Result<(), ConnectionError> {
        for client in data.0.values_mut() {
            client.conn.on_timeout();
        }

        for (packet, source_addr) in listener.recv.try_iter() {
            let mut packet: BytesMut = packet.into();
            let hdr = match quiche::Header::from_slice(&mut packet, quiche::MAX_CONN_ID_LEN) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Failed to parse header: {e:?}");
                    continue;
                }
            };

            let length = data.0.len();

            let client = match data.0.entry(hdr.dcid.into()) {
                Entry::Occupied(entry) => entry.into_mut(),

                Entry::Vacant(entry) => {
                    if hdr.ty != quiche::Type::Initial {
                        continue;
                    }

                    if let Some(block) = data.1.get_mut(&source_addr)
                        && block.block_expiry > Instant::now()
                    {
                        eprintln!("Blocked connection from {source_addr:?}");
                        continue;
                    }

                    if length >= MAX_CLIENT_CONNECTIONS {
                        eprintln!("Max client connections reached");
                        continue;
                    }

                    entry.insert(Self::create_client(source_addr, listener, data.2.clone())?)
                }
            };

            Self::recv_packet(source_addr, packet, client, listener.bind_addr)?;
        }

        for client in data.0.values_mut() {
            match client.state {
                PeerState::AwaitingConnection => {
                    if data
                        .1
                        .get_mut(&client.peer_addr)
                        .and_then(|x| x.block_expiry.checked_duration_since(Instant::now()))
                        .is_some()
                    {
                        client.state = PeerState::Disconnected;
                    }

                    if client.conn.is_closed() {
                        client.state = PeerState::Disconnected;
                        Self::update_blocked_connection(data.1.entry(client.peer_addr));
                    }

                    if client.conn.is_established() {
                        client.state = PeerState::Connected;
                    }
                }

                PeerState::Connected => {
                    if client.conn.is_closed() {
                        client.state = PeerState::Disconnected;
                    }
                }

                PeerState::Disconnected => {}
            }
        }

        data.0
            .retain(|_, client| client.state != PeerState::Disconnected);

        for client in data.0.values_mut() {
            Self::send_packets(client, &listener.send)?;
        }
        Ok(())
    }

    fn update_client(
        data: &mut PeerConnection,
        listener: &UDPListener,
    ) -> Result<(), ConnectionError> {
        data.conn.on_timeout();

        for (packet, source_addr) in listener.recv.try_iter() {
            let packet: BytesMut = packet.into();
            Self::recv_packet(source_addr, packet, data, listener.bind_addr)?;
        }

        match data.state {
            PeerState::AwaitingConnection => {
                if data.conn.is_closed() {
                    data.state = PeerState::Disconnected;
                }

                if data.conn.is_established() {
                    data.state = PeerState::Connected;
                }
            }
            PeerState::Connected => {
                if data.conn.is_closed() {
                    data.state = PeerState::Disconnected;
                }
            }
            PeerState::Disconnected => {
                todo!()
            }
        }

        Self::send_packets(data, &listener.send)?;
        Ok(())
    }

    fn update_blocked_connection(entry: Entry<SocketAddr, BlockedConnection>) {
        entry
            .and_modify(|e| {
                e.block_count += 1;
                e.block_expiry =
                    Instant::now() + Duration::from_secs(e.block_count * e.block_count);
            })
            .or_insert_with(|| BlockedConnection {
                block_count: 1,
                block_expiry: Instant::now() + Duration::from_secs(1),
            });
    }

    fn create_client(
        source_addr: SocketAddr,
        listener: &UDPListener,
        psks: Arc<Mutex<HashMap<[u8; 8], [u8; 32]>>>,
    ) -> Result<PeerConnection, ConnectionError> {
        let mut scid_bytes = [0u8; quiche::MAX_CONN_ID_LEN];
        ring::rand::SystemRandom::new()
            .fill(&mut scid_bytes)
            .unwrap();
        let scid = quiche::ConnectionId::from_vec(scid_bytes.to_vec());

        let ssl_ctx = Self::build_server_ctx(psks).unwrap();

        let conn = quiche::accept(
            &scid,
            None,
            listener.bind_addr,
            source_addr,
            &mut Self::get_config(ssl_ctx),
        )?;

        Ok(PeerConnection {
            id: scid.clone(),
            conn,
            state: PeerState::AwaitingConnection,
            peer_addr: source_addr,
        })
    }

    fn send_packets(
        connection: &mut PeerConnection,
        sender: &SyncSender<(Bytes, SendInfo)>,
    ) -> Result<(), ConnectionError> {
        loop {
            let mut out = BytesMut::zeroed(MAX_DATAGRAM_SIZE);
            match connection.conn.send(&mut out) {
                Ok((length, info)) => {
                    out.truncate(length);
                    let out = Bytes::from(out);
                    sender.send((out, info)).unwrap();
                }
                Err(quiche::Error::Done) => {
                    return Ok(());
                }
                Err(e) => {
                    return Err(ConnectionError::QuicheError(e));
                }
            }
        }
    }

    fn recv_packet(
        from: SocketAddr,
        mut packet: BytesMut,
        connection: &mut PeerConnection,
        bind_addr: SocketAddr,
    ) -> Result<(), ConnectionError> {
        let info = RecvInfo {
            from,
            to: bind_addr,
        };
        connection.conn.recv(&mut packet, info)?;
        Ok(())
    }

    pub fn is_connected(&self, peer: &NetNodesConnectionId) -> bool {
        match self.handler {
            HandlerType::Client(ref c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                c.state == PeerState::Connected
            }
            HandlerType::Server(ref s) => {
                s.0.get(peer)
                    .is_some_and(|peer| peer.state == PeerState::Connected)
            }
        }
    }

    pub fn get_max_dgram_size(&self, peer: &NetNodesConnectionId) -> usize {
        match self.handler {
            HandlerType::Client(ref c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                c.conn.dgram_max_writable_len().map_or(4000, |x| x * 8)
            }
            HandlerType::Server(ref s) => s.0.get(peer).map_or(0, |peer| {
                peer.conn.dgram_max_writable_len().map_or(4000, |x| x * 8)
            }),
        }
    }

    pub fn is_connection_pacing(&self) -> bool {
        self.listener.pacing_notifier.try_recv().is_ok()
    }

    pub fn get_peers(&self, include_connecting: bool) -> Vec<NetNodesConnectionId> {
        match self.handler {
            HandlerType::Client(ref c) => {
                if include_connecting || c.state == PeerState::Connected {
                    vec![c.id.clone().into()]
                } else {
                    vec![]
                }
            }
            HandlerType::Server(ref s) => {
                s.0.iter()
                    .filter_map(|x| {
                        if x.1.state == PeerState::Connected || include_connecting {
                            Some(x.0.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            }
        }
    }

    pub fn get_readable_streams(&self, peer: &NetNodesConnectionId) -> StreamIter {
        match self.handler {
            HandlerType::Client(ref c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                c.conn.readable()
            }
            HandlerType::Server(ref s) => {
                s.0.get(peer)
                    .map_or_else(StreamIter::default, |peer| peer.conn.readable())
            }
        }
    }

    pub fn send_stream(
        &mut self,
        peer: &NetNodesConnectionId,
        stream_id: u64,
        mut data: BitVec<u64, Lsb0>,
    ) -> std::result::Result<(), ConnectionError> {
        data.set_uninitialized(false);

        let data: Vec<u8> = data
            .into_vec()
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect();

        match self.handler {
            HandlerType::Client(ref mut c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                Self::send_inner(c, stream_id, &data)
            }
            HandlerType::Server(ref mut s) => s.0.get_mut(peer).map_or_else(
                || Err(ConnectionError::PeerNotFound),
                |peer| Self::send_inner(peer, stream_id, &data),
            ),
        }
    }

    pub fn send_identifier(
        &mut self,
        identifier: &[u8],
    ) -> std::result::Result<(), ConnectionError> {
        match self.handler {
            HandlerType::Client(ref mut server) => Self::send_inner(server, 0, identifier),
            HandlerType::Server(_) => Err(ConnectionError::InvalidHandlerType),
        }
    }

    fn send_inner(
        conn: &mut PeerConnection,
        stream_id: u64,
        data: &[u8],
    ) -> std::result::Result<(), ConnectionError> {
        let size: u64 = 8 + data.len() as u64;
        conn.conn
            .stream_send(stream_id, &size.to_le_bytes(), false)?;

        match conn.conn.stream_send(stream_id, data, false) {
            Ok(length) => {
                if length == data.len() {
                    Ok(())
                } else {
                    Err(ConnectionError::BufferFull)
                }
            }
            Err(e) => Err(ConnectionError::QuicheError(e)),
        }
    }

    pub fn recv_stream(
        &mut self,
        peer: &NetNodesConnectionId,
        stream_id: u64,
    ) -> std::result::Result<BitVec<u64, Lsb0>, ConnectionError> {
        const STREAM_CHUNK_SIZE: usize = 4096;

        let mut buf = BytesMut::zeroed(STREAM_CHUNK_SIZE);
        let buffer_length: usize;

        match self.handler {
            HandlerType::Client(ref mut c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                match Self::recv_inner(c, stream_id, &mut buf) {
                    Ok(length) => {
                        buffer_length = length;
                    }
                    Err(e) => return Err(e),
                }
            }
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(peer) {
                    match Self::recv_inner(peer, stream_id, &mut buf) {
                        Ok(length) => {
                            buffer_length = length;
                        }
                        Err(e) => return Err(e),
                    }
                } else {
                    return Err(ConnectionError::PeerNotFound);
                }
            }
        }

        let mut buffer_bits = Self::packet_to_bits(&buf.freeze())?;

        buffer_bits.truncate(buffer_length * 8);
        Ok(buffer_bits)
    }

    pub fn recv_stream_bytes(
        &mut self,
        peer: &NetNodesConnectionId,
        stream_id: u64,
        max_length: usize,
    ) -> std::result::Result<BitVec<u8, Lsb0>, ConnectionError> {
        let mut buf = BytesMut::zeroed(max_length);

        match self.handler {
            HandlerType::Client(ref mut c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                match Self::recv_inner(c, stream_id, &mut buf) {
                    Ok(length) => {
                        buf.truncate(length);
                    }
                    Err(e) => return Err(e),
                }
            }
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(peer) {
                    match Self::recv_inner(peer, stream_id, &mut buf) {
                        Ok(length) => {
                            buf.truncate(length);
                        }
                        Err(e) => return Err(e),
                    }
                } else {
                    return Err(ConnectionError::PeerNotFound);
                }
            }
        }

        Ok(Self::unaligned_packet_to_bits(buf))
    }

    fn packet_to_bits(buf: &Bytes) -> Result<BitVec<u64, Lsb0>, ConnectionError> {
        const CHUNK_REQUIRED_ALIGNMENT: usize = 8;

        if !buf.len().is_multiple_of(CHUNK_REQUIRED_ALIGNMENT) {
            return Err(ConnectionError::InvalidDatagram);
        }

        let (buf, []) = buf.as_chunks::<8>() else {
            unreachable!()
        };

        let buf: Vec<u64> = buf.iter().map(|x| u64::from_le_bytes(*x)).collect();

        Ok(BitVec::from_vec(buf))
    }

    fn unaligned_packet_to_bits(buf: BytesMut) -> BitVec<u8, Lsb0> {
        BitVec::from_vec(buf.into())
    }

    fn recv_inner(
        conn: &mut PeerConnection,
        stream_id: u64,
        buf: &mut BytesMut,
    ) -> std::result::Result<usize, ConnectionError> {
        match conn.conn.stream_recv(stream_id, buf) {
            Ok((length, _)) => Ok(length),
            Err(quiche::Error::Done) => Ok(0),
            Err(e) => Err(ConnectionError::QuicheError(e)),
        }
    }

    pub fn send_datagram(
        &mut self,
        peer: &NetNodesConnectionId,
        mut data: BitVec<u64, Lsb0>,
    ) -> std::result::Result<(), ConnectionError> {
        data.set_uninitialized(false);

        let data: Vec<u8> = data
            .into_vec()
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();

        match self.handler {
            HandlerType::Client(ref mut c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                Self::send_dgram_inner(c, data)
            }
            HandlerType::Server(ref mut s) => s.0.get_mut(peer).map_or_else(
                || Err(ConnectionError::PeerNotFound),
                |peer| Self::send_dgram_inner(peer, data),
            ),
        }
    }

    fn send_dgram_inner(
        conn: &mut PeerConnection,
        data: Vec<u8>,
    ) -> std::result::Result<(), ConnectionError> {
        if data.len() > conn.conn.dgram_max_writable_len().unwrap_or(0) {
            return Err(ConnectionError::InvalidDatagramLength);
        }
        conn.conn
            .dgram_send_buf(data)
            .map_err(ConnectionError::QuicheError)
    }

    pub fn recv_datagram(
        &mut self,
        peer: &NetNodesConnectionId,
    ) -> std::result::Result<BitVec<u64, Lsb0>, ConnectionError> {
        let dgram = match self.handler {
            HandlerType::Client(ref mut c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                c.conn.dgram_recv_buf()?
            }
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(peer) {
                    peer.conn.dgram_recv_buf()?
                } else {
                    return Err(ConnectionError::PeerNotFound);
                }
            }
        };
        Self::packet_to_bits(&Bytes::from(dgram))
    }

    pub fn add_client_token(
        &mut self,
        identifier: [u8; 8],
        key: [u8; 32],
    ) -> Result<(), ConnectionError> {
        if let HandlerType::Server(data) = &mut self.handler {
            data.2.lock().unwrap().insert(identifier, key);
            Ok(())
        } else {
            Err(ConnectionError::InvalidHandlerType)
        }
    }

    pub fn disconnect_peer(
        &mut self,
        peer: &NetNodesConnectionId,
        block: bool,
        err_code: u64,
        reason: &str,
    ) {
        match self.handler {
            HandlerType::Client(ref mut c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                let _ = c.conn.close(true, err_code, reason.as_bytes());
            }
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(peer) {
                    if block {
                        Self::update_blocked_connection(s.1.entry(peer.peer_addr));
                    }
                    let _ = peer.conn.close(true, err_code, reason.as_bytes());
                }
            }
        }
    }

    pub fn new_client(
        server_addr: SocketAddr,
        supplied_identity: String,
        supplied_psk: Vec<u8>,
    ) -> Self {
        let ssl_ctx = Self::build_client_ctx(supplied_identity, supplied_psk);
        let mut config = Self::get_config(ssl_ctx);
        let mut id = vec![0u8; quiche::MAX_CONN_ID_LEN];
        ring::rand::SystemRandom::new().fill(&mut id).unwrap();
        let id = ConnectionId::from_vec(id);
        let listener =
            UDPListener::new_client(SocketAddr::new("0.0.0.0".parse().unwrap(), 0), server_addr);
        let conn =
            quiche::connect(None, &id, listener.bind_addr, server_addr, &mut config).unwrap();
        Self {
            handler: HandlerType::Client(Box::new(PeerConnection {
                id,
                conn,
                state: PeerState::AwaitingConnection,
                peer_addr: server_addr,
            })),
            listener,
        }
    }

    pub fn new_server(target_port: u16) -> Self {
        Self {
            handler: HandlerType::Server((
                HashMap::new(),
                HashMap::new(),
                Arc::new(Mutex::new(HashMap::new())),
            )),
            listener: UDPListener::new_server(SocketAddr::new(
                "0.0.0.0".parse().unwrap(),
                target_port,
            )),
        }
    }

    fn get_config(ssl_ctx: SslContextBuilder) -> Config {
        let mut config =
            Config::with_boring_ssl_ctx_builder(quiche::PROTOCOL_VERSION, ssl_ctx).unwrap();
        config.discover_pmtu(true);
        config.set_application_protos(&[b"netnodes-1"]).unwrap();
        config.set_max_idle_timeout(10_000);
        config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_initial_max_data(1_000_000);
        config.set_initial_max_stream_data_bidi_local(900_000);
        config.set_initial_max_stream_data_bidi_remote(900_000);
        config.set_initial_max_stream_data_uni(900_000);
        config.set_initial_max_streams_bidi(10);
        config.set_initial_max_streams_uni(10);
        config.enable_dgram(true, 1000, 1000);
        config.set_disable_active_migration(true);
        config
    }

    fn build_server_ctx(
        psks: Arc<Mutex<HashMap<[u8; 8], [u8; 32]>>>,
    ) -> Result<SslContextBuilder, boring::error::ErrorStack> {
        let mut ctx = SslContextBuilder::new(SslMethod::tls_server())?;

        ctx.set_min_proto_version(Some(SslVersion::TLS1_3))?;
        ctx.set_max_proto_version(Some(SslVersion::TLS1_3))?;

        ctx.set_verify(SslVerifyMode::NONE);

        ctx.set_psk_server_callback(move |_ssl, identity, out| {
            if let Some(Ok(id)) = identity.map(|x| x.try_into())
                && let Entry::Occupied(entry) = psks.lock().unwrap().entry(id)
            {
                let psk = entry.into_mut();
                let key_len = psk.len();
                if out.len() >= key_len {
                    out[..key_len].copy_from_slice(psk);
                    return Ok(key_len);
                }
            }

            Ok(0)
        });

        Ok(ctx)
    }

    fn build_client_ctx(supplied_identity: String, supplied_psk: Vec<u8>) -> SslContextBuilder {
        let mut ctx = SslContextBuilder::new(SslMethod::tls_client()).unwrap();

        ctx.set_min_proto_version(Some(SslVersion::TLS1_3)).unwrap();
        ctx.set_max_proto_version(Some(SslVersion::TLS1_3)).unwrap();

        ctx.set_verify(SslVerifyMode::NONE);

        ctx.set_psk_client_callback(move |_ssl, _hint, identity, psk| {
            let id_bytes = supplied_identity.as_bytes();
            if identity.len() < id_bytes.len() + 1 {
                return Err(boring::error::ErrorStack::get());
            }
            identity[..id_bytes.len()].copy_from_slice(id_bytes);
            identity[id_bytes.len()] = 0;

            let key_len = supplied_psk.len();
            if psk.len() < key_len {
                return Err(boring::error::ErrorStack::get());
            }
            psk[..key_len].copy_from_slice(&supplied_psk);

            Ok(key_len)
        });

        ctx
    }
}

impl Default for ConnectionHandler {
    fn default() -> Self {
        Self {
            handler: HandlerType::Server((
                HashMap::new(),
                HashMap::new(),
                Arc::new(Mutex::new(HashMap::new())),
            )),
            listener: UDPListener::new_server(SocketAddr::new("0.0.0.0".parse().unwrap(), 3444)),
        }
    }
}
