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
