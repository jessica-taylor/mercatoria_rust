//! Runs `HashLookup`/`HashPut` over the network.

use super::log::Log;
use super::message_sender::{MessageSender, QuerySender};
use super::peer_tracker::PeerTracker;
use super::role::Role;
use super::Network;
use crate::crypto::{hash, xor_hash_codes, HashCode};
use crate::hashlookup::{HashLookup, HashPut};
use anyhow::bail;
use async_trait::*;
use std::collections::BTreeSet;
use std::sync::Arc;

/// A `Role` for running `HashLookup`/`HashPut` over the network.
pub struct HashOps<N: Network + 'static> {
    log: Arc<Log>,
    peer_tracker: Arc<PeerTracker<N>>,
    message_sender: Arc<MessageSender<N>>,
    query_sender: Arc<QuerySender<N>>,
}

impl<N: Network> HashOps<N> {
    /// Creates a new `HashOps`.
    pub fn new(
        log: Arc<Log>,
        peer_tracker: Arc<PeerTracker<N>>,
        message_sender: Arc<MessageSender<N>>,
        query_sender: Arc<QuerySender<N>>,
    ) -> HashOps<N> {
        HashOps {
            log,
            peer_tracker,
            message_sender,
            query_sender,
        }
    }

    /// Gets the peers who are most likely to be storing a data corresponding to a particular hash
    /// code.  This uses a basic DHT algorithm.
    pub fn hash_to_storing_peers(&self, code: HashCode) -> BTreeSet<N::Pid> {
        let mut peers: Vec<N::Pid> = self.peer_tracker.get_peers().into_iter().collect();
        peers.sort_by_key(|pid| xor_hash_codes(hash(pid).code, code));
        peers.truncate(30);
        peers.into_iter().collect()
    }
}

#[async_trait]
impl<N: Network + 'static + Send + Sync> HashLookup for HashOps<N> {
    async fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, anyhow::Error> {
        bail!("TODO")
    }
}
