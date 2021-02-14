use std::{future::Future, pin::Pin};

use futures_lite::{future, FutureExt};

use crate::blockdata::{
    DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody,
};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::{HashLookup, HashPut};
use crate::hex_path::{bytes_to_path, HexPath};
use crate::queries::{is_prefix, longest_prefix_length};

/// Checks whether a radix hash node's children are well-formed.
fn children_paths_well_formed<N>(children: &Vec<(HexPath, N)>) -> bool {
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
    child: (HexPath, Hash<DataNode>),
    tree: DataNode,
) -> Result<Hash<DataNode>, anyhow::Error> {
    let new_children = insert_child(child, tree.children);
    hl.put(&DataNode {
        field: tree.field,
        children: new_children,
    })
    .await
}

/// Inserts a field at a given path in a data tree.
fn insert_into_data_tree<HL: HashLookup + HashPut>(
    hl: &mut HL,
    path: HexPath,
    field: Vec<u8>,
    hash_tree: Hash<DataNode>,
) -> Pin<Box<dyn Future<Output = Result<Hash<DataNode>, anyhow::Error>> + Send + '_>> {
    async move {
        let tree = hl.lookup(hash_tree).await?;
        if path.len() == 0 {
            // just replace the field
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
                            path[suffix.len()..].to_vec(),
                            field,
                            child_hash,
                        )
                        .await?;
                        return data_node_insert_child(hl, (suffix, new_child_hash), tree).await;
                    } else {
                        // create an intermediate node
                        let pref_len = longest_prefix_length(&path, &suffix);
                        let mut new_child_hash = hl
                            .put(&DataNode {
                                field: None,
                                children: vec![(suffix[pref_len..].to_vec(), child_hash)],
                            })
                            .await?;
                        // modify the intermediate node
                        new_child_hash = insert_into_data_tree(
                            hl,
                            path[pref_len..].to_vec(),
                            field,
                            new_child_hash,
                        )
                        .await?;
                        return data_node_insert_child(
                            hl,
                            (path[0..pref_len].to_vec(), new_child_hash),
                            tree,
                        )
                        .await;
                    }
                }
            }
            // insert a new child that itself has no children
            let node_hash = hl
                .put(&DataNode {
                    field: Some(field),
                    children: vec![],
                })
                .await?;
            return data_node_insert_child(hl, (path, node_hash), tree).await;
        }
    }
    .boxed()
}
