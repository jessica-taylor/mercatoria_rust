use std::{collections::BTreeMap, future::Future, pin::Pin};

use anyhow::*;
use futures_lite::{future, FutureExt};

use crate::account_transform::{
    field_balance, field_public_key, field_received, field_send, field_stake, run_action,
    AccountTransform,
};
use crate::blockdata::{
    Action, DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody,
};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::{HashLookup, HashPut};
use crate::hex_path::{bytes_to_path, HexPath};
use crate::queries::{is_prefix, longest_prefix_length, lookup_account};

/// Checks whether a radix hash node's children are well-formed.
pub fn children_paths_well_formed<N>(children: &Vec<(HexPath, N)>) -> bool {
    for i in 0..children.len() {
        let (path, _) = &children[i];
        if path.len() == 0 || (i > 0 && path[0] <= children[i - 1].0[0]) {
            return false;
        }
    }
    true
}

/// Checks whether a data node is well-formed.
fn data_node_well_formed(dn: &DataNode) -> bool {
    children_paths_well_formed(&dn.children) && !(dn.children.len() <= 1 && dn.field.is_none())
}

/// Summary statistics about a `QuorumNode`, summing all data at or below the
/// node.
pub struct TreeInfo {
    pub fee: u128,
    pub gas: u128,
    pub new_nodes: u64,
    pub prize: u128,
    pub stake: u128,
    pub new_transactions: u64,
    pub new_quorums: u64,
}

/// Gets `TreeInfo` cached in a `QuorumNodeBody`; `new_transactions` and `new_quorums'
/// are set to 0.
fn cached_tree_info(qnb: &QuorumNodeBody) -> TreeInfo {
    TreeInfo {
        fee: qnb.total_fee,
        gas: qnb.total_gas,
        new_nodes: qnb.new_nodes,
        prize: qnb.total_prize,
        stake: qnb.total_stake,
        new_transactions: 0,
        new_quorums: 0,
    }
}

impl TreeInfo {
    /// `TreeInfo` with all fields set to 0.
    fn zero() -> TreeInfo {
        TreeInfo {
            fee: 0,
            gas: 0,
            new_nodes: 0,
            prize: 0,
            stake: 0,
            new_transactions: 0,
            new_quorums: 0,
        }
    }

    /// Adds fields in two `TreeInfo`s.
    fn plus(self: &TreeInfo, other: &TreeInfo) -> TreeInfo {
        TreeInfo {
            fee: self.fee + other.fee,
            gas: self.gas + other.gas,
            new_nodes: self.new_nodes + other.new_nodes,
            prize: self.prize + other.prize,
            stake: self.stake + other.stake,
            new_transactions: self.new_transactions + other.new_transactions,
            new_quorums: self.new_quorums + other.new_quorums,
        }
    }
}

/// Inserts a child into a list of radix hash children, replacing a child
/// with the same first character if one exists.
fn insert_child<N>(child: (HexPath, N), mut children: Vec<(HexPath, N)>) -> Vec<(HexPath, N)> {
    for i in 0..children.len() {
        if children[i].0[0] >= child.0[0] {
            if children[i].0[0] == child.0[0] {
                children[i] = child;
            } else {
                children.insert(i, child);
            }
            return children;
        }
    }
    children.push(child);
    children
}

/// Modifies a `DataNode` to insert a new child.
async fn data_node_insert_child<HL: HashLookup + HashPut>(
    hl: &mut HL,
    node_count: &mut usize,
    child: (HexPath, Hash<DataNode>),
    tree: DataNode,
) -> Result<Hash<DataNode>, anyhow::Error> {
    let new_children = insert_child(child, tree.children);
    *node_count += 1;
    hl.put(&DataNode {
        field: tree.field,
        children: new_children,
    })
    .await
}

/// Inserts a field at a given path in a data tree.
fn insert_into_data_tree<'a, HL: HashLookup + HashPut>(
    hl: &'a mut HL,
    node_count: &'a mut usize,
    path: HexPath,
    field: Vec<u8>,
    hash_tree: Hash<DataNode>,
) -> Pin<Box<dyn Future<Output = Result<Hash<DataNode>, anyhow::Error>> + Send + 'a>> {
    async move {
        let tree = hl.lookup(hash_tree).await?;
        if path.len() == 0 {
            // just replace the field
            *node_count += 1;
            return hl
                .put(&DataNode {
                    field: Some(field),
                    children: tree.children,
                })
                .await;
        } else {
            for (suffix, child_hash) in tree.children.clone() {
                if suffix[0] == path[0] {
                    if is_prefix(&suffix, &path) {
                        // modify the child
                        let new_child_hash = insert_into_data_tree(
                            hl,
                            node_count,
                            path[suffix.len()..].to_vec(),
                            field,
                            child_hash,
                        )
                        .await?;
                        return data_node_insert_child(
                            hl,
                            node_count,
                            (suffix, new_child_hash),
                            tree,
                        )
                        .await;
                    } else {
                        // create an intermediate node
                        let pref_len = longest_prefix_length(&path, &suffix);
                        *node_count += 1;
                        let mut new_child_hash = hl
                            .put(&DataNode {
                                field: None,
                                children: vec![(suffix[pref_len..].to_vec(), child_hash)],
                            })
                            .await?;
                        // modify the intermediate node
                        new_child_hash = insert_into_data_tree(
                            hl,
                            node_count,
                            path[pref_len..].to_vec(),
                            field,
                            new_child_hash,
                        )
                        .await?;
                        return data_node_insert_child(
                            hl,
                            node_count,
                            (path[0..pref_len].to_vec(), new_child_hash),
                            tree,
                        )
                        .await;
                    }
                }
            }
            // insert a new child that itself has no children
            *node_count += 1;
            let node_hash = hl
                .put(&DataNode {
                    field: Some(field),
                    children: vec![],
                })
                .await?;
            return data_node_insert_child(hl, node_count, (path, node_hash), tree).await;
        }
    }
    .boxed()
}

/// Initializes an account node.  The resulting node is only valid in the genesis
/// block.
pub async fn initialize_account_node<HL: HashLookup + HashPut>(
    hl: &mut HL,
    last_main: Option<Hash<MainBlock>>,
    key: ed25519_dalek::PublicKey,
    balance: u128,
    stake: u128,
) -> Result<(BTreeMap<HexPath, Vec<u8>>, QuorumNodeBody), anyhow::Error> {
    let acct = hash(&key).code;
    let mut fields = BTreeMap::new();
    fields.insert(
        field_balance().path,
        rmp_serde::to_vec_named(&balance).unwrap(),
    );
    fields.insert(field_stake().path, rmp_serde::to_vec_named(&stake).unwrap());
    fields.insert(
        field_public_key().path,
        rmp_serde::to_vec_named(&key).unwrap(),
    );
    let mut data_tree: Hash<DataNode> = hl
        .put(&DataNode {
            field: None,
            children: vec![],
        })
        .await?;
    let mut node_count = 1;
    for (path, value) in fields.clone() {
        data_tree = insert_into_data_tree(hl, &mut node_count, path, value, data_tree).await?;
    }
    let node = QuorumNodeBody {
        last_main: last_main,
        path: bytes_to_path(&acct),
        children: vec![],
        data_tree: Some(data_tree),
        new_action: None,
        prize: 0,
        new_nodes: node_count as u64,
        total_fee: 0,
        total_gas: 0,
        total_stake: stake,
        total_prize: 0,
    };
    Ok((fields, node))
}

/// Causes a given account to run a given action, producing a new account `QuorumNodeBody`.
pub async fn add_action_to_account<HL: HashLookup + HashPut>(
    hl: &mut HL,
    last_main: &MainBlock,
    account: HashCode,
    action: &Action,
    prize: u128,
) -> Result<QuorumNodeBody, anyhow::Error> {
    let (is_init, mut data_tree) = match lookup_account(hl, &last_main.block.body, account).await? {
        None => (
            true,
            hl.put(&DataNode {
                field: None,
                children: vec![],
            })
            .await?,
        ),
        Some(prev_node) => (
            false,
            prev_node
                .body
                .data_tree
                .ok_or_else(|| anyhow!("account has no data tree"))?,
        ),
    };
    let mut at = AccountTransform::new(hl, is_init, account, hash(&last_main));
    run_action(&mut at, action).await?;
    let new_stake = at.get_data_field_or_error(account, &field_stake()).await?;
    let mut node_count = 0;
    for (path, value) in at.fields_set {
        data_tree = insert_into_data_tree(hl, &mut node_count, path, value, data_tree).await?;
    }
    Ok(QuorumNodeBody {
        last_main: Some(hash(last_main)),
        path: bytes_to_path(&account),
        children: vec![],
        data_tree: Some(data_tree),
        prize,
        new_action: Some(hl.put(action).await?),
        new_nodes: (node_count as u64) + 1, // node_count data nodes + 1 quorum node
        total_fee: action.fee,
        total_gas: 0,
        total_stake: new_stake,
        total_prize: prize,
    })
}
