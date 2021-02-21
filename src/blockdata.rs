use crate::crypto::{Hash, HashCode, Signature};
use crate::hashlookup::HashLookup;
use crate::hex_path::{is_postfix, HexPath};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use async_trait::async_trait;

use anyhow::bail;

/// A node in a radix hash tree.
#[async_trait]
pub trait RadixHashNode:
    Sized + DeserializeOwned + Clone + Send + Serialize + DeserializeOwned + Sync
{
    /// Gets the children of the node.
    fn get_children(&self) -> &Vec<(HexPath, Hash<Self>)>;

    /// Replaces the children of the node.
    async fn replace_children<HL: HashLookup>(
        self,
        hl: &HL,
        new_children: Vec<(HexPath, Hash<Self>)>,
    ) -> Result<Self, anyhow::Error>;

    /// Creates a node with a single child.
    async fn from_single_child<HL: HashLookup>(
        hl: &HL,
        child: (HexPath, Hash<Self>),
    ) -> Result<Self, anyhow::Error>;
}

/// The body of a `MainBlock`.  It doesn't contain signatures.
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

/// Options for the blockchain, stored in a `MainBlockBody`.
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

/// A `MainBlockBody` signed by signers.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct PreSignedMainBlock {
    pub body: MainBlockBody,
    pub signatures: Vec<Signature<MainBlockBody>>,
}

/// A `PreSignedMainBlock` signed by the miner.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct MainBlock {
    pub block: PreSignedMainBlock,
    pub signature: Signature<PreSignedMainBlock>,
}

/// Statistics for a quorum node.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct QuorumNodeStats {
    /// Total fee at or under this node, for this iteration.
    pub fee: u128,
    /// Total gas at or under this node, for this iteration.
    pub gas: u128,
    /// New nodes created at or under this node, for this iteration.
    pub new_nodes: u64,
    /// Total prize at or under this node, for this iteration.
    pub prize: u128,
    /// Total stake at or under this node.
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

/// The body of a `QuorumNode`.  It does not contain signatures.
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

/// A `QuorumNodeBody` that may be signed.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct QuorumNode {
    pub body: QuorumNodeBody,
    pub signatures: Option<Hash<Vec<Signature<QuorumNodeBody>>>>,
}

#[async_trait]
impl RadixHashNode for QuorumNode {
    fn get_children(&self) -> &Vec<(HexPath, Hash<QuorumNode>)> {
        &self.body.children
    }

    async fn replace_children<HL: HashLookup>(
        mut self,
        hl: &HL,
        new_children: Vec<(HexPath, Hash<QuorumNode>)>,
    ) -> Result<QuorumNode, anyhow::Error> {
        self.body.children = new_children;
        self.signatures = None;
        self.body.stats = QuorumNodeStats::zero();
        self.body.stats.prize = self.body.prize;
        self.body.stats.new_nodes = 1;
        for (suffix, hash_child) in &self.body.children {
            let child = hl.lookup(*hash_child).await?;
            if child.body.path != [&self.body.path[..], &suffix[..]].concat() {
                bail!("quorum child node has wrong path");
            }
            self.body.stats.stake += child.body.stats.stake;
            if self.body.last_main == child.body.last_main {
                self.body.stats.fee += child.body.stats.fee;
                self.body.stats.gas += child.body.stats.gas;
                self.body.stats.prize += child.body.stats.prize;
                self.body.stats.new_nodes += child.body.stats.new_nodes;
            }
        }
        Ok(self)
    }

    async fn from_single_child<HL: HashLookup>(
        hl: &HL,
        child: (HexPath, Hash<QuorumNode>),
    ) -> Result<QuorumNode, anyhow::Error> {
        let child_node = hl.lookup(child.1).await?;
        if !is_postfix(&child.0, &child_node.body.path) {
            bail!("quorum child node has wrong postfix");
        }
        let mut stats = child_node.body.stats;
        stats.new_nodes += 1;
        Ok(QuorumNode {
            signatures: None,
            body: QuorumNodeBody {
                last_main: child_node.body.last_main,
                path: child_node.body.path[..child_node.body.path.len() - child.0.len()].to_vec(),
                children: vec![child],
                data_tree: None,
                new_action: None,
                prize: 0,
                stats,
            },
        })
    }
}

/// A radix hash node containing account data.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct DataNode {
    pub field: Option<Vec<u8>>,
    pub children: Vec<(HexPath, Hash<DataNode>)>,
}

#[async_trait]
impl RadixHashNode for DataNode {
    fn get_children(&self) -> &Vec<(HexPath, Hash<DataNode>)> {
        &self.children
    }

    async fn replace_children<HL: HashLookup>(
        mut self,
        hl: &HL,
        new_children: Vec<(HexPath, Hash<DataNode>)>,
    ) -> Result<DataNode, anyhow::Error> {
        self.children = new_children;
        Ok(self)
    }

    async fn from_single_child<HL: HashLookup>(
        hl: &HL,
        child: (HexPath, Hash<DataNode>),
    ) -> Result<DataNode, anyhow::Error> {
        Ok(DataNode {
            field: None,
            children: vec![child],
        })
    }
}

/// An action that may be run on an account.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct Action {
    pub last_main: Hash<MainBlock>,
    pub fee: u128,
    pub command: Vec<u8>,
    pub args: Vec<Vec<u8>>,
}

/// Information about a send transaction.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct SendInfo {
    pub last_main: Hash<MainBlock>,
    pub sender: HashCode,
    pub recipient: Option<HashCode>,
    pub send_amount: u128,
    pub initialize_spec: Option<Hash<Vec<u8>>>,
    pub message: Vec<u8>,
}
