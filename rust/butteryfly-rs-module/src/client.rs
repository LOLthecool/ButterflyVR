use crate::common::{DGRAM_HEADER_SIZE, OBJECT_HEADER_SIZE};
use crate::net_nodes::NetworkedNode;
use crate::networker::{ConnectionError, ConnectionHandler};
use crate::serializer::NetworkedValueTypes;
use crate::{common, messages::MessageHandler};
use bitvec::prelude::*;
use godot::prelude::*;
use rand::SeedableRng;
use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;
use std::{cmp, collections::HashMap};

#[derive(Default)]
pub struct NetNodeClient {
    connected: ConnectionStatus,
    uuid: [u8; 16],
    networker: ConnectionHandler,
    networked_nodes: Vec<Gd<NetworkedNode>>,
    owned_nodes: Vec<(Gd<NetworkedNode>, i64)>,
    bandwidth_budget_per_tick: usize,
    // the array here is to make each key unique
    unapplied_packets: BTreeMap<(i8, [u8; 16]), BitVec<u64, Lsb0>>,
    incomplete_messages: HashMap<u64, (Option<usize>, BitVec<u64, Lsb0>)>,
    message_buffer: VecDeque<(BitVec<u64, Lsb0>, u64)>,
    message_handlers: HashMap<u64, Gd<MessageHandler>>,
    server_tick_number: i8,
    current_tick: i8,
}

#[derive(Default, Debug, Clone, PartialEq)]
enum ConnectionStatus {
    AwaitingConnection(Vec<u8>),
    Connected,
    #[default]
    Invalid,
}

pub impl NetNodeClient {
    pub fn register_node(&mut self, new_node_ref: Gd<NetworkedNode>, new_node: &NetworkedNode) {
        if new_node.owner_id == self.uuid.to_vec().to_godot().to_packed_array() {
            self.owned_nodes.push((new_node_ref.clone(), 0));
        }
        self.networked_nodes.push(new_node_ref);
    }
    pub fn unregister_node(&mut self, removed_node_ref: &Gd<NetworkedNode>) {
        let Some(pos) = self
            .networked_nodes
            .iter()
            .position(|x| x == removed_node_ref)
        else {
            return;
        };
        self.networked_nodes.remove(pos);
        let Some(pos) = self
            .owned_nodes
            .iter()
            .position(|x| &x.0 == removed_node_ref)
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
            );
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
    ) {
        self.networker = ConnectionHandler::new_client(server_addr, psk_identifier, psk_key);
        let mut identifier = psk_identifier.as_bytes().to_vec();
        identifier.extend(psk_key);
        self.connected = ConnectionStatus::AwaitingConnection(identifier);
    }
    pub fn disconnect(&mut self) {
        todo!()
    }
    fn tick(&mut self) -> Result<(), ConnectionError> {
        self.networker.update()?;

        let server = self.networker.get_peers(false).pop().unwrap();
        let server = &server;

        let mut random = rand::rngs::SmallRng::from_seed(rand::random());

        for stream in self.networker.get_readable_streams(server) {
            while let Ok(stream_chunk) = self.networker.recv_stream(server, stream) {
                common::handle_stream_chunk(
                    stream,
                    &stream_chunk,
                    &mut self.incomplete_messages,
                    &mut self.message_buffer,
                    &mut self.message_handlers,
                );
            }
        }

        common::handle_datagrams(
            server,
            &mut self.server_tick_number,
            &mut self.unapplied_packets,
            &mut self.networker,
            &mut random,
        );
        Ok(())
    }
    fn update_network_nodes(&mut self) {
        while let Some((_, packet)) = self.unapplied_packets.pop_first() {
            let mut pointer: usize = DGRAM_HEADER_SIZE;
            while pointer + OBJECT_HEADER_SIZE <= packet.len() {
                let next_obj: u16 = packet[pointer..pointer + OBJECT_HEADER_SIZE].load_le();
                pointer += OBJECT_HEADER_SIZE;
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

        let server = self.networker.get_peers(false).pop().unwrap();
        let server = &server;

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

            packet.extend_from_bitslice((self.current_tick as u8).view_bits::<Lsb0>());
            debug_assert_eq!(DGRAM_HEADER_SIZE, packet.len());

            for (node_ref, priority) in &mut self.owned_nodes {
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
    fn tick_priorities(owned_nodes: &mut [(Gd<NetworkedNode>, i64)]) {
        for node in owned_nodes.iter_mut() {
            node.1 += node.0.bind().get_client_priority();
        }

        owned_nodes.sort_by(|a, b| a.1.cmp(&b.1));
    }
    fn physics_process(&mut self, _delta: f64) {
        if let ConnectionStatus::AwaitingConnection(identifier) = self.connected {
            if self
                .networker
                .is_connected(self.networker.get_peers(true).first().unwrap())
            {
                self.connected = ConnectionStatus::Connected;
                if let Err(e) = self.networker.send_identifier(&identifier) {
                    godot_error!("Failed to send identifier: {:?}", e);
                }
            } else {
                return;
            }
        }

        Self::tick_priorities(&mut self.owned_nodes);

        let _ = self
            .tick()
            .inspect_err(|x| godot_error!("error while ticking client: {:?}", x));

        self.update_network_nodes();

        let _ = self
            .send_packets()
            .inspect_err(|x| godot_error!("error while sending packets: {:?}", x));
    }
}
