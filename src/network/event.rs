//! Events that `Role`s may respond to.
use super::message::FullMessage;
use crate::blockdata::{MainBlock, MainBlockBody, QuorumNode};
use crate::crypto::{Hash, Signature};
use crate::network::Network;
use std::collections::BTreeSet;

/// An event that a `Role` may respond to.
pub enum Event<N: Network> {
    /// A new quorum tree has been created.
    NewTree(MainBlock, Hash<QuorumNode>),
    /// The main block has a sufficient number of signatures.
    EnoughMainSignatures(MainBlockBody, BTreeSet<Signature<MainBlockBody>>),
    /// Some amount of time has advanced.
    Tick,
    /// A message has been received.
    Received(FullMessage<N>),
}
