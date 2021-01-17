use std::convert::TryInto;

use openssl::{hash::MessageDigest, pkey::{PKey, Public, Private}, rsa::Rsa, sign::{Signer, Verifier}};
use sha2::{Sha256, Digest};



pub type HashCode = [u8; 32];

pub fn hash_of_bytes(bs: &[u8]) -> HashCode {
    let mut hasher = Sha256::new();
    hasher.update(bs);
    hasher.finalize().as_slice().try_into().expect("digest has wrong length")
}

pub struct Hash<T> {
    code: HashCode,
    phantom: std::marker::PhantomData<T>,
}

pub fn gen_private_key() -> Rsa<Private> {
    Rsa::generate(2048).unwrap()
}

pub fn to_public_key(private: &Rsa<Private>) -> Rsa<Public> {
    Rsa::public_key_from_pem(private.private_key_to_pem().unwrap().as_slice()).unwrap()
}

pub fn sign(key: &Rsa<Private>, msg: &[u8]) -> Vec<u8> {
    let pkey = PKey::from_rsa(key.clone()).unwrap();
    let mut signer = Signer::new(MessageDigest::sha256(), &pkey).unwrap();
    signer.update(msg).unwrap();
    signer.sign_to_vec().unwrap()
}

pub fn verify(key: &Rsa<Public>, msg: &[u8], sig: Vec<u8>) -> bool {
    let pkey = PKey::from_rsa(key.clone()).unwrap();
    let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey).unwrap();
    verifier.update(msg).unwrap();
    verifier.verify(&sig).unwrap()
}
