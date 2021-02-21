use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;

use anyhow::{anyhow, bail};
use futures_lite::{future, FutureExt};

use crate::account_construction::{add_action_to_account, children_paths_well_formed};
use crate::blockdata::{
    Action, DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody,
    QuorumNodeStats,
};
use crate::crypto::{hash, path_to_hash_code, verify_sig, Hash, HashCode};
use crate::hashlookup::{HashLookup, HashPut, HashPutOfHashLookup};
use crate::hex_path::{bytes_to_path, is_prefix, HexPath};
use crate::queries::{
    longest_prefix_length, lookup_account, lookup_quorum_node, quorums_by_prev_block,
};

async fn add_child_to_quorum_node<HL: HashLookup + HashPut>(
    hl: &mut HL,
    last_main: Option<Hash<MainBlock>>,
    parent: QuorumNodeBody,
    child_hash: Hash<QuorumNode>,
) -> Result<QuorumNodeBody, anyhow::Error> {
    bail!("not implemented");
    // let child = hl.lookup(child_hash).await?;
    // if !is_prefix(parent.path, child.body.path) {
    //     bail!("child path must be an extension of parent path");
    // }
    // match last_main {
    //     None => {}
    //     Some(main_hash) => {
    //         verify_endorsed_quorum_node(hl, &hl.lookup(main_hash).await?, &child).await?;
    //     }
    // }
    // let suffix = &child.body.path[parent.path.len()..];
    // if suffix.len() == 0 {
    //     return child.body;
    // }
    // let mut to_insert = (suffix, child_hash);
    // for (c_suffix, c_hash) in &parent.children {
    //     let c = hl.lookup(c.hash).await?;
    //     if c_suffix[0] == suffix[0] {
    //         let prev_child = hl.lookup(c_hash).await?;
    //         if is_prefix(c_suffix, &suffix) {
    //             let c2 = add_child_to_quorum_node(hl, last_main, c.body, child_hash);
    //             to_insert = (c_suffix, hl.put(&QuorumNode {body = c, signatures = None});
    //         } else {
    //         }

    //     }
    // }
}

pub fn new_quorum_node_body<HL: HashLookup + HashPut>(
    hl: &mut HL,
    last_main: &MainBlock,
    path: HexPath,
    possible_children: &Vec<Hash<QuorumNode>>,
) -> Result<Option<QuorumNodeBody>, anyhow::Error> {
    bail!("not implemented");
}
