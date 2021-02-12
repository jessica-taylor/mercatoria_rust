use crate::blockdata::{DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::{HashLookup, HashPut};
use crate::hex_path::{bytes_to_path, HexPath};
use crate::queries::{is_prefix, longest_prefix_length};


fn children_paths_well_formed<N>(children: &Vec<(HexPath, N)>) -> bool {
    for i in 0..children.len() {
        let (path, _) = &children[i];
        if path.len() == 0 || (i > 0 && path[0] <= children[i-1].0[0]) {
            return false;
        }
    }
    true
}

fn data_node_well_formed(dn: &DataNode) -> bool {
    children_paths_well_formed(&dn.children) && !(dn.children.len() <= 1 && dn.value.is_none())
}

struct TreeInfo {
    fee: u128,
    gas: u128,
    new_nodes: u64,
    prize: u128,
    stake: u128,
    new_transactions: u64,
    new_quorums: u64
}

fn cached_tree_info(qnb: &QuorumNodeBody) -> TreeInfo {
    TreeInfo {
        fee: qnb.total_fee,
        gas: qnb.total_gas,
        new_nodes: qnb.new_nodes,
        prize: qnb.total_prize,
        stake: qnb.total_stake,
        new_transactions: 0,
        new_quorums: 0
    }
}

impl TreeInfo {
    fn zero() -> TreeInfo {
        TreeInfo {
            fee: 0,
            gas: 0,
            new_nodes: 0,
            prize: 0,
            stake: 0,
            new_transactions: 0,
            new_quorums: 0
        }
    }
    fn plus(self: &TreeInfo, other: &TreeInfo) -> TreeInfo {
        TreeInfo {
            fee: self.fee + other.fee,
            gas: self.gas + other.gas,
            new_nodes: self.new_nodes + other.new_nodes,
            prize: self.prize + other.prize,
            stake: self.stake + other.stake,
            new_transactions: self.new_transactions + other.new_transactions,
            new_quorums: self.new_quorums + other.new_quorums
        }
    }
}

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

fn modify_data_tree<HL : HashLookup + HashPut>(hl: &mut HL, path: HexPath, field: Vec<u8>, hash_tree: Hash<DataNode>) -> Result<Hash<DataNode>, anyhow::Error> {
    let tree = hl.lookup(hash_tree)?;
    if path.len() == 0 {
        return Ok(hl.put(&DataNode {value: Some(field), children: tree.children}));
    } else {
        for (suffix, child_hash) in tree.children.clone() {
            if suffix[0] == path[0] {
                if is_prefix(&suffix, &path) {
                    let new_child_hash = modify_data_tree(hl, path[suffix.len()..].to_vec(), field, child_hash)?;
                    let new_children = insert_child((suffix, new_child_hash), tree.children);
                    return Ok(hl.put(&DataNode {value: tree.value, children: new_children}));
                } else {
                    let pref_len = longest_prefix_length(&path, &suffix);
                    let new_child_hash = hl.put(&DataNode {value: None, children: vec![(suffix[pref_len..].to_vec(), child_hash)]});
                    let new_children = insert_child((path[0..pref_len].to_vec(), new_child_hash), tree.children);
                    return Ok(hl.put(&DataNode {value: tree.value, children: new_children}));

                }
            }
        }
        let node_hash = hl.put(&DataNode {value: Some(field), children: vec![]});
        let new_children = insert_child((path, node_hash), tree.children);
        return Ok(hl.put(&DataNode {value: tree.value, children: new_children}));
    }
}
