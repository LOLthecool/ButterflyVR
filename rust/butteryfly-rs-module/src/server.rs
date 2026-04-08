// functionallity for the NetNodeManager server
use crate::net_nodes::NetworkedNode;
use crate::networker::ConnectionHandler;
use crate::serializer::*;
use crate::{messages::*, networker};
use bitvec::prelude::*;
use godot::classes::Engine;
use godot::prelude::*;
use quiche::{Connection, ConnectionId};
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{cmp, collections::HashMap};

const CHANNEL_ACK: u16 = u16::MAX;
const CHANNEL_CLIENT_ID: u16 = u16::MAX - 1;

const BYTE: usize = 8;
const BYTES2: usize = 16;
const BYTES8: usize = 64;

const PACKET_HEADER_SIZE: usize = BYTES2 + BYTES8;
const PACKET_HEADER_SIZE_ACK: usize = BYTES2;
const CHANNEL1_HEADER_SIZE: usize = BYTES8;

const HIT_RATE_HISTORY_LENGTH: usize = 128;

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct NetNodeServer {
    clients: HashMap<ConnectionId<'static>, Client>,
    networked_nodes: Vec<Gd<NetworkedNode>>,
    server_networker: ConnectionHandler<'static>,
    message_buffer: VecDeque<BitVec<u64, Lsb0>>,
    message_handlers: HashMap<u16, Gd<MessageHandler>>,
    base: Base<Node>,
}

#[godot_api]
pub impl NetNodeServer {
    #[signal]
    pub fn player_joined(player: u16);
    #[signal]
    pub fn player_left(player: u16);
    pub fn get_player_count(&self) -> usize {
        self.server_networker.get_peer_refs().len()
    }
    pub fn register_node(&mut self, new_node_ref: Gd<NetworkedNode>, new_node: &mut NetworkedNode) {
        self.queue_message(MessageHandler::create_id_sync_message(
            new_node_ref.clone().upcast(),
            new_node.objectid,
            Some(new_node.owner_id),
        ));
        self.networked_nodes.push(new_node_ref);
    }
    pub fn unregister_node(&mut self, removed_node_ref: Gd<NetworkedNode>) {
        for client in self.server_networker.clients.values_mut() {
            if let Some(idx) = client
                .priorities
                .iter()
                .position(|x| x.0 == removed_node_ref)
            {
                client.priorities.remove(idx);
            }
        }
        if let Some(idx) = self
            .networked_nodes
            .iter()
            .position(|x| *x == removed_node_ref)
        {
            self.networked_nodes.remove(idx);
        }
    }
    pub fn unregister_all(&mut self) {
        // todo: update this
        self.next_id = 0;
        self.networked_nodes.clear();
    }
    pub fn get_next_object_id(&mut self) -> u16 {
        self.next_id += 1;
        self.next_id
    }
    pub fn register_message(&mut self, handler: Gd<MessageHandler>, message_type: u16) {
        self.message_handlers.insert(message_type, handler);
    }
    pub fn unregister_message(&mut self, message_type: u16) {
        self.message_handlers.remove(&message_type);
    }
    pub fn queue_message(&mut self, message: BitVec<u64, Lsb0>) {
        self.message_buffer.push_back(message);
    }
    pub fn start_server(&mut self, public_addr: String, bind_addr: String, private_key: [u8; 32]) {
        const PROTOCOL_ID: u64 = 0;
        self.server_networker = ServerNetworker {
            server: Server::new(bind_addr, PROTOCOL_ID, private_key).unwrap(),
            public_address: public_addr,
            private_key,
            ..Default::default()
        };
    }
    pub fn get_next_client(&mut self) -> PackedByteArray {
        let mut result: PackedByteArray = PackedByteArray::new();
        let tmp: [u8; netcode::CONNECT_TOKEN_BYTES] =
            self.server_networker.get_token().try_into_bytes().unwrap();
        result.extend(tmp);
        result
    }
    pub fn register_player_object(&mut self, client_id: u16, object: Gd<Node3D>) {
        let client = self
            .server_networker
            .clients
            .values_mut()
            .find(|x| x.id == client_id);
        if let Some(client) = client {
            client.player_position_object = Some(object);
        }
    }
    fn update_network_nodes(&mut self) {
        for client in self.server_networker.clients.iter_mut() {
            for packet_tuple in client.1.packet_buffers.get_mut(0).unwrap().drain(..) {
                let packet = packet_tuple.0;
                let mut pointer: usize = PACKET_HEADER_SIZE + CHANNEL1_HEADER_SIZE;
                while pointer + BYTES2 <= packet.len() {
                    let next_obj: u16 = packet[pointer..pointer + BYTES2].load_le();
                    pointer += BYTES2;
                    let tmp = self
                        .networked_nodes
                        .iter()
                        .find(|x| Gd::bind(x).objectid == next_obj);
                    if tmp.is_none() {
                        godot_warn!(
                            "got update for nonexistant netnode with objectid: {:#?}",
                            next_obj
                        ); // will give a few spurious errors if we get sync data for a netnode before the creation event
                        break;
                    }
                    let node = Gd::bind(tmp.unwrap());
                    let types_buff: Vec<NetworkedValueTypes> = node.get_networked_values_types();
                    node.update_networked_values(&mut pointer, packet.as_bitslice(), &types_buff);
                }
            }
        }
    }

    fn send_packets_server(&mut self) {
        const PACKET_MAX_SIZE_THRESHOLD: usize = 80;
        const BANDWIDTH_BUDGET: usize = 512000;
        const MAX_SINGLE_PACKET_PAYLOAD_LENGTH: usize = 4800;
        let bandwidth_per_tick =
            BANDWIDTH_BUDGET / Engine::singleton().get_physics_ticks_per_second() as usize;
        let networker = &mut self.server_networker;
        let mut buffer: Vec<(ClientIndex, BitVec<u64>, u16)> = Vec::new();
        for client in networker.clients.iter_mut() {
            client.1.remaining_bandwidth += bandwidth_per_tick;
            'outer: while client.1.remaining_bandwidth > PACKET_MAX_SIZE_THRESHOLD {
                if !client.1.id_received {
                    buffer.push((
                        *client.0,
                        BitVec::<u64>::from_element(client.1.id as u64),
                        CHANNEL_CLIENT_ID,
                    ));
                    client.1.id_received = true;
                    break;
                }
                let mut packet: BitVec<u64> =
                    BitVec::with_capacity(MAX_SINGLE_PACKET_PAYLOAD_LENGTH);
                // acks
                for ack in client.1.waiting_acks.iter() {
                    packet.extend(ack.0.view_bits::<Lsb0>());
                    packet.extend(ack.1.view_bits::<Lsb0>());
                }
                client.1.waiting_acks.clear();
                if !packet.is_empty() {
                    client.1.remaining_bandwidth =
                        client.1.remaining_bandwidth.saturating_sub(packet.len());
                    buffer.push((*client.0, packet, CHANNEL_ACK));
                }
                // channel 3 (messages)
                while self.message_buffer.len() > client.1.message_buffer_position {
                    let mut packet: BitVec<u64> =
                        BitVec::with_capacity(MAX_SINGLE_PACKET_PAYLOAD_LENGTH);
                    let message = &self.message_buffer[client.1.message_buffer_position];
                    packet.extend(message);
                    if (client.1.remaining_bandwidth as i64 - packet.len() as i64) < 0 {
                        break 'outer;
                    }
                    client.1.remaining_bandwidth -= packet.len();
                    buffer.push((*client.0, packet, 3));
                    client.1.message_buffer_position += 1;
                }
                let mut packet: BitVec<u64> =
                    BitVec::with_capacity(MAX_SINGLE_PACKET_PAYLOAD_LENGTH);
                // channel 2 (initial sync)
                if client.1.sync_progress < (self.networked_nodes.len() as u64).saturating_sub(1)
                    && !client.1.finished_sync
                {
                    packet.extend(0u64.view_bits::<Lsb0>());
                    for index in client.1.sync_progress..self.networked_nodes.len() as u64 {
                        let node: GdRef<NetworkedNode> =
                            self.networked_nodes[index as usize].bind();
                        let tmp = node.get_byte_data(&node.get_networked_values_types());
                        if tmp.len() + packet.len()
                            > cmp::min(
                                client.1.remaining_bandwidth,
                                MAX_SINGLE_PACKET_PAYLOAD_LENGTH,
                            )
                        {
                            break;
                        }
                        client.1.sync_progress = index;
                        packet.extend(tmp);
                    }

                    if packet.len() > CHANNEL1_HEADER_SIZE {
                        client.1.remaining_bandwidth -= packet.len();
                        buffer.push((*client.0, packet, 2));
                        continue;
                    }
                } else {
                    client.1.finished_sync = true;
                }
                let mut packet: BitVec<u64> =
                    BitVec::with_capacity(MAX_SINGLE_PACKET_PAYLOAD_LENGTH);
                // channel 1 (syncing)
                packet.extend(
                    (SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64)
                        .view_bits::<Lsb0>(),
                );
                if client.1.priorities.len() != self.networked_nodes.len() {
                    client.1.priorities.clear();
                    for node_ref in self.networked_nodes.iter() {
                        client.1.priorities.push((node_ref.clone(), 0));
                    }
                }
                client.1.priorities.sort_unstable_by(|a, b| a.1.cmp(&b.1));
                for value in client.1.priorities.iter_mut() {
                    let node_ref = &value.0;
                    let node = Gd::bind(node_ref);
                    if (value.1 != 0) && (node.owner_id != client.1.id) {
                        let tmp = node.get_byte_data(&node.get_networked_values_types());
                        if tmp.len() + packet.len()
                            > cmp::min(
                                client.1.remaining_bandwidth,
                                MAX_SINGLE_PACKET_PAYLOAD_LENGTH,
                            )
                        {
                            break;
                        }
                        packet.extend(tmp);
                        value.1 = 0;
                    }
                }
                if packet.len() > CHANNEL1_HEADER_SIZE {
                    client.1.remaining_bandwidth -= packet.len();
                    buffer.push((*client.0, packet, 1));
                    continue;
                }
                packet.clear();

                break;
            }
        }
        for packet in buffer {
            networker.send(packet.1.as_bitslice(), packet.2, packet.0);
        }
    }
    fn tick_server(&mut self) {
        Self::tick_clients(&mut self.clients);

        self.server_networker.update();

        let clients = self.

        if !new_players.is_empty() {
            for player in new_players {
                self.signals().player_joined().emit(player);
            }
        }
        // check for and handle disconnected clients
        loop {
            let mut dc_client: Option<ClientIndex> = None;
            for client in self.server_networker.clients.iter() {
                if self.server_networker.server.client_id(*client.0).is_none() {
                    dc_client = Some(client.0.clone());
                    let id = client.1.id;
                    self.signals().player_left().emit(id);
                    break;
                }
            }
            if dc_client.is_some() {
                self.server_networker.clients.remove(&dc_client.unwrap());
            } else {
                break;
            }
        }
        let networker = &mut self.server_networker;
        let mut current_frame_hit_rates: HashMap<ClientIndex, u64> =
            HashMap::from_iter(networker.clients.iter().map(|x| (*x.0, 0)));
        let mut current_frame_miss_rates: HashMap<ClientIndex, u64> =
            HashMap::from_iter(networker.clients.iter().map(|x| (*x.0, 0)));
        // then for each packet check channel id and:
        for packet_tuple in networker.packet_buffer.drain(..) {
            if !networker.clients.contains_key(&packet_tuple.1) {
                continue;
            }
            let packet = &packet_tuple.0;
            let client = networker.clients.get_mut(&packet_tuple.1).unwrap();
            if packet.len() < BYTES2 {
                godot_warn!("got packet with invalid size");
                continue;
            }
            let mut pointer: usize = 0;
            let channelid: u16 = packet[pointer..pointer + BYTES2].load_le();
            pointer += BYTES2;
            let packet_number: u64 = packet[pointer..pointer + BYTES8].load_le();
            pointer += BYTES8;
            match channelid {
                0 => {
                    // control packets
                }
                // get channel 1 data for inputs and remote owned objects and send to buffer cycle
                1 => {
                    if packet.len() < PACKET_HEADER_SIZE + CHANNEL1_HEADER_SIZE {
                        godot_warn!("got c1 packet with invalid size");
                        continue;
                    }
                    const PACKET_LATENCY_DISCARD_THRESHOLD: Duration = Duration::from_millis(1000);
                    const LATENCY_BUFFER_MAX_SIZE: usize = 8;

                    let packet_send_time_utc: u64 = packet[pointer..pointer + BYTES8].load_le();
                    let packet_send_time: Duration = Duration::from_millis(packet_send_time_utc);
                    let tick_length: Duration = Duration::from_secs_f32(
                        1.0 / Engine::singleton().get_physics_ticks_per_second() as f32,
                    );

                    // initial time sync
                    let current_time: Duration =
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

                    // latency calculations
                    let latency: Duration = current_time - packet_send_time;
                    if latency > PACKET_LATENCY_DISCARD_THRESHOLD {
                        godot_warn!(
                            "ignoring packet with high latency: {:#?}ms",
                            latency.as_millis()
                        );
                        continue;
                    }
                    if client.latency_buffer.len() >= LATENCY_BUFFER_MAX_SIZE {
                        client.latency_buffer.pop_front();
                    }
                    client.latency_buffer.push_back(latency);
                    client.latency = Duration::from_millis(
                        client
                            .latency_buffer
                            .iter()
                            .map(|x| x.as_millis())
                            .sum::<u128>() as u64
                            / client.latency_buffer.len() as u64,
                    );
                    // handle buffers
                    let mut buffer_max_jitter: i128 = (client.packet_buffers.len()) as i128
                        * -(tick_length.as_millis() as i128 / 2);
                    let mut packet_accepted: bool = false;
                    let client_index = packet_tuple.1;
                    for buffer in client.packet_buffers.iter_mut() {
                        let buffer_min_jitter = buffer_max_jitter;
                        buffer_max_jitter += tick_length.as_millis() as i128;

                        if (latency.as_millis() as i128 - client.latency.as_millis() as i128)
                            <= buffer_max_jitter
                            && (latency.as_millis() as i128 - client.latency.as_millis() as i128)
                                >= buffer_min_jitter
                        {
                            buffer.push(packet_tuple);
                            packet_accepted = true;
                            break;
                        }
                    }
                    if packet_accepted {
                        *current_frame_hit_rates.get_mut(&client_index).unwrap() += 1;
                    } else {
                        godot_warn!("ignoring packet due to jitter",);

                        *current_frame_miss_rates.get_mut(&client_index).unwrap() += 1;
                    }
                }
                3 => {
                    client
                        .c3_buffered_packets
                        .insert(packet_number, packet_tuple.0);
                    while let Some(packet) = client
                        .c3_buffered_packets
                        .remove(&client.next_c3_packet_number)
                    {
                        client.next_c3_packet_number += 1;
                        let mut pointer: usize = BYTES2 + BYTES8;
                        self.message_buffer.push_back(packet[pointer..].to_bitvec());
                        let message_type: u16 = packet[pointer..pointer + BYTES2].load_le();
                        pointer += BYTES2;
                        if let Some(handler) = self.message_handlers.get_mut(&message_type) {
                            handler
                                .bind_mut()
                                .handle_message(packet.as_bitslice(), &mut pointer);
                        } else {
                            godot_warn!(
                                "received unhandled message with type: {:#?}",
                                message_type
                            );
                        }
                    }
                }
                5 => {
                    let buffer: Vec<u8> = packet.chunks(BYTE).map(|x| x.load_le::<u8>()).collect();
                    if client.next_c5_packet_number <= packet_number {
                        client
                            .voice_packet_buffer
                            .push((packet_number, buffer[10..].into_iter().copied().collect()));
                    }
                }
                _ => {
                    godot_warn!("unhandled channel: {:#?}", channelid);
                }
            }
        }
        for value in current_frame_hit_rates {
            // dont grow unless packet loss history is partly full to avoid unneeded growth from initial variance
            // no real reason this is here specifically just need somewhere we can get client latency info
            const JITTER_BUFFER_INCREASE_THRESHOLD: f32 = 0.01;
            let client = networker.clients.get_mut(&value.0).unwrap();
            if client.c1_latency_info.c1_miss_rate_average_percent
                > JITTER_BUFFER_INCREASE_THRESHOLD
                && client.c1_latency_info.c1_miss_rate_last_frames.len()
                    >= HIT_RATE_HISTORY_LENGTH / 2
            {
                client.packet_buffers.push_back(Vec::new());
                client.c1_latency_info = LatencyInfo::default()
            }
            networker
                .clients
                .get_mut(&value.0)
                .unwrap()
                .c1_latency_info
                .c1_hit_rate_last_frames
                .push_back(value.1);
        }

        for value in current_frame_miss_rates {
            networker
                .clients
                .get_mut(&value.0)
                .unwrap()
                .c1_latency_info
                .c1_miss_rate_last_frames
                .push_back(value.1);
        }
        for hit_rates in networker
            .clients
            .values_mut()
            .map(|x| &mut x.c1_latency_info.c1_hit_rate_last_frames)
        {
            if hit_rates.len() > HIT_RATE_HISTORY_LENGTH {
                hit_rates.pop_front();
            }
        }
        for miss_rates in networker
            .clients
            .values_mut()
            .map(|x| &mut x.c1_latency_info.c1_miss_rate_last_frames)
        {
            if miss_rates.len() > HIT_RATE_HISTORY_LENGTH {
                miss_rates.pop_front();
            }
        }
        for latency_info in networker
            .clients
            .values_mut()
            .map(|x| &mut x.c1_latency_info)
        {
            latency_info.c1_hit_rate_average =
                latency_info.c1_hit_rate_last_frames.iter().sum::<u64>() as f32
                    / latency_info.c1_hit_rate_last_frames.len() as f32;
            latency_info.c1_miss_rate_average =
                latency_info.c1_miss_rate_last_frames.iter().sum::<u64>() as f32
                    / latency_info.c1_miss_rate_last_frames.len() as f32;
            latency_info.c1_miss_rate_average_percent = latency_info.c1_miss_rate_average
                / (latency_info.c1_hit_rate_average + latency_info.c1_miss_rate_average);
        }
    }
    fn tick_clients(clients: &mut HashMap<ConnectionId, Client>) {
        for client in clients.values_mut() {
            if let Some(mut buffer) = client.packet_buffers.pop_front() {
                buffer.clear();
                client.packet_buffers.push_back(buffer);
            }

            let priorities = client.priorities.iter_mut();
            for priority in priorities {
                if priority.0.bind().owner_id
                    != Vec::from(client.conn.clone()).to_godot().to_packed_array()
                {
                    let p = priority
                        .0
                        .bind()
                        .get_priority(Vec::from(client.conn.clone()).to_godot().to_packed_array());
                    priority.1 += p;
                }
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

#[derive(Debug, Default, Clone)]
enum ClientState {
    #[default]
    AwaitingPSK,
    AwaitingUuid([u8; 40]),
    Connected([u8; 16], ClientSubState)
}

#[derive(Debug, Default, Clone)]
enum ClientSubState {
    #[default]
    EventSync,
    ObjectSync,
    Connected,
}

#[derive(Debug, Default, Clone)]
struct Client {
    conn: ConnectionId<'static>,
    state: ClientState,
    remaining_bandwidth: usize,
    message_buffer_position: usize,
    priorities: Vec<(Gd<NetworkedNode>, i64)>,
    packet_buffers: VecDeque<Vec<BitVec<u64, Lsb0>>>,
}
