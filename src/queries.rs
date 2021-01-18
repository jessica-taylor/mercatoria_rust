
use crate::blockdata::{MainBlock, PreSignedMainBlock, MainBlockBody, QuorumNode, DataNode};
use crate::crypto::{Hash, HashCode};
use crate::hashlookup::HashLookup;

use serde::de::DeserializeOwned;

enum RHFollowPathResult<N> {
    Found(N, Vec<u8>),
    NotFound(N, Vec<u8>),
}

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

fn rh_follow_path<HL : HashLookup, N : DeserializeOwned + Clone, GC: Fn(&N) -> &Vec<(Vec<u8>, Hash<N>)>>(
    hl: &HL, get_children: GC, init_node: N, path: Vec<u8>) -> Result<(N, Vec<u8>), String> {

    let mut path_ix = 0;
    let mut prefix = Vec::<u8>::new();
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
                    prefix = Vec::new();
                    break;
                }
            }
        }
        if !found {
            return Err("RH node not found".to_string());
        }
    }
    return Ok((node, prefix));
}

pub fn quorum_node_follow_path<HL: HashLookup>(hl: &HL, node: &QuorumNode, path: Vec<u8>) -> Result<(QuorumNode, Vec<u8>), String> {
    rh_follow_path(hl, |qn| &qn.body.children, node.clone(), path)
}

pub fn lookup_quorum_node<HL : HashLookup>(hl: &HL, main: &MainBlockBody, path: Vec<u8>) -> Result<(QuorumNode, Vec<u8>), String> {
    quorum_node_follow_path(hl, &hl.lookup(main.tree.clone())?, path)
}
