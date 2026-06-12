#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::doc_markdown)]
// todo: split message handling to seperate node, call deferred from common, server and client get scene access via this node
// message applier just calls the usual handle_message, handle message shouldnt defer itself

//! Low level networking library for the Godot engine.
//!
//! Note: this library is currently for internal use only. It can be used in other projects,
//! but the API is subject to change and is not guaranteed to be stable, and many use cases will be poorly supported.
//!
//! Currently, Godot provides two networking APIs:
//! the low-level MultiplayerPeer API and the high-level MultiplayerAPI API.
//! While these both offer low-level access to send raw packets directly,
//! their high-level implementations lack the control necessary to optimise networking.
//! Sending raw packets, however, requires that the user implements the entire networking protocol themselves.
//! NetNodes intends to be a middle ground, offering the user bit level control over their data representation,
//! while also being as optimised and convenient as possible by default.
//!
//! Features:
//! - zero type overhead by default, encoding and decoding is done symmetrically by a single user defined GDScript function
//! - reliable, ordered messages for sending and receiving data
//! - per frame, unreliable networking for syncing node variables
//! - independent ordered networking streams
//! - flexible node based networking
//!
//! Upcoming features:
//! - unordered unreliable datagram oriented networking functions
//! - multiple serialization options for each data type, both lossless and lossy, with various quantization levels
//! - branching type decoding using previously decoded data, allowing unneeded data to skip encoding
//! - delta encoding, usable by both the type serializer and the user defined GDScript function
//! - certificate authentication for servers

mod client;
mod common;
mod message_manager;
mod messages;
mod net_nodes;
mod networker;
mod serializer;
mod server;

use std::net::IpAddr;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use crate::client::NetNodeClient;
use crate::message_manager::MessageManager;
use crate::messages::MessageHandler;
use crate::net_nodes::NetworkedNode;
use crate::server::NetNodeServer;
use bitvec::prelude::*;
use godot::prelude::*;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

/// Handles networking API access by scripts. Contains an inner client or server that handles the actual networking.
/// This must be registered as an autoload singleton named "NetworkManager" in the project settings
/// in order for NetNodes to work. This is also used by NetworkedNodes and MessageHandlers in the background.
/// The inner client or server must be started before either of them are added to the scene tree.
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
    Client(NetNodeClient),
    Server(NetNodeServer),
}

#[godot_api]
impl NetNodeManager {
    /// Not to be called directly; used by `NetworkedNode` internally.
    /// Registers a `NetworkedNode` with the inner client or server, allowing it to be synced across the network.
    fn register_node(&mut self, node: &mut NetworkedNode) {
        match self.inner {
            Inner::Server(ref mut server) => {
                server.register_node(node);
            }
            Inner::Client(ref mut client) => {
                client.register_node(node);
            }
            Inner::None => {
                godot_warn!("called register_node but no client or server is running");
            }
        }
    }

    /// Not to be called directly; used by `NetworkedNode` internally.
    /// Unregisters a `NetworkedNode` from the inner client or server, preventing it from being synced across the network.
    fn unregister_node(&mut self, node_ref: &Gd<NetworkedNode>) {
        match self.inner {
            Inner::Server(ref mut server) => {
                server.unregister_node(node_ref);
            }
            Inner::Client(ref mut client) => {
                client.unregister_node(node_ref);
            }
            Inner::None => {
                godot_warn!("called unregister_node but no client or server is running");
            }
        }
    }

    #[func]
    fn is_running(&self) -> bool {
        matches!(self.inner, Inner::Server(_) | Inner::Client(_))
    }

    /// Gets the next client that has connected to the server, but has not yet been verified.
    /// This is intended to connect the identifier used by this client to a user account on another server.
    /// To use this, you should keep track of which user received an identifier on the other server,
    /// Get the user's identifier from this function, retrieve the account associated with the user from the other server,
    /// and then call `verify_client` with the user's identifier and account.
    /// If you do not need to associate the client with an external account, you can generate a random identifier for the account.
    /// Once `verify_client` is called, the client finishes the connection process and will be processed on the server.
    /// This is a server-only method.
    #[func]
    fn get_unverified_client(&mut self) -> PackedByteArray {
        if let Inner::Server(ref mut server) = self.inner {
            server
                .get_unverified_client()
                .map_or_else(PackedByteArray::new, |x| x.to_godot().to_packed_array())
        } else {
            godot_error!("called get_unverified_client but we are not a server");
            PackedByteArray::new()
        }
    }

    /// Associates a client with a user account on the server.
    /// This is intended to connect the identifier used by this client to a user account on another server.
    /// To use this, you should keep track of the user who received an identifier on the other server,
    /// get the user's identifier from `get_unverified_client`, retrieve the account associated with the user from the other server,
    /// and then call this function with the user's identifier and account.
    /// If you do not need to associate the client with an external account, you can generate a random identifier for the account.
    /// Once this function is called, the client finishes the connection process and will be processed on the server.
    /// This is a server-only method.
    #[func]
    fn verify_client(&mut self, identifier: PackedByteArray, uuid: PackedByteArray) {
        let Ok(identifier) = identifier.to_vec().try_into() else {
            godot_error!(
                "verify_client(): identifier was not correct size: should be {:?} got {:?}",
                8,
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

        if let Inner::Server(ref mut server) = self.inner {
            server.verify_client(identifier, uuid);
        } else {
            godot_error!("called verify_client but we are not a server");
        }
    }

    /// Reject a client that tries to send an invalid identifier, the client will be disconnected and added to the blocklist.
    /// The blocklist will prevent the client from connecting until the block expires.
    #[func]
    fn reject_client(&mut self, identifier: PackedByteArray) {
        let Ok(identifier) = identifier.to_vec().try_into() else {
            godot_error!(
                "reject_client(): identifier was not correct size: should be {}, got {}",
                8,
                identifier.len()
            );
            return;
        };

        if let Inner::Server(ref mut server) = self.inner {
            server.reject_client(identifier);
        } else {
            godot_error!("called reject_client but we are not a server");
        }
    }

    /// retrieves the list of registered NetworkedNodes.
    #[func]
    fn get_networked_nodes(&self) -> Array<Gd<NetworkedNode>> {
        match self.inner {
            Inner::Client(ref client) => client.get_networked_nodes().to_godot(),
            Inner::Server(ref server) => server.get_networked_nodes().to_godot(),
            Inner::None => {
                godot_error!("called get_networked_nodes but no client or server is running");
                Array::new()
            }
        }
    }

    /// Retrieves the highest RTT (round-trip time) in milliseconds for the current connection.
    /// For the client this is the current RTT, for the server this is the current highest RTT of all connected clients.
    /// Returns -1 if no connection is established, a RTT is not yet available, or another error occurs.
    #[func]
    fn get_highest_rtt_millis(&self) -> i32 {
        match self.inner {
            Inner::Client(ref client) => client
                .get_connection_rtt()
                .map(|x| Duration::as_secs(&x) as i32),
            Inner::Server(ref server) => server
                .get_highest_connection_rtt()
                .map(|x| Duration::as_secs(&x) as i32),
            Inner::None => {
                godot_error!("called get_highest_rtt_millis but no client or server is running");
                None
            }
        }
        .unwrap_or(-1)
    }

    /// Returns the UUID of any clients that have joined the server since the last call to this method.
    /// These clients have finished joining.
    /// This is a server-only method.
    #[func]
    fn get_new_joins(&mut self) -> Array<PackedByteArray> {
        if let Inner::Server(ref mut server) = self.inner {
            server
                .get_new_joins()
                .into_iter()
                .map(|x| x.to_vec().to_godot().to_packed_array())
                .collect::<Array<PackedByteArray>>()
        } else {
            godot_error!("called get_new_joins but we are not a server");
            Array::new()
        }
    }

    /// Returns the UUID of any clients that have left the server since the last call to this method.
    /// These clients have no active connection.
    /// This is a server-only method.
    #[func]
    fn get_dc_clients(&mut self) -> Array<PackedByteArray> {
        if let Inner::Server(ref mut server) = self.inner {
            server
                .get_dc_clients()
                .into_iter()
                .map(|x| x.to_vec().to_godot().to_packed_array())
                .collect::<Array<PackedByteArray>>()
        } else {
            godot_error!("called get_dc_clients but we are not a server");
            Array::new()
        }
    }

    /// Starts a client connection to a server using the given server address and identifier.
    /// The uuid and identifier should corrospond to a uuid and identifier combo from an external server.
    /// If the server does not require an associated account, you may generate both randomly.
    /// Once this function is called, client only and generic networking api methods become available. but it may still take some time to connect.
    #[func]
    fn start_client(
        &mut self,
        server_ip: String,
        server_port: u16,
        uuid: PackedByteArray,
        identifier: PackedByteArray,
    ) {
        let Ok(server_ip) = IpAddr::from_str(&server_ip) else {
            godot_error!("failed to connect to server: invalid IP: {server_ip:?}");
            return;
        };
        let Ok(uuid) = uuid.to_vec().try_into() else {
            godot_error!("failed to connect to server: uuid was wrong length: {uuid:?}");
            return;
        };
        let Ok(identifier) = identifier.to_vec().try_into() else {
            godot_error!(
                "failed to connect to server: psk_identifier was wrong length: {identifier:?}"
            );
            return;
        };

        let message_manager = MessageManager::new_alloc();
        self.base_mut().add_child(&message_manager);

        self.inner = Inner::Client(NetNodeClient::new(
            SocketAddr::new(server_ip, server_port),
            uuid,
            identifier,
            message_manager,
        ));
    }

    /// Starts a server on the given port. The server will always bind to `0.0.0.0`.
    /// Once this function is called, server only and generic networking api methods become available. but it may still take some time for any clients to connect.
    #[func]
    fn start_server(&mut self, bind_port: u16) {
        self.inner = Inner::Server(NetNodeServer::new(bind_port));
    }

    /// Stops the server or client, disconnecting any connected clients and freeing resources.
    /// This is a 'graceful' stop, the client will inform the server before disconnecting,
    /// and the server will wait for all clients to disconnect before stopping.
    /// To forcibly stop a client or server, call [`kill`] instead.
    #[func]
    fn stop(&mut self) {
        match self.inner {
            Inner::Client(ref mut client) => {
                client.disconnect();
            }
            Inner::Server(ref mut server) => {
                server.stop();
            }
            Inner::None => {}
        }
        self.inner = Inner::None;
    }

    /// Immediately stops the server or client, deleting associated resources.
    /// A connected client or server will not be notified of the disconnection.
    #[func]
    fn kill(&mut self) {
        self.inner = Inner::None;
    }

    /// Returns `true` if the node is a server, false if it is a client or not connected.
    #[func]
    pub fn is_server(&self) -> bool {
        matches!(self.inner, Inner::Server(_))
    }

    /// Returns the number of players currently connected to the server.
    /// This is a server-only method.
    #[func]
    fn get_player_count(&self) -> i32 {
        if let Inner::Server(server) = &self.inner {
            server.get_player_count() as i32
        } else {
            godot_error!("tried to get_player_count but we are not a server");
            0
        }
    }

    fn register_message_handler(
        &mut self,
        handler_ref: Gd<MessageHandler>,
        handler: &mut MessageHandler,
    ) {
        match &mut self.inner {
            Inner::Client(_) => {
                // todo: should probably avoid calling this in the first place on client to avoid confusion
                // if you happen to be debugging this, clients register their handlers in common::handle_stream_chunk
                // using message_ids from the server
                // this still gets called on enter_tree in the client so we ignore it
            }
            Inner::Server(server) => {
                server.register_message(handler_ref, handler);
            }
            Inner::None => {
                godot_error!(
                    "tried to register_message_handler but no client or server is running"
                );
            }
        }
    }

    fn unregister_message_handler(&mut self, message_type: u16) {
        match &mut self.inner {
            Inner::Client(client) => client.unregister_message(message_type),
            Inner::Server(server) => server.unregister_message(message_type),
            Inner::None => {
                godot_error!(
                    "tried to unregister_message_handler but no client or server is running"
                );
            }
        }
    }

    fn queue_message(&mut self, message: BitVec<u64, Lsb0>, stream: u64) {
        match &mut self.inner {
            Inner::Client(client) => client.queue_message(message, stream),
            Inner::Server(server) => server.queue_message(message, stream),
            Inner::None => {
                godot_warn!("tried to queue_message but no client or server is running");
            }
        }
    }
}

#[godot_api]
impl INode for NetNodeManager {
    fn physics_process(&mut self, _delta: f64) {
        match &mut self.inner {
            Inner::Client(client) => client.physics_process_inner(),
            Inner::Server(server) => server.physics_process_inner(),
            Inner::None => {}
        }
    }
}
