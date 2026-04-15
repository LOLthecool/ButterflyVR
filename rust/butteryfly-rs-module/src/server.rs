// functionallity for the NetNodeManager server
use crate::messages::*;
use crate::net_nodes::NetworkedNode;
use crate::networker::ConnectionHandler;
use crate::serializer::*;
use bitvec::prelude::*;
use godot::prelude::*;
use quiche::ConnectionId;
use rand::{RngExt, SeedableRng};
use std::collections::{BTreeMap, HashSet, VecDeque, hash_map};
use std::mem;
use std::{cmp, collections::HashMap};

const CHANNEL_ACK: u16 = u16::MAX;
const CHANNEL_CLIENT_ID: u16 = u16::MAX - 1;

const BYTE: usize = 8;
const BYTES2: usize = 16;
const BYTES8: usize = 64;

const DGRAM_HEADER_SIZE: usize = BYTES2;

const HIT_RATE_HISTORY_LENGTH: usize = 128;

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct NetNodeServer {
    clients: HashMap<ConnectionId<'static>, Client>,
    networked_nodes: Vec<Gd<NetworkedNode>>,
    networker: ConnectionHandler,
    message_buffer: VecDeque<(BitVec<u64, Lsb0>, u64)>,
    message_handlers: HashMap<u16, Gd<MessageHandler>>,
    current_tick: usize,
    base: Base<Node>,
}

#[godot_api]
pub impl NetNodeServer {
    #[signal]
    pub fn player_joined(player: Vec<u8>);
    #[signal]
    pub fn player_left(player: Vec<u8>);
    pub fn get_player_count(&self) -> usize {
        self.networker.get_peer_refs().len()
    }
    pub fn register_node(&mut self, new_node_ref: Gd<NetworkedNode>, new_node: &mut NetworkedNode) {
        self.queue_message(
            MessageHandler::create_id_sync_message(
                new_node_ref.clone().upcast(),
                new_node.objectid,
                Some(new_node.owner_id.to_vec().try_into().unwrap()),
            ),
            0,
        );
        self.networked_nodes.push(new_node_ref);
    }
    pub fn unregister_node(&mut self, removed_node_ref: Gd<NetworkedNode>) {
        for client in self.clients.values_mut().filter_map(|x| {
            let ClientState::Connected(ref mut client) = x.state else {
                return None;
            };
            Some(client)
        }) {
            let p = mem::take(&mut client.priorities);
            client.priorities = p.into_iter().filter(|x| x.1 == removed_node_ref).collect()
        }
        if let Some(idx) = self
            .networked_nodes
            .iter()
            .position(|x| *x == removed_node_ref)
        {
            self.networked_nodes.remove(idx);
        }
    }
    pub fn register_message(&mut self, handler: Gd<MessageHandler>, message_type: u16) {
        self.message_handlers.insert(message_type, handler);
    }
    pub fn unregister_message(&mut self, message_type: u16) {
        self.message_handlers.remove(&message_type);
    }
    pub fn queue_message(&mut self, message: BitVec<u64, Lsb0>, stream: u64) {
        self.message_buffer.push_back((message, stream));
    }
    pub fn start_server(&mut self, bind_port: u16) {
        self.networker = ConnectionHandler::new_server(bind_port)
    }
    pub fn get_next_client(&mut self) -> PackedByteArray {
        let psk_identifier = rand::rng().random::<[char; 8]>();
        let psk_key = rand::rng().random::<[u8; 32]>();
        self.networker
            .add_client_token(String::from_iter(psk_identifier.iter()), psk_key);
        let user_identifier = rand::rng().random::<[u8; 40]>();
        PackedByteArray::from(user_identifier)
    }
    fn update_network_nodes(&mut self) {
        for client in self.clients.values_mut() {
            if let ClientState::Connected(ref mut client) = client.state {
                while let Some((_, packet)) = client.unapplied_packets.pop_first() {
                    let mut pointer: usize = DGRAM_HEADER_SIZE;
                    while pointer + BYTES2 <= packet.len() {
                        let next_obj: u16 = packet[pointer..pointer + BYTES2].load_le();
                        pointer += BYTES2;
                        if let Some(tmp) = self
                            .networked_nodes
                            .iter()
                            .find(|x| Gd::bind(x).objectid == next_obj)
                        {
                            let node = Gd::bind(tmp);
                            let types_buff: Vec<NetworkedValueTypes> =
                                node.get_networked_values_types();
                            node.update_networked_values(
                                &mut pointer,
                                packet.as_bitslice(),
                                &types_buff,
                            );
                        } else {
                            // will give a few spurious errors if we get sync data for a netnode before its creation event
                            godot_warn!(
                                "got update for nonexistant netnode with objectid: {:#?}",
                                next_obj
                            );
                            break;
                        }
                    }
                }
            }
        }
    }

    fn send_packets_server(&mut self) {
        const PACKET_MAX_SIZE_THRESHOLD: usize = 80;

        let mut random = rand::rngs::SmallRng::from_seed(rand::random());

        for client in self.clients.values_mut() {
            let conn = &client.conn;
            if let ClientState::Connected(ref mut client) = client.state {
                const MINIMUM_CONNECTION_BANDWIDTH: usize = 512;

                client.bandwidth_budget_per_tick = client
                    .bandwidth_budget_per_tick
                    .max(MINIMUM_CONNECTION_BANDWIDTH);

                let max_dgram_size: usize = self.networker.get_max_dgram_size(conn);

                if self.networker.is_connection_pacing() {
                    client.bandwidth_budget_per_tick /= 2;
                }

                let mut remaining_bandwidth = client.bandwidth_budget_per_tick;

                // channel 3 (messages)
                while self.message_buffer.len() > client.message_buffer_position {
                    let (message, stream) = &self.message_buffer[client.message_buffer_position];

                    if remaining_bandwidth.checked_sub(message.len()).is_none() {
                        break;
                    }
                    remaining_bandwidth -= message.len();

                    client.message_buffer_position += 1;
                    self.networker.send_stream(conn, *stream, message.clone());
                }

                if client.state == ClientSubState::EventSync {
                    if self.message_buffer.len() == client.message_buffer_position {
                        client.state = ClientSubState::ObjectSync(self.networked_nodes.clone());
                    } else {
                        client.bandwidth_budget_per_tick += client.bandwidth_budget_per_tick / 10;
                        continue;
                    }
                }

                // channel 1 (syncing)
                while remaining_bandwidth > PACKET_MAX_SIZE_THRESHOLD {
                    let mut packet: BitVec<u64> = BitVec::with_capacity(max_dgram_size);

                    match client.state {
                        ClientSubState::ObjectSync(ref mut objects) => {
                            if let Some(object) = objects.get_mut(0) {
                                let node_ref = object;
                                let node = Gd::bind(node_ref);

                                let tmp = node.get_byte_data(&node.get_networked_values_types());

                                drop(node);

                                if tmp.len() + packet.len()
                                    > cmp::min(remaining_bandwidth, max_dgram_size)
                                {
                                    break;
                                }

                                packet.extend_from_bitslice(tmp.as_bitslice());
                                objects.pop();
                            } else {
                                client.state = ClientSubState::Connected;
                            }
                        }
                        _ => {
                            self.current_tick += 1;

                            packet.extend_from_bitslice(self.current_tick.view_bits::<Lsb0>());
                            debug_assert_eq!(DGRAM_HEADER_SIZE, packet.len());

                            // todo: this dosent catch some changes to networked_nodes but that should be fine
                            if client.priorities.len() != self.networked_nodes.len() {
                                client.priorities.clear();
                                for node_ref in self.networked_nodes.iter() {
                                    let mut r = [0; 16];
                                    random.fill(&mut r);
                                    client.priorities.insert((0, r), node_ref.clone());
                                }
                            }

                            let old_map = mem::take(&mut client.priorities);
                            client.priorities = old_map
                                .into_iter()
                                .map(|mut value| {
                                    if value.0.0 != 0 {
                                        let node_ref = &value.1;
                                        let node = Gd::bind(node_ref);

                                        let tmp =
                                            node.get_byte_data(&node.get_networked_values_types());

                                        drop(node);

                                        if tmp.len() + packet.len()
                                            > cmp::min(remaining_bandwidth, max_dgram_size)
                                        {
                                            return value;
                                        }

                                        packet.extend_from_bitslice(tmp.as_bitslice());
                                        value.0.0 = 0;
                                    }
                                    value
                                })
                                .collect();

                            if packet.len() > DGRAM_HEADER_SIZE {
                                remaining_bandwidth -= packet.len();
                                self.networker.send_datagram(conn, packet);
                                continue;
                            }

                            break;
                        }
                    }
                }
                if remaining_bandwidth <= PACKET_MAX_SIZE_THRESHOLD {
                    client.bandwidth_budget_per_tick += client.bandwidth_budget_per_tick / 10;
                }
            }
        }
    }
    fn tick_server(&mut self) {
        const MESSAGE_HEADER_SIZE: usize = BYTES2;

        Self::tick_clients(&mut self.clients);

        self.networker.update();

        let clients = HashSet::from_iter(self.networker.get_peers().into_iter());

        let tmp = self.clients.keys().cloned().collect();
        let new_players: Vec<ConnectionId> = clients.difference(&tmp).cloned().collect();

        let tmp = self
            .clients
            .keys()
            .cloned()
            .collect::<HashSet<ConnectionId>>();
        let dc_clients: Vec<ConnectionId> = tmp.difference(&clients).cloned().collect();

        for player in new_players.into_iter() {
            let player: ConnectionId = player;
            self.signals()
                .player_joined()
                .emit(player.into_iter().copied().collect::<Vec<u8>>());
            self.clients.insert(player.clone(), Client::new(player));
        }

        for dc_client in dc_clients {
            self.signals()
                .player_left()
                .emit(dc_client.iter().as_slice().to_vec());
            self.clients.remove(&dc_client);
        }

        let mut random = rand::rngs::SmallRng::from_seed(rand::random());

        for (client_id, client) in self.clients.iter_mut() {
            match client.state {
                ClientState::AwaitingIdentifier => {
                    if let Ok(data) = self.networker.recv_stream_bytes(client_id, 0, 40) {
                        if let Ok(identifier) = data.into_vec().try_into() {
                            client.state = ClientState::AwaitingUuid(identifier);
                        } else {
                            self.networker.disconnect_peer(
                                client_id,
                                true,
                                0,
                                "invalid identifier length",
                            );
                        }
                    }
                }
                ClientState::AwaitingUuid(_) => {}
                ClientState::Connected(ref mut client) => {
                    for stream in self.networker.get_readable_streams(client_id) {
                        while let Ok(stream_chunk) = self.networker.recv_stream(client_id, stream) {
                            if stream_chunk.len() == 0 {
                                break;
                            }

                            let mut stream_chunk = stream_chunk.as_bitslice();

                            if let hash_map::Entry::Occupied(mut entry) =
                                client.incomplete_messages.entry(stream)
                            {
                                let &mut (ref mut length, ref mut incomplete) = entry.get_mut();

                                let length = length.unwrap_or_else(|| {
                                    let missing = incomplete.len() - BYTES8;
                                    // todo: this assumes we will always have enough data to fill the length field
                                    // not sure if that is true
                                    incomplete.extend_from_bitslice(&stream_chunk[..missing]);

                                    *length = Some(incomplete[..BYTES8].load_le());
                                    length.unwrap()
                                });

                                let mut pointer = BYTES8;

                                // should never be 0 since we would have already finished
                                let remaining = length - incomplete.len();

                                if remaining > stream_chunk.len() {
                                    incomplete.extend_from_bitslice(stream_chunk);
                                    continue;
                                }

                                incomplete.extend_from_bitslice(&stream_chunk[..remaining]);

                                let handler: u16 =
                                    incomplete[pointer..pointer + MESSAGE_HEADER_SIZE].load_le();
                                pointer += MESSAGE_HEADER_SIZE;

                                if let Some(handler) = self.message_handlers.get_mut(&handler) {
                                    handler
                                        .bind_mut()
                                        .handle_message(incomplete.as_bitslice(), &mut pointer);
                                }

                                let (_, (_, value)) = entry.remove_entry();
                                self.message_buffer.push_back((value, stream));

                                (_, stream_chunk) = stream_chunk.split_at(remaining);
                            }

                            loop {
                                if stream_chunk.is_empty() {
                                    break;
                                }

                                if stream_chunk.len() < BYTES8 {
                                    client
                                        .incomplete_messages
                                        .insert(stream, (None, stream_chunk.to_bitvec()));
                                    break;
                                }

                                let length: usize = stream_chunk[..BYTES8].load_le();
                                let mut pointer = BYTES8;

                                if stream_chunk.len() < length {
                                    client
                                        .incomplete_messages
                                        .insert(stream, (Some(length), stream_chunk.to_bitvec()));
                                    break;
                                }

                                let incomplete_stream;
                                (incomplete_stream, stream_chunk) = stream_chunk.split_at(length);

                                let handler: u16 = incomplete_stream
                                    [pointer..pointer + MESSAGE_HEADER_SIZE]
                                    .load_le();
                                pointer += MESSAGE_HEADER_SIZE;

                                if let Some(handler) = self.message_handlers.get_mut(&handler) {
                                    handler
                                        .bind_mut()
                                        .handle_message(incomplete_stream, &mut pointer);
                                }
                                self.message_buffer
                                    .push_back((incomplete_stream.to_bitvec(), stream));
                            }
                        }
                    }

                    client.tick_number += 1;

                    let mut late_packets: usize = 0;
                    let mut total_packets: usize = 0;
                    let mut got_next_tick_packet: bool = false;

                    while let Ok(packet) = self.networker.recv_datagram(client_id) {
                        let packet: BitVec<u64> = packet;
                        if packet.len() < DGRAM_HEADER_SIZE {
                            godot_warn!("got c1 packet with invalid size");
                            continue;
                        }

                        let packet_apply_tick: i16 = packet[0..BYTES2].load_le();

                        let relative_apply_tick =
                            packet_apply_tick.wrapping_sub(client.tick_number);

                        total_packets += 1;

                        if relative_apply_tick <= client.tick_number {
                            late_packets += 1;
                            continue;
                        }

                        if relative_apply_tick == client.tick_number + 1 {
                            got_next_tick_packet = true;
                        }

                        let mut r = [0; 16];
                        random.fill(&mut r);

                        client
                            .unapplied_packets
                            .insert((packet_apply_tick, r), packet);
                    }

                    if !got_next_tick_packet {
                        client.tick_number += 1;
                    }

                    if late_packets > (total_packets / 100) {
                        client.tick_number -= 1;
                    }
                }
            }
        }
    }
    fn tick_clients(clients: &mut HashMap<ConnectionId, Client>) {
        for (conn, client) in clients.iter_mut() {
            if let ClientState::Connected(ref mut client) = client.state {
                let old_map = mem::take(&mut client.priorities);
                client.priorities = old_map
                    .into_iter()
                    .map(|mut priority| {
                        let p = priority
                            .1
                            .bind()
                            .get_priority(Vec::from(conn.clone()).to_godot().to_packed_array());
                        priority.0.0 += p;
                        priority
                    })
                    .collect()
            }
        }
    }
}
#[godot_api]
impl INode for NetNodeServer {
    fn physics_process(&mut self, _delta: f64) {
        self.tick_server();
        self.update_network_nodes();
        self.send_packets_server();
    }
}

#[derive(Debug, Clone)]
struct Client {
    conn: ConnectionId<'static>,
    state: ClientState,
}

impl Client {
    fn new(conn: ConnectionId<'static>) -> Self {
        const INITIAL_CLIENT_BANDWIDTH: usize = 8000;
        Self {
            conn,
            state: Default::default(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
enum ClientState {
    #[default]
    AwaitingIdentifier,
    AwaitingUuid([u8; 40]),
    Connected(ConnectedClient),
}

#[derive(Debug, Clone, PartialEq)]
struct ConnectedClient {
    uuid: [u8; 16],
    state: ClientSubState,
    bandwidth_budget_per_tick: usize,
    incomplete_messages: HashMap<u64, (Option<usize>, BitVec<u64, Lsb0>)>,
    message_buffer_position: usize,
    // the array here is to make each key unique
    priorities: BTreeMap<(i64, [u8; 16]), Gd<NetworkedNode>>,
    unapplied_packets: BTreeMap<(i16, [u8; 16]), BitVec<u64, Lsb0>>,
    tick_number: i16,
}

impl ConnectedClient {
    fn new(uuid: [u8; 16]) -> Self {
        const INITIAL_CLIENT_BANDWIDTH: usize = 8000;
        Self {
            uuid: uuid,
            state: Default::default(),
            bandwidth_budget_per_tick: INITIAL_CLIENT_BANDWIDTH,
            incomplete_messages: Default::default(),
            message_buffer_position: Default::default(),
            priorities: Default::default(),
            unapplied_packets: Default::default(),
            tick_number: Default::default(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
enum ClientSubState {
    #[default]
    EventSync,
    ObjectSync(Vec<Gd<NetworkedNode>>),
    Connected,
}
