use bytes::{Bytes, BytesMut};
use quiche::*;
use ring::rand::SecureRandom;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::{self};
use std::time::{Duration, Instant};

const MAX_DATAGRAM_SIZE: usize = 1350;
const PACKET_QUEUE_CAPACITY: usize = 1000;
const MAX_CLIENT_CONNECTIONS: usize = 256;

struct UDPListener {
    send: SyncSender<(Bytes, SendInfo)>,
    recv: Receiver<(BytesMut, SocketAddr)>,
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
        outgoing: SyncSender<(BytesMut, SocketAddr)>,
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
                    socket.send_to(packet.as_ref(), info.to);
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
                socket.send_to(packet.as_ref(), info.to);
            }

            if let Some(packet) = leftover_packet.clone() {
                if outgoing.try_send((packet.0.into(), packet.1)).is_err() {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                } else {
                    leftover_packet = None;
                }
            }

            while let Some((source, packet)) = UDPListener::poll(&mut socket) {
                let packet = packet.freeze();
                if outgoing.try_send((packet.clone().into(), source)).is_err() {
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
                for client in data.0.values_mut() {
                    client.conn.on_timeout();
                }

                for (mut packet, source_addr) in self.listener.recv.try_iter() {
                    let hdr = match quiche::Header::from_slice(
                        packet.as_mut(),
                        quiche::MAX_CONN_ID_LEN,
                    ) {
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
                                &self.listener,
                            ))
                        }
                    };

                    ConnectionHandler::recv_packet(
                        source_addr,
                        packet,
                        client,
                        self.listener.bind_addr,
                    );
                }

                for client in data.0.values_mut() {
                    match client.state {
                        PeerState::AwaitingConnection => {
                            if client.conn.is_closed() {
                                client.state = PeerState::Disconnected;
                            }

                            if client.conn.is_established() {
                                client.state = PeerState::AwaitingIdentity(
                                    Instant::now() + Duration::from_secs(3),
                                );
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
                                self.unverified_clients.push(UnverifiedClients {
                                    id: client.id.clone(),
                                    nonce,
                                    uuid,
                                    verified: false,
                                });
                                client.state = PeerState::AwaitingVerification;
                            }
                        }
                        PeerState::AwaitingVerification => {
                            if let Some(unverified) =
                                self.unverified_clients.iter().find(|x| x.id == client.id)
                            {
                                if unverified.verified {
                                    client.state = PeerState::EventSync;
                                    self.unverified_clients.retain(|x| x.id != client.id);
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
                    ConnectionHandler::send_packets(client, &mut self.listener.send);
                }
            }

            HandlerType::Client(ref mut data, ref mut identifier) => {
                data.conn.on_timeout();

                for (packet, source_addr) in self.listener.recv.try_iter() {
                    ConnectionHandler::recv_packet(
                        source_addr,
                        packet,
                        data,
                        self.listener.bind_addr,
                    );
                }

                match data.state {
                    PeerState::AwaitingConnection => {
                        if data.conn.is_closed() {
                            data.state = PeerState::Disconnected;
                        }

                        if data.conn.is_established() {
                            let length = data
                                .conn
                                .stream_send(0, identifier.as_mut(), false)
                                .unwrap();
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

                ConnectionHandler::send_packets(data, &mut self.listener.send);
            }
        }
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
    fn create_client<'b>(source_addr: SocketAddr, listener: &UDPListener) -> PeerConnection<'a> {
        let mut scid_bytes = [0u8; quiche::MAX_CONN_ID_LEN];
        ring::rand::SystemRandom::new()
            .fill(&mut scid_bytes)
            .unwrap();
        let scid = quiche::ConnectionId::from_vec(scid_bytes.to_vec());

        let conn = quiche::accept(
            &scid,
            None,
            listener.bind_addr,
            source_addr,
            &mut ConnectionHandler::get_config(),
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
        connection.conn.recv(packet.as_mut(), info).unwrap();
    }
    fn new_client(server_addr: SocketAddr, identifier: BytesMut) -> Self {
        let mut config = ConnectionHandler::get_config();
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
            handler: HandlerType::Server((HashMap::new(), HashMap::new())),
            listener: UDPListener::new_server(SocketAddr::new(
                "0.0.0.0".parse().unwrap(),
                target_port,
            )),
            unverified_clients: Vec::new(),
        }
    }
    fn get_config() -> Config {
        let mut config = Config::new(quiche::PROTOCOL_VERSION).unwrap();
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
}
