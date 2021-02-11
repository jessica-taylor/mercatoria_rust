use std::collections::BTreeMap;

use crate::crypto::{hash_of_bytes, Hash, HashCode};

use serde::{de::DeserializeOwned, Serialize};

pub trait HashLookup {
    fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, String>;
    fn lookup<T: DeserializeOwned>(&self, hash: Hash<T>) -> Result<T, String> {
        rmp_serde::from_read(self.lookup_bytes(hash.code)?.as_slice()).map_err(|e| e.to_string())
    }
}

pub trait HashPut {
    fn put_bytes(&mut self, bs: &[u8]) -> HashCode;
    fn put<T: Serialize>(&mut self, val: &T) -> Hash<T> {
        let code = self.put_bytes(&rmp_serde::to_vec_named(val).unwrap());
        Hash {
            code,
            phantom: std::marker::PhantomData,
        }
    }
}

pub struct MapHashLookup {
    map: BTreeMap<HashCode, Vec<u8>>,
}

impl MapHashLookup {
    pub fn new() -> MapHashLookup {
        MapHashLookup {
            map: BTreeMap::new(),
        }
    }
}

impl HashLookup for MapHashLookup {
    fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, String> {
        match self.map.get(&hash) {
            None => Err("not found".to_string()),
            Some(x) => Ok(x.clone()),
        }
    }
}

impl HashPut for MapHashLookup {
    fn put_bytes(&mut self, bs: &[u8]) -> HashCode {
        let code = hash_of_bytes(&bs);
        self.map.insert(code, bs.to_vec());
        code
    }
}
