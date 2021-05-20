//! Functionality for constructing and updating account nodes.
use std::{collections::BTreeMap, future::Future, pin::Pin};

use anyhow::*;
use futures_lite::FutureExt;

use crate::account_transform::{
    field_balance, field_public_key, field_stake, run_action, AccountTransform,
};
use crate::blockdata::{
    AccountInit, Action, DataNode, MainBlock, QuorumNodeBody, QuorumNodeStats, RadixChildren,
    RadixHashNode,
};
use crate::crypto::{hash, Hash, HashCode};
use crate::hashlookup::{HashLookup, HashPut};
use crate::hex_path::{bytes_to_path, is_prefix, u4, HexPath};
use crate::queries::{longest_prefix_length, lookup_account};

/// Checks whether a data node is well-formed.
pub fn data_node_well_formed(dn: &DataNode) -> bool {
    !(dn.children.len() <= 1 && dn.field.is_none())
}

impl<N> RadixChildren<N> {
    /// Inserts a child into a list of radix hash children, replacing a child
    /// with the same first character if one exists.
    fn insert_child(&mut self, path: &[u4], node: N) -> Option<(HexPath, N)> {
        self.0[path.get(0)?.0 as usize].replace((HexPath((&path[1..]).to_owned()), node))
    }
}

/// Modifies a `RadixHashNode` to insert a new child.
async fn rh_node_insert_child<HL: HashLookup + HashPut, N: RadixHashNode>(
    hl: &mut HL,
    node_count: &mut usize,
    path: &[u4],
    child: Hash<N>,
    tree: N,
) -> Result<Hash<N>, anyhow::Error> {
    let mut children = tree.get_children().to_owned();
    children.insert_child(path, child);
    *node_count += 1;
    hl.put(&tree.replace_children(hl, children).await?).await
}

/// Modifies a node at a given path in a radix hash tree.
/// `node_count` is incremented by the number of nodes created.
/// `path` is the path to insert at.
/// `get_new_node` is called with the old node (if it exists), getting the node to insert.
/// `hash_tree` is the top of the initial tree.
/// Returns the top of the new tree.
pub fn insert_into_rh_tree<
    'a,
    HL: HashLookup + HashPut,
    N: 'a + RadixHashNode + core::fmt::Debug,
    GN: 'a + Send + Sized + FnOnce(Option<N>) -> Result<N, anyhow::Error>,
>(
    hl: &'a mut HL,
    node_count: &'a mut usize,
    path: &'a [u4],
    get_new_node: GN,
    hash_tree: Hash<N>,
) -> Pin<Box<dyn Future<Output = Result<Hash<N>, anyhow::Error>> + Send + 'a>> {
    async move {
        let tree = hl.lookup(hash_tree).await?;
        if path.is_empty() {
            // just replace the node
            *node_count += 1;
            return hl.put(&get_new_node(Some(tree))?).await;
        } else if let Some((suffix, child_hash)) = tree.get_children().0[path[0].0 as usize].clone()
        {
            if is_prefix(&suffix[..], &path[1..]) {
                // modify the child
                let new_child_hash = insert_into_rh_tree(
                    hl,
                    node_count,
                    &path[1 + suffix.len()..],
                    get_new_node,
                    child_hash,
                )
                .await?;
                return rh_node_insert_child(
                    hl,
                    node_count,
                    &path[..1 + suffix.len()],
                    new_child_hash,
                    tree,
                )
                .await;
            } else {
                // create an intermediate node
                let pref_len = longest_prefix_length(&path[1..], &suffix[..]);
                *node_count += 1;
                let mut new_child_hash = hl
                    .put(
                        &N::from_single_child(
                            hl,
                            (HexPath(suffix[pref_len..].to_vec()), child_hash),
                        )
                        .await?,
                    )
                    .await?;
                // modify the intermediate node
                new_child_hash = insert_into_rh_tree(
                    hl,
                    node_count,
                    &path[1 + pref_len..],
                    get_new_node,
                    new_child_hash,
                )
                .await?;
                return rh_node_insert_child(
                    hl,
                    node_count,
                    &path[..1 + pref_len],
                    new_child_hash,
                    tree,
                )
                .await;
            }
        } else {
            // insert a new child that itself has no children
            *node_count += 1;
            let node_hash = hl.put(&get_new_node(None)?).await?;
            return rh_node_insert_child(hl, node_count, path, node_hash, tree).await;
        }
    }
    .boxed()
}

/// Inserts a field at a given path in a data tree.
pub async fn insert_into_data_tree<'a, HL: HashLookup + HashPut>(
    hl: &'a mut HL,
    node_count: &'a mut usize,
    path: &[u4],
    field: Vec<u8>,
    hash_tree: Hash<DataNode>,
) -> Result<Hash<DataNode>, anyhow::Error> {
    let replace = |option_node: Option<DataNode>| match option_node {
        None => Ok(DataNode {
            field: Some(field),
            children: RadixChildren::default(),
        }),
        Some(mut n) => {
            n.field = Some(field);
            Ok(n)
        }
    };
    insert_into_rh_tree(hl, node_count, path, replace, hash_tree).await
}

/// Initializes an account node.  The resulting node is only valid in the genesis
/// block.
pub async fn initialize_account_node<HL: HashLookup + HashPut>(
    hl: &mut HL,
    last_main: Option<Hash<MainBlock>>,
    init: &AccountInit,
) -> Result<(BTreeMap<HexPath, Vec<u8>>, QuorumNodeBody), anyhow::Error> {
    let acct = hash(&init.public_key).code;
    let mut fields = BTreeMap::new();
    fields.insert(
        field_balance().path,
        rmp_serde::to_vec_named(&init.balance).unwrap(),
    );
    fields.insert(
        field_stake().path,
        rmp_serde::to_vec_named(&init.stake).unwrap(),
    );
    fields.insert(
        field_public_key().path,
        rmp_serde::to_vec_named(&init.public_key).unwrap(),
    );
    let mut data_tree: Hash<DataNode> = hl
        .put(&DataNode {
            field: None,
            children: RadixChildren::default(),
        })
        .await?;
    let mut node_count = 1;
    for (path, value) in fields.clone() {
        data_tree = insert_into_data_tree(hl, &mut node_count, &path[..], value, data_tree).await?;
    }
    let node = QuorumNodeBody {
        last_main,
        path: bytes_to_path(&acct),
        children: RadixChildren::default(),
        data_tree: Some(data_tree),
        new_action: None,
        prize: 0,
        stats: QuorumNodeStats {
            new_nodes: node_count as u64,
            fee: 0,
            gas: 0,
            stake: init.stake,
            prize: 0,
        },
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
                children: RadixChildren::default(),
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
        data_tree = insert_into_data_tree(hl, &mut node_count, &path[..], value, data_tree).await?;
    }
    Ok(QuorumNodeBody {
        last_main: Some(hash(last_main)),
        path: bytes_to_path(&account),
        children: RadixChildren::default(),
        data_tree: Some(data_tree),
        prize,
        new_action: Some(hl.put(action).await?),
        stats: QuorumNodeStats {
            new_nodes: (node_count as u64) + 1, // node_count data nodes + 1 quorum node
            fee: action.fee,
            gas: 0,
            stake: new_stake,
            prize,
        },
    })
}
