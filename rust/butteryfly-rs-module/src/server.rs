// functionallity for the NetNodeManager server
use crate::net_nodes::NetworkedNode;
use crate::networker::ConnectionHandler;
use crate::serializer::*;
use crate::{messages::*, networker};
use bitvec::prelude::*;
use godot::classes::Engine;
use godot::prelude::*;
use quiche::{Connection, ConnectionId};
use rand::{RngExt, SeedableRng};
use std::collections::btree_map::{Entry, OccupiedEntry};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fmt::DebugTuple;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
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
    networker: ConnectionHandler<'static>,
    message_buffer: VecDeque<BitVec<u64, Lsb0>>,
    message_handlers: HashMap<u16, Gd<MessageHandler>>,
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
        self.queue_message(MessageHandler::create_id_sync_message(
            new_node_ref.clone().upcast(),
            new_node.objectid,
            Some(new_node.owner_id),
        ));
        self.networked_nodes.push(new_node_ref);
    }
    pub fn unregister_node(&mut self, removed_node_ref: Gd<NetworkedNode>) {
        for client in self.networker.clients.values_mut() {
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
        self.networker = ServerNetworker {
            server: Server::new(bind_addr, PROTOCOL_ID, private_key).unwrap(),
            public_address: public_addr,
            private_key,
            ..Default::default()
        };
    }
    pub fn get_next_client(&mut self) -> PackedByteArray {
        let mut result: PackedByteArray = PackedByteArray::new();
        let tmp: [u8; netcode::CONNECT_TOKEN_BYTES] =
            self.networker.get_token().try_into_bytes().unwrap();
        result.extend(tmp);
        result
    }
    pub fn register_player_object(&mut self, client_id: u16, object: Gd<Node3D>) {
        let client = self
            .networker
            .clients
            .values_mut()
            .find(|x| x.id == client_id);
        if let Some(client) = client {
            client.player_position_object = Some(object);
        }
    }
    fn update_network_nodes(&mut self) {
        for client in self.clients.values_mut() {
            while let Some(mut entry) = client.unapplied_packets.first_entry() {
                let packet = entry.get_mut();
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

    fn send_packets_server(&mut self) {
        const PACKET_MAX_SIZE_THRESHOLD: usize = 80;
        const MAX_SINGLE_PACKET_PAYLOAD_LENGTH: usize = 4800;

        for client in self.clients.values_mut() {
            'outer: while client.remaining_bandwidth > PACKET_MAX_SIZE_THRESHOLD {
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

                    if packet.len() > DGRAM_HEADER_SIZE {
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
                if packet.len() > DGRAM_HEADER_SIZE {
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
        const PACKET_LATENCY_DISCARD_THRESHOLD: Duration = Duration::from_millis(1000);
        const LATENCY_BUFFER_MAX_SIZE: usize = 8;

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
        }

        for dc_client in dc_clients {
            self.signals()
                .player_left()
                .emit(dc_client.iter().as_slice().to_vec());
            self.clients.remove(&dc_client);
        }

        let mut random = rand::rngs::SmallRng::from_seed(rand::random());

        for (client_id, client) in self.clients.iter_mut() {
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

                let relative_apply_tick = packet_apply_tick.wrapping_sub(client.tick_number);

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
    fn tick_clients(clients: &mut HashMap<ConnectionId, Client>) {
        for client in clients.values_mut() {
            let priorities = client.priorities.iter_mut();
            for priority in priorities {
                let p = priority
                    .0
                    .bind()
                    .get_priority(Vec::from(client.conn.clone()).to_godot().to_packed_array());
                priority.1 += p;
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
    Connected([u8; 16], ClientSubState),
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
    // the array here is to make each key unique
    unapplied_packets: BTreeMap<(i16, [u8; 16]), BitVec<u64, Lsb0>>,
    tick_number: i16,
}
