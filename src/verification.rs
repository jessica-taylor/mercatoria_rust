
use std::collections::BTreeMap;

use anyhow::{bail, anyhow};

use crate::blockdata::{DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody, Action};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::{HashLookup, HashPut, HashPutOfHashLookup};
use crate::hex_path::{bytes_to_path, HexPath};
use crate::queries::{is_prefix, longest_prefix_length, lookup_account};
use crate::account_construction::add_action_to_account;

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
