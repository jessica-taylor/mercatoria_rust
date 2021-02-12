use std::cmp::min;

use crate::blockdata::{DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::HashLookup;
use crate::hex_path::{bytes_to_path, HexPath};

use serde::de::DeserializeOwned;
use anyhow::{anyhow, bail};

/// Is the first vector a prefix of the second?
pub fn is_prefix<T: Eq>(pre: &Vec<T>, full: &Vec<T>) -> bool {
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

/// What is the length of the longest common prefix between two vectors?
pub fn longest_prefix_length<T: Eq>(xs: &Vec<T>, ys: &Vec<T>) -> usize {
    for i in 0..min(xs.len(), ys.len()) {
        if xs[i] != ys[i] {
            return i;
        }
    }
    min(xs.len(), ys.len())
}

/// Follows a path in a radix hash tree.
fn rh_follow_path<
    HL: HashLookup,
    N: DeserializeOwned + Clone,
    GC: Fn(&N) -> &Vec<(HexPath, Hash<N>)>,
>(
    hl: &HL,
    get_children: GC,
    init_node: N,
    path: &HexPath,
) -> Result<(N, HexPath), anyhow::Error> {
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
            bail!("RH node not found");
        }
    }
    Ok((node, prefix))
}

/// Follows a path starting from a `QuorumNode` going down.  Returns a node
/// along with the extra path starting from that node, which is always
/// a postfix of the original path.
pub fn quorum_node_follow_path<HL: HashLookup>(
    hl: &HL,
    node: &QuorumNode,
    path: &HexPath,
) -> Result<(QuorumNode, HexPath), anyhow::Error> {
    rh_follow_path(hl, |qn| &qn.body.children, node.clone(), path)
}

/// Looks up a quorum node in a given main block body.
pub fn lookup_quorum_node<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
    path: &HexPath,
) -> Result<(QuorumNode, HexPath), anyhow::Error> {
    quorum_node_follow_path(hl, &hl.lookup(main.tree)?, path)
}

/// Looks up an account in a given main block body.
pub fn lookup_account<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
    acct: HashCode,
) -> Result<QuorumNode, anyhow::Error> {
    let (qn, postfix) = lookup_quorum_node(hl, main, &bytes_to_path(&acct))?;
    if postfix.len() != 0 {
        bail!("account not found");
    }
    Ok(qn)
}

/// Follows a path starting from a `DataNode` going down.  Returns a node
/// along with the extra path starting from that node, which is always
/// a postfix of the original path.
pub fn data_node_follow_path<HL: HashLookup>(
    hl: &HL,
    node: &DataNode,
    path: &HexPath,
) -> Result<(DataNode, HexPath), anyhow::Error> {
    rh_follow_path(hl, |dn| &dn.children, node.clone(), path)
}

/// Looks up data given an account `QuorumNode`.
pub fn lookup_data_in_account<HL: HashLookup>(
    hl: &HL,
    qn: &QuorumNode,
    path: &HexPath,
) -> Result<Option<Vec<u8>>, anyhow::Error> {
    let top_dn = hl.lookup(
        qn.body
            .data_tree
            .ok_or(anyhow!("no data tree"))?
    )?;
    let (dn, postfix) = data_node_follow_path(hl, &top_dn, path)?;
    if postfix.len() != 0 {
        return Ok(None);
    }
    Ok(dn.field)
}

/// Finds a block with a given version starting from the given block
/// going backwards.
pub fn block_with_version<HL : HashLookup>(
    hl: &HL,
    mb: &MainBlockBody,
    version: u64
) -> Result<MainBlockBody, anyhow::Error> {
    let v = mb.version;
    if version > v {
        bail!("version higher than given main block version");
    }
    if version == v {
        return Ok(mb.clone());
    }
    match &mb.prev {
        None => bail!("tried to get version before the first block"),
        Some(hash) => block_with_version(hl, &hl.lookup(hash.clone())?.block.body, version)
    }
}

/// Gets the random seed for a given main block.  The random seed changes
/// with a period equal to `random_seed_period` in the main options.
/// TODO real randomness
fn random_seed_of_block<HL : HashLookup>(
    hl: &HL,
    main: &MainBlockBody
) -> Result<HashCode, anyhow::Error> {
    let period = hl.lookup(main.options)?.random_seed_period;
    let version_to_get = main.version / u64::from(period) * u64::from(period);
    Ok(hash(&block_with_version(hl, main, version_to_get)?).code)
}


/// Gets the account whose stake corresponds to the given index.
/// We can imagine lining all accounts in a tree side-by-side, with
/// the length of the account proportional to its stake; the index
/// determines how far along this line to go to select an account,
/// enabling randomly selecting an account proportional to its stake.
fn stake_indexed_account<HL: HashLookup>(
    hl: &HL,
    qn: &QuorumNode,
    stake_ix: u128,
) -> Result<HashCode, anyhow::Error> {
    if stake_ix >= qn.body.total_stake {
        bail!("index exceeds total stake");
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

/// Gets a random account proportional to its stake.  This function is
/// technically deterministic; its randomness is determined by the
/// `seed` and `rand_id` parameters.
pub fn random_account<HL : HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
    seed: HashCode,
    rand_id: String
) -> Result<HashCode, anyhow::Error> {
    let rand_period = hl.lookup(main.options)?.random_seed_period;
    let mut rounded = main.version / u64::from(rand_period) * u64::from(rand_period);
    if rounded > 0 {
        rounded = rounded - u64::from(rand_period);
    }
    let stake_main = block_with_version(hl, main, rounded)?;
    let top = hl.lookup(stake_main.tree)?;
    if top.body.total_stake == 0 {
        bail!("can't select random account when there is no stake");
    }
    let full_id = format!("random_account {:?} {} {}", seed, main.version, rand_id);
    let rand_hash = hash(&full_id).code;
    let mut rand_val: u128 = 0;
    for i in 0..16 {
        rand_val *= 256;
        rand_val += u128::from(rand_hash[i]);
    }
    stake_indexed_account(hl, &top, rand_val % top.body.total_stake)
}

/// Gets the miner and signers of a block following a given block.
pub fn miner_and_signers_by_prev_block<HL : HashLookup>(
    hl: &HL,
    main: &MainBlock
) -> Result<(HashCode, Vec<HashCode>), anyhow::Error> {
    let body = &main.block.body;
    let num_signers = hl.lookup(body.options)?.main_block_signers;
    let seed = random_seed_of_block(hl, body)?;
    let miner = random_account(hl, body, seed, "miner".to_string())?;
    let mut signers = Vec::new();
    for i in 0..num_signers {
        signers.push(random_account(hl, body, seed, format!("signer {}", i))?);
    }
    Ok((miner, signers))
}

/// Selects quorums for a given path, given the block before the block of
/// the relevant quorum tree.
pub fn quorums_by_prev_block<HL : HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
    path: HexPath
) -> Result<Vec<(Vec<HashCode>, u32)>, anyhow::Error> {
    let sizes_thresholds = hl.lookup(main.options)?.quorum_sizes_thresholds;
    let period = hl.lookup(main.options)?.quorum_period;
    let mut base_version = main.version / u64::from(period) * u64::from(period);
    if base_version > 0 {
        base_version = base_version - u64::from(period);
    }
    let rand_acct_main = block_with_version(hl, main, base_version)?;
    let seed = random_seed_of_block(hl, &rand_acct_main)?;
    let mut quorums = Vec::new();
    for i in 0..sizes_thresholds.len() {
        let (size, threshold) = sizes_thresholds[i];
        let mut members = Vec::new();
        for j in 0..size {
            members.push(random_account(hl, &rand_acct_main, seed, format!("quorum {:?} {} {}", path, i, j))?);
        }
        quorums.push((members, threshold));
    }
    Ok(quorums)
}
