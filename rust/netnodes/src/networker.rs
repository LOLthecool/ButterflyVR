use bitvec::order::Lsb0;
use bitvec::vec::BitVec;
use boring::asn1::Asn1Integer;
use boring::asn1::Asn1Time;
use boring::bn::BigNum;
use boring::ec::EcGroup;
use boring::ec::EcKey;
use boring::hash::MessageDigest;
use boring::nid::Nid;
use boring::pkey::PKey;
use boring::ssl::SslContextBuilder;
use boring::ssl::SslMethod;
use boring::x509::X509;
use boring::x509::X509NameBuilder;
use boring::x509::extension::BasicConstraints;
use boring::x509::extension::SubjectAlternativeName;
use bytes::{Bytes, BytesMut};
use godot::global::godot_error;
use quiche::Config;
use quiche::Connection;
use quiche::ConnectionId;
use quiche::RecvInfo;
use quiche::SendInfo;
use quiche::StreamIter;
use rand::TryRng;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::mem;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::{self};
use std::time::{Duration, Instant};

use crate::common::NetNodesConnectionId;

pub const MAX_DATAGRAM_SIZE: usize = 1350;
const PACKET_QUEUE_CAPACITY: usize = 1024;
pub const MAX_CLIENT_CONNECTIONS: usize = 256;

#[derive(Debug)]
#[allow(dead_code)]
pub enum NetNodesError {
    NotSupported,
    PeerNotFound,
    Disconnected,
    BufferFull,
    InvalidDatagramLength,
    InvalidDatagram,
    ThreadPanic,
    QuicheError(quiche::Error),
    SocketError(std::io::Error),
}

impl PartialEq for NetNodesError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::NotSupported, Self::NotSupported)
            | (Self::PeerNotFound, Self::PeerNotFound)
            | (Self::Disconnected, Self::Disconnected)
            | (Self::BufferFull, Self::BufferFull)
            | (Self::InvalidDatagramLength, Self::InvalidDatagramLength)
            | (Self::InvalidDatagram, Self::InvalidDatagram)
            | (Self::ThreadPanic, Self::ThreadPanic)
            | (Self::QuicheError(_), Self::QuicheError(_))
            | (Self::SocketError(_), Self::SocketError(_)) => true,

            (Self::NotSupported, _)
            | (_, Self::PeerNotFound)
            | (_, Self::Disconnected)
            | (_, Self::BufferFull)
            | (_, Self::InvalidDatagramLength)
            | (_, Self::InvalidDatagram)
            | (_, Self::ThreadPanic)
            | (_, Self::QuicheError(_))
            | (_, Self::SocketError(_))
            | (_, Self::NotSupported) => false,
        }
    }
}

impl From<quiche::Error> for NetNodesError {
    fn from(err: quiche::Error) -> Self {
        Self::QuicheError(err)
    }
}

type NetworkerThread = thread::JoinHandle<Result<(), NetNodesError>>;

#[derive(Debug)]
struct UDPListener {
    send: SyncSender<(Bytes, SendInfo)>,
    recv: Receiver<(Bytes, SocketAddr)>,
    send_thread: NetworkerThread,
    recv_thread: NetworkerThread,
    bind_addr: SocketAddr,
    pacing_notifier: Receiver<()>,
    socket: Arc<UdpSocket>,
}

impl UDPListener {
    fn new_client(bind_addr: SocketAddr, server_addr: SocketAddr) -> Self {
        let socket = Arc::new(UdpSocket::bind(bind_addr).unwrap());
        socket.connect(server_addr).unwrap();

        let (send_tx, send_rx) = sync_channel(PACKET_QUEUE_CAPACITY);
        let (recv_tx, recv_rx) = sync_channel(PACKET_QUEUE_CAPACITY);

        let (excessive_pacing_notifier_tx, excessive_pacing_notifier_rx) = sync_channel(1);

        let (send_thread, recv_thread) = Self::spawn_threads(
            send_rx,
            recv_tx,
            excessive_pacing_notifier_tx,
            socket.clone(),
        );

        Self {
            send: send_tx,
            recv: recv_rx,
            send_thread,
            recv_thread,
            bind_addr: socket.local_addr().unwrap(),
            pacing_notifier: excessive_pacing_notifier_rx,
            socket,
        }
    }

    fn new_server(bind_addr: SocketAddr) -> Self {
        let socket = Arc::new(UdpSocket::bind(bind_addr).unwrap());

        let (send_tx, send_rx) = sync_channel(PACKET_QUEUE_CAPACITY);
        let (recv_tx, recv_rx) = sync_channel(PACKET_QUEUE_CAPACITY);

        let (excessive_pacing_notifier_tx, excessive_pacing_notifier_rx) = sync_channel(1);

        let (send_thread, recv_thread) = Self::spawn_threads(
            send_rx,
            recv_tx,
            excessive_pacing_notifier_tx,
            socket.clone(),
        );

        Self {
            send: send_tx,
            recv: recv_rx,
            send_thread,
            recv_thread,
            bind_addr: socket.local_addr().unwrap(),
            pacing_notifier: excessive_pacing_notifier_rx,
            socket,
        }
    }

    fn spawn_threads(
        send_rx: Receiver<(Bytes, SendInfo)>,
        recv_tx: SyncSender<(Bytes, SocketAddr)>,
        excessive_pacing_notifier_tx: SyncSender<()>,
        socket: Arc<UdpSocket>,
    ) -> (NetworkerThread, NetworkerThread) {
        (
            Self::spawn_send_thread(send_rx, excessive_pacing_notifier_tx, socket.clone()),
            Self::spawn_recv_thread(recv_tx, socket),
        )
    }

    fn spawn_send_thread(
        send_rx: Receiver<(Bytes, SendInfo)>,
        excessive_pacing_notifier_tx: SyncSender<()>,
        socket: Arc<UdpSocket>,
    ) -> NetworkerThread {
        let socket_ref = socket.clone();

        thread::spawn(move || {
            Self::sending_thread(&send_rx, &excessive_pacing_notifier_tx, &socket_ref)
        })
    }

    fn spawn_recv_thread(
        recv_tx: SyncSender<(Bytes, SocketAddr)>,
        socket: Arc<UdpSocket>,
    ) -> NetworkerThread {
        thread::spawn(move || Self::receiving_thread(&recv_tx, &socket))
    }

    fn is_running(&self) -> bool {
        (!self.send_thread.is_finished()) && (!self.recv_thread.is_finished())
    }

    fn get_errors_and_reset(&mut self) -> (Option<NetNodesError>, Option<NetNodesError>) {
        let socket = self.socket.clone();

        let (send_tx, send_rx);
        let (recv_tx, recv_rx);

        let (excessive_pacing_notifier_tx, excessive_pacing_notifier_rx);

        let send_thread;
        let recv_thread;

        let mut send_error = None;
        let mut recv_error = None;

        if self.send_thread.is_finished() {
            (send_tx, send_rx) = sync_channel(PACKET_QUEUE_CAPACITY);
            (excessive_pacing_notifier_tx, excessive_pacing_notifier_rx) = sync_channel(1);

            send_thread =
                Self::spawn_send_thread(send_rx, excessive_pacing_notifier_tx, socket.clone());

            send_error = mem::replace(&mut self.send_thread, send_thread)
                .join()
                .unwrap_or(Err(NetNodesError::ThreadPanic))
                .err();

            self.send = send_tx;
            self.pacing_notifier = excessive_pacing_notifier_rx;
        }

        if self.recv_thread.is_finished() {
            (recv_tx, recv_rx) = sync_channel(PACKET_QUEUE_CAPACITY);

            recv_thread = Self::spawn_recv_thread(recv_tx, socket);

            recv_error = mem::replace(&mut self.recv_thread, recv_thread)
                .join()
                .unwrap_or(Err(NetNodesError::ThreadPanic))
                .err();

            self.recv = recv_rx;
        }

        (send_error, recv_error)
    }

    fn receiving_thread(
        outgoing: &SyncSender<(Bytes, SocketAddr)>,
        socket: &Arc<UdpSocket>,
    ) -> Result<(), NetNodesError> {
        loop {
            let mut buffer = BytesMut::zeroed(MAX_DATAGRAM_SIZE);
            let (len, from) = socket
                .recv_from(&mut buffer)
                .map_err(NetNodesError::SocketError)?;
            buffer.truncate(len);
            let packet = buffer.freeze();
            outgoing.send((packet, from)).unwrap();
        }
    }

    fn sending_thread(
        incoming: &Receiver<(Bytes, SendInfo)>,
        excessive_pacing_notifier: &SyncSender<()>,
        socket: &Arc<UdpSocket>,
    ) -> Result<(), NetNodesError> {
        // generally we don't want to be queuing packets to send across multiple ticks
        // better to just send less data per frame in the priority accumulator
        const MAX_PACING_DELAY: Duration = Duration::from_millis(17);

        let mut delayed_packets: VecDeque<(Bytes, SendInfo)> =
            VecDeque::with_capacity(PACKET_QUEUE_CAPACITY);

        loop {
            let now = Instant::now();

            while delayed_packets.front().is_some_and(|x| x.1.at <= now) {
                let (packet, info) = delayed_packets.pop_front().unwrap();
                socket
                    .send_to(&packet, info.to)
                    .map_err(NetNodesError::SocketError)?;
            }

            let (packet, info) = match incoming.recv_timeout(
                delayed_packets
                    .iter()
                    .fold(Instant::now() + Duration::from_millis(16), |acc, x| {
                        acc.min(x.1.at)
                    })
                    .saturating_duration_since(now),
            ) {
                Ok(packet_info) => packet_info,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err(NetNodesError::ThreadPanic),
            };

            if info.at <= now {
                socket
                    .send_to(&packet, info.to)
                    .map_err(NetNodesError::SocketError)?;
            } else if delayed_packets.len() > PACKET_QUEUE_CAPACITY
                || info.at.saturating_duration_since(now) > MAX_PACING_DELAY
            {
                // only check for excessive pacing on a per tick basis
                // so we dont care beyond a single event getting through
                let _ = excessive_pacing_notifier.try_send(());
            } else {
                delayed_packets.push_back((packet, info));
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
    ring::hmac::Key,
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
    pub fn update(&mut self) -> Result<(), NetNodesError> {
        let (mut send_error, mut recv_error) = (None, None);

        if !self.listener.is_running() {
            (send_error, recv_error) = self.listener.get_errors_and_reset();
        }

        match self.handler {
            HandlerType::Server(ref mut data) => {
                Self::update_server(data, &self.listener)?;
            }

            HandlerType::Client(ref mut data) => {
                Self::update_client(data, &self.listener)?;
            }
        }

        if let Some(error) = send_error {
            return Err(error);
        }
        if let Some(error) = recv_error {
            return Err(error);
        }
        Ok(())
    }

    fn update_server(data: &mut ServerState, listener: &UDPListener) -> Result<(), NetNodesError> {
        for client in data.0.values_mut() {
            client.conn.on_timeout();
        }

        for (packet, source_addr) in listener.recv.try_iter() {
            let mut packet: BytesMut = packet.into();
            let hdr = match quiche::Header::from_slice(&mut packet, quiche::MAX_CONN_ID_LEN) {
                Ok(v) => v,
                Err(e) => {
                    godot_error!("Failed to parse header: {e:?}");
                    continue;
                }
            };

            let dcid = ConnectionId::from_ref(hdr.dcid.clone().first_chunk::<16>().unwrap()).into();

            let length = data.0.len();

            let client = match data.0.entry(dcid) {
                Entry::Occupied(entry) => entry.into_mut(),

                Entry::Vacant(entry) => {
                    if hdr.ty != quiche::Type::Initial {
                        godot_error!("got non initial packet from new client");
                        continue;
                    }

                    if hdr.version != quiche::PROTOCOL_VERSION {
                        godot_error!("Unsupported protocol version: {}", hdr.version);
                        continue;
                    }

                    if let Some(block) = data.1.get_mut(&source_addr)
                        && block.block_expiry > Instant::now()
                    {
                        godot_error!("Blocked connection from {source_addr:?}");
                        continue;
                    }

                    if length >= MAX_CLIENT_CONNECTIONS {
                        godot_error!("Max client connections reached");
                        continue;
                    }
                    entry.insert(Self::create_client(source_addr, hdr.dcid, listener)?)
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
    ) -> Result<(), NetNodesError> {
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
                return Err(NetNodesError::Disconnected);
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
        dcid: quiche::ConnectionId,
        listener: &UDPListener,
    ) -> Result<PeerConnection, NetNodesError> {
        let scid = NetNodesConnectionId::from(dcid.clone());
        let scid = ConnectionId::from(scid);

        let mut conn = quiche::accept(
            &scid,
            None,
            listener.bind_addr,
            source_addr,
            &mut Self::get_config_server(),
        )?;

        #[cfg(debug_assertions)]
        match std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(format!("/tmp/butterfly-server-{scid:?}.qlog"))
        {
            Ok(file) => {
                conn.set_qlog(
                    Box::new(file),
                    "butteryfly-rs client connection".to_string(),
                    format!("cid={scid:?}"),
                );
            }
            Err(e) => {
                godot_error!("qlog: could not open log file: {e}");
            }
        }

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
    ) -> Result<(), NetNodesError> {
        loop {
            let mut out = BytesMut::zeroed(MAX_DATAGRAM_SIZE);
            match connection.conn.send(&mut out) {
                Ok((length, info)) => {
                    out.truncate(length);
                    let out = Bytes::from(out);
                    sender
                        .send((out, info))
                        .map_err(|_| NetNodesError::ThreadPanic)?;
                }
                Err(quiche::Error::Done) => {
                    return Ok(());
                }
                Err(e) => {
                    return Err(NetNodesError::QuicheError(e));
                }
            }
        }
    }

    fn recv_packet(
        from: SocketAddr,
        mut packet: BytesMut,
        connection: &mut PeerConnection,
        bind_addr: SocketAddr,
    ) -> Result<(), NetNodesError> {
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
                c.conn.dgram_max_writable_len().map_or(0, |x| x * 8)
            }
            HandlerType::Server(ref s) => s.0.get(peer).map_or(0, |peer| {
                peer.conn.dgram_max_writable_len().map_or(0, |x| x * 8)
            }),
        }
    }

    pub fn is_connection_pacing(&self) -> bool {
        self.listener.pacing_notifier.try_recv().is_ok()
    }

    pub fn get_connection_rtt(&self, peer: &NetNodesConnectionId) -> Option<Duration> {
        match self.handler {
            HandlerType::Client(ref c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                c.conn.path_stats().map(|x| x.rtt).max()
            }
            HandlerType::Server(ref s) => {
                s.0.get(peer)
                    .and_then(|peer| peer.conn.path_stats().map(|x| x.rtt).max())
            }
        }
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
    ) -> std::result::Result<(), NetNodesError> {
        data.set_uninitialized(false);

        let data_len = data.len().next_multiple_of(8) / 8;

        let data: Vec<u8> = data
            .into_vec()
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .take(data_len)
            .collect();

        match self.handler {
            HandlerType::Client(ref mut c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                Self::send_inner(c, stream_id, &data)
            }
            HandlerType::Server(ref mut s) => {
                s.0.get_mut(peer)
                    .map_or(Err(NetNodesError::PeerNotFound), |peer| {
                        Self::send_inner(peer, stream_id, &data)
                    })
            }
        }
    }

    fn send_inner(
        conn: &mut PeerConnection,
        stream_id: u64,
        data: &[u8],
    ) -> std::result::Result<(), NetNodesError> {
        let size: u64 = data.len() as u64;

        match conn.conn.stream_writable(stream_id, size as usize) {
            Ok(true) | Err(quiche::Error::InvalidStreamState(_)) => {}
            Ok(false) => return Err(NetNodesError::BufferFull),
            Err(e) => return Err(NetNodesError::QuicheError(e)),
        }

        conn.conn
            .stream_send(stream_id, &size.to_le_bytes(), false)?;

        match conn.conn.stream_send(stream_id, data, false) {
            Ok(length) => {
                if length == data.len() {
                    Ok(())
                } else {
                    panic!(
                        "failed to send entire message despite stream being writable. this is irrecoverable."
                    )
                }
            }
            Err(e) => panic!("error while sending mesage body: {e:?}. this is irrecoverable."),
        }
    }

    pub fn recv_stream(
        &mut self,
        peer: &NetNodesConnectionId,
        stream_id: u64,
        chunk_length_bytes: usize,
    ) -> std::result::Result<BitVec<u64, Lsb0>, NetNodesError> {
        let mut buf = BytesMut::zeroed(chunk_length_bytes);
        let recv_length: usize;

        match self.handler {
            HandlerType::Client(ref mut c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                match Self::recv_inner(c, stream_id, &mut buf) {
                    Ok(length) => {
                        recv_length = length;
                    }
                    Err(e) => return Err(e),
                }
            }
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(peer) {
                    match Self::recv_inner(peer, stream_id, &mut buf) {
                        Ok(length) => {
                            recv_length = length;
                        }
                        Err(e) => return Err(e),
                    }
                } else {
                    return Err(NetNodesError::PeerNotFound);
                }
            }
        }

        buf.resize(buf.len().next_multiple_of(8), 0);

        let mut buf = Self::packet_to_bits(&buf.freeze())?;
        buf.truncate(chunk_length_bytes.min(recv_length) * 8);

        Ok(buf)
    }

    pub fn recv_stream_bytes(
        &mut self,
        peer: &NetNodesConnectionId,
        stream_id: u64,
        chunk_length_bytes: usize,
    ) -> std::result::Result<Vec<u8>, NetNodesError> {
        let mut buf = BytesMut::zeroed(chunk_length_bytes);
        let recv_length: usize;

        match self.handler {
            HandlerType::Client(ref mut c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                match Self::recv_inner(c, stream_id, &mut buf) {
                    Ok(length) => {
                        recv_length = length;
                    }
                    Err(e) => return Err(e),
                }
            }
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(peer) {
                    match Self::recv_inner(peer, stream_id, &mut buf) {
                        Ok(length) => {
                            recv_length = length;
                        }
                        Err(e) => return Err(e),
                    }
                } else {
                    return Err(NetNodesError::PeerNotFound);
                }
            }
        }

        buf.truncate(recv_length);

        Ok(buf.to_vec())
    }

    fn packet_to_bits(buf: &Bytes) -> Result<BitVec<u64, Lsb0>, NetNodesError> {
        const CHUNK_REQUIRED_ALIGNMENT: usize = 8;

        if !buf.len().is_multiple_of(CHUNK_REQUIRED_ALIGNMENT) {
            return Err(NetNodesError::InvalidDatagram);
        }

        let (buf, []) = buf.as_chunks::<8>() else {
            unreachable!()
        };

        let buf: Vec<u64> = buf.iter().map(|x| u64::from_le_bytes(*x)).collect();

        Ok(BitVec::from_vec(buf))
    }

    fn recv_inner(
        conn: &mut PeerConnection,
        stream_id: u64,
        buf: &mut BytesMut,
    ) -> std::result::Result<usize, NetNodesError> {
        match conn.conn.stream_recv(stream_id, buf) {
            Ok((length, _)) => Ok(length),
            Err(quiche::Error::Done) => Ok(0),
            Err(e) => Err(NetNodesError::QuicheError(e)),
        }
    }

    pub fn send_datagram(
        &mut self,
        peer: &NetNodesConnectionId,
        mut data: BitVec<u64, Lsb0>,
    ) -> std::result::Result<(), NetNodesError> {
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
                || Err(NetNodesError::PeerNotFound),
                |peer| Self::send_dgram_inner(peer, data),
            ),
        }
    }

    fn send_dgram_inner(
        conn: &mut PeerConnection,
        data: Vec<u8>,
    ) -> std::result::Result<(), NetNodesError> {
        if data.len() > conn.conn.dgram_max_writable_len().unwrap_or(0) {
            return Err(NetNodesError::InvalidDatagramLength);
        }
        conn.conn
            .dgram_send_buf(data)
            .map_err(NetNodesError::QuicheError)
    }

    pub fn recv_datagram(
        &mut self,
        peer: &NetNodesConnectionId,
    ) -> std::result::Result<BitVec<u64, Lsb0>, NetNodesError> {
        let mut dgram = match self.handler {
            HandlerType::Client(ref mut c) => {
                let peer: ConnectionId = peer.into();
                debug_assert_eq!(peer, c.id);
                c.conn.dgram_recv_buf()?
            }
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(peer) {
                    peer.conn.dgram_recv_buf()?
                } else {
                    return Err(NetNodesError::PeerNotFound);
                }
            }
        };
        let old_len = dgram.len();
        dgram.resize(old_len.next_multiple_of(8), 0);
        let mut dgram = Self::packet_to_bits(&Bytes::from(dgram))?;
        dgram.truncate(old_len * 8);
        Ok(dgram)
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
                    peer.state = PeerState::Disconnected;
                }
            }
        }
    }

    pub fn new_client(server_addr: SocketAddr) -> Self {
        let mut config = Self::get_config_client();
        let mut id = vec![0u8; quiche::MAX_CONN_ID_LEN];
        rand::rngs::SysRng.try_fill_bytes(&mut id).unwrap();
        let id = ConnectionId::from_vec(id);
        let listener =
            UDPListener::new_client(SocketAddr::new("0.0.0.0".parse().unwrap(), 0), server_addr);
        let mut conn =
            quiche::connect(None, &id, listener.bind_addr, server_addr, &mut config).unwrap();

        match std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("/tmp/butterfly.qlog")
        {
            Ok(file) => {
                #[cfg(debug_assertions)]
                conn.set_qlog(
                    Box::new(file),
                    "butteryfly-rs client connection".to_string(),
                    format!("cid={id:?}"),
                );
            }
            Err(e) => {
                godot_error!("qlog: could not open log file: {e}");
            }
        }

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
                ring::hmac::Key::generate(
                    ring::hmac::HMAC_SHA256,
                    &ring::rand::SystemRandom::new(),
                )
                .unwrap(),
            )),
            listener: UDPListener::new_server(SocketAddr::new(
                "0.0.0.0".parse().unwrap(),
                target_port,
            )),
        }
    }

    fn get_config_server() -> Config {
        let mut builder = X509::builder().unwrap();

        builder.set_version(2).unwrap();

        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let ec_key = EcKey::generate(&group).unwrap();
        let pkey = PKey::from_ec_key(ec_key).unwrap();

        let mut name_builder = X509NameBuilder::new().unwrap();
        name_builder.append_entry_by_text("C", "UK").unwrap();
        name_builder
            .append_entry_by_text("O", "ButterflyVR")
            .unwrap();
        name_builder
            .append_entry_by_text("CN", "instance.butterflyvr.net")
            .unwrap();
        let name = name_builder.build();

        let serial_bn = BigNum::from_u32(rand::random()).unwrap();
        let serial = Asn1Integer::from_bn(&serial_bn).unwrap();
        builder.set_serial_number(&serial).unwrap();

        let not_before = Asn1Time::days_from_now(0).unwrap();
        let not_after = Asn1Time::days_from_now(365).unwrap();
        builder.set_not_before(&not_before).unwrap();
        builder.set_not_after(&not_after).unwrap();

        builder.set_subject_name(&name).unwrap();
        builder.set_issuer_name(&name).unwrap();

        builder.set_pubkey(&pkey).unwrap();

        let basic_constraints = BasicConstraints::new().critical().ca().build().unwrap();
        builder.append_extension(basic_constraints).unwrap();

        let ctx = builder.x509v3_context(None, None);

        let san = SubjectAlternativeName::new()
            .dns("localhost")
            .ip("127.0.0.1")
            .build(&ctx)
            .unwrap();
        builder.append_extension(san).unwrap();

        builder.sign(&pkey, MessageDigest::sha256()).unwrap();

        let cert = builder.build();

        let mut context: SslContextBuilder = SslContextBuilder::new(SslMethod::tls()).unwrap();
        context.set_private_key(&pkey).unwrap();

        context.set_certificate(&cert).unwrap();

        let mut config =
            Config::with_boring_ssl_ctx_builder(quiche::PROTOCOL_VERSION, context).unwrap();

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
    fn get_config_client() -> Config {
        let mut config = Config::new(quiche::PROTOCOL_VERSION).unwrap();
        config.verify_peer(false);
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
}
