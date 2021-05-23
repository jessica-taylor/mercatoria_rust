//! Functionality for timed graph computations run over the network.
#![allow(dead_code)]

use super::*;
use anyhow::*;
use futures::{
    prelude::*,
    stream::{self},
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::hash::Hash;

/// Specification of a timed graph computation run over the network.
#[async_trait]
pub trait ComputeGraph {
    /// The type of computations.
    type Comp: Eq + Hash + Clone;

    /// The type of messages (inputs/outputs of computations).
    type Msg: Eq + Hash;

    /// The type of addresses (parties who send/receive messages and do computations).
    type Addr: Eq + Hash;

    /// At what time should a given computation be run?  (epoch milliseconds)
    async fn when_to_run(&self, c: &Self::Comp) -> Result<i64, anyhow::Error>;

    /// What set of addresses runs a given computation?
    async fn who_runs(&self, c: &Self::Comp) -> Result<Vec<Self::Addr>, anyhow::Error>;

    /// What computation is a given message an input to?
    async fn message_comp(&self, m: &Self::Msg) -> Result<Option<Self::Comp>, anyhow::Error>;

    /// Runs a computation with input messages, returning output messages.
    async fn run_comp(
        &self,
        c: Self::Comp,
        ms: HashSet<Self::Msg>,
    ) -> Result<Vec<Self::Msg>, anyhow::Error>;
}

/// The state of a computation that may be run, stored by a `ComputeNode`.
enum ComputationState<M> {
    /// Computation has already finished.
    Done,
    /// Computation is not run by this address.
    NotMe,
    /// Input messages have been accumulated and the computation has not been run.
    Inputs(HashSet<M>),
}

/// The state of an address-having network node running computations.
pub struct ComputeNode<G: ComputeGraph> {
    /// The `ComputeGraph` specifying the computation graph.
    pub graph: G,
    /// The address of this node.
    pub addr: G::Addr,
    /// States of different computations.
    states: HashMap<G::Comp, ComputationState<G::Msg>>,
    /// Computations indexed by the time at which they are run (epoch milliseconds).
    by_time: BTreeMap<i64, HashSet<G::Comp>>,
}

impl<G: ComputeGraph> ComputeNode<G> {
    /// Receives new messages corresponding to a computation.
    pub async fn receive(
        &mut self,
        c: G::Comp,
        mut new_msgs: HashSet<G::Msg>,
    ) -> Result<(), anyhow::Error> {
        let mut comp_was_none = false;

        match self.states.get_mut(&c) {
            Some(ComputationState::Done) => {}
            Some(ComputationState::Inputs(msgs)) => {
                msgs.extend(new_msgs.drain());
            }
            None => {
                comp_was_none = true;
            }
            _ => {}
        }

        if comp_was_none {
            if !self.graph.who_runs(&c).await?.contains(&self.addr) {
                self.states.insert(c, ComputationState::NotMe);
            } else {
                let time = self.graph.when_to_run(&c).await?;
                self.by_time.entry(time).or_default().insert(c.clone());
                self.states.insert(c, ComputationState::Inputs(new_msgs));
            }
        }

        Ok(())
    }

    /// Receives a message.
    pub async fn receive_msg(&mut self, m: G::Msg) -> Result<(), anyhow::Error> {
        if let Some(c) = self.graph.message_comp(&m).await? {
            self.receive(c, std::iter::once(m).collect()).await?;
        }
        Ok(())
    }

    /// Ensures that a computation will be run eventually.
    pub async fn receive_comp(&mut self, c: G::Comp) -> Result<(), anyhow::Error> {
        self.receive(c, std::iter::empty().collect()).await
    }

    /// Updates time forward, running queued computations accordingly.
    pub async fn tick<F, Fut>(&mut self, send_msg: &F, now: i64) -> Result<(), anyhow::Error>
    where
        F: for<'a> Fn(&G::Comp, &G::Msg, G::Addr) -> Fut,
        Fut: Future<Output = Result<(), anyhow::Error>>,
    {
        let mut split = self.by_time.split_off(&now);
        std::mem::swap(&mut split, &mut self.by_time);
        let comps = split
            .into_iter()
            .flat_map(|(_, cs)| cs)
            .map(|c| {
                let entry = self.states.get_mut(&c)?;
                let mut state = ComputationState::Done;
                std::mem::swap(entry, &mut state);
                match state {
                    ComputationState::Inputs(m) => Some((c, m)),
                    _ => None,
                }
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| anyhow!("a computation with a timestamp must have an Inputs state"))?;

        let g = &self.graph;
        stream::iter(comps)
            .map(|(c, m)| g.run_comp(c, m))
            .buffer_unordered(64)
            // stream output messages and flatten
            .map_ok(|msgs| stream::iter(msgs).map(Ok))
            .try_flatten()
            // send messages
            .map_ok(|m| async move {
                if let Some(c) = g.message_comp(&m).await? {
                    let addrs = g.who_runs(&c).await?;
                    stream::iter(addrs)
                        .map(|a| send_msg(&c, &m, a))
                        .buffer_unordered(64)
                        .try_collect()
                        .await
                } else {
                    Ok(())
                }
            })
            .try_buffer_unordered(64)
            // wait until they're all done, fail at first failure
            .try_collect()
            .await
    }
}
