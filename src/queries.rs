
use crate::hex_path::{HexPath, bytes_to_path};
use crate::blockdata::{MainBlock, PreSignedMainBlock, MainBlockBody, QuorumNode, DataNode};
use crate::crypto::{Hash, HashCode};
use crate::hashlookup::HashLookup;

use serde::de::DeserializeOwned;

fn is_prefix<T: Eq>(pre: &Vec<T>, full: &Vec<T>) -> bool {
    if pre.len() > full.len() {
        return false;
    }
    for i in 0..pre.len() {
        if pre[i] != full[i] {
            return false;
        }
    }
    true
}

fn rh_follow_path<HL : HashLookup, N : DeserializeOwned + Clone, GC: Fn(&N) -> &Vec<(HexPath, Hash<N>)>>(
    hl: &HL, get_children: GC, init_node: N, path: HexPath) -> Result<(N, HexPath), String> {

    let mut path_ix = 0;
    let mut prefix = HexPath::new();
    let mut node = init_node;
    while path_ix < path.len() {
        prefix.push(path[path_ix]);
        path_ix += 1;
        let mut found = false;
        for (postfix, child_hash) in get_children(&node) {
            if is_prefix(&prefix, &postfix) {
                found = true;
                if prefix == *postfix {
                    node = hl.lookup((*child_hash).clone()).unwrap();
                    prefix = HexPath::new();
                    break;
                }
            }
        }
        if !found {
            return Err("RH node not found".to_string());
        }
    }
    Ok((node, prefix))
}

pub fn quorum_node_follow_path<HL: HashLookup>(hl: &HL, node: &QuorumNode, path: HexPath) -> Result<(QuorumNode, HexPath), String> {
    rh_follow_path(hl, |qn| &qn.body.children, node.clone(), path)
}

pub fn lookup_quorum_node<HL : HashLookup>(hl: &HL, main: &MainBlockBody, path: HexPath) -> Result<(QuorumNode, HexPath), String> {
    quorum_node_follow_path(hl, &hl.lookup(main.tree.clone())?, path)
}

pub fn lookup_account<HL : HashLookup>(hl: &HL, main: &MainBlockBody, acct: HashCode) -> Result<QuorumNode, String> {
    let (qn, postfix) = lookup_quorum_node(hl, main, bytes_to_path(&acct))?;
    if postfix.len() != 0 {
        return Err("account not found".to_string());
    }
    Ok(qn)
}

pub fn data_node_follow_path<HL : HashLookup>(hl: &HL, node: &DataNode, path: HexPath) -> Result<(DataNode, HexPath), String> {
    rh_follow_path(hl, |dn| &dn.children, node.clone(), path)
}
