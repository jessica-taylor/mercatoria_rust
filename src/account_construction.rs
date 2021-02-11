use crate::blockdata::{DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::HashLookup;
use crate::hex_path::{bytes_to_path, HexPath};
use crate::queries::is_prefix;


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

// enum DataTreeDiff {
//     ChangedField(Option<Vec<u8>>, Option<Vec<u8>>),
//     CreatedNode,
//     DeletedNode,
//     MalformedNode,
//     TooDeep
// }
// 
// const data_tree_max_depth: usize = 1 + 64;
// 
// fn data_tree_differences<HL : HashLookup>(
//     hl: &HL,
//     path: HexPath,
//     old_suffix: HexPath,
//     hash_old_tree: Hash<DataNode>,
//     hash_new_tree: Hash<DataNode>,
//     diffs: &mut Vec<(HexPath, DataTreeDiff)>
// ) -> Result<(), anyhow::Error> {
//     if path.len() > data_tree_max_depth {
//         diffs.push((path, DataTreeDiff::TooDeep));
//         return Ok(());
//     }
//     if old_suffix.len() == 0 && hash_old_tree == hash_new_tree {
//         return Ok(());
//     }
//     let new_tree = hl.lookup(hash_new_tree)?;
//     if !data_node_well_formed(&new_tree) {
//         diffs.push((path, DataTreeDiff::MalformedNode));
//         return Ok(());
//     }
//     diffs.push((path.clone(), DataTreeDiff::CreatedNode));
//     let (old_value, old_children) = if old_suffix.len() == 0 {
//         let old_tree = hl.lookup(hash_old_tree)?;
//         (old_tree.value, old_tree.children)
//     } else {
//         (None, vec![(old_suffix, hash_old_tree)])
//     };
//     if old_value != new_tree.value {
//         diffs.push((path.clone(), DataTreeDiff::ChangedField(old_value, new_tree.value)));
//     }
//     for (new_child_suffix, new_child) in new_tree.children {
//         let mut found = false;
//         for (old_child_suffix, old_child) in old_children {
//             if is_prefix(new_child_suffix, old_child_suffix) {
//                 found = true;
//                 let rest_suffix = old_child_suffix[new_child_suffix.len()..].to_vec();
//                 data_tree_differences(hl, path ++ new_child_suffix, rest_suffix, old_child, new_child, diffs)?;
//                 break outer;
//             }
//         }
//         if !found {
//             report_new_nodes(path ++ new_child_suffix, new_child)?;
//         }
//     }
//     for (old_child_suffix, _) in old_children {
//     }
//     Ok(())
// }
// 
