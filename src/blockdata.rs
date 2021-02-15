use crate::crypto::{Hash, HashCode, Signature};
use crate::hex_path::HexPath;

use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct MainBlockBody {
    pub prev: Option<Hash<MainBlock>>,
    pub version: u64,
    pub timestamp_ms: i64,
    pub tree: Hash<QuorumNode>,
    pub options: Hash<MainOptions>,
    // signer slashes
    // miner slashes
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
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

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct PreSignedMainBlock {
    pub body: MainBlockBody,
    pub signatures: Vec<Signature<MainBlockBody>>,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct MainBlock {
    pub block: PreSignedMainBlock,
    pub signature: Signature<PreSignedMainBlock>,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct QuorumNodeStats {
    pub fee: u128,
    pub gas: u128,
    pub new_nodes: u64,
    pub prize: u128,
    pub stake: u128,
}


impl QuorumNodeStats {
    /// `QuorumNodeStats` with all fields set to 0.
    pub fn zero() -> QuorumNodeStats {
        QuorumNodeStats {
            fee: 0,
            gas: 0,
            new_nodes: 0,
            prize: 0,
            stake: 0,
        }
    }

    /// Adds fields in two `QuorumNodeStats`s.
    pub fn plus(self: &QuorumNodeStats, other: &QuorumNodeStats) -> QuorumNodeStats {
        QuorumNodeStats {
            fee: self.fee + other.fee,
            gas: self.gas + other.gas,
            new_nodes: self.new_nodes + other.new_nodes,
            prize: self.prize + other.prize,
            stake: self.stake + other.stake,
        }
    }
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct QuorumNodeBody {
    pub last_main: Option<Hash<MainBlock>>,
    pub path: HexPath,
    pub children: Vec<(HexPath, Hash<QuorumNode>)>,
    pub data_tree: Option<Hash<DataNode>>,
    pub new_action: Option<Hash<Action>>,
    pub prize: u128,
    pub stats: QuorumNodeStats,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct QuorumNode {
    pub body: QuorumNodeBody,
    pub signatures: Option<Hash<Vec<Signature<QuorumNodeBody>>>>,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct DataNode {
    pub field: Option<Vec<u8>>,
    pub children: Vec<(HexPath, Hash<DataNode>)>,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct Action {
    pub last_main: Hash<MainBlock>,
    pub fee: u128,
    pub command: Vec<u8>,
    pub args: Vec<Vec<u8>>,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct SendInfo {
    pub last_main: Hash<MainBlock>,
    pub sender: HashCode,
    pub recipient: Option<HashCode>,
    pub send_amount: u128,
    pub initialize_spec: Option<Hash<Vec<u8>>>,
    pub message: Vec<u8>,
}
