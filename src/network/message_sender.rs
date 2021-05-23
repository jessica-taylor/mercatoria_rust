//! Sending of messages over the network.

use super::message::{Message, MessageContent, MessageId};
use super::role::Role;
use super::Network;
use anyhow::anyhow;
use std::sync::Arc;

/// Sends messages.
pub struct MessageSender<N: Network> {
    message_id: u64,
    network: Arc<N>,
}

impl<N: Network> Role<N> for MessageSender<N> {}

impl<N: Network> MessageSender<N> {
    /// Creates a new `MessageSender`.
    pub fn new(network: Arc<N>) -> MessageSender<N> {
        MessageSender {
            message_id: 0,
            network,
        }
    }
    /// Returns a new message ID unique to this `MessageSender`.
    pub fn reserve_message_id(&mut self) -> MessageId {
        self.message_id += 1;
        self.message_id
    }
    /// Sends a message.
    pub async fn send_message(
        &mut self,
        recip: N::Pid,
        msg: MessageContent,
    ) -> Result<(), anyhow::Error> {
        let msg = Message::<N> {
            content: msg,
            sender: self.network.get_network_pid(),
            id: self.reserve_message_id(),
        };
        self.network
            .send(&recip, rmp_serde::to_vec_named(&msg).unwrap())
            .await
    }
}
