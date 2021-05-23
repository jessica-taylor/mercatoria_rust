//! Messages that network nodes may receive.
use crate::blockdata::{Action, MainBlock, MainBlockBody, QuorumNode, QuorumNodeBody};
use crate::crypto::{Hash, HashCode, Signature};
use crate::hex_path::HexPath;
use crate::network::Network;

pub type MessageId = u64;

/// A reply to a `Message`.
pub enum Reply {
    /// Reply to `StoreRequest`.
    StoreReply(Option<Vec<u8>>),
    /// Reply to `BestMainRequest`.
    BestMainReply(Hash<MainBlock>),
}

/// The content of a message.
pub enum MessageContent {
    /// A reply to another message.
    Reply(MessageId, Reply),
    /// Requests a value from the store by its hash code.
    StoreRequest(HashCode),
    /// Puts a value in the store.
    StorePut(Vec<u8>),
    /// Requests the best `MainBlock`.
    BestMainRequest,
    /// Notifies of a new candidate most recent `MainBlock`.
    NewBestMain(Hash<MainBlock>),
    /// Notifies of creation of a new quorum tree.
    NextTree(Hash<QuorumNode>),
    /// Notifies of a signature to a `MainBlockBody`.
    MainSignature(MainBlockBody, Signature<MainBlockBody>),
    /// Notifies of an action undertaken by a given account.
    Action(HashCode, Action),
    /// Notifies of a valid `QuorumNodeBody`.
    ValidQuorumNodeBody(QuorumNodeBody),
    /// Notifies of a signature of a `QuorumNodeBody`.
    QuorumSignature(QuorumNodeBody, Signature<QuorumNodeBody>),
    /// Notifies of an endorsed `QuorumNode` to be included in the quorum tree.
    EndorsedQuorumNode(HexPath, Hash<QuorumNode>),
}

/// A message containing extra identifying information.
pub struct FullMessage<N: Network> {
    /// The content of the message.
    pub content: MessageContent,
    /// The sender's `Pid`.
    pub sender: N::Pid,
    /// The ID of the message, ensuring uniqueness.
    pub id: MessageId,
}
