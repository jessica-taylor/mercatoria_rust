//! Sending of messages over the network.

use super::log::Log;
use super::message::{Message, MessageContent, MessageId, Reply};
use super::role::Role;
use super::Network;
use anyhow::anyhow;
use std::sync::{Arc, RwLock};

/// Sends messages.
pub struct MessageSender<N: Network> {
    message_id: RwLock<u64>,
    network: Arc<N>,
}

impl<N: Network> Role<N> for MessageSender<N> {}

impl<N: Network> MessageSender<N> {
    /// Creates a new `MessageSender`.
    pub fn new(network: Arc<N>) -> MessageSender<N> {
        MessageSender {
            message_id: RwLock::new(0),
            network,
        }
    }
    /// Returns a new message ID unique to this `MessageSender`.
    pub fn reserve_message_id(&self) -> MessageId {
        let mut mid = self.message_id.write().unwrap();
        *mid += 1;
        *mid
    }
    /// Sends a message.
    pub async fn send_message(
        &self,
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

/// Sends queries (messages that receive replies)
pub struct QuerySender<N: Network> {
    network: Arc<N>,
    log: Arc<Log>,
    sender: Arc<MessageSender<N>>,
    handlers: Vec<(MessageId, i64, Box<FnOnce(Result<Reply, String>) -> ()>)>,
}

impl<N: Network> QuerySender<N> {
    /// Creates a new `QuerySender`.
    fn new(network: Arc<N>, log: Arc<Log>, sender: Arc<MessageSender<N>>) -> QuerySender<N> {
        QuerySender {
            network,
            log,
            sender,
            handlers: Vec::new(),
        }
    }
    // async fn send_and_receive_reply(&mut self, u64, N::Pid, MessageContent) -> Result<Reply, anyhow::Error> {
    //     let mid = self.sender.reserve_message_id();
    // }
}
