use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;

use futures_lite::{future, FutureExt};
use anyhow::{anyhow, bail};

use crate::account_construction::{add_action_to_account, children_paths_well_formed};
use crate::blockdata::{
    Action, DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody, QuorumNodeStats
};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode, verify_sig};
use crate::hashlookup::{HashLookup, HashPut, HashPutOfHashLookup};
use crate::hex_path::{bytes_to_path, HexPath};
use crate::queries::{is_prefix, longest_prefix_length, lookup_account, lookup_quorum_node, quorums_by_prev_block};

async fn verify_data_tree<HL: HashLookup>(
    hl: &HL,
    last_main: &MainBlock,
    account: HashCode,
    acct_node: &QuorumNodeBody,
) -> Result<(), anyhow::Error> {
    let action = hl
        .lookup(
            acct_node
                .new_action
                .ok_or(anyhow!("new account node must have an action"))?,
        )
        .await?;
    let mut hp = HashPutOfHashLookup::new(hl);
    let qnb_expected =
        add_action_to_account(&mut hp, last_main, account, &action, acct_node.prize).await?;
    if *acct_node != qnb_expected {
        bail!("account node is not the expected one");
    }
    Ok(())
}

async fn quorum_node_body_score<HL: HashLookup>(
    hl: &HL,
    last_main: &MainBlock,
    qnb: &QuorumNodeBody,
) -> Result<Option<u128>, anyhow::Error> {
    let opts = hl.lookup(last_main.block.body.options).await?;
    let pos = qnb.stats.fee;
    let neg = qnb.stats.prize + opts.gas_cost * qnb.stats.gas;
    if neg > pos {
        Ok(None)
    } else {
        Ok(Some(pos - neg))
    }
}

async fn verify_well_formed_quorum_node_body<HL: HashLookup>(
    hl: &HL,
    last_main: &MainBlock,
    qnb: &QuorumNodeBody,
) -> Result<(), anyhow::Error> {
    if qnb.last_main != Some(hash(&last_main)) {
        bail!("bad last_main for quorum node");
    }
    let depth = qnb.path.len();
    if depth > 64 {
        bail!("quorum node depth is too high");
    }
    if !children_paths_well_formed(&qnb.children) {
        bail!("quorum node children are malformed");
    }
    if depth > 0 && qnb.children.len() == 1 {
        bail!("non-root quorum node has only one child");
    }
    if depth == 64 {
        if qnb.children.len() != 0 {
            bail!("account node must have no children");
        }
        if qnb.data_tree.is_none() {
            bail!("account node must have a data tree");
        }
    } else {
        if qnb.children.len() == 0 {
            bail!("non-account quorum node must have children");
        }
        if qnb.data_tree.is_some() {
            bail!("non-account quorum node must have no data tree");
        }
        if qnb.new_action.is_some() {
            bail!("non-account quorum node must have no new action");
        }
    }
    if quorum_node_body_score(hl, last_main, qnb).await?.is_none() {
        bail!("quorum node has invalid score");
    }
    Ok(())
}

async fn verify_endorsed_quorum_node<HL: HashLookup>(
    hl: &HL,
    last_main: &MainBlock,
    node: &QuorumNode
) -> Result<(), anyhow::Error> {
    verify_well_formed_quorum_node_body(hl, last_main, &node.body).await?;
    match node.signatures {
        None => {
            if node.body.prize != 0 {
                bail!("node with no signatures must have no prize");
            }
            verify_valid_quorum_node_body(hl, last_main, &node.body).await?;
        }
        Some(sigs_hash) => {
            let sigs = hl.lookup(sigs_hash).await?;
            let mut acct_set = BTreeSet::<HashCode>::new();
            for sig in &sigs {
                if !verify_sig(&node.body, &sig) {
                    bail!("quorum node signature invalid");
                }
                acct_set.insert(hash(&sig.key).code);
            }
            if acct_set.len() != sigs.len() {
                bail!("duplicate signature keys");
            }
            let quorums = quorums_by_prev_block(hl, &last_main.block.body, node.body.path.clone()).await?;
            let mut satisfied = false;
            'outer: for (quorum, threshold) in quorums {
                if sigs.len() as u32 >= threshold {
                    for quorum_acct in quorum {
                        if !acct_set.contains(&quorum_acct) {
                            continue 'outer;
                        }
                    }
                    satisfied = true;
                    break;
                }
            }
            if !satisfied {
                bail!("no quorum is satisfied");
            }
        }
    }
    Ok(())
}

// TODO: consider replacing this with construction
fn verify_valid_quorum_node_body<'a, HL: HashLookup>(
    hl: &'a HL,
    last_main: &'a MainBlock,
    qnb: &'a QuorumNodeBody
) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + 'a>> {
    async move {
        verify_well_formed_quorum_node_body(hl, last_main, qnb).await?;
        if qnb.path.len() == 64 {
            verify_data_tree(hl, last_main, path_to_hash_code(qnb.path.clone()), qnb).await?;
        } else {
            match lookup_quorum_node(hl, &last_main.block.body, &qnb.path).await? {
                None => {}
                Some((prev_node, suffix)) => {
                    'outer: for (prev_child_suffix, _) in &prev_node.body.children {
                        if is_prefix(&suffix, prev_child_suffix) {
                            for (new_child_suffix, _) in &qnb.children {
                                if is_prefix(new_child_suffix, &suffix[prev_child_suffix.len()..]) {
                                    continue 'outer;
                                }
                            }
                            bail!("new node drops a child present in old node");
                        }
                    }
                }
            }
            let mut expected_stats = QuorumNodeStats::zero();
            for (child_suffix, child_hash) in &qnb.children {
                let child = hl.lookup(*child_hash).await?;
                if child.body.path != [&qnb.path[..], &child_suffix[..]].concat() {
                    bail!("child path is not correct based on parent path and suffix");
                }
                if Some((child.clone(), vec![])) == lookup_quorum_node(hl, &last_main.block.body, &child.body.path).await? {
                    expected_stats.stake += child.body.stats.stake;
                } else {
                    verify_endorsed_quorum_node(hl, last_main, &child).await?;
                    expected_stats = expected_stats.plus(&child.body.stats);
                }
            }
            expected_stats.new_nodes += 1;
            expected_stats.prize += qnb.prize;
            if qnb.stats != expected_stats {
                bail!("tree info is not expected based on child tree infos");
            }
        }
        Ok(())
    }.boxed()
}

