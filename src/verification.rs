use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;

use anyhow::{anyhow, bail};
use futures_lite::{future, FutureExt};
use serde::Serialize;

use crate::account_construction::{add_action_to_account, children_paths_well_formed};
use crate::blockdata::{
    Action, DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody,
    QuorumNodeStats, RadixHashNode,
};
use crate::crypto::{hash, path_to_hash_code, verify_sig, Hash, HashCode, Signature};
use crate::hashlookup::{HashLookup, HashPut, HashPutOfHashLookup};
use crate::hex_path::{bytes_to_path, is_prefix, HexPath};
use crate::queries::{
    longest_prefix_length, lookup_account, lookup_quorum_node, miner_and_signers_by_prev_block,
    quorums_by_prev_block,
};

/// A score for a `QuorumNodeBody` represented its fee minus its total cost (prize and gas).
pub async fn quorum_node_body_score<HL: HashLookup>(
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

/// Verifies that a `QuorumNodyBody` satisfies some well-formedness conditions.
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

fn signatures_to_signers<T: Serialize>(
    sigs: &Vec<Signature<T>>,
    signed: &T,
) -> Result<BTreeSet<HashCode>, anyhow::Error> {
    let mut signers = BTreeSet::<HashCode>::new();
    for sig in sigs {
        if !verify_sig(signed, &sig) {
            bail!("signature invalid");
        }
        signers.insert(hash(&sig.key).code);
    }
    if signers.len() != sigs.len() {
        bail!("duplicate signature keys");
    }
    Ok(signers)
}

/// Verifies that a quorum node is endorsed (i.e. either has enough
/// signatures or has no signatures but is valid).
pub async fn verify_endorsed_quorum_node<HL: HashLookup>(
    hl: &HL,
    last_main: &MainBlock,
    node: &QuorumNode,
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
            let signers = signatures_to_signers(&sigs, &node.body)?;
            let quorums =
                quorums_by_prev_block(hl, &last_main.block.body, node.body.path.clone()).await?;
            let mut satisfied = false;
            'outer: for (quorum, threshold) in quorums {
                if sigs.len() as u32 >= threshold {
                    // did all quorum members sign?
                    for quorum_acct in quorum {
                        if !signers.contains(&quorum_acct) {
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

/// Verifies that a quorum node is valid, in the sense that it
/// follows correctly from the old node and the new children.
fn verify_valid_quorum_node_body<'a, HL: HashLookup>(
    hl: &'a HL,
    last_main: &'a MainBlock,
    qnb: &'a QuorumNodeBody,
) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + 'a>> {
    async move {
        verify_well_formed_quorum_node_body(hl, last_main, qnb).await?;
        if qnb.path.len() == 64 {
            // run the action to produce the expected new node
            let account = path_to_hash_code(qnb.path.clone());
            let action = hl
                .lookup(
                    qnb.new_action
                        .ok_or(anyhow!("new account node must have an action"))?,
                )
                .await?;
            let mut hp = HashPutOfHashLookup::new(hl);
            let qnb_expected =
                add_action_to_account(&mut hp, last_main, account, &action, qnb.prize).await?;
            if *qnb != qnb_expected {
                bail!("account node is not the expected one");
            }
        } else {
            match lookup_quorum_node(hl, &last_main.block.body, &qnb.path).await? {
                None => {}
                Some((prev_node, suffix)) => {
                    // check that all old children are present
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
            // check that new children are endorsed
            for (child_suffix, child_hash) in &qnb.children {
                let child = hl.lookup(*child_hash).await?;
                if Some((child.clone(), vec![]))
                    != lookup_quorum_node(hl, &last_main.block.body, &child.body.path).await?
                {
                    verify_endorsed_quorum_node(hl, last_main, &child).await?;
                }
            }
            // check child paths and stats
            let qn = QuorumNode {
                body: qnb.clone(),
                signatures: None,
            };
            if qn
                != qn
                    .clone()
                    .replace_children(hl, qnb.children.clone())
                    .await?
            {
                bail!("quorum node is not expected based on its children");
            }
        }
        Ok(())
    }
    .boxed()
}

/// Verifies that a `MainBlockBody` is well-formed.
async fn verify_well_formed_main_block_body<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
) -> Result<(), anyhow::Error> {
    let opts = hl.lookup(main.options).await?;
    if opts.timestamp_period_ms == 0 || main.timestamp_ms % (opts.timestamp_period_ms as i64) != 0 {
        bail!("main must have timestamp that is 0 mod timestamp_period_ms");
    }
    match main.prev {
        None => {
            if main.version != 0 {
                bail!("genesis block must have version 0");
            }
        }
        Some(prev_hash) => {
            let prev = hl.lookup(prev_hash).await?;
            if main.version != prev.block.body.version + 1 {
                bail!("main must advance version by 1");
            }
            if main.timestamp_ms <= prev.block.body.timestamp_ms {
                bail!("main must advance timestamp");
            }
        }
    }
    Ok(())
}

/// Verifies that a `MainBlockBody` is valid.
pub async fn verify_valid_main_block_body<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
) -> Result<(), anyhow::Error> {
    verify_well_formed_main_block_body(hl, main).await?;
    let top = hl.lookup(main.tree).await?;
    if top.body.path.len() != 0 {
        bail!("top quorum node must have empty path");
    }
    match main.prev {
        None => Ok(()),
        Some(prev_hash) => {
            let prev = hl.lookup(prev_hash).await?;
            if main.options != prev.block.body.options {
                bail!("options must not change");
            }
            if main.tree != prev.block.body.tree {
                verify_endorsed_quorum_node(hl, &prev, &top).await?;
            }
            Ok(())
        }
    }
}

/// Verifies that a `PreSignedMainBlock` is endorsed, i.e. is well-formed and contains enough valid
/// signatures.
pub async fn verify_endorsed_pre_signed_main_block<HL: HashLookup>(
    hl: &HL,
    main: &PreSignedMainBlock,
) -> Result<(), anyhow::Error> {
    match main.body.prev {
        None => bail!("genesis block is never endorsed"),
        Some(prev_hash) => {
            let prev = hl.lookup(prev_hash).await?;
            verify_well_formed_main_block_body(hl, &main.body).await?;
            let mut signers = signatures_to_signers(&main.signatures, &main.body)?;
            let (_miner, needed_signers) = miner_and_signers_by_prev_block(hl, &prev).await?;
            let mut count = 0;
            for signer in needed_signers {
                if signers.contains(&signer) {
                    count += 1;
                }
            }
            let opts = hl.lookup(main.body.options).await?;
            if count < opts.main_block_signatures_required {
                bail!("not enough main signatures");
            }
            Ok(())
        }
    }
}

/// Verifies that a `MainBlock` is endorsed, i.e. is well-formed and contains enough valid
/// signatures.
pub async fn verify_endorsed_main_block<HL: HashLookup>(
    hl: &HL,
    main: &MainBlock,
) -> Result<(), anyhow::Error> {
    match main.block.body.prev {
        None => bail!("genesis block is never endorsed"),
        Some(prev_hash) => {
            let prev = hl.lookup(prev_hash).await?;
            verify_endorsed_pre_signed_main_block(hl, &main.block);
            let mut signers = signatures_to_signers(&vec![main.signature.clone()], &main.block)?;
            let (miner, _signers) = miner_and_signers_by_prev_block(hl, &prev).await?;
            if !signers.contains(&miner) {
                bail!("main must be signed by miner");
            }
            Ok(())
        }
    }
}

/// Verifies that a `MainBlock` is valid and endorsed.
pub async fn verify_valid_endorsed_main_block<HL: HashLookup>(
    hl: &HL,
    main: &MainBlock,
) -> Result<(), anyhow::Error> {
    verify_valid_main_block_body(hl, &main.block.body).await?;
    verify_endorsed_main_block(hl, main).await?;
    Ok(())
}
