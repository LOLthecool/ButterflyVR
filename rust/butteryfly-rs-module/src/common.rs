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

pub const MESSAGE_HEADER_SIZE: usize = BYTES8;
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
    message_handlers: &mut HashMap<u64, Gd<MessageHandler>>,
) {
    if stream_chunk.is_empty() {
        return;
    }

    let mut stream_chunk = stream_chunk.as_bitslice();

    if let hash_map::Entry::Occupied(mut entry) = incomplete_message_buffer.entry(stream) {
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
            return;
        }

        incomplete.extend_from_bitslice(&stream_chunk[..remaining]);

        let handler: u64 = incomplete[pointer..pointer + MESSAGE_HEADER_SIZE].load_le();
        pointer += MESSAGE_HEADER_SIZE;

        if let Some(handler) = message_handlers.get_mut(&handler) {
            handler
                .bind_mut()
                .handle_message(incomplete.as_bitslice(), &mut pointer);
        }

        let (_, (_, value)) = entry.remove_entry();
        message_buffer.push_back((value, stream));

        (_, stream_chunk) = stream_chunk.split_at(remaining);
    }

    loop {
        if stream_chunk.is_empty() {
            break;
        }

        if stream_chunk.len() < BYTES8 {
            incomplete_message_buffer.insert(stream, (None, stream_chunk.to_bitvec()));
            break;
        }

        let length: usize = stream_chunk[..BYTES8].load_le();
        let mut pointer = BYTES8;

        if stream_chunk.len() < length {
            incomplete_message_buffer.insert(stream, (Some(length), stream_chunk.to_bitvec()));
            break;
        }

        let incomplete_stream;
        (incomplete_stream, stream_chunk) = stream_chunk.split_at(length);

        let handler: u64 = incomplete_stream[pointer..pointer + MESSAGE_HEADER_SIZE].load_le();
        pointer += MESSAGE_HEADER_SIZE;

        if let Some(handler) = message_handlers.get_mut(&handler) {
            handler
                .bind_mut()
                .handle_message(incomplete_stream, &mut pointer);
        }
        message_buffer.push_back((incomplete_stream.to_bitvec(), stream));
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
    let mut got_next_tick_packet: bool = false;

    while let Ok(packet) = networker.recv_datagram(client_id) {
        let packet: BitVec<u64> = packet;
        if packet.len() < DGRAM_HEADER_SIZE {
            godot_warn!("got c1 packet with invalid size");
            continue;
        }

        let packet_apply_tick: i8 = packet[0..BYTE].load_le();

        let relative_apply_tick = packet_apply_tick.wrapping_sub(*tick_number);

        total_packets += 1;

        if relative_apply_tick <= *tick_number {
            late_packets += 1;
            continue;
        }

        if relative_apply_tick == *tick_number + 1 {
            got_next_tick_packet = true;
        }

        let mut r = [0; 16];
        random.fill(&mut r);

        unapplied_packets.insert((packet_apply_tick, r), packet);
    }

    if !got_next_tick_packet {
        *tick_number += 1;
    }

    if late_packets > (total_packets / 100) {
        *tick_number -= 1;
    }
}
