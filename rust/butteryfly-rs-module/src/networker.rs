use bitvec::prelude::*;
use mio::*;
use quiche::*;
use std::collections::HashMap;
use std::collections::{HashSet, VecDeque};
use std::net;
use std::time::{Duration, Instant};

const UDP_SOCKET: Token = Token(0);

struct UDPListener {
    poll: Poll,
    events: Events,
}

impl UDPListener {
    fn new(bind_addr: net::SocketAddr) -> Self {
        let poll = Poll::new().unwrap();
        let events = Events::with_capacity(128);
        let mut udp_socket = net::UdpSocket::bind(bind_addr).unwrap();
        poll.registry()
            .register(&mut udp_socket, UDP_SOCKET, Interest::READABLE)
            .unwrap();
        Self { poll, events }
    }
}

struct ConnectionHandler {}
impl Default for ConnectionHandler {
    fn default() -> Self {
        Self {}
    }
}
impl ConnectionHandler {
    fn update(&mut self) {}
    fn connect(&mut self) {}
}
