use crate::crypto::{Hash, HashCode, Signature};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MainBlockBody {
    pub prev: Option<Hash<MainBlockBody>>,
    pub version: u64,
    pub timestamp_ms: i64,
    pub tree: Hash<QuorumNode>,
    pub options: Hash<MainOptions>,
    // signer slashes
    // miner slashes
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MainOptions {
    pub gas_cost: u128,
    pub gas_limit: u128,
    pub timestamp_period_ms: u32,
    pub main_block_signers: u32,
    pub main_block_signatures_required: u32,
    pub random_seed_period: u32,
    pub quorum_period: u32,
    pub max_quorum_depth: u32,
    pub quorum_sizes_thresholds: Vec<(u32, u32)>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PreSignedMainBlock {
    pub body: MainBlockBody,
    pub signatures: Vec<Signature<MainBlockBody>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MainBlock {
    pub block: PreSignedMainBlock,
    pub signature: Signature<PreSignedMainBlock>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuorumNodeBody {
    pub last_main: Option<Hash<MainBlock>>,
    pub path: Vec<u8>,
    pub children: Vec<(Vec<u8>, Hash<QuorumNode>)>,
    pub data_tree: Option<Hash<DataNode>>,
    pub new_action: Option<Hash<Action>>,
    pub prize: u128,
    pub new_nodes: u64,
    pub total_fee: u128,
    pub total_gas: u128,
    pub total_prize: u128,
    pub total_stake: u128,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuorumNode {
    pub body: QuorumNodeBody,
    pub signatures: Option<Hash<Vec<Signature<QuorumNodeBody>>>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DataNode {
    pub value: Option<Vec<u8>>,
    pub children: Vec<(Vec<u8>, Hash<DataNode>)>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Action {
    pub last_main: Hash<MainBlock>,
    pub fee: u128,
    pub command: Vec<u8>,
    pub args: Vec<Vec<u8>>,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SendInfo {
    pub last_main: Hash<MainBlock>,
    pub sender: HashCode,
    pub recipient: Option<HashCode>,
    pub send_amount: u128,
    pub initialize_spec: Option<Hash<Vec<u8>>>,
    pub message: Vec<u8>,
}
