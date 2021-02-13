
use std::collections::BTreeMap;

use anyhow::anyhow;

use crate::blockdata::{DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody, Action};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::{HashLookup, HashPut};
use crate::hex_path::{bytes_to_path, HexPath};
use crate::queries::{is_prefix, longest_prefix_length, lookup_account};

fn verify_data_tree<HL: HashLookup>(
    hl: &HL,
    main: &MainBlock,
    account: HashCode,
    acct_node: &QuorumNodeBody
) -> Result<(), anyhow::Error> {
    let action = hl.lookup(acct_node.new_action.ok_or(anyhow!("new account node must have an action"))?)?;
    Ok(())

}
