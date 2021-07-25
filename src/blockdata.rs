//! The data structures used to construct the blockchain.

use crate::crypto::{self, Hash, HashCode, Signature};
use crate::hashlookup::HashLookup;
use crate::hex_path::{is_postfix, u4, HexPath};
use ed25519_dalek::{Keypair, PublicKey};

use anyhow::{anyhow, bail};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// The children of a radix tree node.  In the vector, the
/// index is the first hex digit of the child's suffix; the
/// value is `None` if there is no child with that starting
/// hex digit; otherwise, the first element in the pair
/// consists of the remaining hex digits.
#[derive(Eq, PartialEq, Debug, Deserialize, Serialize, Clone)]
pub struct RadixChildren<T>(pub [Option<(HexPath, T)>; 16]);

/// The children of a radix hash tree node.
pub type RadixHashChildren<T> = RadixChildren<Hash<T>>;

impl<T> Default for RadixChildren<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> RadixChildren<T> {
    /// Creates `RadixChildren` from a single child.
    fn from_single_child(mut prefix: HexPath, hash: T) -> Option<Self> {
        let mut out = Self::default();
        let c = prefix.0.drain(0..1).next()?.0 as usize;
        out.0[c] = Some((prefix, hash));
        Some(out)
    }

    /// Iterates over children; the items are `(path, child)` pairs,
    /// where the path is the full suffix of the child relative to the parent.
    pub fn iter_entries(&self) -> impl Iterator<Item = (HexPath, &T)> {
        self.0.iter().enumerate().flat_map(|(i, x)| {
            x.as_ref().map(|(path, child)| {
                let mut path2 = path.clone();
                path2.0.insert(0, u4(i as u8));
                (path2, child)
            })
        })
    }

    /// The number of children.
    pub fn len(&self) -> usize {
        self.iter_entries().count()
    }
}

/// A node in a radix hash tree.
#[async_trait]
pub trait RadixHashNode:
    Sized + DeserializeOwned + Clone + Send + Serialize + DeserializeOwned + Sync
{
    /// Gets the children of the node.
    fn get_children(&self) -> &RadixHashChildren<Self>;

    /// Replaces the children of the node.
    async fn replace_children<HL: HashLookup>(
        self,
        hl: &HL,
        new_children: RadixHashChildren<Self>,
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
    /// The hash code of the previous node (`None` if this is the genesis block).
    pub prev: Option<Hash<MainBlock>>,
    /// The version, which is `0` for the genesis block, and increases by `1` each block.
    pub version: u64,
    /// The timestamp of the block creation, as epoch milliseconds rounded based on
    /// `timestamp_period_ms`.
    pub timestamp_ms: i64,
    /// The radix hash tree storing account data.
    pub tree: Hash<QuorumNode>,
    /// The options.
    pub options: Hash<MainOptions>,
    // signer slashes
    // miner slashes
}

/// Options for the blockchain, stored in a `MainBlockBody`.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct MainOptions {
    /// The cost of using a single unit of gas, in currency units.
    pub gas_cost: u128,
    /// The maximum gas per contract operation.
    pub gas_limit: u128,
    /// The period of time by which a new block may be created.
    pub timestamp_period_ms: u32,
    /// The number of signers of a main block.
    pub main_block_signers: u32,
    /// The number of signers who must sign for the main block to be valid.
    pub main_block_signatures_required: u32,
    /// The number of blocks created between creations of new random data.
    pub random_seed_period: u32,
    /// The number of blocks created between reshufflings of quorums.
    pub quorum_period: u32,
    /// The maximum depth at which a quorum may sign a node.
    pub max_quorum_depth: u32,
    /// To be endorsed, there must be some `(a, b)` in this vector such that
    /// there are at least `b` signatures by members of a quorum of size `a`.
    pub quorum_sizes_thresholds: Vec<(u32, u32)>,
}

/// A `MainBlockBody` signed by signers.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct PreSignedMainBlock {
    /// The body of the main block.
    pub body: MainBlockBody,
    /// Signatures by signers.
    pub signatures: Vec<Signature<MainBlockBody>>,
}

impl PreSignedMainBlock {
    pub fn sign(body: MainBlockBody, keys: &Vec<&Keypair>) -> Self {
        let signatures: Vec<Signature<MainBlockBody>> =
            keys.iter().map(|k| crypto::sign(k, body.clone())).collect();
        PreSignedMainBlock { body, signatures }
    }
}

/// A `PreSignedMainBlock` signed by the miner.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct MainBlock {
    /// The body of the main block, signed by signers.
    pub block: PreSignedMainBlock,
    /// The miner's signature.
    pub signature: Signature<PreSignedMainBlock>,
}

impl MainBlock {
    pub fn sign(block: PreSignedMainBlock, key: &Keypair) -> Self {
        let signature = crypto::sign(key, block.clone());
        MainBlock { block, signature }
    }
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
    /// The hash code of the previous `MainBlock` at time of creation, `None` if
    /// this is part of the genesis block's tree.
    pub last_main: Option<Hash<MainBlock>>,
    /// The path from the top node to this one.
    pub path: HexPath,
    /// The children of this node.
    pub children: RadixHashChildren<QuorumNode>,
    /// For leaf nodes, the data tree of the corresponding account.
    pub data_tree: Option<Hash<DataNode>>,
    /// For leaf nodes, the action that was just applied to this account.
    pub new_action: Option<Hash<Action>>,
    /// The prize for including this node, not including prizes of ddescendents.
    pub prize: u128,
    /// Statistics for this node.
    pub stats: QuorumNodeStats,
}

/// A `QuorumNodeBody` that may be signed.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct QuorumNode {
    /// The body of the `QuorumNode`.
    pub body: QuorumNodeBody,
    /// Signatures by quorum members.
    pub signatures: Option<Hash<Vec<Signature<QuorumNodeBody>>>>,
}

impl QuorumNodeBody {
    // TODO: grep for QuorumNode { ... None }, replace
    /// Creates a `QuorumNode` with no signatures.
    pub fn into_unsigned(self) -> QuorumNode {
        QuorumNode {
            body: self,
            signatures: None,
        }
    }
}

#[async_trait]
impl RadixHashNode for QuorumNode {
    fn get_children(&self) -> &RadixHashChildren<Self> {
        &self.body.children
    }

    async fn replace_children<HL: HashLookup>(
        mut self,
        hl: &HL,
        new_children: RadixHashChildren<Self>,
    ) -> Result<QuorumNode, anyhow::Error> {
        self.body.children = new_children;
        self.signatures = None;
        self.body.stats = QuorumNodeStats::zero();
        self.body.stats.prize = self.body.prize;
        self.body.stats.new_nodes = 1;
        for (suffix, hash_child) in self.body.children.iter_entries() {
            let child = hl.lookup(*hash_child).await?;
            if child.body.path.0 != [&self.body.path[..], &suffix[..]].concat() {
                bail!(
                    "quorum child node has wrong path; child path is {}, parent path is {}, suffix is {}",
                    child.body.path,
                    self.body.path,
                    suffix
                );
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
        if !is_postfix(&child.0[..], &child_node.body.path[..]) {
            bail!("quorum child node has wrong postfix");
        }
        let mut stats = child_node.body.stats;
        stats.new_nodes += 1;
        Ok(QuorumNode {
            signatures: None,
            body: QuorumNodeBody {
                last_main: child_node.body.last_main,
                path: HexPath(
                    child_node.body.path[..child_node.body.path.len() - child.0.len()].to_vec(),
                ),
                children: RadixHashChildren::from_single_child(child.0, child.1)
                    .ok_or_else(|| anyhow!("child hex path must not be empty"))?,
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
    pub children: RadixHashChildren<DataNode>,
}

#[async_trait]
impl RadixHashNode for DataNode {
    fn get_children(&self) -> &RadixHashChildren<Self> {
        &self.children
    }

    async fn replace_children<HL: HashLookup>(
        mut self,
        _hl: &HL,
        new_children: RadixHashChildren<Self>,
    ) -> Result<DataNode, anyhow::Error> {
        self.children = new_children;
        Ok(self)
    }

    async fn from_single_child<HL: HashLookup>(
        _hl: &HL,
        child: (HexPath, Hash<DataNode>),
    ) -> Result<DataNode, anyhow::Error> {
        let children = RadixHashChildren::from_single_child(child.0, child.1)
            .ok_or_else(|| anyhow!("child hex path must not be empty"))?;

        Ok(DataNode {
            field: None,
            children,
        })
    }
}

/// An action that may be run on an account.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct Action {
    /// The hash code of the last main block.
    pub last_main: Hash<MainBlock>,
    /// The fee paid for this action.
    pub fee: u128,
    /// The command to run, e.g. b"send".
    pub command: Vec<u8>,
    /// The arguments of the command.
    pub args: Vec<Vec<u8>>,
}

/// Information about a send transaction.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct SendInfo {
    /// The hash code of the last main block.
    pub last_main: Hash<MainBlock>,
    /// The sender of this send.
    pub sender: HashCode,
    /// The recipient of this send.
    pub recipient: HashCode,
    /// The amount of money sent.
    pub send_amount: u128,
    /// Information about how to initialize a new account created through this transaction.
    pub initialize_spec: Option<Hash<Vec<u8>>>,
    /// A message sent with this transnaction.
    pub message: Vec<u8>,
}

/// Information to initialize an account in the genesis block.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
pub struct AccountInit {
    /// The public key of the account.
    pub public_key: PublicKey,
    /// The initial balance.
    pub balance: u128,
    /// The initial stake.
    pub stake: u128,
}
