use std::collections::BTreeMap;

use crate::crypto::{Hash, HashCode, hash_of_bytes};

use serde::{Serialize, de::DeserializeOwned};



pub trait HashLookup {
    fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, String>;
    fn lookup<T: DeserializeOwned>(&self, hash: Hash<T>) -> Result<T, String> {
        serde_cbor::from_slice(&self.lookup_bytes(hash.code)?).map_err(|e| e.to_string())
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
    fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, String> {
        match self.map.get(&hash) {
            None => Err("not found".to_string()),
            Some(x) => Ok(x.clone())
        }
    }
}

impl HashPut for MapHashLookup {
    fn put_bytes(&mut self, bs: Vec<u8>) {
        self.map.insert(hash_of_bytes(&bs), bs);
    }
}
