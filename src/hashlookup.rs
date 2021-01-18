use std::collections::BTreeMap;

use crate::crypto::{Hash, HashCode, hash_of_bytes};

use serde::{Serialize, de::DeserializeOwned};



pub trait HashLookup {
    fn lookup_bytes(&self, hash: HashCode) -> Option<Vec<u8>>;
    fn lookup<T: DeserializeOwned>(&self, hash: Hash<T>) -> Option<T> {
        match self.lookup_bytes(hash.code) {
            None => None,
            Some(bytes) => {
                match serde_cbor::from_slice(&bytes) {
                    Ok(x) => Some(x),
                    Err(_e) => None,
                }
            }
        }
    }
}

pub trait HashPut {
    fn put_bytes(&mut self, bs: Vec<u8>);
    fn put<T: Serialize>(&mut self, val: &T) {
        self.put_bytes(serde_cbor::to_vec(val).unwrap());
    }
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
    fn lookup_bytes(&self, hash: HashCode) -> Option<Vec<u8>> {
        match self.map.get(&hash) {
            None => None,
            Some(x) => Some(x.clone())
        }
    }
}

impl HashPut for MapHashLookup {
    fn put_bytes(&mut self, bs: Vec<u8>) {
        self.map.insert(hash_of_bytes(&bs), bs);
    }
}
