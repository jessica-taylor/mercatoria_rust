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

#[async_trait]
trait ComputeGraph {
    type Comp: Eq + Hash + Clone;
    type Msg: Eq + Hash;
    type Addr: Eq + Hash;
    async fn when_to_run(&self, c: &Self::Comp) -> Result<i64, anyhow::Error>;
    async fn who_runs(&self, c: &Self::Comp) -> Result<Vec<Self::Addr>, anyhow::Error>;
    async fn message_comp(&self, m: &Self::Msg) -> Result<Option<Self::Comp>, anyhow::Error>;
    async fn run_comp(
        &self,
        c: Self::Comp,
        ms: HashSet<Self::Msg>,
    ) -> Result<Vec<Self::Msg>, anyhow::Error>;
}

enum ComputationState<M> {
    Done,
    NotMe,
    Inputs(HashSet<M>),
}

struct ComputeNode<G: ComputeGraph> {
    graph: G,
    addr: G::Addr,
    states: HashMap<G::Comp, ComputationState<G::Msg>>,
    by_time: BTreeMap<i64, HashSet<G::Comp>>,
}

impl<G: ComputeGraph> ComputeNode<G> {
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

    pub async fn receive_msg(&mut self, m: G::Msg) -> Result<(), anyhow::Error> {
        if let Some(c) = self.graph.message_comp(&m).await? {
            self.receive(c, std::iter::once(m).collect()).await?;
        }
        Ok(())
    }

    pub async fn receive_comp(&mut self, c: G::Comp) -> Result<(), anyhow::Error> {
        self.receive(c, std::iter::empty().collect()).await
    }

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
        stream::iter(comps.into_iter().map(Ok))
            .try_for_each_concurrent(None, |(c, m)| async move {
                let msgs = g.run_comp(c, m).await?;
                stream::iter(msgs.into_iter().map(Ok))
                    .try_for_each_concurrent(None, |m| async move {
                        if let Some(c) = g.message_comp(&m).await? {
                            let addrs = g.who_runs(&c).await?;
                            stream::iter(addrs.into_iter().map(Ok))
                                .try_for_each_concurrent(None, |a| send_msg(&c, &m, a))
                                .await?;
                        }

                        Ok(())
                    })
                    .await
            })
            .await
    }
}
