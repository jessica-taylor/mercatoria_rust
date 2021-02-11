use crate::blockdata::{DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::HashLookup;
use crate::hex_path::{bytes_to_path, HexPath};

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

fn rh_follow_path<
    HL: HashLookup,
    N: DeserializeOwned + Clone,
    GC: Fn(&N) -> &Vec<(HexPath, Hash<N>)>,
>(
    hl: &HL,
    get_children: GC,
    init_node: N,
    path: HexPath,
) -> Result<(N, HexPath), String> {
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
                    node = hl.lookup(*child_hash).unwrap();
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

pub fn quorum_node_follow_path<HL: HashLookup>(
    hl: &HL,
    node: &QuorumNode,
    path: HexPath,
) -> Result<(QuorumNode, HexPath), String> {
    rh_follow_path(hl, |qn| &qn.body.children, node.clone(), path)
}

pub fn lookup_quorum_node<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
    path: HexPath,
) -> Result<(QuorumNode, HexPath), String> {
    quorum_node_follow_path(hl, &hl.lookup(main.tree)?, path)
}

pub fn lookup_account<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
    acct: HashCode,
) -> Result<QuorumNode, String> {
    let (qn, postfix) = lookup_quorum_node(hl, main, bytes_to_path(&acct))?;
    if postfix.len() != 0 {
        return Err("account not found".to_string());
    }
    Ok(qn)
}

pub fn data_node_follow_path<HL: HashLookup>(
    hl: &HL,
    node: &DataNode,
    path: HexPath,
) -> Result<(DataNode, HexPath), String> {
    rh_follow_path(hl, |dn| &dn.children, node.clone(), path)
}

pub fn lookup_data_in_account<HL: HashLookup>(
    hl: &HL,
    qn: &QuorumNode,
    path: HexPath,
) -> Result<Vec<u8>, String> {
    let top_dn = hl.lookup(
        qn.body
            .data_tree
            .ok_or("no data tree".to_string())?,
    )?;
    let (dn, postfix) = data_node_follow_path(hl, &top_dn, path)?;
    if postfix.len() != 0 {
        return Err("data not found".to_string());
    }
    dn.value.ok_or("data not found".to_string())
}

pub fn block_with_version<HL : HashLookup>(
    hl: &HL,
    mb: &MainBlockBody,
    version: u64
) -> Result<MainBlockBody, String> {
    let v = mb.version;
    if version > v {
        return Err("version higher than given main block version".to_string());
    }
    if version == v {
        return Ok(mb.clone());
    }
    match &mb.prev {
        None => Err("tried to get version before the first block".to_string()),
        Some(hash) => block_with_version(hl, &hl.lookup(hash.clone())?.block.body, version)
    }
}

/// TODO real randomness
pub fn random_seed_of_block<HL : HashLookup>(
    hl: &HL,
    main: &MainBlockBody
) -> Result<HashCode, String> {
    let period = hl.lookup(main.options)?.random_seed_period;
    let version_to_get = main.version / u64::from(period) * u64::from(period);
    Ok(hash(&block_with_version(hl, main, version_to_get)?).code)
}


pub fn stake_indexed_account<HL: HashLookup>(
    hl: &HL,
    qn: &QuorumNode,
    stake_ix: u128,
) -> Result<HashCode, String> {
    if stake_ix >= qn.body.total_stake {
        return Err("index exceeds total stake".to_string());
    }
    let path = qn.body.path.clone();
    if path.len() == 64 {
        return Ok(path_to_hash_code(path));
    }
    let mut children = Vec::new();
    for (_, child_hash) in qn.body.children.clone() {
        children.push(hl.lookup(child_hash)?);
    }
    let child_stakes: Vec<u128> = children.iter().map(|ch| ch.body.total_stake).collect();
    let mut sum_so_far = 0;
    for i in 0..children.len() {
        if stake_ix < sum_so_far + child_stakes[i] {
            return stake_indexed_account(hl, qn, stake_ix - sum_so_far);
        }
        sum_so_far += child_stakes[i];
    }
    panic!("total stake does not equal sum of child node total stakes!")
}

// pub fn random_account<HL : HashLookup>(
//     hl: &HL,
//     main: &MainBlockBody,
//     seed: HashCode,
//     rand_id: String
// ) -> Result<HashCode, String> {
//     let rand_period = hl.lookup(main.options)?.random_seed_period;
//     let mut rounded = main.version / u64::from(rand_period) * u64::from(rand_period);
//     if rounded > 0 {
//         rounded = rounded - rand_period;
//     }
//     let stake_main = block_with_version(hl, main, rounded)?;
//     let top = hl.lookup(stake_main.tree);
//     if top.body.total_stake == 0 {
//         Err("can't select random account when there is no stake")
//     }
// 
//     stake_indexed_account(hl, top, rand % top.body.total_stake)
// }

