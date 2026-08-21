use crate::common::{self, MessageAccess};
use crate::common::{
    DGRAM_HEADER_SIZE, InternalMessage, OBJECT_HEADER_SIZE, generate_internal_message,
};
use crate::message_manager::MessageManager;
use crate::net_nodes::NetworkedNode;
use crate::networker::{ConnectionHandler, NetNodesError};
use crate::serializer::NetworkedValueTypes;
use bitvec::prelude::*;
use godot::prelude::*;
use rand::SeedableRng;
use std::collections::HashMap;
use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug)]
pub struct NetNodeClient {
    connected: ConnectionStatus,
    uuid: [u8; 16],
    networker: ConnectionHandler,
    networked_nodes: Vec<Gd<NetworkedNode>>,
    // the Vec is sorted in *ascending* order, generally we should go through it in reverse
    // so that higher priority packets are sent first
    // this is done for consistency with the server's BTreeMap
    owned_nodes: Vec<(Gd<NetworkedNode>, i64)>,
    bandwidth_budget_per_tick: usize,
    // the array here is to make each key unique
    unapplied_packets: BTreeMap<(i8, [u8; 16]), BitVec<u64, Lsb0>>,
    incomplete_messages: HashMap<u64, (Option<usize>, BitVec<u64, Lsb0>)>,
    message_buffer: VecDeque<(BitVec<u64, Lsb0>, u64)>,
    server_tick_number: i8,
    current_tick: i8,
    message_manager: Gd<MessageManager>,
}

#[derive(Debug, Clone, PartialEq)]
enum ConnectionStatus {
    AwaitingConnection([u8; 8]),
    Connected,
    Disconnected,
}

impl NetNodeClient {
    pub fn register_node(&mut self, new_node: &NetworkedNode) {
        if new_node.owner_id == self.uuid.to_vec().to_godot().to_packed_array() {
            self.owned_nodes.push((new_node.to_gd().clone(), 0));
        }
        self.networked_nodes.push(new_node.to_gd());
    }
    pub fn unregister_node(&mut self, removed_node_ref: &Gd<NetworkedNode>) {
        if let Some(pos) = self
            .networked_nodes
            .iter()
            .position(|x| x == removed_node_ref)
        {
            self.networked_nodes.remove(pos);
        } else {
            return;
        }

        if let Some(pos) = self
            .owned_nodes
            .iter()
            .position(|x| &x.0 == removed_node_ref)
        {
            self.owned_nodes.remove(pos);
        }
    }

    pub fn unregister_message(&mut self, message_type: u16) {
        self.message_manager
            .bind_mut()
            .unregister_message(message_type);
    }

    pub fn get_networked_nodes(&self) -> &[Gd<NetworkedNode>] {
        &self.networked_nodes
    }

    pub fn queue_message(&mut self, message: BitVec<u64, Lsb0>, stream: u64) {
        self.message_buffer.push_back((message, stream));
    }

    pub fn get_connection_rtt(&self) -> Option<Duration> {
        let client = self.networker.get_peers(true).pop().unwrap();
        self.networker.get_connection_rtt(&client)
    }

    pub fn new(
        server_addr: SocketAddr,
        uuid: [u8; 16],
        identifier: [u8; 8],
        message_manager: Gd<MessageManager>,
    ) -> Self {
        Self {
            connected: ConnectionStatus::AwaitingConnection(identifier),
            uuid,
            networker: ConnectionHandler::new_client(server_addr),
            networked_nodes: Vec::new(),
            owned_nodes: Vec::new(),
            bandwidth_budget_per_tick: 0,
            unapplied_packets: BTreeMap::new(),
            incomplete_messages: HashMap::new(),
            message_buffer: VecDeque::new(),
            server_tick_number: 0,
            current_tick: 0,
            message_manager,
        }
    }
    pub fn disconnect(&mut self) {
        let tmp = self.networker.get_peers(true);
        let Some(server) = tmp.first() else {
            return;
        };
        self.networker
            .disconnect_peer(server, false, 0, "player disconnected");
        self.connected = ConnectionStatus::Disconnected;
    }

    pub fn has_disconnected(&self) -> bool {
        matches!(self.connected, ConnectionStatus::Disconnected)
    }

    fn tick(&mut self) -> Result<(), NetNodesError> {
        let server = self.networker.get_peers(false).pop().unwrap();
        let server = &server;

        let mut random = rand::rngs::SmallRng::from_seed(rand::random());

        for stream in self.networker.get_readable_streams(server) {
            let (length, data) = self.incomplete_messages.entry(stream).or_default();
            common::handle_stream(
                stream,
                server,
                (length, data),
                &mut self.networker,
                &mut self.message_buffer,
                MessageAccess::Manager(&mut self.message_manager),
            )?;
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

                if next_obj == 0 {
                    if !packet[pointer..].any() {
                        // reached the padding
                        break;
                    }
                    godot_error!("got object with invalid id 0");
                    break;
                }

                if let Some(tmp) = self
                    .networked_nodes
                    .iter_mut()
                    .find(|x| Gd::bind(x).objectid == next_obj)
                {
                    let mut node = Gd::bind_mut(tmp);
                    let types_buff: Vec<NetworkedValueTypes> = node.get_networked_values_types();
                    node.update_networked_values(&mut pointer, packet.as_bitslice(), &types_buff);
                } else {
                    // will give a few spurious errors if we get sync data for a netnode before its creation event
                    godot_warn!(
                        "got update for nonexistent netnode with objectid: {:#?}",
                        next_obj
                    );
                    break;
                }
            }
        }
    }
    fn send_packets(&mut self) -> Result<(), NetNodesError> {
        const PACKET_MAX_SIZE_THRESHOLD: usize = 80;
        const MINIMUM_CONNECTION_BANDWIDTH: usize = 512;

        self.current_tick = self.current_tick.wrapping_add(1);

        let server = self.networker.get_peers(false).pop().unwrap();
        let server = &server;

        self.bandwidth_budget_per_tick = self
            .bandwidth_budget_per_tick
            .max(MINIMUM_CONNECTION_BANDWIDTH);

        let max_dgram_size: usize = self.networker.get_max_dgram_size(server);

        // this might be too aggressive,
        // but we really want to avoid congestion so we dont spike latency
        if self.networker.is_connection_pacing() {
            self.bandwidth_budget_per_tick /= 2;
        }

        let mut remaining_bandwidth = self.bandwidth_budget_per_tick;

        let mut capacity_reached: bool = false;

        // channel 3 (messages)
        while let Some((message, stream)) = self.message_buffer.pop_front() {
            if remaining_bandwidth.checked_sub(message.len()).is_none() {
                self.message_buffer.push_front((message, stream));
                capacity_reached = true;
                break;
            }
            remaining_bandwidth -= message.len();

            if let Err(e) = self.networker.send_stream(server, stream, message.clone()) {
                self.message_buffer.push_front((message, stream));
                if e != NetNodesError::BufferFull {
                    godot_error!("error while sending message to server: {e:?}");
                }
            }
        }

        // channel 1 (syncing)
        while remaining_bandwidth > PACKET_MAX_SIZE_THRESHOLD {
            let mut packet: BitVec<u64> = BitVec::with_capacity(max_dgram_size);

            packet.extend_from_bitslice((self.current_tick.cast_unsigned()).view_bits::<Lsb0>());
            debug_assert_eq!(DGRAM_HEADER_SIZE, packet.len());

            // iterate in reverse because the Vec is sorted ascendingly
            // this is done for consistency with the server's BTreeMap
            for (node_ref, priority) in self.owned_nodes.iter_mut().rev() {
                if *priority != 0 {
                    let node = Gd::bind(node_ref);

                    let tmp = node.get_byte_data(&node.get_networked_values_types());

                    drop(node);

                    if tmp.len() + packet.len() > max_dgram_size {
                        break;
                    }
                    if tmp.len() + packet.len() > remaining_bandwidth {
                        capacity_reached = true;
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
        if capacity_reached {
            self.bandwidth_budget_per_tick += self.bandwidth_budget_per_tick / 10;
        }
        Ok(())
    }
    fn tick_priorities(owned_nodes: &mut [(Gd<NetworkedNode>, i64)]) {
        for node in owned_nodes.iter_mut() {
            node.1 += node.0.bind().get_client_priority();
        }

        // sort ascendingly for consistency with the server's BTreeMap
        owned_nodes.sort_by(|a, b| a.1.cmp(&b.1));
    }
    pub fn physics_process_inner(&mut self) {
        let tmp = self.networker.get_peers(true);
        let Some(server) = tmp.first() else {
            godot_error!("client is not connected to the server");
            self.disconnect();
            return;
        };

        if let Err(e) = self.networker.update() {
            godot_error!("failed to update client networker: {e:?}");
            if e == NetNodesError::Disconnected {
                self.disconnect();
            }
            return;
        }

        if let ConnectionStatus::AwaitingConnection(ref identifier) = self.connected {
            if self.networker.is_connected(server) {
                let packet = generate_internal_message(InternalMessage::ClientId(*identifier));
                if let Err(e) = self.networker.send_stream(server, 0, packet) {
                    godot_error!("failed to send identifier {e:?}");
                    return;
                }
                self.connected = ConnectionStatus::Connected;
            } else {
                return;
            }
        }

        Self::tick_priorities(&mut self.owned_nodes);

        if let Err(e) = self.tick() {
            godot_error!("failed to tick client: {e:?}");
        }

        self.update_network_nodes();

        if let Err(e) = self.send_packets() {
            godot_error!("error while sending packets: {e:?}");
        }

        if let Err(e) = self.networker.update() {
            godot_error!("failed to update client networker: {e:?}");
        }
    }
}

impl Drop for NetNodeClient {
    fn drop(&mut self) {
        self.message_manager.queue_free();
    }
}
