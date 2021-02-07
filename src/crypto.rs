use ed25519_dalek::{Keypair, PublicKey, Signature as Sig, Signer, Verifier};
use rand::prelude::*;
use serde::{Deserialize, Serialize};

use crate::hex_path::HexPath;

/// A blake3 hash code.
pub type HashCode = [u8; 32];

/// A SHA256 hash code that is tagged as being a hash code of a particular serializable type.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Hash<T> {
    pub code: HashCode,
    phantom: std::marker::PhantomData<T>,
}

/// Gets the SHA256 hash code of a byte array.
pub fn hash_of_bytes(bs: &[u8]) -> HashCode {
    blake3::hash(bs).into()
}

/// Gets the SHA256 hash of a serialiable data value.
pub fn hash<T: Serialize, Deserialize>(v: T) -> Hash<T> {
    Hash {
        code: hash_of_bytes(serde_cbor::to_vec(&v).unwrap().as_slice()),
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
        hc[i] = path[2 * i].value * 16 + path[2 * i + 1].value;
    }
    hc
}

/// A RSA signature that is tagged as being the signature of a particular serializable type.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Signature<T> {
    pub sig: Sig,
    phantom: std::marker::PhantomData<T>,
}

/// Generates a RSA private key.
pub fn gen_private_key() -> Keypair {
    Keypair::generate(&mut thread_rng())
}

// pub fn gen_private_key() -> Rsa<Private> {
//     Rsa::generate(2048).unwrap()
// }

/// Converts a RSA private key to a public key.
pub fn to_public_key(private: &Keypair) -> PublicKey {
    private.public
}

/// Signs a byte array with a given RSA key.
pub fn sign_bytes(key: &Keypair, msg: &[u8]) -> Sig {
    key.sign(msg)
}

/// Verifies that a given signature of a given byte array is valid.
pub fn verify_sig_bytes(key: &PublicKey, msg: &[u8], sig: &Sig) -> bool {
    key.verify(msg, sig).is_ok()
}

/// Signs a serializable value with a given RSA key.
pub fn sign<T: Serialize, Deserialize>(key: &Keypair, msg: T) -> Signature<T> {
    Signature {
        sig: sign_bytes(key, &serde_cbor::to_vec(&msg).unwrap()),
        phantom: std::marker::PhantomData,
    }
}

/// Verifies that a given signature of a given serializable value is valid.
pub fn verify_sig<T: Serialize, Deserialize>(key: &PublicKey, msg: &T, sig: &Signature<T>) -> bool {
    verify_sig_bytes(key, &serde_cbor::to_vec(msg).unwrap(), &sig.sig)
}
