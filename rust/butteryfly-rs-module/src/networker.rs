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
use ring::rand::SecureRandom;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::io;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::{self};
use std::time::{Duration, Instant};

const MAX_DATAGRAM_SIZE: usize = 1350;
const PACKET_QUEUE_CAPACITY: usize = 1000;
const MAX_CLIENT_CONNECTIONS: usize = 256;

enum ConnectionError {
    // todo: replace with specific error types
    Generic(Box<dyn std::error::Error + Send + Sync + 'static>),
}

struct UDPListener {
    send: SyncSender<(Bytes, SendInfo)>,
    recv: Receiver<(Bytes, SocketAddr)>,
    bind_addr: SocketAddr,
    pacing_notifier: Receiver<()>,
}

impl UDPListener {
    fn new_client(bind_addr: SocketAddr, server_addr: SocketAddr) -> Self {
        let socket = UdpSocket::bind(bind_addr).unwrap();
        socket.set_nonblocking(true);
        socket.connect(server_addr).unwrap();

        let (send_tx, send_rx) = sync_channel(PACKET_QUEUE_CAPACITY);
        let (recv_tx, recv_rx) = sync_channel(PACKET_QUEUE_CAPACITY);

        let (excessive_pacing_notifier_tx, excessive_pacing_notifier_rx) = sync_channel(1);

        thread::spawn(move || {
            UDPListener::listening_thread(send_rx, recv_tx, excessive_pacing_notifier_tx, socket);
        });

        Self {
            send: send_tx,
            recv: recv_rx,
            bind_addr,
            pacing_notifier: excessive_pacing_notifier_rx,
        }
    }
    fn new_server(bind_addr: SocketAddr) -> Self {
        let socket = UdpSocket::bind(bind_addr).unwrap();
        socket.set_nonblocking(true);

        let (send_tx, send_rx) = sync_channel(PACKET_QUEUE_CAPACITY);
        let (recv_tx, recv_rx) = sync_channel(PACKET_QUEUE_CAPACITY);

        let (excessive_pacing_notifier_tx, excessive_pacing_notifier_rx) = sync_channel(1);

        thread::spawn(move || {
            UDPListener::listening_thread(send_rx, recv_tx, excessive_pacing_notifier_tx, socket);
        });

        Self {
            send: send_tx,
            recv: recv_rx,
            bind_addr,
            pacing_notifier: excessive_pacing_notifier_rx,
        }
    }

    // retrieves a single packet from the socket, if available
    fn poll(socket: &mut UdpSocket) -> Option<(SocketAddr, BytesMut)> {
        let mut buffer = BytesMut::with_capacity(MAX_DATAGRAM_SIZE);
        match socket.recv_from(&mut buffer) {
            Ok((len, from)) => {
                buffer.truncate(len);
                Some((from, buffer))
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => None,
            Err(e) => panic!("failed to poll udp socket: {:?}", e),
        }
    }
    // todo: check if this needs to handle channel disconnections
    fn listening_thread(
        incoming: Receiver<(Bytes, SendInfo)>,
        outgoing: SyncSender<(Bytes, SocketAddr)>,
        excessive_pacing_notifier: SyncSender<()>,
        mut socket: UdpSocket,
    ) {
        // generally we dont want to be queuing packets to send across multiple ticks
        // better to just send less data per frame in the priority accumulator
        const MAX_PACING_DELAY: u64 = 16;

        let mut delayed_packets: VecDeque<(Bytes, SendInfo)> =
            VecDeque::with_capacity(PACKET_QUEUE_CAPACITY);

        // to avoid blocking we cant always send
        let mut leftover_packet: Option<(Bytes, SocketAddr)> = None;

        loop {
            let now = Instant::now();

            for (packet, info) in incoming.try_iter() {
                if info.at <= now {
                    socket.send_to(&packet, info.to);
                } else {
                    if delayed_packets.len() > PACKET_QUEUE_CAPACITY
                        || info.at.saturating_duration_since(now)
                            > Duration::from_millis(MAX_PACING_DELAY)
                    {
                        // only care about excessive pacing on a per tick basis
                        // so we dont care beyond a single event getting through
                        let _ = excessive_pacing_notifier.try_send(());
                    } else {
                        delayed_packets.push_back((packet, info));
                    }
                }
            }

            while delayed_packets.get(0).map(|x| x.1.at <= now) == Some(true) {
                let (packet, info) = delayed_packets.pop_front().unwrap();
                socket.send_to(&packet, info.to);
            }

            if let Some(packet) = leftover_packet.clone() {
                if outgoing.try_send((packet.0, packet.1)).is_err() {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                } else {
                    leftover_packet = None;
                }
            }

            while let Some((source, packet)) = UDPListener::poll(&mut socket) {
                let packet = packet.freeze();
                if outgoing.try_send((packet.clone(), source)).is_err() {
                    leftover_packet = Some((packet, source));
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PeerState {
    AwaitingConnection,
    AwaitingIdentity(Instant),
    AwaitingVerification,
    EventSync,
    InitObjectSync,
    Connected,
    Disconnected,
}

struct PeerConnection<'a> {
    id: ConnectionId<'a>,
    conn: Connection,
    state: PeerState,
    peer_addr: SocketAddr,
}

enum HandlerType<'a> {
    Server(
        (
            HashMap<quiche::ConnectionId<'a>, PeerConnection<'a>>,
            HashMap<SocketAddr, BlockedConnection>,
            Arc<Mutex<HashMap<String, [u8; 32]>>>,
        ),
    ),
    Client(PeerConnection<'a>, BytesMut),
}

struct BlockedConnection {
    block_count: u64,
    block_expiry: Instant,
}

struct ConnectionHandler<'a> {
    handler: HandlerType<'a>,
    listener: UDPListener,
    unverified_clients: Vec<UnverifiedClients<'a>>,
}

struct UnverifiedClients<'a> {
    id: ConnectionId<'a>,
    nonce: [u8; 16],
    uuid: [u8; 16],
    verified: bool,
}

impl<'a> ConnectionHandler<'a> {
    fn update(&mut self) {
        match self.handler {
            HandlerType::Server(ref mut data) => {
                Self::update_server(data, &mut self.listener, &mut self.unverified_clients);
            }

            HandlerType::Client(ref mut data, ref mut identifier) => {
                Self::update_client(data, identifier, &mut self.listener);
            }
        }
    }

    fn update_server(
        data: &mut (
            HashMap<ConnectionId<'_>, PeerConnection<'a>>,
            HashMap<SocketAddr, BlockedConnection>,
            Arc<Mutex<HashMap<String, [u8; 32]>>>,
        ),
        listener: &mut UDPListener,
        unverified_clients: &mut Vec<UnverifiedClients<'a>>,
    ) {
        for client in data.0.values_mut() {
            client.conn.on_timeout();
        }

        for (packet, source_addr) in listener.recv.try_iter() {
            let mut packet: BytesMut = packet.into();
            let hdr = match quiche::Header::from_slice(&mut packet, quiche::MAX_CONN_ID_LEN) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("Failed to parse header: {:?}", e);
                    continue;
                }
            };

            let length = data.0.len();

            let client = match data.0.entry(hdr.dcid) {
                Entry::Occupied(entry) => entry.into_mut(),

                Entry::Vacant(entry) => {
                    if hdr.ty != quiche::Type::Initial {
                        continue;
                    }

                    if let Some(block) = data.1.get_mut(&source_addr)
                        && block.block_expiry > Instant::now()
                    {
                        eprintln!("Blocked connection from {:?}", source_addr);
                        continue;
                    }

                    if length >= MAX_CLIENT_CONNECTIONS {
                        eprintln!("Max client connections reached");
                        continue;
                    }

                    entry.insert(ConnectionHandler::create_client(
                        source_addr,
                        &listener,
                        data.2.clone(),
                    ))
                }
            };

            ConnectionHandler::recv_packet(source_addr, packet, client, listener.bind_addr);
        }

        for client in data.0.values_mut() {
            match client.state {
                PeerState::AwaitingConnection => {
                    if client.conn.is_closed() {
                        client.state = PeerState::Disconnected;
                        ConnectionHandler::update_blocked_connection(
                            data.1.entry(client.peer_addr),
                        );
                    }

                    if client.conn.is_established() {
                        client.state =
                            PeerState::AwaitingIdentity(Instant::now() + Duration::from_secs(3));
                    }
                }

                PeerState::AwaitingIdentity(timeout) => {
                    if client.conn.is_closed() || Instant::now() > timeout {
                        client.state = PeerState::Disconnected;
                        ConnectionHandler::update_blocked_connection(
                            data.1.entry(client.peer_addr),
                        );
                    }

                    if client.conn.stream_readable(0) {
                        let mut buf = [0u8; 32];
                        let _ = client.conn.stream_recv(0, &mut buf);

                        let nonce = buf[..16].try_into().unwrap();
                        let uuid = buf[16..].try_into().unwrap();

                        unverified_clients.push(UnverifiedClients {
                            id: client.id.clone(),
                            nonce,
                            uuid,
                            verified: false,
                        });

                        client.state = PeerState::AwaitingVerification;
                    }
                }

                PeerState::AwaitingVerification => {
                    if let Some(unverified) = unverified_clients.iter().find(|x| x.id == client.id)
                    {
                        if unverified.verified {
                            client.state = PeerState::EventSync;

                            unverified_clients.retain(|x| x.id != client.id);
                        }
                    } else {
                        client.state = PeerState::Disconnected;

                        ConnectionHandler::update_blocked_connection(
                            data.1.entry(client.peer_addr),
                        );
                    }
                }

                PeerState::EventSync | PeerState::InitObjectSync | PeerState::Connected => {
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
            ConnectionHandler::send_packets(client, &mut listener.send);
        }
    }

    fn update_client(
        data: &mut PeerConnection,
        identifier: &mut BytesMut,
        listener: &mut UDPListener,
    ) {
        data.conn.on_timeout();

        for (packet, source_addr) in listener.recv.try_iter() {
            let packet: BytesMut = packet.into();
            ConnectionHandler::recv_packet(source_addr, packet, data, listener.bind_addr);
        }

        match data.state {
            PeerState::AwaitingConnection => {
                if data.conn.is_closed() {
                    data.state = PeerState::Disconnected;
                }

                if data.conn.is_established() {
                    let length = data.conn.stream_send(0, identifier, false).unwrap();
                    assert_eq!(length, identifier.len());
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
            _ => {
                panic!("client in unexpected state: {:?}", data.state)
            }
        }

        ConnectionHandler::send_packets(data, &mut listener.send);
    }

    fn update_blocked_connection(entry: Entry<SocketAddr, BlockedConnection>) {
        entry
            .and_modify(|e| {
                e.block_count += 1;
                e.block_expiry = Instant::now() + Duration::from_secs(e.block_count * e.block_count)
            })
            .or_insert(BlockedConnection {
                block_count: 1,
                block_expiry: Instant::now(),
            });
    }
    fn create_client<'b>(
        source_addr: SocketAddr,
        listener: &UDPListener,
        psks: Arc<Mutex<HashMap<String, [u8; 32]>>>,
    ) -> PeerConnection<'a> {
        let mut scid_bytes = [0u8; quiche::MAX_CONN_ID_LEN];
        ring::rand::SystemRandom::new()
            .fill(&mut scid_bytes)
            .unwrap();
        let scid = quiche::ConnectionId::from_vec(scid_bytes.to_vec());

        let ssl_ctx = ConnectionHandler::build_server_ctx(psks).unwrap();

        let conn = quiche::accept(
            &scid,
            None,
            listener.bind_addr,
            source_addr,
            &mut ConnectionHandler::get_config(ssl_ctx),
        )
        .unwrap();

        PeerConnection {
            id: scid.clone(),
            conn,
            state: PeerState::AwaitingConnection,
            peer_addr: source_addr,
        }
    }
    fn send_packets(connection: &mut PeerConnection, sender: &mut SyncSender<(Bytes, SendInfo)>) {
        loop {
            let mut out = BytesMut::zeroed(MAX_DATAGRAM_SIZE);
            match connection.conn.send(&mut out) {
                Ok((length, info)) => {
                    out.truncate(length);
                    let out = Bytes::from(out);
                    sender.send((out, info));
                }
                Err(quiche::Error::Done) => {
                    break;
                }
                Err(e) => {
                    panic!("error sending: {:?}", e)
                }
            }
        }
    }
    fn recv_packet(
        from: SocketAddr,
        mut packet: BytesMut,
        connection: &mut PeerConnection<'a>,
        bind_addr: SocketAddr,
    ) {
        let info = RecvInfo {
            from,
            to: bind_addr,
        };
        connection.conn.recv(&mut packet, info).unwrap();
    }

    pub fn get_peers(&self) -> Vec<ConnectionId<'_>> {
        match self.handler {
            HandlerType::Client(ref c, _) => vec![c.id.clone()],
            HandlerType::Server(ref s) => s.0.keys().cloned().collect(),
        }
    }

    pub fn get_peer_refs(&self) -> Vec<&ConnectionId<'_>> {
        match self.handler {
            HandlerType::Client(ref c, _) => vec![&c.id],
            HandlerType::Server(ref s) => s.0.keys().collect(),
        }
    }

    pub fn send_stream(
        &mut self,
        peer: ConnectionId<'static>,
        stream_id: u64,
        mut data: BitVec<u64, Lsb0>,
    ) -> std::result::Result<(), ConnectionError> {
        if stream_id == 0 {
            panic!("tried to send netnode data on the internal stream");
        }

        data.set_uninitialized(false);

        let data: Vec<u8> = data
            .into_vec()
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();

        match self.handler {
            HandlerType::Client(ref mut c, _) => Self::send_inner(c, stream_id, &data),
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(&peer) {
                    Self::send_inner(peer, stream_id, &data)
                } else {
                    Err(ConnectionError::Generic(Box::new(io::Error::new(
                        ErrorKind::NotFound,
                        "peer not found",
                    ))))
                }
            }
        }
    }

    fn send_inner(
        conn: &mut PeerConnection,
        stream_id: u64,
        data: &[u8],
    ) -> std::result::Result<(), ConnectionError> {
        match conn.conn.stream_send(stream_id, data, false) {
            Ok(length) => {
                if length != data.len() {
                    Err(ConnectionError::Generic(Box::new(io::Error::new(
                        ErrorKind::WouldBlock,
                        "stream buffer is full",
                    ))))
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(ConnectionError::Generic(Box::new(e))),
        }
    }

    pub fn recv_stream(
        &mut self,
        peer: ConnectionId<'static>,
        stream_id: u64,
    ) -> std::result::Result<BitVec<u64, Lsb0>, ConnectionError> {
        const STREAM_CHUNK_SIZE: usize = 512;

        if stream_id == 0 {
            eprintln!("tried to read from internal stream");
            return Ok(BitVec::new());
        }

        let mut buf = BytesMut::zeroed(STREAM_CHUNK_SIZE);
        let buffer_length: usize;

        match self.handler {
            HandlerType::Client(ref mut c, _) => match Self::recv_inner(c, stream_id, &mut buf) {
                Ok(length) => {
                    buffer_length = length;
                }
                Err(e) => return Err(e),
            },
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(&peer) {
                    match Self::recv_inner(peer, stream_id, &mut buf) {
                        Ok(length) => {
                            buffer_length = length;
                        }
                        Err(e) => return Err(e),
                    }
                } else {
                    return Err(ConnectionError::Generic(Box::new(io::Error::new(
                        ErrorKind::NotFound,
                        "peer not found",
                    ))));
                }
            }
        };

        let mut buffer_bits = Self::packet_to_bits(buf);

        buffer_bits.truncate(buffer_length * 8);
        Ok(buffer_bits)
    }

    fn packet_to_bits(buf: BytesMut) -> BitVec<u64, Lsb0> {
        const CHUNK_REQUIRED_ALIGNMENT: usize = 8;

        assert_eq!(buf.len() % CHUNK_REQUIRED_ALIGNMENT, 0);

        let (buf, []) = buf.as_chunks::<8>() else {
            unreachable!()
        };

        let buf: Vec<u64> = buf
            .into_iter()
            .map(|x| u64::from_le_bytes((*x).into()))
            .collect();

        BitVec::from_vec(buf)
    }

    fn recv_inner(
        conn: &mut PeerConnection,
        stream_id: u64,
        buf: &mut BytesMut,
    ) -> std::result::Result<usize, ConnectionError> {
        match conn.conn.stream_recv(stream_id, buf) {
            Ok((length, _)) => Ok(length),
            Err(quiche::Error::Done) => Ok(0),
            Err(e) => return Err(ConnectionError::Generic(Box::new(e))),
        }
    }

    pub fn send_datagram(
        &mut self,
        peer: ConnectionId<'static>,
        mut data: BitVec<u64, Lsb0>,
    ) -> std::result::Result<(), ConnectionError> {
        data.set_uninitialized(false);

        let data: Vec<u8> = data
            .into_vec()
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();

        match self.handler {
            HandlerType::Client(ref mut c, _) => Self::send_dgram_inner(c, data),
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(&peer) {
                    Self::send_dgram_inner(peer, data)
                } else {
                    Err(ConnectionError::Generic(Box::new(io::Error::new(
                        ErrorKind::NotFound,
                        "peer not found",
                    ))))
                }
            }
        }
    }

    fn send_dgram_inner(
        conn: &mut PeerConnection,
        data: Vec<u8>,
    ) -> std::result::Result<(), ConnectionError> {
        if data.len() > conn.conn.dgram_max_writable_len().unwrap_or(0) {
            return Err(ConnectionError::Generic(Box::new(io::Error::new(
                ErrorKind::InvalidData,
                "datagram too large",
            ))));
        }
        match conn.conn.dgram_send_buf(data) {
            Ok(_) => Ok(()),
            Err(e) => Err(ConnectionError::Generic(Box::new(e))),
        }
    }

    pub fn recv_datagram(
        &mut self,
        peer: ConnectionId<'static>,
    ) -> std::result::Result<BitVec<u64, Lsb0>, ConnectionError> {
        let dgram = match self.handler {
            HandlerType::Client(ref mut c, _) => c.conn.dgram_recv_buf().unwrap_or(Vec::new()),
            HandlerType::Server(ref mut s) => {
                if let Some(peer) = s.0.get_mut(&peer) {
                    peer.conn.dgram_recv_buf().unwrap_or(Vec::new())
                } else {
                    return Err(ConnectionError::Generic(Box::new(io::Error::new(
                        ErrorKind::NotFound,
                        "peer not found",
                    ))));
                }
            }
        };
        Ok(Self::packet_to_bits(BytesMut::from(Bytes::from(dgram))))
    }

    fn new_client(
        server_addr: SocketAddr,
        identifier: BytesMut,
        supplied_identity: String,
        supplied_psk: Vec<u8>,
    ) -> Self {
        let ssl_ctx = ConnectionHandler::build_client_ctx(supplied_identity, supplied_psk);
        let mut config = ConnectionHandler::get_config(ssl_ctx);
        let mut id = vec![0u8; quiche::MAX_CONN_ID_LEN];
        ring::rand::SystemRandom::new().fill(&mut id).unwrap();
        let id = ConnectionId::from_vec(id);
        let listener = UDPListener::new_client(
            SocketAddr::new("0.0.0.0".parse().unwrap(), server_addr.port()),
            server_addr,
        );
        let conn =
            quiche::connect(None, &id, listener.bind_addr, server_addr, &mut config).unwrap();
        Self {
            handler: HandlerType::Client(
                PeerConnection {
                    id,
                    conn,
                    state: PeerState::AwaitingConnection,
                    peer_addr: server_addr,
                },
                identifier,
            ),
            listener,
            unverified_clients: Vec::new(),
        }
    }

    fn new_server(target_port: u16) -> Self {
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
            unverified_clients: Vec::new(),
        }
    }
    fn get_config(ssl_ctx: SslContextBuilder) -> Config {
        let mut config =
            Config::with_boring_ssl_ctx_builder(quiche::PROTOCOL_VERSION, ssl_ctx).unwrap();
        config.discover_pmtu(true);
        config.set_application_protos(&[b"netnodes-1"]);
        config.set_max_idle_timeout(10000);
        config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
        config.set_initial_max_data(1000000);
        config.set_initial_max_stream_data_bidi_local(900000);
        config.set_initial_max_stream_data_bidi_remote(900000);
        config.set_initial_max_stream_data_uni(900000);
        config.set_initial_max_streams_bidi(10);
        config.set_initial_max_streams_uni(10);
        config.enable_dgram(true, 1000, 1000);
        config.set_disable_active_migration(true);
        config
    }

    fn build_server_ctx(
        psks: Arc<Mutex<HashMap<String, [u8; 32]>>>,
    ) -> Result<SslContextBuilder, boring::error::ErrorStack> {
        let mut ctx = SslContextBuilder::new(SslMethod::tls_server())?;

        ctx.set_min_proto_version(Some(SslVersion::TLS1_3))?;
        ctx.set_max_proto_version(Some(SslVersion::TLS1_3))?;

        ctx.set_verify(SslVerifyMode::NONE);

        ctx.set_psk_server_callback(move |_ssl, identity, out| {
            if let Some(id) = identity {
                if let Some(psk) = psks.lock().unwrap().get(&*String::from_utf8_lossy(id)) {
                    let key_len = psk.len();
                    if out.len() >= key_len {
                        out[..key_len].copy_from_slice(psk);
                        return Ok(key_len);
                    }
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
