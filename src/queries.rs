use std::cmp::min;

use crate::blockdata::{DataNode, MainBlock, MainBlockBody, QuorumNode, RadixHashNode};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::HashLookup;
use crate::hex_path::{bytes_to_path, is_prefix, u4, HexPath};

use anyhow::{anyhow, bail};
use serde::de::DeserializeOwned;

/// What is the length of the longest common prefix between two vectors?
pub fn longest_prefix_length<T: Eq>(xs: &[T], ys: &[T]) -> usize {
    xs.iter().zip(ys.iter()).take_while(|(x, y)| x == y).count()
}

/// Follows a path in a radix hash tree.
pub async fn rh_follow_path<HL: HashLookup, N: RadixHashNode>(
    hl: &HL,
    mut node: N,
    mut path: &[u4],
) -> Result<Option<(N, HexPath)>, anyhow::Error> {
    loop {
        if path.is_empty() {
            break;
        }

        let ix = path[0].value as usize;
        let rest = &path[1..];
        let children = node.get_children();

        if children.0[ix].is_none() {
            break;
        }

        let (prefix, child_hash) = children.0[ix].as_ref().unwrap();

        if is_prefix(prefix, rest) {
            path = &path[prefix.len()..];
            node = hl.lookup(*child_hash).await?;
            continue;
        } else if is_prefix(rest, prefix) {
            break;
        } else {
            return Ok(None);
        }
    }

    Ok(Some((node, path.to_owned())))
}

/// Follows a path starting from a `QuorumNode` going down.  Returns a node
/// along with the extra path starting from that node, which is always
/// a postfix of the original path.
pub async fn quorum_node_follow_path<HL: HashLookup>(
    hl: &HL,
    node: &QuorumNode,
    path: &HexPath,
) -> Result<Option<(QuorumNode, HexPath)>, anyhow::Error> {
    rh_follow_path(hl, node.clone(), path).await
}

/// Looks up a quorum node in a given main block body.
pub async fn lookup_quorum_node<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
    path: &HexPath,
) -> Result<Option<(QuorumNode, HexPath)>, anyhow::Error> {
    quorum_node_follow_path(hl, &hl.lookup(main.tree).await?, path).await
}

/// Looks up an account in a given main block body.
pub async fn lookup_account<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
    acct: HashCode,
) -> Result<Option<QuorumNode>, anyhow::Error> {
    match lookup_quorum_node(hl, main, &bytes_to_path(&acct)).await? {
        None => Ok(None),
        Some((qn, postfix)) => {
            if postfix.len() != 0 {
                Ok(None)
            } else {
                Ok(Some(qn))
            }
        }
    }
}

/// Follows a path starting from a `DataNode` going down.  Returns a node
/// along with the extra path starting from that node, which is always
/// a postfix of the original path.
pub async fn data_node_follow_path<HL: HashLookup>(
    hl: &HL,
    node: &DataNode,
    path: &HexPath,
) -> Result<Option<(DataNode, HexPath)>, anyhow::Error> {
    rh_follow_path(hl, node.clone(), path).await
}

/// Looks up data given an account `QuorumNode`.
pub async fn lookup_data_in_account<HL: HashLookup>(
    hl: &HL,
    qn: &QuorumNode,
    path: &HexPath,
) -> Result<Option<Vec<u8>>, anyhow::Error> {
    let top_dn = hl
        .lookup(qn.body.data_tree.ok_or(anyhow!("no data tree"))?)
        .await?;
    match data_node_follow_path(hl, &top_dn, path).await? {
        None => Ok(None),
        Some((dn, postfix)) => {
            if postfix.len() != 0 {
                Ok(None)
            } else {
                Ok(dn.field)
            }
        }
    }
}

/// Finds a block with a given version starting from the given block
/// going backwards.
pub async fn block_with_version<HL: HashLookup>(
    hl: &HL,
    mut mb: &MainBlockBody,
    version: u64,
) -> Result<MainBlockBody, anyhow::Error> {
    let mut placeholder;
    loop {
        let v = mb.version;
        if version > v {
            bail!("version higher than given main block version");
        }
        if version == v {
            return Ok(mb.clone());
        }
        match &mb.prev {
            None => bail!("tried to get version before the first block"),
            Some(hash) => {
                placeholder = hl.lookup(hash.clone()).await?.block.body;
                mb = &placeholder;
            }
        }
    }
}

/// Gets the random seed for a given main block.  The random seed changes
/// with a period equal to `random_seed_period` in the main options.
/// TODO real randomness
async fn random_seed_of_block<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
) -> Result<HashCode, anyhow::Error> {
    let period = hl.lookup(main.options).await?.random_seed_period;
    let version_to_get = main.version / u64::from(period) * u64::from(period);
    Ok(hash(&block_with_version(hl, main, version_to_get).await?).code)
}

/// Gets the account whose stake corresponds to the given index.
/// We can imagine lining all accounts in a tree side-by-side, with
/// the length of the account proportional to its stake; the index
/// determines how far along this line to go to select an account,
/// enabling randomly selecting an account proportional to its stake.
async fn stake_indexed_account<HL: HashLookup>(
    hl: &HL,
    qn: &QuorumNode,
    mut stake_ix: u128,
) -> Result<HashCode, anyhow::Error> {
    'outer: loop {
        if stake_ix >= qn.body.stats.stake {
            bail!("index exceeds total stake");
        }
        let path = qn.body.path.clone();
        if path.len() == 64 {
            return Ok(path_to_hash_code(path));
        }
        let mut children = Vec::new();
        for (_, child_hash) in qn.body.children.iter_entries() {
            children.push(hl.lookup(*child_hash).await?);
        }
        let child_stakes: Vec<u128> = children.iter().map(|ch| ch.body.stats.stake).collect();
        let mut sum_so_far = 0;
        for i in 0..children.len() {
            if stake_ix < sum_so_far + child_stakes[i] {
                stake_ix -= sum_so_far;
                continue 'outer;
            }
            sum_so_far += child_stakes[i];
        }
        bail!("total stake does not equal sum of child node total stakes!")
    }
}

/// Gets a random account proportional to its stake.  This function is
/// technically deterministic; its randomness is determined by the
/// `seed` and `rand_id` parameters.
pub async fn random_account<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
    seed: HashCode,
    rand_id: String,
) -> Result<HashCode, anyhow::Error> {
    let rand_period = hl.lookup(main.options).await?.random_seed_period;
    let mut rounded = main.version / u64::from(rand_period) * u64::from(rand_period);
    if rounded > 0 {
        rounded = rounded - u64::from(rand_period);
    }
    let stake_main = block_with_version(hl, main, rounded).await?;
    let top = hl.lookup(stake_main.tree).await?;
    if top.body.stats.stake == 0 {
        bail!("can't select random account when there is no stake");
    }
    let full_id = format!("random_account {:?} {} {}", seed, main.version, rand_id);
    let rand_hash = hash(&full_id).code;
    let mut rand_val: u128 = 0;
    for i in 0..16 {
        rand_val *= 256;
        rand_val += u128::from(rand_hash[i]);
    }
    stake_indexed_account(hl, &top, rand_val % top.body.stats.stake).await
}

/// Gets the miner and signers of a block following a given block.
pub async fn miner_and_signers_by_prev_block<HL: HashLookup>(
    hl: &HL,
    main: &MainBlock,
) -> Result<(HashCode, Vec<HashCode>), anyhow::Error> {
    let body = &main.block.body;
    let num_signers = hl.lookup(body.options).await?.main_block_signers;
    let seed = random_seed_of_block(hl, body).await?;
    let miner = random_account(hl, body, seed, "miner".to_string()).await?;
    let mut signers = Vec::new();
    for i in 0..num_signers {
        signers.push(random_account(hl, body, seed, format!("signer {}", i)).await?);
    }
    Ok((miner, signers))
}

/// Selects quorums for a given path, given the block before the block of
/// the relevant quorum tree.
pub async fn quorums_by_prev_block<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
    path: HexPath,
) -> Result<Vec<(Vec<HashCode>, u32)>, anyhow::Error> {
    let sizes_thresholds = hl.lookup(main.options).await?.quorum_sizes_thresholds;
    let period = hl.lookup(main.options).await?.quorum_period;
    let mut base_version = main.version / u64::from(period) * u64::from(period);
    if base_version > 0 {
        base_version = base_version - u64::from(period);
    }
    let rand_acct_main = block_with_version(hl, main, base_version).await?;
    let seed = random_seed_of_block(hl, &rand_acct_main).await?;
    let mut quorums = Vec::new();
    for i in 0..sizes_thresholds.len() {
        let (size, threshold) = sizes_thresholds[i];
        let mut members = Vec::new();
        for j in 0..size {
            members.push(
                random_account(
                    hl,
                    &rand_acct_main,
                    seed,
                    format!("quorum {:?} {} {}", path, i, j),
                )
                .await?,
            );
        }
        quorums.push((members, threshold));
    }
    Ok(quorums)
}
