#![allow(warnings)]

use std::collections::BTreeMap;

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
use crate::verification::{quorum_node_body_score, verify_endorsed_quorum_node};

async fn add_child_to_quorum_node<HL: HashLookup + HashPut>(
    hl: &mut HL,
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

async fn make_immediate_parent<HL: HashLookup + HashPut>(
    hl: &mut HL,
    last_main: &MainBlock,
    path: &HexPath,
    children: Vec<(QuorumNode, usize)>,
) -> Result<(QuorumNode, usize), anyhow::Error> {
    let mut parent = match lookup_quorum_node(hl, &last_main.block.body, &path).await? {
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
    let mut tot_score = 0;
    for (child, score) in children {
        tot_score += score;
        let child_hash = hl.put(&child).await?;
        let parent_hash = hl.put(&parent).await?;
        let new_parent_hash = add_child_to_quorum_node(hl, child_hash, parent_hash).await?;
        parent = hl.lookup(new_parent_hash).await?;
    }
    Ok((parent, tot_score))
}

// TODO jack fixup
async fn best_super_node<HL: HashLookup + HashPut>(
    hl: &mut HL,
    last_main: &MainBlock,
    super_path: HexPath,
    input_children: Vec<(QuorumNode, usize)>,
) -> Result<QuorumNode, anyhow::Error> {
    let mut best = BTreeMap::<HexPath, (QuorumNode, usize)>::new();
    for i in (super_path.len()..(64 + 1)).rev() {
        let mut candidates = Vec::<(QuorumNode, usize)>::new();
        for (child, score) in &input_children {
            assert!(is_prefix(&super_path, &child.body.path));
            if child.body.path.len() == i {
                candidates.push((child.clone(), *score));
            }
        }
        let mut i_path_map = BTreeMap::<HexPath, Vec<(QuorumNode, usize)>>::new();
        for (child, score) in best.values() {
            let i_path = child.body.path[0..i].to_vec();
            if !i_path_map.contains_key(&i_path) {
                i_path_map.insert(i_path.to_vec(), Vec::new());
            }
            i_path_map
                .get_mut(&i_path)
                .unwrap()
                .push((child.clone(), *score));
        }
        for (i_path, chs) in i_path_map {
            candidates.push(make_immediate_parent(hl, last_main, &i_path, chs).await?);
        }
        for (child, score) in candidates {
            if best
                .get(&child.body.path)
                .map_or_else(|| true, |x| x.1 < score)
            {
                best.insert(child.body.path.clone(), (child, score));
            }
        }
    }
    Ok((best.get(&super_path).unwrap().0.clone()))
}
