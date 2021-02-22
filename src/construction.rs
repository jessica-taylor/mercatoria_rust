#![allow(warnings)]

use anyhow::bail;

use crate::account_construction::insert_into_rh_tree;
use crate::blockdata::{
    Action, DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody,
    QuorumNodeStats, RadixChildren,
};
use crate::crypto::{hash, path_to_hash_code, verify_sig, Hash, HashCode};
use crate::hashlookup::{HashLookup, HashPut};
use crate::hex_path::{is_prefix, HexPath};
use crate::queries::{
    longest_prefix_length, lookup_account, lookup_quorum_node, quorums_by_prev_block,
};
use crate::verification::verify_endorsed_quorum_node;

async fn add_child_to_quorum_node<HL: HashLookup + HashPut>(
    hl: &mut HL,
    _last_main: Option<Hash<MainBlock>>,
    child_hash: Hash<QuorumNode>,
    parent_hash: Hash<QuorumNode>,
) -> Result<Hash<QuorumNode>, anyhow::Error> {
    let parent = hl.lookup(parent_hash).await?;
    let child = hl.lookup(child_hash).await?;
    if !is_prefix(&parent.body.path, &child.body.path) {
        bail!("child path must extend parent path");
    }
    let mut node_count = 0;
    let replace = |option_node: Option<QuorumNode>| match option_node {
        None => Ok(child.clone()),
        Some(node) => {
            if node.body.path != child.body.path {
                bail!("path of old node must match new child");
            }
            Ok(child.clone())
        }
    };
    insert_into_rh_tree(hl, &mut node_count, &child.body.path, replace, parent_hash).await
}

pub async fn new_quorum_node_body<HL: HashLookup + HashPut>(
    hl: &mut HL,
    last_main: &MainBlock,
    path: HexPath,
    possible_children: &Vec<Hash<QuorumNode>>,
) -> Result<Option<QuorumNodeBody>, anyhow::Error> {
    let _initial = match lookup_quorum_node(hl, &last_main.block.body, &path).await? {
        Some((old_node, suffix)) if suffix.is_empty() => old_node,
        _ => QuorumNode {
            signatures: None,
            body: QuorumNodeBody {
                last_main: None,
                path: path.clone(),
                children: RadixChildren::default(),
                data_tree: None,
                new_action: None,
                prize: 0,
                stats: QuorumNodeStats::zero(),
            },
        },
    };
    let mut ver_children = Vec::new();
    for child_hash in possible_children {
        let child = hl.lookup(*child_hash).await?;
        if is_prefix(&path, &child.body.path) {
            match verify_endorsed_quorum_node(hl, last_main, &child).await {
                Ok(()) => {
                    ver_children.push(child);
                }
                Err(_) => {}
            }
        }
    }
    // TODO: finish
    bail!("not implemented");
}
