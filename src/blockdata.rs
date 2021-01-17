use crate::crypto::{Hash, HashCode, Signature};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct MainBlockBody {
    prev: Option<Hash<MainBlockBody>>,
    version: u64,
    timestamp_ms: i64,
    tree: Option<HashCode>,
    options: HashCode,
    // signer slashes
    // miner slashes
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PreSignedMainBlock {
    body: MainBlockBody,
    signatures: Vec<Signature<MainBlockBody>>,
}


