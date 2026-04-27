use crate::{
    NetNodeManager,
    common::MESSAGE_HEADER_SIZE,
    serializer::{self, NetworkedValueTypes},
};
use bitvec::prelude::*;
use godot::prelude::*;

/// A node for sending reliable, ordered messages over the network.
/// Whenever a message is sent on a client, the server will receive it, 'clean' it using `clean_message`,
/// processes it using `process_message`, and then broadcasts the cleaned message to all clients.
/// When the server sends a message, it processes it directly and broadcasts it to all clients.
/// Messages are ordered when on the same stream, but can arrive out of order relative to other streams
/// or the state of NetworkedNodes.
/// Each `MessageHandler` has a unique `message_id`; ID 0 is used for internal messages such as syncing IDs
/// for `MessageHandler`s or `NetworkedNode`s.
/// A client or server must be running before a `MessageHandler` can be added to the scene tree.
#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct MessageHandler {
    message_id: u64,
    #[export]
    stream: i32,
    network_manager: Option<Gd<NetNodeManager>>,
    base: Base<Node>,
}

#[allow(unused)]
#[godot_api]
pub impl MessageHandler {
    /// Determines how values in a message are encoded in the packet.
    /// Types are provided using the enum values. Encoding must be valid for the variant type.
    /// Called in a loop with incrementing `idx` until `-1` is returned.
    /// `previous_value` is the value of the last decoded/encoded index, or `Nil` if this is the first value.
    /// This can be used to implement conditional decoding of values.
    /// If you skip a value when decoding, it is recommended to return `Nil` for that index.
    #[func(virtual)]
    fn get_value_type(&mut self, previous_value: Variant, idx: i64) -> i64 {
        unimplemented!()
    }
    /// This function applies the effects of a decoded message to the client/server.
    /// This is called after the message has been cleaned and therefore should trust the values in the message.
    #[func(virtual)]
    fn process_message(&mut self, values: VarArray) {
        unimplemented!()
    }
    /// This function acts as a form of anti-cheat; when the server receives a message it cleans it first.
    /// This cleaning can change values before the message is applied or sent to the clients.
    /// Removing, adding, or changing the types of values is not intended and will likely cause the message to fail to decode.
    /// A message failing to decode instantly stops the instance, as it could desync the client and server.
    #[func]
    fn clean_message(&mut self, values: VarArray) -> VarArray {
        values
    }
    /// This should be called by a user-defined function to send a message to the network.
    /// It takes a set of values and types and then generates and sends a packet to the network.
    #[func]
    fn send_message_final(&mut self, values: VarArray, types: Array<i64>) {
        let packet = Self::generate_packet(values, types, self.message_id);

        if self
            .network_manager
            .as_mut()
            .unwrap()
            .bind_mut()
            .is_server()
        {
            let mut pointer = MESSAGE_HEADER_SIZE;
            self.handle_message(packet.clone().as_bitslice(), &mut pointer, true);
        }
        self.network_manager
            .as_mut()
            .unwrap()
            .bind_mut()
            .queue_message(packet, self.stream as u64);
    }
    pub fn handle_message(
        &mut self,
        packet: &BitSlice<u64, Lsb0>,
        pointer: &mut usize,
        is_authoritative: bool,
    ) -> (VarArray, Array<i64>) {
        let mut idx = 0;
        let mut last_value = Variant::nil();
        let mut values: VarArray = VarArray::new();
        let mut types: Array<i64> = Array::new();
        while *pointer < packet.len() {
            let value_type =
                NetworkedValueTypes::try_from(self.get_value_type(last_value, idx)).unwrap();
            last_value = serializer::decode_with_known_type(packet, pointer, value_type).unwrap();
            values.push(&last_value);
            types.push(value_type as i64);
            idx += 1;
        }
        if !is_authoritative {
            values = self.clean_message(values);
        }
        let tmp = values.clone();
        self.run_deferred(move |this| this.process_message(values));
        (tmp, types)
    }
    pub fn generate_packet(
        values: VarArray,
        types: Array<i64>,
        message_id: u64,
    ) -> BitVec<u64, Lsb0> {
        if values.len() != types.len() {
            godot_warn!("invalid call to generate_packet");
            return BitVec::new();
        }
        if types
            .iter_shared()
            .any(|x| NetworkedValueTypes::try_from(x).is_err())
        {
            godot_warn!("invalid call to generate_packet");
            return BitVec::new();
        }

        let types = types
            .iter_shared()
            .map(|x| NetworkedValueTypes::try_from(x).unwrap())
            .collect::<Vec<NetworkedValueTypes>>();
        let mut packet: BitVec<u64, Lsb0> = BitVec::new();
        packet.extend(message_id.view_bits::<Lsb0>());
        for value in values.iter_shared().enumerate() {
            packet.extend(serializer::encode_with_known_type(&value.1, types[value.0]));
        }
        packet
    }
    pub fn get_message_id(&self) -> u64 {
        self.message_id
    }
}

#[godot_api]
impl INode for MessageHandler {
    fn enter_tree(&mut self) {
        self.message_id = rand::random();

        self.network_manager = Some(
            self.base()
                .get_node_as::<NetNodeManager>("/root/NetworkManager"),
        );

        self.base()
            .get_node_as::<NetNodeManager>("/root/NetworkManager")
            .bind_mut()
            .register_message_handler(self.to_gd(), self.message_id);
    }
    fn exit_tree(&mut self) {
        self.network_manager
            .as_mut()
            .unwrap()
            .bind_mut()
            .unregister_message_handler(self.message_id);
    }
}
