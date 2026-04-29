use crate::{messages::MessageHandler, networker::ConnectionHandler};
use bitvec::prelude::*;
use godot::prelude::*;
use rand::RngExt;
use std::{
    collections::{BTreeMap, HashMap, VecDeque, hash_map},
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

pub fn handle_stream_chunk(
    stream: u64,
    stream_chunk: &BitVec<u64, Lsb0>,
    incomplete_message_buffer: &mut HashMap<u64, (Option<usize>, BitVec<u64, Lsb0>)>,
    message_buffer: &mut VecDeque<(BitVec<u64, Lsb0>, u64)>,
    message_handlers: &mut HashMap<u16, Gd<MessageHandler>>,
    is_server: bool,
) {
    if stream_chunk.is_empty() {
        return;
    }

    let mut stream_chunk = stream_chunk.as_bitslice();

    if let hash_map::Entry::Occupied(mut entry) = incomplete_message_buffer.entry(stream) {
        let &mut (ref mut length, ref mut incomplete) = entry.get_mut();

        let length_bytes_missing = BYTES8.saturating_sub(incomplete.len());
        if stream_chunk.len() < length_bytes_missing {
            incomplete.extend_from_bitslice(stream_chunk);
            return;
        }

        let length = length.unwrap_or_else(|| {
            incomplete.extend_from_bitslice(&stream_chunk[..length_bytes_missing]);

            (_, stream_chunk) = stream_chunk.split_at(length_bytes_missing);

            *length = Some(incomplete[..BYTES8].load_le());
            // length is in bytes but we need it in bits
            length.unwrap() * 8
        });

        let remaining = length - incomplete.len();
        // should never be 0 since we would have already finished
        assert!(remaining > 0);

        if remaining > stream_chunk.len() {
            incomplete.extend_from_bitslice(stream_chunk);
            return;
        }

        incomplete.extend_from_bitslice(&stream_chunk[..remaining]);

        let mut pointer = BYTES8;
        let handler: u16 = incomplete[pointer..pointer + MESSAGE_HEADER_SIZE].load_le();
        pointer += MESSAGE_HEADER_SIZE;

        if let Some(handler) = message_handlers.get_mut(&handler) {
            let (values, types) =
                handler
                    .bind_mut()
                    .handle_message(incomplete, &mut pointer, !is_server);
            if is_server {
                message_buffer.push_back((
                    MessageHandler::generate_packet(
                        &values,
                        &types,
                        handler.bind().get_message_id(),
                    ),
                    stream,
                ));
            }
        } else {
            godot_error!("received a message but had no handler for it: {handler:?}")
        }

        (_, stream_chunk) = stream_chunk.split_at(remaining);
        entry.remove_entry();
    }

    loop {
        if stream_chunk.is_empty() {
            break;
        }

        if stream_chunk.len() < BYTES8 {
            incomplete_message_buffer.insert(stream, (None, stream_chunk.to_bitvec()));
            break;
        }

        // length is in bytes but we need it in bits
        let length: usize = stream_chunk[..BYTES8].load_le::<usize>() * 8;
        let mut pointer = BYTES8;

        if stream_chunk.len() < length {
            incomplete_message_buffer.insert(stream, (Some(length), stream_chunk.to_bitvec()));
            break;
        }

        let incomplete_stream;
        (incomplete_stream, stream_chunk) = stream_chunk.split_at(length);

        let handler: u16 = incomplete_stream[pointer..pointer + MESSAGE_HEADER_SIZE].load_le();
        pointer += MESSAGE_HEADER_SIZE;

        // clients treat messages from the server as authoritative, the server does not
        if let Some(handler) = message_handlers.get_mut(&handler) {
            let (values, types) =
                handler
                    .bind_mut()
                    .handle_message(incomplete_stream, &mut pointer, !is_server);
            if is_server {
                message_buffer.push_back((
                    MessageHandler::generate_packet(
                        &values,
                        &types,
                        handler.bind().get_message_id(),
                    ),
                    stream,
                ));
            }
        }
    }
}

pub fn handle_datagrams(
    client_id: &NetNodesConnectionId,
    tick_number: &mut i8,
    unapplied_packets: &mut BTreeMap<(i8, [u8; 16]), BitVec<u64, Lsb0>>,
    networker: &mut ConnectionHandler,
    random: &mut rand::rngs::SmallRng,
) {
    *tick_number += 1;

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
    if !got_soon_packet {
        *tick_number += 1;
    }

    // 25 is not the percentage, this triggers when packets loss > 4%
    if late_packets * 25 > total_packets {
        *tick_number -= 1;
    }
}

pub enum InternalMessage {
    ClientId([u8; 40]),
    NetNodeId((u16, VecDeque<u32>)),
    MessageHandlerId((u16, VecDeque<u32>)),
}

pub fn send_internal_message(
    peer: &NetNodesConnectionId,
    networker: &mut ConnectionHandler,
    message: InternalMessage,
) {
    match message {
        InternalMessage::ClientId(id) => {
            let id: Vec<u8> = id.into_iter().chain([0; 14]).collect();
            let (id, _) = id.as_chunks::<8>();
            let id: Vec<u64> = id.into_iter().map(|x| u64::from_le_bytes(*x)).collect();

            let data: BitVec<u64, Lsb0> = BitVec::from_vec(id);

            networker.send_stream(peer, 0, data);
        }
        InternalMessage::NetNodeId((node_id, path)) => {
            let mut packet: BitVec<u64, Lsb0> = BitVec::new();

            packet.push(true);

            packet.extend(node_id.view_bits::<Lsb0>());

            packet.extend(path.len().view_bits::<Lsb0>());
            for idx in path {
                packet.extend(idx.view_bits::<Lsb0>());
            }

            networker.send_stream(peer, 0, packet);
        }
        InternalMessage::MessageHandlerId((handler_id, path)) => {
            let mut packet: BitVec<u64, Lsb0> = BitVec::new();
            packet.push(false);

            packet.extend(handler_id.view_bits::<Lsb0>());

            packet.extend(path.len().view_bits::<Lsb0>());
            for idx in path {
                packet.extend(idx.view_bits::<Lsb0>());
            }

            networker.send_stream(peer, 0, packet);
        }
    }
}

pub fn decode_internal_message(
    packet: &BitSlice<u64, Lsb0>,
    pointer: &mut usize,
) -> InternalMessage {
}

fn get_node_path(node: Gd<Node>) -> VecDeque<u32> {
    let mut path = VecDeque::new();

    let mut current = node;
    let mut idx = current.get_index();

    while idx != -1 {
        path.push_front(idx as u32);
        current = current.get_parent().unwrap();
        idx = current.get_index();
    }

    path
}
