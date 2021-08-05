//! Cryptographic types and operations.

use ed25519_dalek::{Keypair, PublicKey, Signature as Sig, Signer, Verifier};
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use crate::hex_path::HexPath;

/// A blake3 hash code.
pub type HashCode = [u8; 32];

/// Xors two hash codes.
pub fn xor_hash_codes(a: HashCode, b: HashCode) -> HashCode {
    let mut c = [0; 32];
    for i in 0..32 {
        c[i] = a[i] ^ b[i];
    }
    c
}

/// A blake3 hash code that is tagged as being a hash code of a particular serializable type.
#[derive(Serialize, Deserialize, Debug)]
pub struct Hash<T> {
    /// The hash code.
    pub code: HashCode,
    /// Phantom data for the type `T`.
    pub(crate) phantom: std::marker::PhantomData<fn() -> T>,
}

impl<T> Clone for Hash<T> {
    fn clone(&self) -> Self {
        Self {
            code: self.code,
            phantom: PhantomData,
        }
    }
}

impl<T> Copy for Hash<T> {}

impl<T> PartialEq for Hash<T> {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
    }
}

impl<T> Eq for Hash<T> {}

/// Gets the blake3 hash code of a byte array.
pub fn hash_of_bytes(bs: &[u8]) -> HashCode {
    blake3::hash(bs).into()
}

/// Gets the blake3 hash of a serialiable data.0.
pub fn hash<T: Serialize>(v: &T) -> Hash<T> {
    Hash {
        code: hash_of_bytes(rmp_serde::to_vec_named(v).unwrap().as_slice()),
        phantom: std::marker::PhantomData,
    }
}

/// Converts a `HexPath` that is 64 digits long to a `HashCode`.
pub fn path_to_hash_code(path: HexPath) -> HashCode {
    if path.len() != 64 {
        panic!("path to convert to hash code must be 64 bytes");
    }
    let mut hc = [0; 32];
    for i in 0..32 {
        hc[i] = path[2 * i].0 * 16 + path[2 * i + 1].0;
    }
    hc
}

/// An ed25519 signature that is tagged as being the signature of a particular serializable type.
#[derive(Serialize, Deserialize, Debug)]
pub struct Signature<T> {
    /// The signature.
    pub sig: Sig,
    /// THe public key.
    pub key: PublicKey,
    /// Phantom data for the type `T`.
    phantom: std::marker::PhantomData<T>,
}

impl<T> Signature<T> {
    pub fn account(&self) -> HashCode {
        hash(&self.key).code
    }
}

impl<T> Clone for Signature<T> {
    fn clone(&self) -> Self {
        Self {
            sig: self.sig,
            key: self.key,
            phantom: PhantomData,
        }
    }
}

impl<T> Copy for Signature<T> {}

impl<T> PartialEq for Signature<T> {
    fn eq(&self, other: &Self) -> bool {
        self.sig == other.sig
    }
}

impl<T> Eq for Signature<T> {}

/// Generates a ed25519 private key.
pub fn gen_private_key() -> Keypair {
    Keypair::generate(&mut thread_rng())
}

/// Signs a serializable.0 with a given ed25519 key.
pub fn sign<T: Serialize>(key: &Keypair, msg: T) -> Signature<T> {
    Signature {
        sig: key.sign(&rmp_serde::to_vec_named(&msg).unwrap()),
        key: key.public,
        phantom: std::marker::PhantomData,
    }
}

/// Verifies that a given signature of a given serializable.0 is valid.
pub fn verify_sig<T: Serialize>(msg: &T, sig: &Signature<T>) -> bool {
    sig.key
        .verify(&rmp_serde::to_vec_named(msg).unwrap(), &sig.sig)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify() {
        const MSG: &str = "This is a test message to be signed";
        let keys = gen_private_key();
        let signature = sign(&keys, MSG);
        assert!(verify_sig(&MSG, &signature), "Invalid signature")
    }
}
