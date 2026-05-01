use crate::{
    NetNodeManager,
    serializer::{self, NetworkedValueTypes},
};

use bitvec::prelude::*;
use godot::prelude::*;

/// A networked node that can be used to synchronize state between clients and the server.
/// Every physics tick, the node will call [`get_byte_data`] and send the result to the server
/// using the type information in [`get_networked_values_types`] and values in [`get_networked_values`].
/// It will also receive updates from the server using [`set_networked_values`].
/// A client or server must be running before a `NetworkedNode` can be added to the scene tree.
#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct NetworkedNode {
    /// The unique object ID of this node. It is assigned by the server when the node is added to the scene tree,
    /// and synced to clients using the internal `MessageHandler`.
    #[var]
    pub objectid: u16,
    /// The UUID of the owner of this node.
    /// In a game server, the owner will be able to update this node on the server's behalf; however, the server may subject given values to anti-cheat checks.
    /// In an instance server, the updates from the owner will be applied directly on other clients and
    /// applied immediately by the owner, effectively giving the owner 0 latency.
    #[var]
    pub owner_id: PackedByteArray,
    base: Base<Node>,
}

#[allow(unused, clippy::unused_self)]
#[godot_api]
impl NetworkedNode {
    /// Used by the server when updating this node's priority value. Priority is accumulated on a tick-by-tick basis.
    /// Every tick, the nodes with the highest priority are sent by the network and have their priority reset to 0.
    /// Priority is calculated separately for each client, and the `client_id` parameter should be used to differentiate between clients.
    /// For example, you can divide a base priority value by the distance between the `NetworkedNode`
    /// and the client's character's position.
    #[func(virtual)]
    pub fn get_server_priority(&self, clientid: PackedByteArray) -> i64 {
        panic!("node has no impl for get_server_priority. this should never happen")
    }

    /// Used by the client when updating this node's priority value. Priority is accumulated on a tick-by-tick basis.
    /// Every tick, the nodes with the highest priority are sent by the network and have their priority reset to 0.
    /// The client will only send updates for nodes it owns, and this generally means the client will have
    /// sufficient bandwidth to send every node on every tick.
    /// This function should only matter in cases where the client is syncing many nodes to the server.
    #[func(virtual)]
    pub fn get_client_priority(&self) -> i64 {
        panic!("node has no impl for get_client_priority. this should never happen")
    }

    /// Returns the networked values of this node as a VarArray.
    /// This should always return every value that can be synced by the client, and any conditional values
    /// should be excluded when getting the types.
    #[func(virtual)]
    pub fn get_networked_values(&self) -> VarArray {
        panic!("node has no impl for get_networked_values. this should never happen")
    }

    /// Sets the networked values of this node from a VarArray.
    // TODO: Add more documentation.
    #[func(virtual)]
    pub fn set_networked_values(&self, values: VarArray) {
        panic!("node has no impl for set_networked_values. this should never happen")
    }

    /// Determines how values from `get_networked_values` are encoded in the packet.
    /// Types are provided using the enum values. Encoding must be valid for the variant type.
    /// Called in a loop with incrementing `idx` until `-1` is returned.
    #[func(virtual)]
    fn get_networked_value_type(&self, idx: i64) -> i64 {
        panic!("node has no impl for get_networked_values_type. this should never happen")
    }

    /// Called when the owner disconnects from the server.
    /// Default implementation does nothing, but can be overridden to handle cleanup.
    /// An example is the player's character, which deletes itself when the owner disconnects.
    #[func]
    pub fn on_owner_dc(&mut self) {}

    /// Generates a packet chunk containing the values from `get_networked_values`, encoding each value using the network value types.
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

    /// Decodes a packet chunk into the variant values used in `set_networked_values`.
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
    /// Collects network value types into a `Vec` by calling `get_networked_value_type` until `-1` is returned.
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
        if self.base().has_meta("owner_id") {
            self.owner_id = PackedByteArray::from_variant(&self.base().get_meta("owner_id"));
        }
        self.base()
            .get_node_as::<NetNodeManager>("/root/NetworkManager")
            .bind_mut()
            .register_node(self.to_gd(), &self);
    }
    fn exit_tree(&mut self) {
        self.base()
            .get_node_as::<NetNodeManager>("/root/NetworkManager")
            .bind_mut()
            .unregister_node(&self.to_gd());
    }
}
