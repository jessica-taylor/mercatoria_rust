use std::collections::BTreeMap;

use crate::crypto::{HashCode, hash_of_bytes};




pub trait HashLookup {
    fn hash_lookup(&self, hash: HashCode) -> Option<Vec<u8>>;
}

pub trait HashPut {
    fn hash_put(&mut self, bs: Vec<u8>);
}

pub struct MapHashLookup {
    map: BTreeMap<HashCode, Vec<u8>>,
}

impl MapHashLookup {
    pub fn new() -> MapHashLookup {
        MapHashLookup {map: BTreeMap::new()}
    }
}

impl HashLookup for MapHashLookup {
    fn hash_lookup(&self, hash: HashCode) -> Option<Vec<u8>> {
        match self.map.get(&hash) {
            None => None,
            Some(x) => Some(x.clone())
        }
    }
}

impl HashPut for MapHashLookup {
    fn hash_put(&mut self, bs: Vec<u8>) {
        self.map.insert(hash_of_bytes(&bs), bs);
    }
}
