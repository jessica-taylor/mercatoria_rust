//! A trait for event-handling.
use super::event::Event;
use crate::network::Network;
use async_trait::*;

/// Handles network events.
#[async_trait]
pub trait Role<N: Network> {
    /// Handles a network event.
    async fn handle_event(&self, _event: &Event<N>) {}
}
