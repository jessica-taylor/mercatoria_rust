
use std::collections::BTreeMap;

use anyhow::{bail, anyhow};

use crate::blockdata::{DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody, Action};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::{HashLookup, HashPut, HashPutOfHashLookup};
use crate::hex_path::{bytes_to_path, HexPath};
use crate::queries::{is_prefix, longest_prefix_length, lookup_account};
use crate::account_construction::{children_paths_well_formed, add_action_to_account};

fn verify_data_tree<HL: HashLookup>(
    hl: &HL,
    last_main: &MainBlock,
    account: HashCode,
    acct_node: &QuorumNodeBody
) -> Result<(), anyhow::Error> {
    let action = hl.lookup(acct_node.new_action.ok_or(anyhow!("new account node must have an action"))?)?;
    let mut hp = HashPutOfHashLookup::new(hl);
    let qnb_expected = add_action_to_account(&mut hp, last_main, account, &action, acct_node.prize)?;
    if *acct_node != qnb_expected {
        bail!("account node is not the expected one");
    }
    Ok(())
}

fn quorum_node_body_score<HL: HashLookup>(
    hl: &HL,
    last_main: &MainBlock,
    qnb: &QuorumNodeBody
) -> Result<Option<u128>, anyhow::Error> {
    let opts = hl.lookup(last_main.block.body.options)?;
    let pos = qnb.total_fee;
    let neg = qnb.total_prize + opts.gas_cost * qnb.total_gas;
    if neg > pos {
        Ok(None)
    } else {
        Ok(Some(pos - neg))
    }
}

fn verify_well_formed_quorum_node_body<HL: HashLookup>(
    hl: &HL,
    last_main: &MainBlock,
    qnb: &QuorumNodeBody
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
    if quorum_node_body_score(hl, last_main, qnb)?.is_none() {
        bail!("quorum node has invalid score");
    }
    Ok(())
}
