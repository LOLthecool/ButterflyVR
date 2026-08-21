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
/// a MessageHandler with id 0 will be ignored when trying to send or receive messages.
/// A client or server must be running before a `MessageHandler` can be added to the scene tree.
#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct MessageHandler {
    /// only available to gdscript in debug mode!
    #[cfg_attr(debug_assertions, export)]
    pub message_id: u16,
    /// only available to gdscript in debug mode!
    #[cfg_attr(debug_assertions, export)]
    stream: i32,
    network_manager: Option<Gd<NetNodeManager>>,
    base: Base<Node>,
}

#[allow(unused, clippy::unused_self)]
#[godot_api]
impl MessageHandler {
    /// Determines how values in a message are encoded in the packet.
    /// Types are provided using the enum values. Encoding must be valid for the variant type.
    /// Called in a loop with incrementing `idx` until `End` is returned.
    /// `previous_value` is the value of the last decoded/encoded index, or `Nil` if this is the first value.
    /// This can be used to implement conditional decoding of values.
    /// If you skip a value when decoding, it is recommended to return `Nil` for that index.
    #[func(virtual)]
    fn get_value_type(&mut self, previous_value: Variant, idx: i64) -> NetworkedValueTypes {
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
    /// by default, this function returns the values unchanged.
    #[func(virtual)]
    fn clean_message(&mut self, values: VarArray) -> VarArray {
        values
    }
    /// This should be called by a user-defined function to send a message to the network.
    /// It takes a set of values and types and then generates and sends a packet to the network.
    #[func]
    fn send_message_final(&mut self, values: VarArray, types: Array<NetworkedValueTypes>) {
        if self.message_id == 0 {
            godot_error!("tried to send message with id 0");
            return;
        }

        let packet = Self::generate_packet(&values, &types, self.message_id);

        // need to do this before handling on server side since handling a message could queue other messages
        self.network_manager
            .as_mut()
            .unwrap()
            .bind_mut()
            .queue_message(packet.clone(), self.stream.cast_unsigned().into());

        if self
            .network_manager
            .as_mut()
            .unwrap()
            .bind_mut()
            .is_server()
        {
            let mut pointer = MESSAGE_HEADER_SIZE;
            self.handle_message(packet.as_bitslice(), &mut pointer, false, true);
        }
    }
    pub fn handle_message(
        &mut self,
        packet: &BitSlice<u64, Lsb0>,
        pointer: &mut usize,
        clean_message: bool,
        run_deferred: bool,
    ) -> (VarArray, Array<NetworkedValueTypes>) {
        let mut idx = 0;
        let mut last_value = Variant::nil();
        let mut values: VarArray = VarArray::new();
        let mut types: Array<NetworkedValueTypes> = Array::new();
        while *pointer < packet.len() {
            let value_type = self.get_value_type(last_value.clone(), idx);
            if value_type == NetworkedValueTypes::End {
                break;
            }
            let Some(last_value) = serializer::decode_with_known_type(packet, pointer, value_type)
            else {
                if clean_message {
                    // message source is untrustworthy, ignore bad packets from clients
                    return (VarArray::new(), Array::new());
                }
                godot_error!("failed to decode message from the server.");
                // todo: disconnect instead of panicking
                panic!("failed to decode message from the server.");
            };
            values.push(&last_value);
            types.push(value_type);
            idx += 1;
        }
        if clean_message {
            values = self.clean_message(values);
        }
        let tmp = values.clone();
        if run_deferred {
            self.run_deferred(move |this| this.process_message(values));
        } else {
            self.process_message(values);
        }
        (tmp, types)
    }
    pub fn generate_packet(
        values: &VarArray,
        types: &Array<NetworkedValueTypes>,
        message_id: u16,
    ) -> BitVec<u64, Lsb0> {
        if values.len() != types.len() {
            godot_warn!(
                "invalid call to generate_packet, length mismatch: values={:?} types={:?}",
                values.len(),
                types.len()
            );
            return BitVec::new();
        }

        let mut packet: BitVec<u64, Lsb0> = BitVec::new();
        packet.extend(message_id.view_bits::<Lsb0>());
        for value in values.iter_shared().enumerate() {
            packet.extend(serializer::encode_with_known_type(
                &value.1,
                types.at(value.0),
            ));
        }
        packet
    }
}

#[godot_api]
impl INode for MessageHandler {
    fn enter_tree(&mut self) {
        let network_manager = self
            .base()
            .get_node_as::<NetNodeManager>("/root/NetworkManager");
        self.network_manager = Some(network_manager);

        let network_manager = self.network_manager.clone();
        network_manager
            .unwrap()
            .bind_mut()
            .register_message_handler(self.to_gd(), self);
    }
    fn exit_tree(&mut self) {
        let network_manager = self.network_manager.clone();
        network_manager
            .unwrap()
            .bind_mut()
            .unregister_message_handler(self.message_id);
    }
}
