mod client;
mod messages;
mod net_nodes;
mod networker;
mod serializer;
mod server;

use std::net::IpAddr;
use std::net::SocketAddr;
use std::str::FromStr;

use crate::client::*;
use crate::messages::MessageHandler;
use crate::net_nodes::*;
use crate::server::*;
use bitvec::prelude::*;
use godot::prelude::*;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

#[derive(GodotClass)]
#[class(init, base=Node)]
struct NetNodeManager {
    client: Option<Gd<NetNodeClient>>,
    server: Option<Gd<NetNodeServer>>,
    is_server: bool,
    base: Base<Node>,
}

#[godot_api]
impl NetNodeManager {
    fn register_node(&mut self, node_ref: Gd<NetworkedNode>, node: &mut NetworkedNode) {
        if self.server.is_some() {
            self.server
                .as_mut()
                .unwrap()
                .bind_mut()
                .register_node(node_ref);
        } else if self.client.is_some() {
            self.client
                .as_mut()
                .unwrap()
                .bind_mut()
                .register_node(node_ref, node);
        } else {
            godot_warn!("called register_node but no client or server is running");
        }
    }
    fn unregister_node(&mut self, node_ref: Gd<NetworkedNode>) {
        if self.server.is_some() {
            self.server
                .as_mut()
                .unwrap()
                .bind_mut()
                .unregister_node(node_ref);
        } else if self.client.is_some() {
            self.client
                .as_mut()
                .unwrap()
                .bind_mut()
                .unregister_node(node_ref);
        } // can get called when no client or server is active after client dc so we ignore that case here
    }
    #[func]
    fn get_next_object_id(&mut self) -> u16 {
        if self.server.is_some() {
            return self
                .server
                .as_mut()
                .unwrap()
                .bind_mut()
                .get_next_object_id();
        } else {
            panic!("called get_next_object_id but we are not a server");
        }
    }
    #[func]
    fn get_unverified_client(&mut self) -> PackedByteArray {
        if self.server.is_some() {
            return self
                .server
                .as_mut()
                .unwrap()
                .bind_mut()
                .get_unverified_client()
                .map(|x| x.to_godot().to_packed_array())
                .unwrap_or(PackedByteArray::new());
        } else {
            panic!("called get_unverified_client but we are not a server");
        }
    }
    #[func]
    fn verify_client(&mut self, identifier: PackedByteArray, uuid: PackedByteArray) {
        let Ok(identifier) = identifier.to_vec().try_into() else {
            godot_error!(
                "verify_client(): identifier was not correct size: should be {:?} got {:?}",
                40,
                identifier.len()
            );
            return;
        };

        let Ok(uuid) = uuid.to_vec().try_into() else {
            godot_error!(
                "verify_client(): uuid was not correct size: should be {:?} got {:?}",
                16,
                uuid.len()
            );
            return;
        };

        if self.server.is_some() {
            return self
                .server
                .as_mut()
                .unwrap()
                .bind_mut()
                .verify_client(identifier, uuid);
        } else {
            godot_error!("called verify_client but we are not a server");
        }
    }
    #[func]
    fn start_client(
        &mut self,
        server_ip: String,
        server_port: i32,
        psk_identifier: String,
        psk_key: PackedByteArray,
        identifier: PackedByteArray,
    ) {
        let c = NetNodeClient::new_alloc();
        self.base_mut().add_child(&c);
        self.client = Some(c);
        self.client.as_mut().unwrap().bind_mut().start_client(
            SocketAddr::new(IpAddr::from_str(&server_ip).unwrap(), server_port as u16),
            psk_identifier,
            psk_key.to_vec(),
            identifier.to_vec().try_into().unwrap(),
        );
    }
    #[func]
    fn start_server(&mut self, bind_port: u16) {
        let s = NetNodeServer::new_alloc();
        self.base_mut().add_child(&s);
        self.server = Some(s);
        self.server
            .as_mut()
            .unwrap()
            .bind_mut()
            .start_server(bind_port);
        self.is_server = true;
    }
    #[func]
    fn stop(&mut self) {
        self.is_server = false;
        if self.client.is_some() {
            self.client.as_mut().unwrap().bind_mut().disconnect();
            self.client.as_mut().unwrap().queue_free();
            self.client = None;
        } else if self.server.is_some() {
            todo!(
                "server does not have graceful stop functionality yet, ensure clients have disconnected then kill the server process"
            )
        }
    }
    #[func]
    fn get_next_client(&mut self) -> PackedByteArray {
        if self.server.is_some() {
            match self.server.as_mut().unwrap().bind_mut().get_next_client() {
                Ok(client) => PackedByteArray::from(client),
                Err(e) => {
                    godot_error!("failed to get next client: {:?}", e);
                    PackedByteArray::new()
                }
            }
        } else {
            panic!("called get_next_client() but we are not a server");
        }
    }
    #[func]
    pub fn is_server(&self) -> bool {
        self.is_server
    }
    #[func]
    fn get_player_count(&self) -> i32 {
        if self.server.is_some() {
            return self.server.as_ref().unwrap().bind().get_player_count() as i32;
        } else {
            panic!("tried to get_player_count but we are not a server");
        }
    }
    fn register_message_handler(&mut self, handler: Gd<MessageHandler>, message_type: u64) {
        if self.client.is_some() {
            return self
                .client
                .as_mut()
                .unwrap()
                .bind_mut()
                .register_message(handler, message_type);
        } else if self.server.is_some() {
            return self
                .server
                .as_mut()
                .unwrap()
                .bind_mut()
                .register_message(handler, message_type);
        } else {
            panic!("tried to register_message_handler but no client or server is running");
        }
    }
    fn unregister_message_handler(&mut self, message_type: u64) {
        if self.client.is_some() {
            return self
                .client
                .as_mut()
                .unwrap()
                .bind_mut()
                .unregister_message(message_type);
        } else if self.server.is_some() {
            return self
                .server
                .as_mut()
                .unwrap()
                .bind_mut()
                .unregister_message(message_type);
        } else {
            panic!("tried to unregister_message_handler but no client or server is running");
        }
    }
    fn queue_message(&mut self, message: BitVec<u64, Lsb0>, stream: u64) {
        if self.client.is_some() {
            self.client
                .as_mut()
                .unwrap()
                .bind_mut()
                .queue_message(message, stream);
        } else if self.server.is_some() {
            self.server
                .as_mut()
                .unwrap()
                .bind_mut()
                .queue_message(message, stream);
        } else {
            godot_warn!("tried to queue_message but no client or server is running");
        }
    }
    #[signal]
    pub fn player_joined(player: u16);
    #[signal]
    pub fn player_left(player: u16);
}
