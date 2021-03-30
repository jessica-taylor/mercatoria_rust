use std::collections::BTreeMap;

use anyhow::bail;

use crate::account_transform::{field_balance, field_received, field_stake};
use crate::blockdata::{
    Action, DataNode, MainBlock, MainBlockBody, MainOptions, PreSignedMainBlock, QuorumNode,
    QuorumNodeBody, QuorumNodeStats, RadixChildren, SendInfo,
};
use crate::crypto::{hash, path_to_hash_code, verify_sig, Hash, HashCode};
use crate::hashlookup::{HashLookup, HashPut};
use crate::hex_path::{bytes_to_path, is_prefix, HexPath};
use crate::queries::{
    longest_prefix_length, lookup_account, lookup_quorum_node, quorums_by_prev_block,
};

pub struct AccountState {
    pub fields: BTreeMap<HexPath, Vec<u8>>,
}

impl AccountState {
    pub fn empty() -> AccountState {
        AccountState {
            fields: BTreeMap::new(),
        }
    }

    pub fn sends(&self) -> Vec<SendInfo> {
        let mut res = Vec::new();
        for (path, value) in &self.fields {
            if path.len() >= 4 && path[0..4].to_vec() == bytes_to_path(b"send") {
                res.push(rmp_serde::from_read::<_, SendInfo>(value.as_slice()).unwrap());
            }
        }
        res
    }

    pub fn has_received(&self, send: &SendInfo) -> bool {
        match self.fields.get(&field_received(hash(send)).path) {
            None => false,
            Some(value) => rmp_serde::from_read::<_, bool>(value.as_slice()).unwrap(),
        }
    }

    pub fn balance(&self) -> u128 {
        rmp_serde::from_read::<_, u128>(self.fields.get(&field_balance().path).unwrap().as_slice())
            .unwrap()
    }
}

async fn add_data_tree_to_account_state<HL: HashLookup>(
    hl: &HL,
    path: HexPath,
    node: Hash<DataNode>,
    state: &mut AccountState,
) -> Result<(), anyhow::Error> {
    let node = hl.lookup(node).await?;
    match node.field {
        None => {}
        Some(value) => {
            state.fields.insert(path.clone(), value);
        }
    }
    for (suffix, child) in node.children.iter_entries() {
        let child_path = vec![path.clone(), suffix.clone()].concat();
        add_data_tree_to_account_state(hl, child_path, *child, state);
    }
    Ok(())
}

pub async fn get_account_state<HL: HashLookup>(
    hl: &HL,
    node: Hash<DataNode>,
) -> Result<AccountState, anyhow::Error> {
    let mut state = AccountState::empty();
    add_data_tree_to_account_state(hl, vec![], node, &mut state).await?;
    Ok(state)
}

pub struct MainState {
    pub accounts: BTreeMap<HashCode, AccountState>,
}
