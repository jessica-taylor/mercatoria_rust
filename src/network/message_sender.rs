//! Sending of messages over the network.

use super::event::Event;
use super::log::Log;
use super::message::{Message, MessageContent, MessageId, Reply};
use super::role::Role;
use super::Network;
use anyhow::anyhow;
use async_trait::*;
use core::pin::Pin;
use std::borrow::BorrowMut;
use std::future::Future;
use std::marker::Send;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};

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
    /// Sends a message with an already-reserved `MessageId`.
    pub async fn send_message_with_id(
        &self,
        mid: MessageId,
        recip: N::Pid,
        msg: MessageContent,
    ) -> Result<(), anyhow::Error> {
        let msg = Message::<N> {
            content: msg,
            sender: self.network.get_network_pid(),
            id: mid,
        };
        self.network
            .send(&recip, rmp_serde::to_vec_named(&msg).unwrap())
            .await
    }
    /// Sends a message.
    pub async fn send_message(
        &self,
        recip: N::Pid,
        msg: MessageContent,
    ) -> Result<(), anyhow::Error> {
        self.send_message_with_id(self.reserve_message_id(), recip, msg)
            .await
    }
}

struct QueryResult<R> {
    value: Arc<RwLock<Option<R>>>,
}

impl<R> Clone for QueryResult<R> {
    fn clone(&self) -> Self {
        QueryResult {
            value: self.value.clone(),
        }
    }
}

impl<R> QueryResult<R> {
    fn new() -> QueryResult<R> {
        QueryResult {
            value: Arc::new(RwLock::new(None)),
        }
    }
    fn produce_result(&self, res: R) {
        let mut value = self.value.write().unwrap();
        match (*value) {
            Some(_) => panic!("wrote a query result twice"),
            None => {
                (*value) = Some(res);
            }
        }
    }
}

impl<R> Future for QueryResult<R> {
    type Output = R;
    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<R> {
        let mut value = self.value.write().unwrap();
        match &(*value) {
            None => Poll::Pending,
            Some(_) => {
                let mut out = None;
                std::mem::swap(&mut out, value.borrow_mut());
                Poll::Ready(out.unwrap())
            }
        }
    }
}

type Handler = (
    MessageId,
    i64,
    Box<Send + Sync + FnOnce(Result<Reply, anyhow::Error>) -> ()>,
);

/// Sends queries (messages that receive replies)
pub struct QuerySender<N: Network + 'static> {
    network: Arc<N>,
    log: Arc<Log>,
    sender: Arc<MessageSender<N>>,
    handlers: RwLock<Vec<Handler>>,
}

impl<N: Network + 'static> QuerySender<N> {
    /// Creates a new `QuerySender`.
    fn new(network: Arc<N>, log: Arc<Log>, sender: Arc<MessageSender<N>>) -> QuerySender<N> {
        QuerySender {
            network,
            log,
            sender,
            handlers: RwLock::new(Vec::<Handler>::new()),
        }
    }
    /// Sends a message, returning the reply.
    async fn send_and_receive_reply(
        self: Arc<Self>,
        timeout_ms: u32,
        recip: N::Pid,
        msg: MessageContent,
    ) -> Result<Reply, anyhow::Error> {
        let mid = self.sender.reserve_message_id();
        let send_time = self.network.get_network_time().await?.timestamp();
        let timeout_time = send_time + (timeout_ms as i64);
        let q_result = QueryResult::<Result<Reply, anyhow::Error>>::new();
        let q_result2 = q_result.clone();
        let handler = move |result| {
            q_result2.produce_result(result);
        };
        let mut handlers = self.handlers.write().unwrap();
        (*handlers).push((mid, timeout_time, Box::new(handler)));
        self.sender.send_message_with_id(mid, recip, msg).await?;
        q_result.await
    }
}

#[async_trait]
impl<N: Network + 'static + Send + Sync> Role<N> for QuerySender<N> {
    async fn handle_event(&self, event: &Event<N>) {
        match event {
            Event::Received(msg) => match &msg.content {
                MessageContent::Reply(replied_to, rep) => {
                    let mut handlers = self.handlers.write().unwrap();
                    for i in 0..(*handlers).len() {
                        if (*handlers)[i].0 == *replied_to {
                            let (_, _, handler) = (*handlers).remove(i);
                            drop(handlers);
                            handler(Ok(rep.clone()));
                            return;
                        }
                    }
                }
                _ => {}
            },
            Event::Tick => {
                let curr_time = self.network.get_network_time().await.unwrap().timestamp();
                let mut handlers = self.handlers.write().unwrap();
                let mut ix = 0;
                while ix < (*handlers).len() {
                    if (*handlers)[ix].1 > curr_time {
                        break;
                    }
                    ix += 1;
                }
                let mut done_handlers = handlers.split_off(ix);
                std::mem::swap(&mut done_handlers, &mut *handlers);
                drop(handlers);
                for (_, _, handler) in done_handlers {
                    handler(Err(anyhow!("timeout")));
                }
            }
            _ => {}
        }
    }
}
