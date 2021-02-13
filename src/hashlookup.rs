use std::collections::BTreeMap;

use crate::crypto::{hash_of_bytes, Hash, HashCode};

use serde::{de::DeserializeOwned, Serialize};
use anyhow::bail;

/// A trait for looking up values by their hash code.
pub trait HashLookup {
    /// Looks up a byte array by its hash code.
    fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, anyhow::Error>;

    /// Looks up a serializable value by the hash code of its serialization.
    fn lookup<T: DeserializeOwned>(&self, hash: Hash<T>) -> Result<T, anyhow::Error> {
        Ok(rmp_serde::from_read(self.lookup_bytes(hash.code)?.as_slice())?)
    }
}

/// A trait for inserting new values associated with their hash codes.
pub trait HashPut {
    /// Inserts a byte array, associating it with its hash code.
    fn put_bytes(&mut self, bs: &[u8]) -> HashCode;

    /// Inserts a serializable value, associating it with its hash code.
    fn put<T: Serialize>(&mut self, val: &T) -> Hash<T> {
        let code = self.put_bytes(&rmp_serde::to_vec_named(val).unwrap());
        Hash {
            code,
            phantom: std::marker::PhantomData,
        }
    }
}

/// A implementation of `HashLookup` and `HashPut` that stores a map.
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
    fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, anyhow::Error> {
        match self.map.get(&hash) {
            None => bail!("not found"),
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

/// A `HashLookup + HashPut` implementation made of an underlying
/// `HashLookup` and a cache of put values.
pub struct HashPutOfHashLookup<'a, HL: HashLookup> {
    pub hl: &'a HL,
    pub put_values: BTreeMap<HashCode, Vec<u8>>,
}

impl<'a, HL: HashLookup> HashPutOfHashLookup<'a, HL> {
    /// Creates a new `HashPutOfHashLookup` from an underlying `HashLookup`.
    pub fn new(hl: &'a HL) -> HashPutOfHashLookup<'a, HL> {
        HashPutOfHashLookup {
            hl: hl,
            put_values: BTreeMap::new()
        }
    }
}

impl<'a, HL: HashLookup> HashLookup for HashPutOfHashLookup<'a, HL> {
    fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, anyhow::Error> {
        match self.put_values.get(&hash) {
            Some(x) => Ok(x.clone()),
            None => self.hl.lookup_bytes(hash)
        }
    }
}

impl<'a, HL: HashLookup> HashPut for HashPutOfHashLookup<'a, HL> {
    fn put_bytes(&mut self, bs: &[u8]) -> HashCode {
        let code = hash_of_bytes(&bs);
        self.put_values.insert(code, bs.to_vec());
        code
    }
}
