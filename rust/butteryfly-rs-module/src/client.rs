// functionallity for the NetNodeManager client
use crate::messages::*;
use crate::net_nodes::NetworkedNode;
use crate::networker::{ConnectionError, ConnectionHandler};
use crate::serializer::*;
use bitvec::prelude::*;
use godot::prelude::*;
use rand::{RngExt, SeedableRng};
use std::collections::{BTreeMap, VecDeque, hash_map};
use std::net::SocketAddr;
use std::{cmp, collections::HashMap};

const BYTES2: usize = 16;
const BYTES8: usize = 64;

const DGRAM_HEADER_SIZE: usize = BYTES2;

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct NetNodeClient {
    connected: ConnectionStatus,
    uuid: [u8; 16],
    networker: ConnectionHandler,
    networked_nodes: Vec<Gd<NetworkedNode>>,
    owned_nodes: Vec<(Gd<NetworkedNode>, i64)>,
    bandwidth_budget_per_tick: usize,
    // the array here is to make each key unique
    unapplied_packets: BTreeMap<(i16, [u8; 16]), BitVec<u64, Lsb0>>,
    incomplete_messages: HashMap<u64, (Option<usize>, BitVec<u64, Lsb0>)>,
    message_buffer: VecDeque<(BitVec<u64, Lsb0>, u64)>,
    message_handlers: HashMap<u64, Gd<MessageHandler>>,
    server_tick_number: i16,
    current_tick: i16,
    base: Base<Node>,
}

#[derive(Default, Debug, Clone, PartialEq)]
enum ConnectionStatus {
    AwaitingConnection([u8; 40]),
    Connected,
    #[default]
    Invalid,
}

#[godot_api]
pub impl NetNodeClient {
    pub fn register_node(&mut self, new_node_ref: Gd<NetworkedNode>, new_node: &mut NetworkedNode) {
        if new_node.owner_id == self.uuid.to_vec().to_godot().to_packed_array() {
            self.owned_nodes.push((new_node_ref.clone(), 0));
        }
        self.networked_nodes.push(new_node_ref);
    }
    pub fn unregister_node(&mut self, removed_node_ref: Gd<NetworkedNode>) {
        let Some(pos) = self
            .networked_nodes
            .iter()
            .position(|x| *x == removed_node_ref)
        else {
            return;
        };
        self.networked_nodes.remove(pos);
        let Some(pos) = self
            .owned_nodes
            .iter()
            .position(|x| x.0 == removed_node_ref)
        else {
            return;
        };
        self.owned_nodes.remove(pos);
    }
    pub fn register_message(&mut self, handler: Gd<MessageHandler>, message_type: u64) {
        if self.message_handlers.contains_key(&message_type) {
            godot_warn!(
                "tried to register duplicate handlers for message type {:#?}",
                message_type
            )
        }
        self.message_handlers.insert(message_type, handler);
    }
    pub fn unregister_message(&mut self, message_type: u64) {
        self.message_handlers.remove(&message_type);
    }
    pub fn queue_message(&mut self, message: BitVec<u64, Lsb0>, stream: u64) {
        self.message_buffer.push_back((message, stream));
    }
    pub fn start_client(
        &mut self,
        server_addr: SocketAddr,
        psk_identifier: String,
        psk_key: Vec<u8>,
        identifier: [u8; 40],
    ) {
        self.networker = ConnectionHandler::new_client(server_addr, psk_identifier, psk_key);
        self.connected = ConnectionStatus::AwaitingConnection(identifier)
    }
    pub fn disconnect(&mut self) {}
    fn tick(&mut self) -> Result<(), ConnectionError> {
        const MESSAGE_HEADER_SIZE: usize = BYTES8;

        self.networker.update()?;

        let server = &self.networker.get_peers(false)[0];

        let mut random = rand::rngs::SmallRng::from_seed(rand::random());

        for stream in self.networker.get_readable_streams(server) {
            while let Ok(stream_chunk) = self.networker.recv_stream(server, stream) {
                if stream_chunk.len() == 0 {
                    break;
                }

                let mut stream_chunk = stream_chunk.as_bitslice();

                if let hash_map::Entry::Occupied(mut entry) = self.incomplete_messages.entry(stream)
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

                    let handler: u64 = incomplete[pointer..pointer + MESSAGE_HEADER_SIZE].load_le();
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
                        self.incomplete_messages
                            .insert(stream, (None, stream_chunk.to_bitvec()));
                        break;
                    }

                    let length: usize = stream_chunk[..BYTES8].load_le();
                    let mut pointer = BYTES8;

                    if stream_chunk.len() < length {
                        self.incomplete_messages
                            .insert(stream, (Some(length), stream_chunk.to_bitvec()));
                        break;
                    }

                    let incomplete_stream;
                    (incomplete_stream, stream_chunk) = stream_chunk.split_at(length);

                    let handler: u64 =
                        incomplete_stream[pointer..pointer + MESSAGE_HEADER_SIZE].load_le();
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

        self.current_tick += 1;

        let mut late_packets: usize = 0;
        let mut total_packets: usize = 0;
        let mut got_next_tick_packet: bool = false;

        while let Ok(packet) = self.networker.recv_datagram(server) {
            let packet: BitVec<u64> = packet;
            if packet.len() < DGRAM_HEADER_SIZE {
                godot_warn!("got c1 packet with invalid size");
                continue;
            }

            let packet_apply_tick: i16 = packet[0..BYTES2].load_le();

            let relative_apply_tick = packet_apply_tick.wrapping_sub(self.server_tick_number);

            total_packets += 1;

            if relative_apply_tick <= self.server_tick_number {
                late_packets += 1;
                continue;
            }

            if relative_apply_tick == self.server_tick_number + 1 {
                got_next_tick_packet = true;
            }

            let mut r = [0; 16];
            random.fill(&mut r);

            self.unapplied_packets
                .insert((packet_apply_tick, r), packet);
        }

        if !got_next_tick_packet {
            self.server_tick_number += 1;
        }

        if late_packets > (total_packets / 100) {
            self.server_tick_number -= 1;
        }
        Ok(())
    }
    fn update_network_nodes(&mut self) {
        while let Some((_, packet)) = self.unapplied_packets.pop_first() {
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
                    let types_buff: Vec<NetworkedValueTypes> = node.get_networked_values_types();
                    node.update_networked_values(&mut pointer, packet.as_bitslice(), &types_buff);
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
    fn send_packets(&mut self) -> Result<(), ConnectionError> {
        const PACKET_MAX_SIZE_THRESHOLD: usize = 80;
        const MINIMUM_CONNECTION_BANDWIDTH: usize = 512;

        let server = &self.networker.get_peers(false)[0];

        self.bandwidth_budget_per_tick = self
            .bandwidth_budget_per_tick
            .max(MINIMUM_CONNECTION_BANDWIDTH);

        let max_dgram_size: usize = self.networker.get_max_dgram_size(server);

        if self.networker.is_connection_pacing() {
            self.bandwidth_budget_per_tick /= 2;
        }

        let mut remaining_bandwidth = self.bandwidth_budget_per_tick;

        // channel 3 (messages)
        while let Some((message, stream)) = self.message_buffer.pop_front() {
            if remaining_bandwidth.checked_sub(message.len()).is_none() {
                break;
            }
            remaining_bandwidth -= message.len();

            self.networker
                .send_stream(server, stream, message.clone())?;
        }

        // channel 1 (syncing)
        while remaining_bandwidth > PACKET_MAX_SIZE_THRESHOLD {
            let mut packet: BitVec<u64> = BitVec::with_capacity(max_dgram_size);

            self.current_tick = self.current_tick.wrapping_add(1);

            packet.extend_from_bitslice((self.current_tick as u16).view_bits::<Lsb0>());
            debug_assert_eq!(DGRAM_HEADER_SIZE, packet.len());

            for (node_ref, priority) in self.owned_nodes.iter_mut() {
                if *priority != 0 {
                    let node = Gd::bind(node_ref);

                    let tmp = node.get_byte_data(&node.get_networked_values_types());

                    drop(node);

                    if tmp.len() + packet.len() > cmp::min(remaining_bandwidth, max_dgram_size) {
                        break;
                    }

                    packet.extend_from_bitslice(tmp.as_bitslice());
                    *priority = 0;
                }
            }

            if packet.len() > DGRAM_HEADER_SIZE {
                remaining_bandwidth -= packet.len();
                self.networker.send_datagram(server, packet)?;
                continue;
            }

            break;
        }
        if remaining_bandwidth <= PACKET_MAX_SIZE_THRESHOLD {
            self.bandwidth_budget_per_tick += self.bandwidth_budget_per_tick / 10;
        }
        Ok(())
    }
    fn tick_priorities(&mut self) {
        for node in self.owned_nodes.iter_mut() {
            node.1 += node.0.bind().get_client_priority();
        }

        self.owned_nodes.sort_by(|a, b| a.1.cmp(&b.1));
    }
}
#[godot_api]
impl INode for NetNodeClient {
    fn physics_process(&mut self, _delta: f64) {
        // todo:
        // splip self methods into smaller fuctions
        // splt sections of methods that dont require self into new functions
        // split common functionality with server into common.rs
        // clean up serializers.rs and net_nodes.rs
        // fix messages.rs
        // run through ai
        // unit tests
        // run unit tests through ai
        // final manual check
        // e2e testing
        // handle disconnection
        if let ConnectionStatus::AwaitingConnection(identifier) = self.connected {
            if self
                .networker
                .is_connected(&self.networker.get_peers(true)[0])
            {
                self.connected = ConnectionStatus::Connected;
                if let Err(e) = self.networker.send_identifier(&identifier) {
                    eprintln!("Failed to send identifier: {:?}", e);
                }
            } else {
                return;
            }
        }

        self.tick_priorities();
        let _ = self
            .tick()
            .inspect_err(|x| godot_error!("error while ticking client: {:?}", x));
        self.update_network_nodes();
        let _ = self
            .send_packets()
            .inspect_err(|x| godot_error!("error while sending packets: {:?}", x));
    }
}
