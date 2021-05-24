//! Tracking of a node's peers.

use super::message::{Message, MessageContent, MessageId};
use super::role::Role;
use super::Network;
use anyhow::anyhow;
use std::collections::BTreeSet;
use std::sync::RwLock;

/// Tracks the node's peers.
pub struct PeerTracker<N: Network> {
    peers: RwLock<BTreeSet<N::Pid>>,
}

impl<N: Network> Role<N> for PeerTracker<N> {}

impl<N: Network> PeerTracker<N> {
    /// Creates a new `PeerTracker`.
    pub fn new() -> PeerTracker<N> {
        PeerTracker {
            peers: RwLock::new(BTreeSet::new()),
        }
    }
    /// Gets the node's peers.
    pub fn get_peers(&self) -> BTreeSet<N::Pid> {
        let peers = self.peers.read().unwrap();
        (*peers).clone()
    }
    /// Adds new peers.
    pub fn add_peers(&self, mut new_peers: BTreeSet<N::Pid>) {
        let mut peers = self.peers.write().unwrap();
        (*peers).append(&mut new_peers);
    }
}
