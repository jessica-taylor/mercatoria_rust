use crate::crypto::{Hash, HashCode, Signature};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct MainBlockBody {
    prev: Option<Hash<MainBlockBody>>,
    version: u64,
    timestamp_ms: i64,
    tree: Option<Hash<QuorumNode>>,
    options: HashCode,
    // signer slashes
    // miner slashes
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PreSignedMainBlock {
    body: MainBlockBody,
    signatures: Vec<Signature<MainBlockBody>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MainBlock {
    block: PreSignedMainBlock,
    signature: Signature<PreSignedMainBlock>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct QuorumNodeBody {
    last_main: Option<Hash<MainBlock>>,
    path: Vec<u8>,
    children: Vec<(Vec<u8>, Hash<QuorumNode>)>,
    data_tree: Option<HashCode>,
    new_action: Option<HashCode>,
    prize: u128,
    new_nodes: u64,
    total_fee: u128,
    total_gas: u128,
    total_prize: u128,
    total_stake: u128,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct QuorumNode {
    body: QuorumNodeBody,
    signatures: Option<Hash<Vec<Signature<QuorumNodeBody>>>>,
}

