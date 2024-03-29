//! Functionality for creating and modifying the data structures of the blockchain.
use std::collections::BTreeMap;

use anyhow::bail;

use crate::account_construction::{initialize_account_node, insert_into_rh_tree};
use crate::blockdata::{
    AccountInit, MainBlock, MainBlockBody, MainOptions, QuorumNode, QuorumNodeBody,
    QuorumNodeStats, RadixChildren,
};
use crate::crypto::Hash;
use crate::hashlookup::{HashLookup, HashPut};
use crate::hex_path::{is_prefix, HexPath};
use crate::queries::lookup_quorum_node;

use crate::verification::verify_endorsed_quorum_node;

/// Adds a descendent to a quorum node.  It does not have to be an
/// immediate child.  It replaces any old node at that path.
async fn add_child_to_quorum_node<HL: HashLookup + HashPut>(
    hl: &mut HL,
    child_hash: Hash<QuorumNode>,
    parent_hash: Hash<QuorumNode>,
) -> Result<Hash<QuorumNode>, anyhow::Error> {
    let parent = hl.lookup(parent_hash).await?;
    let child = hl.lookup(child_hash).await?;
    if !is_prefix(&parent.body.path[..], &child.body.path[..]) {
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
    // need to use child path relative parent since we consider the subtree rooted at parent
    let relative_path = &child.body.path[parent.body.path.len()..];
    insert_into_rh_tree(
        hl,
        &mut node_count,
        relative_path,
        replace,
        parent_hash,
    )
    .await
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
/// Finds the best parent node of a set of children.
pub async fn best_super_node<HL: HashLookup + HashPut>(
    hl: &mut HL,
    last_main: &MainBlock,
    super_path: HexPath,
    input_children: Vec<(QuorumNode, usize)>,
) -> Result<QuorumNode, anyhow::Error> {
    let mut best = BTreeMap::<HexPath, (QuorumNode, usize)>::new();
    for i in (super_path.len()..(64 + 1)).rev() {
        let mut candidates = Vec::<(QuorumNode, usize)>::new();
        for (child, score) in &input_children {
            assert!(is_prefix(&super_path[..], &child.body.path[..]));
            if child.body.path.len() == i {
                candidates.push((child.clone(), *score));
            }
        }
        let mut i_path_map = BTreeMap::<HexPath, Vec<(QuorumNode, usize)>>::new();
        for (child, score) in best.values() {
            let i_path = child.body.path[0..i].to_vec();
            if !i_path_map.contains_key(&HexPath(i_path[..].to_vec())) {
                i_path_map.insert(HexPath(i_path.to_vec()), Vec::new());
            }
            i_path_map
                .get_mut(&HexPath(i_path))
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
    Ok(best.get(&super_path).unwrap().0.clone())
}

/// Constructs the body of the genesis block.
pub async fn genesis_block_body<HL: HashLookup + HashPut>(
    hl: &mut HL,
    account_inits: &Vec<AccountInit>,
    timestamp_ms: i64,
    opts: MainOptions,
) -> Result<MainBlockBody, anyhow::Error> {
    let mut stats = QuorumNodeStats::zero();
    stats.new_nodes += 1;
    let mut top = hl
        .put(&QuorumNode {
            body: QuorumNodeBody {
                last_main: None,
                path: HexPath(vec![]),
                children: RadixChildren::default(),
                data_tree: None,
                new_action: None,
                prize: 0,
                stats,
            },
            signatures: None,
        })
        .await?;
    let opts_hash = hl.put(&opts).await?;
    for init in account_inits {
        let (_, acct_node_body) = initialize_account_node(hl, None, init).await?;
        let acct_node = hl
            .put(&QuorumNode {
                body: acct_node_body,
                signatures: None,
            })
            .await?;
        top = add_child_to_quorum_node(hl, acct_node, top).await?;
    }
    Ok(MainBlockBody {
        prev: None,
        version: 0,
        timestamp_ms,
        tree: top,
        options: opts_hash,
    })
}

/// Creates the body of the next main block given an already-constructed quorum tree.
pub async fn next_main_block_body<HL: HashLookup>(
    hl: &HL,
    timestamp_ms: i64,
    prev_hash: Hash<MainBlock>,
    top_hash: Hash<QuorumNode>,
) -> Result<MainBlockBody, anyhow::Error> {
    let prev = hl.lookup(prev_hash).await?;
    let opts = hl.lookup(prev.block.body.options).await?;
    if timestamp_ms % (opts.timestamp_period_ms as i64) != 0
        || timestamp_ms <= prev.block.body.timestamp_ms
    {
        bail!("invalid timestamp in next_main_block_body");
    }
    let top = hl.lookup(top_hash).await?;
    if top.body.path.len() != 0 {
        bail!("next_main_block_body must be called with root QuorumNode");
    }
    if top_hash != prev.block.body.tree {
        verify_endorsed_quorum_node(hl, &prev, &top).await?;
    }
    Ok(MainBlockBody {
        prev: Some(prev_hash),
        version: prev.block.body.version + 1,
        timestamp_ms,
        tree: top_hash,
        options: prev.block.body.options,
    })
}
