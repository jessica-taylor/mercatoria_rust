use crate::blockdata::{DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::HashLookup;
use crate::hex_path::{bytes_to_path, HexPath};

fn children_paths_well_formed<N>(children: Vec<(HexPath, N)>) -> bool {
    for i in 0..children.len() {
        let (path, _) = &children[i];
        if path.len() == 0 || (i > 0 && path[0] <= children[i-1].0[0]) {
            return false;
        }
    }
    true
}
