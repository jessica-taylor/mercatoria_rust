use std::collections::BTreeMap;

use crate::crypto::HashCode;




pub trait HashLookup {
    fn hash_lookup(&self, hash: HashCode) -> Option<Vec<u8>>;
}

pub trait HashPut {
    fn hash_put(&mut self, bs: Vec<u8>);
}

pub struct MapHashLookup {
    map: BTreeMap<HashCode, Vec<u8>>,
}

impl HashLookup for MapHashLookup {
    fn hash_lookup(&self, hash: HashCode) -> Option<Vec<u8>> {
        match self.map.get(&hash) {
            None => None,
            Some(x) => Some(x.clone())
        }
    }
}
