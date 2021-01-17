use std::convert::TryInto;

use openssl::{hash::MessageDigest, pkey::Private, rsa::Rsa, sign::{Signer, Verifier}};
use sha2::{Sha256, Digest};



type HashCode = [u8; 32];

fn hash_of_bytes(bs: &[u8]) -> HashCode {
    let mut hasher = Sha256::new();
    hasher.update(bs);
    hasher.finalize().as_slice().try_into().expect("digest has wrong length")
}

fn gen_key_pair() -> Rsa<Private> {
    Rsa::generate(2048).unwrap()
}


