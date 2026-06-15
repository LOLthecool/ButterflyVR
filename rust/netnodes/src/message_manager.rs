use crate::{
    common::{InternalMessage, MESSAGE_HEADER_SIZE, decode_internal_message},
    messages::MessageHandler,
};
use bitvec::{order::Lsb0, vec::BitVec};
use godot::prelude::*;
use std::collections::{HashMap, hash_map::Entry};

#[derive(GodotClass)]
#[class(init, base=Node)]
pub struct MessageManager {
    base: Base<Node>,
    message_handlers: HashMap<u16, Gd<MessageHandler>>,
}

#[godot_api]
impl MessageManager {
    pub fn handle_internal_message(&mut self, data: BitVec<u64, Lsb0>) {
        let mut pointer = MESSAGE_HEADER_SIZE;
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
                InternalMessage::MessageHandlerIdAssign((id, handler)) => {
                    self.assign_handler(id, handler)
                }
            }
        } else {
            godot_error!("failed to decode internal message");
        }
    }

    pub fn handle_message(
        &mut self,
        data: &mut BitVec<u64, Lsb0>,
        handler_id: u16,
        pointer: &mut usize,
    ) {
        if let Some(handler) = self.message_handlers.get_mut(&handler_id) {
            let _ = handler
                .bind_mut()
                .handle_message(data, pointer, false, false);
        } else {
            godot_error!("no handler for message type {handler_id:?}");
        }
    }

    pub fn assign_handler(&mut self, message_type: u16, mut handler: Gd<MessageHandler>) {
        if let Entry::Vacant(x) = self.message_handlers.entry(message_type) {
            handler.bind_mut().message_id = message_type;
            x.insert(handler);
        } else {
            godot_warn!(
                "tried to register duplicate handlers for message type {:#?}",
                message_type
            );
        }
    }

    pub fn unregister_message(&mut self, message_type: u16) {
        self.message_handlers.remove(&message_type);
    }
}
