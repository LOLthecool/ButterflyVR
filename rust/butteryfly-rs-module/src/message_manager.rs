use crate::{
    NetNodeManager,
    common::{self, InternalMessage, decode_internal_message},
    messages::MessageHandler,
};
use bitvec::{order::Lsb0, vec::BitVec};
use godot::prelude::*;
use std::collections::{HashMap, hash_map::Entry};

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct MessageManager {
    base: Base<Node>,
    client: Option<Gd<NetNodeManager>>,
    message_handlers: HashMap<u16, Gd<MessageHandler>>,
}

#[godot_api]
impl MessageManager {
    pub fn handle_internal_message(&mut self, data: BitVec<u64, Lsb0>) {
        let mut pointer = common::MESSAGE_HEADER_SIZE;
        if let Ok(msg) = decode_internal_message(&data, &mut pointer, &self.base())
            .inspect_err(|e| godot_error!("{e:?}"))
        {
            match msg {
                InternalMessage::ClientId(_) => {
                    unreachable!();
                }
                InternalMessage::NetNodeIdAssign((id, mut node)) => {
                    node.bind_mut().objectid = id;
                }
                InternalMessage::MessageHandlerIdAssign((id, mut handler)) => {
                    if self
                        .client
                        .as_mut()
                        .unwrap()
                        .bind_mut()
                        .assign_handler(id, handler.clone())
                        .is_err()
                    {
                        godot_warn!(
                            "tried to register duplicate handlers for message type {:#?}",
                            id
                        );
                    }
                    handler.bind_mut().message_id = id;
                }
            }
        } else {
            godot_error!("failed to decode internal message");
        }
    }

    pub fn assign_handler(
        &mut self,
        message_type: u16,
        handler: Gd<MessageHandler>,
    ) -> Result<(), ()> {
        if let Entry::Vacant(x) = self.message_handlers.entry(message_type) {
            x.insert(handler);
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn unregister_message(&mut self, message_type: u16) {
        self.message_handlers.remove(&message_type);
    }
}
