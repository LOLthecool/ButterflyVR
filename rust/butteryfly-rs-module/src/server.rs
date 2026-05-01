use crate::common::{
    BYTES2, DGRAM_HEADER_SIZE, InternalMessage, NetNodesConnectionId, OBJECT_HEADER_SIZE,
    generate_internal_message,
};
use crate::net_nodes::NetworkedNode;
use crate::networker::{ConnectionHandler, NetNodesError};
use crate::serializer::NetworkedValueTypes;
use crate::{common, messages::MessageHandler};
use bitvec::prelude::*;
use godot::prelude::*;
use quiche::ConnectionId;
use rand::{RngExt, SeedableRng};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::mem;
use std::{cmp, collections::HashMap};

#[derive(Debug)]
pub struct NetNodeServer {
    clients: HashMap<NetNodesConnectionId, Client>,
    networked_nodes: Vec<Gd<NetworkedNode>>,
    networker: ConnectionHandler,
    // todo: add a way for MessageHandlers to mark old messages as irrelevant so we can discard them
    // right now this just grows forever
    // as a tf2 dev would say "this leaks memory. too bad!"
    message_buffer: VecDeque<(BitVec<u64, Lsb0>, u64)>,
    message_handlers: HashMap<u16, Gd<MessageHandler>>,
    current_tick: i8,
    last_netnode_id: u16,
    last_message_id: u16,
}

impl NetNodeServer {
    pub fn get_player_count(&self) -> usize {
        self.networker.get_peers(false).len()
    }

    pub fn register_node(&mut self, mut new_node_ref: Gd<NetworkedNode>) {
        new_node_ref.bind_mut().objectid = self.get_next_object_id();

        let message = generate_internal_message(InternalMessage::NetNodeId((
            new_node_ref.bind().objectid,
            new_node_ref.clone().upcast(),
        )))
        .unwrap();
        self.queue_message(message, 0);

        self.networked_nodes.push(new_node_ref);
    }

    pub fn unregister_node(&mut self, removed_node_ref: &Gd<NetworkedNode>) {
        for client in self.clients.values_mut().filter_map(|x| {
            let ClientState::Connected(ref mut client) = x.state else {
                return None;
            };
            Some(client)
        }) {
            let p = mem::take(&mut client.priorities);
            client.priorities = p.into_iter().filter(|x| &x.1 != removed_node_ref).collect();
        }
        if let Some(idx) = self
            .networked_nodes
            .iter()
            .position(|x| x == removed_node_ref)
        {
            self.networked_nodes.remove(idx);
        }
    }

    pub fn get_next_object_id(&mut self) -> u16 {
        // todo: should probably try to reuse object ids from removed nodes
        // if we manage to actually have no free ids, panicing is fine
        self.last_netnode_id = self.last_netnode_id.checked_add(1).unwrap();
        self.last_netnode_id
    }

    pub fn get_next_message_id(&mut self) -> u16 {
        self.last_message_id = self.last_message_id.checked_add(1).unwrap();
        self.last_message_id
    }

    pub fn register_message(&mut self, mut handler: Gd<MessageHandler>, message_type: u16) {
        handler.bind_mut().message_id = self.get_next_message_id();

        let message = generate_internal_message(InternalMessage::MessageHandlerId((
            handler.bind().message_id,
            handler.clone().upcast(),
        )))
        .unwrap();
        self.queue_message(message, 0);

        self.message_handlers.insert(message_type, handler);
    }

    pub fn unregister_message(&mut self, message_type: u16) {
        self.message_handlers.remove(&message_type);
    }

    pub fn queue_message(&mut self, message: BitVec<u64, Lsb0>, stream: u64) {
        self.message_buffer.push_back((message, stream));
    }

    pub fn new(bind_port: u16) -> Self {
        Self {
            clients: HashMap::new(),
            networked_nodes: Vec::new(),
            networker: ConnectionHandler::new_server(bind_port),
            message_buffer: VecDeque::new(),
            message_handlers: HashMap::new(),
            current_tick: 0,
            last_netnode_id: 0,
            last_message_id: 0,
        }
    }

    pub fn get_next_client(&mut self) -> Result<[u8; 40], NetNodesError> {
        let psk_identifier = rand::rng().random::<[u8; 8]>();
        let psk_key = rand::rng().random::<[u8; 32]>();
        self.networker.add_client_token(psk_identifier, psk_key)?;
        let mut token = Vec::from(psk_identifier);
        token.extend(&psk_key);
        Ok(token.try_into().unwrap())
    }

    fn tick_client_priorities(
        clients: &mut HashMap<NetNodesConnectionId, Client>,
        networked_nodes: &[Gd<NetworkedNode>],
    ) {
        let mut random = rand::rngs::SmallRng::from_seed(rand::random());
        for (conn, client) in clients.iter_mut() {
            if let ClientState::Connected(ref mut client) = client.state {
                // this doesn't catch some changes to networked_nodes, but that should be fine
                if client.priorities.len() != networked_nodes.len() {
                    client.priorities.clear();
                    for node_ref in networked_nodes {
                        let mut r = [0; 16];
                        random.fill(&mut r);
                        client.priorities.insert((0, r), node_ref.clone());
                    }
                }
                let old_map = mem::take(&mut client.priorities);
                client.priorities = old_map
                    .into_iter()
                    .map(|mut priority| {
                        let p = priority.1.bind().get_server_priority(
                            Vec::from(ConnectionId::from(conn))
                                .to_godot()
                                .to_packed_array(),
                        );
                        priority.0.0 += p;
                        priority
                    })
                    .collect();
            }
        }
    }

    fn tick(&mut self) -> std::result::Result<(), NetNodesError> {
        Self::tick_client_priorities(&mut self.clients, &self.networked_nodes);

        self.networker.update()?;

        Self::update_client_list(&mut self.clients, &self.networker);

        let mut random = rand::rngs::SmallRng::from_seed(rand::random());

        for (client_id, client) in &mut self.clients {
            match client.state {
                ClientState::AwaitingIdentifier(ref mut data) => {
                    if let Ok(new_data) =
                        self.networker
                            .recv_stream_bytes(client_id, 0, 48 - data.len())
                    {
                        data.extend(new_data.into_vec());

                        if data.len() >= 48 {
                            // need to skip the length prefix
                            client.state = ClientState::AwaitingUuid(data[8..].try_into().unwrap());
                        }
                    }
                }
                ClientState::AwaitingUuid(_) => {}
                ClientState::Connected(ref mut client) => {
                    for stream in self.networker.get_readable_streams(client_id) {
                        while let Ok(stream_chunk) = self.networker.recv_stream(client_id, stream) {
                            common::handle_stream_chunk(
                                stream,
                                &stream_chunk,
                                &mut client.incomplete_messages,
                                &mut self.message_buffer,
                                &mut self.message_handlers,
                                true,
                                None,
                            );
                        }
                    }

                    common::handle_datagrams(
                        client_id,
                        &mut client.tick_number,
                        &mut client.unapplied_packets,
                        &mut self.networker,
                        &mut random,
                    );
                }
            }
        }
        Ok(())
    }

    fn update_network_nodes(&mut self) {
        for client in self.clients.values_mut() {
            if let ClientState::Connected(ref mut client) = client.state {
                while let Some(((apply_tick, _), _)) = client.unapplied_packets.first_key_value() {
                    if apply_tick - client.tick_number > 0 {
                        break;
                    }

                    let (_, packet) = client.unapplied_packets.pop_first().unwrap();

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

    fn send_packets(&mut self) -> std::result::Result<(), NetNodesError> {
        const PACKET_MAX_SIZE_THRESHOLD: usize = 80;

        for (conn, client) in &mut self.clients {
            if let ClientState::Connected(ref mut client) = client.state {
                const MINIMUM_CONNECTION_BANDWIDTH: usize = 512;

                client.bandwidth_budget_per_tick = client
                    .bandwidth_budget_per_tick
                    .max(MINIMUM_CONNECTION_BANDWIDTH);

                let max_dgram_size: usize = self.networker.get_max_dgram_size(conn);

                // this might be too aggressive,
                // but we really want to avoid congestion so we dont spike latency
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
                    self.networker.send_stream(conn, *stream, message.clone())?;
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

                    if let ClientSubState::ObjectSync(ref mut objects) = client.state {
                        self.current_tick = self.current_tick.wrapping_add(1);

                        packet.extend_from_bitslice(
                            (self.current_tick.cast_unsigned()).view_bits::<Lsb0>(),
                        );

                        while let Some(object) = objects.last_mut() {
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
                        }

                        if objects.last_mut().is_none() {
                            client.state = ClientSubState::Connected;
                        }

                        remaining_bandwidth -= packet.len();
                        self.networker.send_datagram(conn, packet)?;
                        continue;
                    } else {
                        self.current_tick = self.current_tick.wrapping_add(1);

                        packet.extend_from_bitslice(
                            (self.current_tick.cast_unsigned()).view_bits::<Lsb0>(),
                        );
                        debug_assert_eq!(DGRAM_HEADER_SIZE, packet.len());

                        // iterate in reverse because the btree is sorted ascendingly
                        let old_map = mem::take(&mut client.priorities);
                        client.priorities = old_map
                            .into_iter()
                            .rev()
                            .map(|mut value| {
                                if value.0.0 != 0 {
                                    let node_ref = &value.1;
                                    let node = Gd::bind(node_ref);

                                    let tmp =
                                        node.get_byte_data(&node.get_networked_values_types());

                                    if tmp.len() + packet.len() + BYTES2
                                        > cmp::min(remaining_bandwidth, max_dgram_size)
                                    {
                                        drop(node);
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
                            self.networker.send_datagram(conn, packet)?;
                            continue;
                        }

                        break;
                    }
                }
                if remaining_bandwidth <= PACKET_MAX_SIZE_THRESHOLD {
                    client.bandwidth_budget_per_tick += client.bandwidth_budget_per_tick / 10;
                }
            }
        }
        Ok(())
    }

    fn update_client_list(
        clients: &mut HashMap<NetNodesConnectionId, Client>,
        networker: &ConnectionHandler,
    ) {
        let client_list: HashSet<NetNodesConnectionId> =
            networker.get_peers(false).into_iter().collect();
        let tmp = clients.keys().cloned().collect();

        let new_players: Vec<&NetNodesConnectionId> = client_list.difference(&tmp).collect();
        let dc_clients: Vec<&NetNodesConnectionId> = tmp.difference(&client_list).collect();

        for player in new_players {
            let player: &NetNodesConnectionId = player;
            clients.insert(player.clone(), Client::new());
        }

        for dc_client in dc_clients {
            clients.remove(dc_client);
        }
    }

    pub fn get_unverified_client(&self) -> Option<[u8; 40]> {
        self.clients.iter().find_map(|x| match x.1.state {
            ClientState::AwaitingUuid(x) => Some(x),
            _ => None,
        })
    }

    pub fn verify_client(&mut self, identifier: [u8; 40], uuid: [u8; 16]) {
        if let Some(client) = self
            .clients
            .iter_mut()
            .find(|x| x.1.state == ClientState::AwaitingUuid(identifier))
        {
            client.1.state = ClientState::Connected(ConnectedClient::new(uuid));
        }
    }

    pub fn stop(&mut self) {
        for client in self.networker.get_peers(true) {
            self.networker
                .disconnect_peer(&client, false, 0, "server shutting down");
        }
        let _ = self.networker.update();
    }

    pub fn physics_process_inner(&mut self) {
        let _ = self
            .tick()
            .inspect_err(|x| godot_error!("error while ticking server: {:?}", x));

        self.update_network_nodes();

        let _ = self
            .send_packets()
            .inspect_err(|x| godot_error!("error while sending packets: {:?}", x));
    }
}

#[derive(Debug, Clone)]
struct Client {
    state: ClientState,
}

impl Client {
    fn new() -> Self {
        Self {
            state: ClientState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ClientState {
    AwaitingIdentifier(Vec<u8>),
    AwaitingUuid([u8; 40]),
    Connected(ConnectedClient),
}

impl Default for ClientState {
    fn default() -> Self {
        Self::AwaitingIdentifier(Vec::new())
    }
}
#[derive(Debug, Clone, PartialEq)]
struct ConnectedClient {
    uuid: [u8; 16],
    state: ClientSubState,
    bandwidth_budget_per_tick: usize,
    incomplete_messages: HashMap<u64, (Option<usize>, BitVec<u64, Lsb0>)>,
    message_buffer_position: usize,
    // the array here is to make each key unique
    // the BTree is sorted in *ascending* order, generally we should go through it in reverse
    // so that higher priority packets are sent first
    priorities: BTreeMap<(i64, [u8; 16]), Gd<NetworkedNode>>,
    unapplied_packets: BTreeMap<(i8, [u8; 16]), BitVec<u64, Lsb0>>,
    tick_number: i8,
}

impl ConnectedClient {
    fn new(uuid: [u8; 16]) -> Self {
        const INITIAL_CLIENT_BANDWIDTH: usize = 8000;
        Self {
            uuid,
            state: ClientSubState::default(),
            bandwidth_budget_per_tick: INITIAL_CLIENT_BANDWIDTH,
            incomplete_messages: HashMap::new(),
            message_buffer_position: 0,
            priorities: BTreeMap::new(),
            unapplied_packets: BTreeMap::new(),
            tick_number: 0,
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
