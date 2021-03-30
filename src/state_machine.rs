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

struct AccountState {
    fields: BTreeMap<HexPath, Vec<u8>>,
}

impl AccountState {
    fn sends(&self) -> Vec<SendInfo> {
        let mut res = Vec::new();
        for (path, value) in &self.fields {
            if path.len() >= 4 && path[0..4].to_vec() == bytes_to_path(b"send") {
                res.push(rmp_serde::from_read(value.as_slice()).unwrap());
            }
        }
        res
    }
    fn has_received(&self, sender: HashCode, send: &SendInfo) -> bool {
        match self.fields.get(&field_received(hash(send)).path) {
            None => false,
            Some(value) => rmp_serde::from_read::<_, bool>(value.as_slice()).unwrap(),
        }
    }
}
