use super::event::Event;
use crate::network::Network;
use async_trait::*;

#[async_trait]
pub trait Role<N: Network> {
    async fn handle_event(&mut self, event: &Event<N>) {}
}
