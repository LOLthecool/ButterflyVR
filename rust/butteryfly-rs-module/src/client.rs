// functionallity for the NetNodeManager client
use crate::messages::*;
use crate::net_nodes::NetworkedNode;
use crate::serializer::*;
use bitvec::prelude::*;
use godot::classes::Engine;
use godot::prelude::*;
use std::collections::{HashSet, VecDeque};
use std::time::UNIX_EPOCH;
use std::time::{Duration, Instant, SystemTime};
use std::{cmp, collections::HashMap};

const CHANNEL_ACK: u16 = u16::MAX;

const BYTE: usize = 8;
const BYTES2: usize = 16;
const BYTES8: usize = 64;

const PACKET_HEADER_SIZE: usize = BYTES2 + BYTES8;
const PACKET_HEADER_SIZE_ACK: usize = BYTES2;
const CHANNEL1_HEADER_SIZE: usize = BYTES8;

const HIT_RATE_HISTORY_LENGTH: usize = 128;

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct NetNodeClient {
    #[var]
    pub id: u16,
    next_id: u16,
    pub networked_nodes: Vec<Gd<NetworkedNode>>,
    owned_nodes: Vec<(Gd<NetworkedNode>, i64)>,
    pub client_networker: ClientNetworker,
    packet_buffers: VecDeque<Vec<BitVec<u64, Lsb0>>>,
    message_buffer: VecDeque<BitVec<u64, Lsb0>>,
    message_handlers: HashMap<u16, Gd<MessageHandler>>,
    c1_miss_rate_average_percent: f32,
    c1_miss_rate_average: f32,
    c1_hit_rate_average: f32,
    c1_miss_rates_last_frames: VecDeque<u64>,
    c1_hit_rates_last_frames: VecDeque<u64>,
    next_c3_packet_number: u64,
    c3_buffered_packets: HashMap<u64, BitVec<u64, Lsb0>>,
    c0_seen_packets: HashSet<u64>,
    remaining_bandwidth: usize,
    voice_manager: voice::VoiceStreamManager,
    next_c5_packet_number: u64,
    voice_packet_buffer: Vec<(u64, Vec<u8>)>,
    encoder_stream: usize,
    decoder_stream: Option<usize>,
    pub audio_output_buffer: Vec<f32>,
    //borrowing rules stop us from using Base() when we need to so this gives us another way to access the scene tree
    // should try and do this properly later
    workaround: Option<Gd<Node>>,
    base: Base<Node>,
}

#[godot_api]
pub impl NetNodeClient {
    pub fn register_node(&mut self, new_node_ref: Gd<NetworkedNode>, new_node: &mut NetworkedNode) {
        if new_node.owner_id == self.id {
            self.owned_nodes.push((new_node_ref.clone(), 0));
        }
        self.networked_nodes.push(new_node_ref);
    }
    pub fn unregister_node(&mut self, removed_node_ref: Gd<NetworkedNode>) {
        self.networked_nodes.remove(
            self.networked_nodes
                .iter()
                .position(|x| *x == removed_node_ref)
                .unwrap(),
        );
        if let Some(n) = self
            .owned_nodes
            .iter()
            .position(|x| x.0 == removed_node_ref)
        {
            self.owned_nodes.remove(n);
        }
    }
    pub fn unregister_all(&mut self) {
        self.next_id = 0;
        self.networked_nodes.clear();
    }
    pub fn register_message(&mut self, handler: Gd<MessageHandler>, message_type: u16) {
        if self.message_handlers.contains_key(&message_type) {
            godot_warn!(
                "tried to register duplicate handlers for message type {:#?}",
                message_type
            )
        }
        self.message_handlers.insert(message_type, handler);
    }
    pub fn unregister_message(&mut self, message_type: u16) {
        self.message_handlers.remove(&message_type);
    }
    pub fn queue_message(&mut self, message: BitVec<u64, Lsb0>) {
        self.message_buffer.push_back(message);
    }
    pub fn start_client(&mut self, arr: PackedByteArray) {
        self.workaround = Some(Node::new_alloc());
        let reference = self.workaround.clone();
        self.base_mut().add_child(&reference.unwrap());
        self.client_networker.client = Some(Client::new(&arr.to_vec()).unwrap());
        self.client_networker.client.as_mut().unwrap().connect();
        self.client_networker
            .send(BitVec::<u64, Lsb0>::new().as_bitslice(), CHANNEL_ACK);
        self.packet_buffers.push_back(Vec::new());
        self.packet_buffers.push_back(Vec::new());
        self.encoder_stream = self.voice_manager.create_encoder();
    }
    pub fn transmit_audio(&mut self, sample_buffer: PackedVector2Array) {
        if sample_buffer.len() != voice::FRAME_LENGTH {
            godot_warn!("got malformed sample buffer");
            return;
        }
        let tmp: Vec<f32> = sample_buffer
            .to_vec()
            .into_iter()
            .map(|x| (x.x + x.y) / 2.0)
            .collect();
        let buffer = self.voice_manager.encode_audio(self.encoder_stream, &tmp);
        let mut buffer_bits: BitVec<u64, Lsb0> = BitVec::with_capacity(buffer.len() * BYTE);
        for byte in buffer {
            buffer_bits.extend(byte.view_bits::<Lsb0>());
        }
        self.client_networker.send(buffer_bits.as_bitslice(), 5);
    }
    pub fn get_audio(&self) -> Vec<f32> {
        self.audio_output_buffer.clone()
    }
    pub fn disconnect(&mut self) -> Result<(), netcode::Error> {
        self.client_networker
            .client
            .as_mut()
            .unwrap()
            .disconnect()?;
        Ok(())
    }
    fn tick_client(&mut self) {
        const CHANNEL_CLIENT_ID: u16 = u16::MAX - 1;
        self.client_networker.poll();
        if !self
            .client_networker
            .client
            .as_mut()
            .unwrap()
            .is_connected()
        {
            return;
        }
        // total up packets used and ignored this frame
        // missed packets are specifically c1 packets ignored because the jitter buffer couldnt contain them
        let mut current_frame_hit_rate: u64 = 0;
        let mut current_frame_miss_rate: u64 = 0;

        let networker = &mut self.client_networker;
        for packet in networker.packet_buffer.drain(..) {
            if packet.len() < BYTES2 {
                godot_warn!("got packet with invalid size");
                continue;
            }
            let mut pointer: usize = 0;
            let channelid: u16 = packet[pointer..pointer + BYTES2].load_le();
            pointer += BYTES2;
            // packet number is present in all non-ack packets so we get the value here even though some channels dont need it
            let packet_number: u64 = packet[pointer..pointer + BYTES8].load_le();
            pointer += BYTES8;
            match channelid {
                // channel 1 is for netnode updates from priority accumulation and is the most common packet type handled
                1 => {
                    if networker.state == ClientState::AwaitingID {
                        continue;
                    }
                    // this will result in ignoring c2 packets arriving out of order after the first c1 packet
                    // but c2 is only for cases where many objects need to be synced so actual effect should be negligible
                    networker.state = ClientState::Connected;

                    if packet.len() < PACKET_HEADER_SIZE + CHANNEL1_HEADER_SIZE {
                        godot_warn!("got c1 packet with invalid size");
                        continue;
                    }

                    // helps reduce some issues that could be caused by a connection drop which might confuse the latency / jitter calculations
                    const PACKET_LATENCY_DISCARD_THRESHOLD: Duration = Duration::from_millis(1000);
                    // the number of previous ticks we will store latency information for, higher number means less variance but slowing reaction
                    const LATENCY_BUFFER_MAX_SIZE: usize = 8;
                    // expand the jitter buffer if packets lost to jitter exceeds this fraction (0.1 == 10% packet loss)
                    // larger jitter buffer increases latency as we need to give out of order packets more time to arrive before processing the current ones
                    const JITTER_BUFFER_INCREASE_THRESHOLD: f32 = 0.01;

                    // grow buffer if needed
                    // dont grow unless packet loss history is partly full to avoid unneeded growth from initial variance
                    if self.c1_miss_rate_average_percent > JITTER_BUFFER_INCREASE_THRESHOLD
                        && self.c1_miss_rates_last_frames.len() >= (HIT_RATE_HISTORY_LENGTH / 2)
                    {
                        self.packet_buffers.push_back(Vec::new());
                        self.c1_hit_rates_last_frames.clear();
                        self.c1_miss_rates_last_frames.clear();
                    }
                    // send times are transmitted in milliseconds u64 but we convert to a duration for some calculations
                    // most of this could probably be cleaner
                    let packet_send_time_utc: u64 = packet[pointer..pointer + BYTES8].load_le();
                    let packet_send_time: Duration = Duration::from_millis(packet_send_time_utc);
                    let tick_length: Duration = Duration::from_secs_f32(
                        1.0 / Engine::singleton().get_physics_ticks_per_second() as f32,
                    );

                    let current_time: Duration =
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                    let latency: Duration = current_time.saturating_sub(packet_send_time); // this is a bad workaround for an issue with current time sometimes being earlier than send time, todo: fix this
                    if latency > PACKET_LATENCY_DISCARD_THRESHOLD {
                        godot_warn!(
                            "ignoring packet with high latency: {:#?}ms",
                            latency.as_millis()
                        );
                        continue;
                    }
                    if networker.latency_buffer.len() >= LATENCY_BUFFER_MAX_SIZE {
                        networker.latency_buffer.pop_front();
                    }
                    networker.latency_buffer.push_back(latency);
                    // average latency for all packets, will be different from packet latency because of jitter
                    networker.latency = Duration::from_millis(
                        networker
                            .latency_buffer
                            .iter()
                            .map(|x| x.as_millis())
                            .sum::<u128>() as u64
                            / networker.latency_buffer.len() as u64,
                    );
                    let mut buffer_max_jitter: i128 = (self.packet_buffers.len()) as i128
                        * -(tick_length.as_millis() as i128 / 2);
                    let mut packet_accepted: bool = false;

                    for buffer in self.packet_buffers.iter_mut() {
                        let buffer_min_jitter = buffer_max_jitter;
                        buffer_max_jitter += tick_length.as_millis() as i128;

                        if (latency.as_millis() as i128 - networker.latency.as_millis() as i128)
                            <= buffer_max_jitter
                            && (latency.as_millis() as i128 - networker.latency.as_millis() as i128)
                                >= buffer_min_jitter
                        {
                            buffer.push(packet);
                            packet_accepted = true;
                            break;
                        }
                    }
                    if packet_accepted {
                        current_frame_hit_rate += 1;
                    } else {
                        godot_warn!("ignoring packet due to jitter");
                        current_frame_miss_rate += 1;
                    }
                }
                // essentially channel 1 but reliable and unordered
                2 => {
                    if networker.state == ClientState::AwaitingID {
                        continue;
                    }
                    if networker.state == ClientState::Connected {
                        // small memory savings
                        // todo: check what happens if we disconnect with c2 packets in transit
                        self.c0_seen_packets.clear();
                        continue;
                    }
                    // yes this stores the packet number of every packet we get but c2 is only used once during initial connection
                    if !(self.c0_seen_packets.contains(&packet_number)) {
                        if self.packet_buffers.len() == 0 {
                            self.packet_buffers[0].push(packet)
                        }
                        self.c0_seen_packets.insert(packet_number);
                    }
                }
                // handles reliable, ordered messages from the server or other clients called messages
                3 => {
                    self.c3_buffered_packets.insert(packet_number, packet);
                    while let Some(packet) =
                        self.c3_buffered_packets.remove(&self.next_c3_packet_number)
                    {
                        self.next_c3_packet_number += 1;
                        let mut pointer: usize = BYTES2 + BYTES8;
                        let message_type: u16 = packet[pointer..pointer + BYTES2].load_le();
                        pointer += BYTES2;
                        let root = self
                            .workaround
                            .as_mut()
                            .and_then(|x| x.get_tree().and_then(|x| x.get_root()));
                        if message_type == 0 {
                            root.clone().unwrap().apply_deferred(move |_this| {
                                MessageHandler::handle_id_sync_message(
                                    packet.as_bitslice(),
                                    &mut pointer,
                                    root.clone().unwrap().upcast(),
                                )
                            });
                        } else if let Some(handler) = self.message_handlers.get_mut(&message_type) {
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
                    if self.next_c5_packet_number <= packet_number {
                        self.voice_packet_buffer
                            .push((packet_number, buffer[10..].into_iter().copied().collect()));
                    }
                }
                CHANNEL_CLIENT_ID => {
                    // sets the id of the client, must happen before anything else
                    self.id = packet[pointer..pointer + BYTES8].load_le::<u64>() as u16;
                    networker.state = ClientState::InitialSync;
                }
                _ => {
                    godot_warn!("unhandled channel: {:#?}", channelid)
                }
            }
        }
        // logic for updating packet jitter loss calculations
        if current_frame_hit_rate == 0 && current_frame_miss_rate == 0 {
            return;
        }
        self.c1_hit_rates_last_frames
            .push_back(current_frame_hit_rate);
        self.c1_miss_rates_last_frames
            .push_back(current_frame_miss_rate);
        if self.c1_hit_rates_last_frames.len() > HIT_RATE_HISTORY_LENGTH {
            self.c1_hit_rates_last_frames.pop_front();
        }
        if self.c1_miss_rates_last_frames.len() > HIT_RATE_HISTORY_LENGTH {
            self.c1_miss_rates_last_frames.pop_front();
        }
        self.c1_hit_rate_average = self.c1_hit_rates_last_frames.iter().sum::<u64>() as f32
            / self.c1_hit_rates_last_frames.len() as f32;
        self.c1_miss_rate_average = self.c1_miss_rates_last_frames.iter().sum::<u64>() as f32
            / self.c1_miss_rates_last_frames.len() as f32;
        self.c1_miss_rate_average_percent = self.c1_miss_rate_average as f32
            / (self.c1_hit_rate_average + self.c1_miss_rate_average) as f32;
    }
    fn update_network_nodes(&mut self) {
        for packet in self.packet_buffers.get(0).unwrap() {
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
                    break; // need to stop here because we dont know how long this missing object is
                }
                let node = Gd::bind(tmp.unwrap());
                let types_buff: Vec<NetworkedValueTypes> = node.get_networked_values_types();
                if !node.update_networked_values(&mut pointer, packet.as_bitslice(), &types_buff) {
                    // failed to decode something so need to stop again since dont know remaining length
                    break;
                }
            }
        }
    }
    fn send_packets_client(&mut self) {
        const BANDWIDTH_BUDGET: usize = 128000;
        const PACKET_MAX_SIZE_THRESHOLD: usize = 80;
        const MAX_SINGLE_PACKET_PAYLOAD_LENGTH: usize = 4800;
        self.remaining_bandwidth +=
            BANDWIDTH_BUDGET / Engine::singleton().get_physics_ticks_per_second() as usize;
        while self.remaining_bandwidth > PACKET_MAX_SIZE_THRESHOLD {
            let mut packet: BitVec<u64, Lsb0> =
                BitVec::with_capacity(MAX_SINGLE_PACKET_PAYLOAD_LENGTH);
            for ack in &self.client_networker.waiting_acks {
                packet.extend(ack.0.view_bits::<Lsb0>());
                packet.extend(ack.1.view_bits::<Lsb0>());
            }
            self.client_networker.waiting_acks.clear();
            if !packet.is_empty() {
                self.client_networker.send(&packet, CHANNEL_ACK);
                self.remaining_bandwidth = self.remaining_bandwidth.saturating_sub(packet.len());
            }

            // dont send packets until we are synced, not sure if this is important or not so might remove
            if self.client_networker.state == ClientState::InitialSync {
                return;
            }
            packet.clear();
            // channel 3 (messages)
            while let Some(message) = self.message_buffer.pop_front() {
                packet.extend(&message);
                if (self.remaining_bandwidth as i64 - packet.len() as i64) < 0 {
                    break;
                }
                self.remaining_bandwidth -= packet.len();
                self.client_networker.send(&packet.as_bitslice(), 3);
            }
            // channel 1
            packet.clear();
            packet.extend(
                (SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64)
                    .view_bits::<Lsb0>(),
            );
            for node_ref in self.owned_nodes.iter_mut() {
                let node = Gd::bind(&node_ref.0);
                let types_buff: Vec<NetworkedValueTypes> = node.get_networked_values_types();
                if node_ref.1 != 0 && node.objectid != 0 {
                    let tmp = node.get_byte_data(&types_buff);
                    if tmp.len() + packet.len()
                        > cmp::min(self.remaining_bandwidth, MAX_SINGLE_PACKET_PAYLOAD_LENGTH)
                    {
                        break;
                    }
                    packet.extend(tmp);
                    node_ref.1 = 0;
                }
            }
            if packet.len() > CHANNEL1_HEADER_SIZE {
                self.remaining_bandwidth -= packet.len();
                self.client_networker.send(&packet, 1);
                continue;
            }
            packet.clear();
            break;
        }
    }
    fn handle_audio_input(&mut self) {
        if self.decoder_stream.is_none() {
            self.decoder_stream = Some(self.voice_manager.create_stereo_decoder());
        }
        if self.voice_packet_buffer.len() < 3 {
            // if buffer is empty then we are consuming packets too fast for the server to keep up and need to slow down
            return;
        }
        let buffer: Vec<u8>;
        if let Some(packet_idx) = self
            .voice_packet_buffer
            .iter()
            .position(|x| x.0 == self.next_c5_packet_number)
        {
            buffer = self.voice_packet_buffer.remove(packet_idx).1;
        } else {
            buffer = Vec::new();
        }
        self.next_c5_packet_number += 1;
        self.audio_output_buffer = self
            .voice_manager
            .decode_stereo_audio(self.decoder_stream.unwrap(), &buffer);
    }
}
#[godot_api]
impl INode for NetNodeClient {
    fn physics_process(&mut self, _delta: f64) {
        if self.client_networker.state == ClientState::Disconnected {
            return;
        }
        // check for freed network nodes
        let mut idx = 0;
        while let Some(node) = self.networked_nodes.get(idx) {
            // "Using this method is often indicative of bad design" - the docs
            if !node.is_instance_valid() {
                self.networked_nodes.remove(idx);
            } else {
                idx += 1;
            }
        }
        self.packet_buffers
            .push_back(Vec::with_capacity(self.packet_buffers[0].len()));
        self.packet_buffers.pop_front();
        for node in self.owned_nodes.iter_mut() {
            node.1 += node.0.bind().get_priority(self.id);
        }
        self.owned_nodes.sort_by(|a, b| a.1.cmp(&b.1));
        self.tick_client();
        self.update_network_nodes();
        self.handle_audio_input();
        self.send_packets_client();
    }
}

#[derive(Default, PartialEq, Debug)]
pub enum ClientState {
    #[default]
    AwaitingID,
    InitialSync,
    Connected,
    Disconnected,
}
