//! Tracking of a node's peers.

use super::message::{Message, MessageContent, MessageId};
use super::role::Role;
use super::Network;
use anyhow::anyhow;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Tracks the node's peers.
pub struct PeerTracker<N: Network> {
    peers: BTreeSet<N::Pid>,
}

impl<N: Network> Role<N> for PeerTracker<N> {}

impl<N: Network> PeerTracker<N> {
    /// Creates a new `PeerTracker`.
    pub fn new() -> PeerTracker<N> {
        PeerTracker {
            peers: BTreeSet::new(),
        }
    }
    /// Gets the node's peers.
    pub fn get_peers(&self) -> &BTreeSet<N::Pid> {
        &self.peers
    }
    /// Adds new peers.
    pub fn add_peers(&mut self, mut new_peers: BTreeSet<N::Pid>) {
        self.peers.append(&mut new_peers);
    }
}
