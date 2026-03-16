use bitvec::prelude::*;
use quiche::*;
use ring::rand::SecureRandom;
use std::cell::LazyCell;
use std::collections::HashMap;
use std::collections::{HashSet, VecDeque};
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime};

const MAX_DATAGRAM_SIZE: usize = 1350;

struct UDPListener {
    socket: UdpSocket,
    packets: Vec<(SocketAddr, usize, [u8; MAX_DATAGRAM_SIZE])>,
}

impl UDPListener {
    fn new(bind_addr: SocketAddr) -> Self {
        // user can send 512kb/s, thats around 25 packets / frame, assuming 20 users max
        // we get 500 packets per frame max. technically possible we run out but unlikely
        const PACKET_CAPACITY: usize = 500;
        let socket = UdpSocket::bind(bind_addr).unwrap();
        socket.set_nonblocking(true);
        let packets = Vec::with_capacity(PACKET_CAPACITY);
        Self { socket, packets }
    }
    fn poll(&mut self) {
        self.packets.clear();
        loop {
            let mut buffer = [0u8; MAX_DATAGRAM_SIZE];
            match self.socket.recv_from(&mut buffer) {
                Ok((len, from)) => self.packets.push((from, len, buffer)),
                Err(e) if e.kind() == ErrorKind::WouldBlock => return,
                Err(e) => panic!("failed to poll udp socket: {:?}", e),
            }
        }
    }
}

enum PeerState {
    AwaitingConnection,
    AwaitingIdentity(SystemTime),
    EventSync,
    ObjectSync,
    Connected,
    Disconnected,
}

struct PeerConnection<'a> {
    id: ConnectionId<'a>,
    conn: Connection,
    state: PeerState,
}

enum HandlerType<'a> {
    Server(
        (
            HashMap<SocketAddr, PeerConnection<'a>>,
            HashMap<SocketAddr, BlockedConnection>,
        ),
    ),
    Client(PeerConnection<'a>),
}

struct BlockedConnection {
    block_count: usize,
    block_expiry: SystemTime,
}

struct ConnectionHandler<'a> {
    data: HandlerType<'a>,
    listener: UDPListener,
}

impl<'a> ConnectionHandler<'a> {
    fn update(&mut self) {}
    fn new_client(server_addr: SocketAddr) -> Self {
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
        let mut id = vec![0u8; quiche::MAX_CONN_ID_LEN];
        ring::rand::SystemRandom::new().fill(&mut id).unwrap();
        let id = ConnectionId::from_vec(id);
        let conn = quiche::connect(
            None,
            &id,
            SocketAddr::new("0.0.0.0".parse().unwrap(), server_addr.port()),
            server_addr,
            &mut config,
        )
        .unwrap();
        Self {
            data: HandlerType::Client(PeerConnection {
                id,
                conn,
                state: PeerState::AwaitingConnection,
            }),
            listener: UDPListener::new(SocketAddr::new(
                "0.0.0.0".parse().unwrap(),
                server_addr.port(),
            )),
        }
    }
    fn new_server(target_port: u16) -> Self {
        Self {
            data: HandlerType::Server((HashMap::new(), HashMap::new())),
            listener: UDPListener::new(SocketAddr::new("0.0.0.0".parse().unwrap(), target_port)),
        }
    }
}
