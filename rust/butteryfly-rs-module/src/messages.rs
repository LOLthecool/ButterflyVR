use crate::{
    NetNodeManager,
    serializer::{self, NetworkedValueTypes},
};
use bitvec::prelude::*;
use godot::prelude::*;

const BYTES2: usize = 16;

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct MessageHandler {
    message_id: u64,
    #[export]
    stream: i32,
    network_manager: Option<Gd<NetNodeManager>>,
    base: Base<Node>,
}
#[godot_api]
pub impl MessageHandler {
    #[func(virtual)]
    fn get_value_type(&mut self, _previous_value: Variant, _idx: i64) -> i64 {
        unimplemented!()
    }
    #[func(virtual)]
    fn process_message(&mut self, _values: VarArray) {
        unimplemented!()
    }
    #[func]
    fn send_message_final(&mut self, values: VarArray, types: Array<i64>) {
        if values.len() != types.len() {
            godot_warn!("invalid call to send_message_final");
            return;
        }
        if types
            .iter_shared()
            .any(|x| NetworkedValueTypes::try_from(x).is_err())
        {
            godot_warn!("invalid call to send_message_final");
            return;
        }

        let types = types
            .iter_shared()
            .map(|x| NetworkedValueTypes::try_from(x).unwrap())
            .collect::<Vec<NetworkedValueTypes>>();
        let mut packet: BitVec<u64, Lsb0> = BitVec::new();
        packet.extend(self.message_id.view_bits::<Lsb0>());
        for value in values.iter_shared().enumerate() {
            packet.extend(serializer::encode_with_known_type(
                &value.1,
                &types[value.0],
            ));
        }

        if self
            .network_manager
            .as_mut()
            .unwrap()
            .bind_mut()
            .is_server()
        {
            self.handle_message(packet.clone().as_bitslice(), &mut BYTES2.clone());
        }
        self.network_manager
            .as_mut()
            .unwrap()
            .bind_mut()
            .queue_message(packet, self.stream as u64);
    }
    pub fn handle_message(&mut self, packet: &BitSlice<u64, Lsb0>, pointer: &mut usize) {
        let mut idx = 0;
        let mut last_value = Variant::nil();
        let mut values: VarArray = VarArray::new();
        while *pointer < packet.len() {
            let value_type =
                &NetworkedValueTypes::try_from(self.get_value_type(last_value, idx)).unwrap();
            last_value = serializer::decode_with_known_type(packet, pointer, value_type).unwrap();
            values.push(&last_value);
            idx += 1;
        }
        self.run_deferred(|this| this.process_message(values));
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
