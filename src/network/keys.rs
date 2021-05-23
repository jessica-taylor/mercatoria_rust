//! Storing of cryptographic keys.

use super::role::Role;
use super::Network;
use crate::crypto::{hash, sign, HashCode, Signature};
use ed25519_dalek::Keypair;
use serde::Serialize;

/// Stores cryptographic keys.
struct Keys {
    /// The key pair.
    pub keypair: Keypair,
}

impl<N: Network> Role<N> for Keys {}

impl Keys {
    /// Creates a new `Keys`.
    fn new(keypair: Keypair) -> Keys {
        Keys { keypair }
    }

    /// Gets the account corresponding to the `Keys`.
    fn this_account(&self) -> HashCode {
        hash(&self.keypair.public).code
    }

    /// Signs a message.
    fn sign<T: Serialize>(&self, msg: T) -> Signature<T> {
        sign(&self.keypair, msg)
    }
}
