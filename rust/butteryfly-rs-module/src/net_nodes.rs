// methods and functionality for NetworkedNode
use crate::{
    NetNodeManager,
    serializer::{self, NetworkedValueTypes},
};

use bitvec::prelude::*;
use godot::prelude::*;
#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct NetworkedNode {
    #[var]
    pub objectid: u16,
    #[var]
    pub owner_id: PackedByteArray,
    base: Base<Node>,
}

#[godot_api]
pub impl NetworkedNode {
    // intended to be overriden, the higher the number returned here the more often this node will be updated compared to other nodes
    #[func(virtual)]
    pub fn get_server_priority(&self, _clientid: PackedByteArray) -> i64 {
        panic!("node has no impl for get_server_priority. this should never happen")
    }

    #[func(virtual)]
    pub fn get_client_priority(&self) -> i64 {
        panic!("node has no impl for get_client_priority. this should never happen")
    }

    // intended to be overriden, the array should contain all values used in set_networked_values. you must ensure these two functions can interpret each other regardless of the state of either client or server
    #[func(virtual)]
    pub fn get_networked_values(&self) -> VarArray {
        panic!("node has no impl for get_networked_values. this should never happen")
    }
    // intended to be overriden, this is where you update the properties of the node with the values from the server. you must ensure these two functions can interpret each other regardless of the state of either client or server
    #[func(virtual)]
    pub fn set_networked_values(&self, _values: VarArray) {
        panic!("node has no impl for set_networked_values. this should never happen")
    }
    #[func(virtual)]
    pub fn on_owner_dc(&mut self) {
        panic!("node has no impl for on_owner_dc. this should never happen")
    }
    // intended to be overriden, determines how values from get_networked_values are encoded in the packet, types are provided using the enum values. encoding must be valid for the variant type
    // called in loop with incrementing idx until -1 is returned
    #[func(virtual)]
    fn get_networked_value_type(&self, _idx: i64) -> i64 {
        panic!("node has no impl for get_networked_values_type. this should never happen")
    }

    // generates a packet chunk containing the values from get_networked_values, encodes each value using the network value types
    pub fn get_byte_data(&self, types: &[NetworkedValueTypes]) -> BitVec {
        const AVERAGE_OBJECT_SIZE: usize = 128; // estimated average size, prefers to overallocate than underallocate, probably a better way to do this
        let data: VarArray = self.get_networked_values();
        let mut byte_data: BitVec = BitVec::with_capacity(data.len() * AVERAGE_OBJECT_SIZE);

        byte_data.extend(self.objectid.view_bits::<Lsb0>());
        for (index, inner_value) in data.iter_shared().enumerate() {
            // network values and network value types must match
            byte_data.extend(serializer::encode_with_known_type(
                &inner_value,
                types[index],
            ));
        }
        byte_data
    }

    // decodes a packet chunk into the variant values used in set_networked_values
    pub fn update_networked_values(
        &self,
        pointer: &mut usize,
        data: &BitSlice<u64>,
        types: &[NetworkedValueTypes],
    ) -> bool {
        let mut values: VarArray = VarArray::new();
        while values.len() < types.len() {
            if let Some(value) =
                serializer::decode_with_known_type(data, pointer, types[values.len()])
            {
                values.push(&value);
            } else {
                godot_warn!("failed to decode {:#?}", types[values.len()]);
                return false;
            }
        }
        self.set_networked_values(values);
        true
    }
    // collects network value types into a vec by calling get_networked_value_type until -1 is returned
    pub fn get_networked_values_types(&self) -> Vec<NetworkedValueTypes> {
        let mut values: Vec<NetworkedValueTypes> = Vec::new();
        for i in 0..1000 {
            let tmp = self.get_networked_value_type(i);
            if tmp == -1 {
                break;
            }
            values.push(NetworkedValueTypes::try_from(tmp).unwrap());
        }
        values
    }
}
#[godot_api]
impl INode for NetworkedNode {
    fn enter_tree(&mut self) {
        if self
            .base()
            .get_node_as::<NetNodeManager>("/root/NetworkManager")
            .bind_mut()
            .is_server()
        {
            self.objectid = self
                .base()
                .get_node_as::<NetNodeManager>("/root/NetworkManager")
                .bind_mut()
                .get_next_object_id();
        }
        if let Some(parent) = self.base().get_parent()
            && parent.has_meta("owner_id")
        {
            self.owner_id = PackedByteArray::from_variant(&parent.get_meta("owner_id"));
        }
        self.base()
            .get_node_as::<NetNodeManager>("/root/NetworkManager")
            .bind_mut()
            .register_node(self.to_gd(), self);
    }
    fn exit_tree(&mut self) {
        self.base()
            .get_node_as::<NetNodeManager>("/root/NetworkManager")
            .bind_mut()
            .unregister_node(&self.to_gd());
    }
}
