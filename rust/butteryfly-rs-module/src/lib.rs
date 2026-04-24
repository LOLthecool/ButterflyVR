#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

//! Low level networking library for the Godot engine.
//!
//! Currently, Godot provides two networking APIs:
//! the low-level [`MultiplayerPeer`] API and the high-level [`MultiplayerAPI`] API.
//! while these both offer low level access to send raw packets directly,
//! their high level implementations lack the control necessary to optimise networking.
//! sending raw packets, however, requires that the user implements the entire networking protocol themselves.
//! NetNodes intends to be a middle ground, offering the user bit level control over their data representation,
//! while also being as optimised and convienient as possible by default.
//!
//! note: this library is currently for internal use only. it can be used in other projects,
//! but the API is subject to change and is not guaranteed to be stable, and many usercases will be poorly supported
//!
//! features:
//! - zero type overhead by default, encoding and decoding is done symmetrically by a single user defined GDScript function
//! - multiple serialization options for each data type, both lossless and lossy, with various quantization levels
//! - branching type decoding using previously decoded data, allowing unneeded data to skip encoding
//! - reliable, ordered messages for sending and receiving data
//! - independent ordered networking streams
//! - flexible node based networking
//! - secure quic based networking
//!
//! upcoming features:
//! - delta encoding, usable by both the type serializer and the user defined GDScript function
//!
//! getting started:
//!

// todo:
// doc strings
// run through ai
// unit tests

mod client;
mod common;
mod messages;
mod net_nodes;
mod networker;
mod serializer;
mod server;

use std::net::IpAddr;
use std::net::SocketAddr;
use std::str::FromStr;

use crate::client::NetNodeClient;
use crate::messages::MessageHandler;
use crate::net_nodes::NetworkedNode;
use crate::server::NetNodeServer;
use bitvec::prelude::*;
use godot::prelude::*;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

#[derive(GodotClass)]
#[class(init, base=Node)]
struct NetNodeManager {
    inner: Inner,
    base: Base<Node>,
}

#[derive(Debug, Default)]
enum Inner {
    #[default]
    None,
    Client(Gd<NetNodeClient>),
    Server(Gd<NetNodeServer>),
}

#[godot_api]
impl NetNodeManager {
    fn register_node(&mut self, node_ref: Gd<NetworkedNode>, node: &mut NetworkedNode) {
        match self.inner {
            Inner::Server(ref mut server) => {
                server.bind_mut().register_node(node_ref);
            }
            Inner::Client(ref mut client) => {
                client.bind_mut().register_node(node_ref, node);
            }
            Inner::None => {
                godot_warn!("called register_node but no client or server is running");
            }
        }
    }
    fn unregister_node(&mut self, node_ref: &Gd<NetworkedNode>) {
        match self.inner {
            Inner::Server(ref mut server) => {
                server.bind_mut().unregister_node(node_ref);
            }
            Inner::Client(ref mut client) => {
                client.bind_mut().unregister_node(node_ref);
            }
            Inner::None => {
                // can get called when no client or server is active after client dc so we ignore that case here
            }
        }
    }
    #[func]
    fn get_next_object_id(&mut self) -> u16 {
        match self.inner {
            Inner::Server(ref mut server) => server.bind_mut().get_next_object_id(),
            _ => {
                godot_error!("called get_next_object_id but we are not a server");
                0
            }
        }
    }
    #[func]
    fn get_unverified_client(&mut self) -> PackedByteArray {
        match self.inner {
            Inner::Server(ref mut server) => server
                .bind_mut()
                .get_unverified_client()
                .map_or_else(PackedByteArray::new, |x| x.to_godot().to_packed_array()),
            _ => {
                godot_error!("called get_unverified_client but we are not a server");
                PackedByteArray::new()
            }
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

        match self.inner {
            Inner::Server(ref mut server) => {
                server.bind_mut().verify_client(identifier, uuid);
            }
            _ => {
                godot_error!("called verify_client but we are not a server");
            }
        }
    }
    #[func]
    fn start_client(
        &mut self,
        server_ip: String,
        server_port: u16,
        psk_identifier: String,
        psk_key: PackedByteArray,
        identifier: PackedByteArray,
    ) {
        let mut c = NetNodeClient::new_alloc();
        self.base_mut().add_child(&c);
        c.bind_mut().start_client(
            SocketAddr::new(IpAddr::from_str(&server_ip).unwrap(), server_port),
            psk_identifier,
            psk_key.to_vec(),
            identifier.to_vec().try_into().unwrap(),
        );
        self.inner = Inner::Client(c);
    }
    #[func]
    fn start_server(&mut self, bind_port: u16) {
        let mut s = NetNodeServer::new_alloc();
        self.base_mut().add_child(&s);
        s.bind_mut().start_server(bind_port);
        self.inner = Inner::Server(s);
    }
    #[func]
    fn stop(&mut self) {
        match self.inner {
            Inner::Client(ref mut client) => {
                client.bind_mut().disconnect();
                client.queue_free();
            }
            Inner::Server(_) => {
                todo!(
                    "server does not have graceful stop functionality yet, ensure clients have disconnected then kill the server process"
                )
            }
            Inner::None => {}
        }
    }
    #[func]
    fn get_next_client(&mut self) -> PackedByteArray {
        match self.inner {
            Inner::Server(ref mut server) => match server.bind_mut().get_next_client() {
                Ok(client) => PackedByteArray::from(client),
                Err(e) => {
                    godot_error!("failed to get next client: {:?}", e);
                    PackedByteArray::new()
                }
            },
            _ => {
                godot_error!("called get_next_client() but we are not a server");
                PackedByteArray::new()
            }
        }
    }
    #[func]
    pub fn is_server(&self) -> bool {
        matches!(self.inner, Inner::Server(_))
    }
    #[func]
    fn get_player_count(&self) -> i32 {
        match &self.inner {
            Inner::Server(server) => server.bind().get_player_count() as i32,
            _ => {
                godot_error!("tried to get_player_count but we are not a server");
                0
            }
        }
    }
    fn register_message_handler(&mut self, handler: Gd<MessageHandler>, message_type: u64) {
        match &mut self.inner {
            Inner::Client(client) => client.bind_mut().register_message(handler, message_type),
            Inner::Server(server) => server.bind_mut().register_message(handler, message_type),
            _ => {
                godot_error!(
                    "tried to register_message_handler but no client or server is running"
                );
            }
        }
    }
    fn unregister_message_handler(&mut self, message_type: u64) {
        match &mut self.inner {
            Inner::Client(client) => client.bind_mut().unregister_message(message_type),
            Inner::Server(server) => server.bind_mut().unregister_message(message_type),
            _ => {
                godot_error!(
                    "tried to unregister_message_handler but no client or server is running"
                );
            }
        }
    }
    fn queue_message(&mut self, message: BitVec<u64, Lsb0>, stream: u64) {
        match &mut self.inner {
            Inner::Client(client) => client.bind_mut().queue_message(message, stream),
            Inner::Server(server) => server.bind_mut().queue_message(message, stream),
            _ => {
                godot_warn!("tried to queue_message but no client or server is running");
            }
        }
    }
    #[signal]
    pub fn player_joined(player: u16);
    #[signal]
    pub fn player_left(player: u16);
}
