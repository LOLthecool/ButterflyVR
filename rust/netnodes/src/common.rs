use crate::{
    message_manager::MessageManager,
    messages::MessageHandler,
    net_nodes::NetworkedNode,
    networker::{ConnectionHandler, NetNodesError},
};
use bitvec::prelude::*;
use godot::prelude::*;
use rand::RngExt;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    rc::Rc,
};

pub const BYTE: usize = 8;
pub const BYTES2: usize = BYTE * 2;
pub const BYTES4: usize = BYTE * 4;
pub const BYTES8: usize = BYTE * 8;

pub const MESSAGE_HEADER_SIZE: usize = BYTES2;
pub const DGRAM_HEADER_SIZE: usize = BYTE;
pub const OBJECT_HEADER_SIZE: usize = BYTES2;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetNodesConnectionId {
    id_bytes: Rc<Vec<u8>>,
}

impl From<quiche::ConnectionId<'_>> for NetNodesConnectionId {
    fn from(value: quiche::ConnectionId<'_>) -> Self {
        Self {
            id_bytes: Rc::new(value.into()),
        }
    }
}

impl<'a> From<&'a NetNodesConnectionId> for quiche::ConnectionId<'a> {
    fn from(value: &'a NetNodesConnectionId) -> Self {
        Self::from_ref(Rc::as_ref(&value.id_bytes).as_slice())
    }
}

impl From<NetNodesConnectionId> for quiche::ConnectionId<'_> {
    fn from(value: NetNodesConnectionId) -> Self {
        Self::from_vec(Rc::unwrap_or_clone(value.id_bytes))
    }
}

pub enum MessageAccess<'a> {
    Handlers(&'a mut HashMap<u16, Gd<MessageHandler>>),
    Manager(&'a mut Gd<MessageManager>),
}

pub fn handle_stream(
    stream: u64,
    peer: &NetNodesConnectionId,
    previous_data: (&mut Option<usize>, &mut BitVec<u64, Lsb0>),
    networker: &mut ConnectionHandler,
    message_buffer: &mut VecDeque<(BitVec<u64, Lsb0>, u64)>,
    // this function requires that message access corrosponds to the peer being either a client or a server
    // clients should use manager while servers should use handlers
    mut message_access: MessageAccess,
) -> Result<(), NetNodesError> {
    let (previous_length_bytes, data) = previous_data;

    loop {
        if previous_length_bytes.is_none() {
            data.extend_from_bitslice(&networker.recv_stream(
                peer,
                stream,
                8 - (data.len() / BYTE),
            )?);
            if data.len() == BYTES8 {
                *previous_length_bytes = Some(data.load_le::<usize>());
                data.clear();
            } else {
                return Ok(());
            }
        }

        let length_bytes = previous_length_bytes.as_mut().unwrap();

        assert!(data.len().is_multiple_of(BYTE));
        let remaining = *length_bytes - (data.len() / BYTE);

        data.extend_from_bitslice(&networker.recv_stream(peer, stream, remaining)?);

        if (data.len() / BYTE) != *length_bytes {
            return Ok(());
        }

        let handler: u16 = data[0..MESSAGE_HEADER_SIZE].load_le();
        let mut pointer = MESSAGE_HEADER_SIZE;

        if handler == 0 {
            if let MessageAccess::Manager(ref mut manager) = message_access {
                let data = data.to_owned();
                manager.run_deferred(|this| this.handle_internal_message(data));
            } else {
                godot_error!("server got internal message after client_id");
            }
        } else {
            match message_access {
                MessageAccess::Handlers(ref mut message_handlers) => {
                    if let Some(handler) = message_handlers.get_mut(&handler) {
                        let (values, types) =
                            handler
                                .bind_mut()
                                .handle_message(data, &mut pointer, true, true);
                        message_buffer.push_back((
                            MessageHandler::generate_packet(
                                &values,
                                &types,
                                handler.bind().message_id,
                            ),
                            stream,
                        ));
                    } else {
                        godot_error!("received a message but had no handler for it: {handler:?}");
                    }
                }
                MessageAccess::Manager(ref mut manager) => {
                    let mut data = data.clone();
                    manager.run_deferred(move |this| {
                        this.handle_message(&mut data, handler, &mut pointer)
                    });
                }
            }
        }

        *previous_length_bytes = None;
        data.clear();
    }
}

pub fn handle_datagrams(
    client_id: &NetNodesConnectionId,
    tick_number: &mut i8,
    unapplied_packets: &mut BTreeMap<(i8, [u8; 16]), BitVec<u64, Lsb0>>,
    networker: &mut ConnectionHandler,
    random: &mut rand::rngs::SmallRng,
) {
    *tick_number = tick_number.wrapping_add(1);

    let mut late_packets: usize = 0;
    let mut total_packets: usize = 0;
    let mut got_soon_packet: bool = false;

    while let Ok(packet) = networker.recv_datagram(client_id) {
        let packet: BitVec<u64> = packet;
        if packet.len() < DGRAM_HEADER_SIZE {
            godot_warn!("got c1 packet with invalid size");
            continue;
        }

        let packet_apply_tick: i8 = packet[0..BYTE].load_le();

        let relative_apply_tick = packet_apply_tick.wrapping_sub(*tick_number);

        total_packets += 1;

        if relative_apply_tick <= 0 {
            late_packets += 1;
            continue;
        }

        if relative_apply_tick <= 2 {
            got_soon_packet = true;
        }

        let mut r = [0; 16];
        random.fill(&mut r);

        unapplied_packets.insert((packet_apply_tick, r), packet);
    }

    // if no packets will be processed soon we can skip ahead without the user noticing too much
    // can happen if network latency decreases since we get future packets sooner
    if (!got_soon_packet) && total_packets > 0 && late_packets == 0 {
        *tick_number = tick_number.wrapping_add(1);
    }

    // 25 is not the percentage, this triggers when late packets > 4%
    if late_packets * 25 > total_packets {
        *tick_number = tick_number.wrapping_sub(1);
    }
}

pub enum InternalMessage {
    ClientId([u8; 8]),
    NetNodeIdAssign((u16, Gd<NetworkedNode>)),
    MessageHandlerIdAssign((u16, Gd<MessageHandler>)),
}

pub fn generate_internal_message(message: InternalMessage) -> BitVec<u64, Lsb0> {
    let mut packet: BitVec<u64, Lsb0> = BitVec::new();
    packet.extend(0u16.view_bits::<Lsb0>());
    match message {
        InternalMessage::ClientId(id) => {
            let id: Vec<u8> = id.to_vec();
            let (id, _) = id.as_chunks::<8>();
            let id: Vec<u64> = id.iter().map(|x| u64::from_le_bytes(*x)).collect();

            packet.extend(BitVec::<u64, Lsb0>::from_vec(id));

            packet
        }
        InternalMessage::NetNodeIdAssign((node_id, node)) => {
            packet.push(true);

            packet.extend(node_id.view_bits::<Lsb0>());

            let path = get_node_path(node.upcast::<Node>());

            packet.extend((path.len() as u32).view_bits::<Lsb0>());
            for idx in path {
                packet.extend(idx.view_bits::<Lsb0>());
            }

            packet
        }
        InternalMessage::MessageHandlerIdAssign((handler_id, node)) => {
            packet.push(false);

            packet.extend(handler_id.view_bits::<Lsb0>());

            let path = get_node_path(node.upcast::<Node>());

            packet.extend((path.len() as u32).view_bits::<Lsb0>());
            for idx in path {
                packet.extend(idx.view_bits::<Lsb0>());
            }

            packet
        }
    }
}

pub fn decode_internal_message(
    packet: &BitSlice<u64, Lsb0>,
    pointer: &mut usize,
    scene_access: &Gd<Node>,
) -> Result<InternalMessage, NetNodesError> {
    if packet.len() < *pointer + 1 + BYTES2 + BYTES4 {
        return Err(NetNodesError::InvalidDatagramLength);
    }

    let is_netnode = packet[*pointer];
    *pointer += 1;

    if is_netnode {
        let node_id = packet[*pointer..*pointer + BYTES2].load_le();
        *pointer += BYTES2;

        let path_length = packet[*pointer..*pointer + BYTES4].load_le();
        *pointer += BYTES4;
        let mut path = VecDeque::with_capacity(path_length);

        for _ in 0..path_length {
            let segment = packet[*pointer..*pointer + BYTES4].load_le();
            *pointer += BYTES4;
            path.push_back(segment);
        }

        let mut node: Option<Gd<Node>> = scene_access
            .get_tree_or_null()
            .and_then(|x| x.get_root())
            .map(Gd::upcast);

        while let Some(idx) = path.pop_front() {
            let Some(n) = node else {
                return Err(NetNodesError::InvalidDatagram);
            };
            node = n.get_child_ex(idx).include_internal(true).done();
        }

        if let Some(Ok(node)) = node.clone().map(Gd::try_cast) {
            Ok(InternalMessage::NetNodeIdAssign((node_id, node)))
        } else {
            Err(NetNodesError::InvalidDatagram)
        }
    } else {
        let handler_id = packet[*pointer..*pointer + BYTES2].load_le();
        *pointer += BYTES2;

        let path_length = packet[*pointer..*pointer + BYTES4].load_le();
        *pointer += BYTES4;
        let mut path = VecDeque::with_capacity(path_length);

        for _ in 0..path_length {
            let segment = packet[*pointer..*pointer + BYTES4].load_le();
            *pointer += BYTES4;
            path.push_back(segment);
        }

        let mut node: Option<Gd<Node>> = scene_access
            .get_tree_or_null()
            .and_then(|x| x.get_root())
            .map(Gd::upcast);

        while let Some(idx) = path.pop_front() {
            let Some(n) = node else {
                return Err(NetNodesError::InvalidDatagram);
            };
            node = n.get_child_ex(idx).include_internal(true).done();
        }

        if let Some(Ok(node)) = node.clone().map(Gd::try_cast) {
            Ok(InternalMessage::MessageHandlerIdAssign((handler_id, node)))
        } else {
            Err(NetNodesError::InvalidDatagram)
        }
    }
}

fn get_node_path(node: Gd<Node>) -> VecDeque<u32> {
    let mut path = VecDeque::new();

    let mut current = node;
    let mut idx = current.get_index_ex().include_internal(true).done();

    while idx != -1 {
        path.push_front(idx.try_into().unwrap());
        // parent will always exist if idx != -1
        current = current.get_parent().unwrap();
        idx = current.get_index_ex().include_internal(true).done();
    }

    path
}
