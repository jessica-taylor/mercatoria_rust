use std::collections::BTreeMap;

use crate::crypto::{hash_of_bytes, Hash, HashCode};

use anyhow::bail;
use async_trait::*;
use serde::{de::DeserializeOwned, Serialize};

#[async_trait]
pub trait HashLookup: Send + Sync {
    async fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, anyhow::Error>;
    async fn lookup<T: DeserializeOwned + Send>(&self, hash: Hash<T>) -> Result<T, anyhow::Error> {
        Ok(rmp_serde::from_read(
            self.lookup_bytes(hash.code).await?.as_slice(),
        )?)
    }
}

#[async_trait]
pub trait HashPut: Send + Sync {
    async fn put_bytes(&mut self, bs: &[u8]) -> Result<HashCode, anyhow::Error>;
    async fn put<T: Serialize + Send + Sync>(&mut self, val: &T) -> Result<Hash<T>, anyhow::Error> {
        let code = self
            .put_bytes(&rmp_serde::to_vec_named(val).unwrap())
            .await?;
        Ok(Hash {
            code,
            phantom: std::marker::PhantomData,
        })
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

#[async_trait]
impl HashLookup for MapHashLookup {
    async fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, anyhow::Error> {
        match self.map.get(&hash) {
            None => bail!("not found"),
            Some(x) => Ok(x.clone()),
        }
    }
}

#[async_trait]
impl HashPut for MapHashLookup {
    async fn put_bytes(&mut self, bs: &[u8]) -> Result<HashCode, anyhow::Error> {
        let code = hash_of_bytes(&bs);
        self.map.insert(code, bs.to_vec());
        Ok(code)
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
            hl,
            put_values: BTreeMap::new(),
        }
    }
}

#[async_trait]
impl<'a, HL: HashLookup> HashLookup for HashPutOfHashLookup<'a, HL> {
    async fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, anyhow::Error> {
        match self.put_values.get(&hash) {
            Some(x) => Ok(x.clone()),
            None => self.hl.lookup_bytes(hash).await,
        }
    }
}

#[async_trait]
impl<'a, HL: HashLookup> HashPut for HashPutOfHashLookup<'a, HL> {
    async fn put_bytes(&mut self, bs: &[u8]) -> Result<HashCode, anyhow::Error> {
        let code = hash_of_bytes(&bs);
        self.put_values.insert(code, bs.to_vec());
        Ok(code)
    }
}
