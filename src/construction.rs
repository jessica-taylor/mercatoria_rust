use std::collections::BTreeMap;

use anyhow::bail;
use ed25519_dalek::PublicKey;
use serde::{Deserialize, Serialize};

use crate::account_construction::{initialize_account_node, insert_into_rh_tree};
use crate::blockdata::{
    Action, DataNode, MainBlock, MainBlockBody, MainOptions, PreSignedMainBlock, QuorumNode,
    QuorumNodeBody, QuorumNodeStats, RadixChildren,
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
    Ok(best.get(&super_path).unwrap().0.clone())
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct AccountInit {
    pub public_key: PublicKey,
    pub balance: u128,
    pub stake: u128,
}

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
                path: vec![],
                children: RadixChildren::default(),
                data_tree: None,
                new_action: None,
                prize: 0,
                stats,
            },
            signatures: None,
        })
        .await?;
    for init in account_inits {
        let (_, acct_node_body) =
            initialize_account_node(hl, None, init.public_key, init.balance, init.stake).await?;
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
        options: hl.put(&opts).await?,
    })
}

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
