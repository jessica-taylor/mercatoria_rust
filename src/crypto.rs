use std::convert::TryInto;

use openssl::{hash::MessageDigest, pkey::{PKey, Public, Private}, rsa::Rsa, sign::{Signer, Verifier}};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};



pub type HashCode = [u8; 32];

#[derive(Serialize, Deserialize, Debug)]
pub struct Hash<T> {
    pub code: HashCode,
    phantom: std::marker::PhantomData<T>,
}

pub fn hash_of_bytes(bs: &[u8]) -> HashCode {
    let mut hasher = Sha256::new();
    hasher.update(bs);
    hasher.finalize().as_slice().try_into().expect("digest has wrong length")
}

pub fn hash<T : Serialize, Deserialize>(v: T) -> Hash<T> {
    Hash {code: hash_of_bytes(serde_cbor::to_vec(&v).unwrap().as_slice()), phantom: std::marker::PhantomData}
}


pub type Sig = Vec<u8>;

#[derive(Serialize, Deserialize, Debug)]
pub struct Signature<T> {
    pub sig: Sig,
    phantom: std::marker::PhantomData<T>,
}

pub fn gen_private_key() -> Rsa<Private> {
    Rsa::generate(2048).unwrap()
}

pub fn to_public_key(private: &Rsa<Private>) -> Rsa<Public> {
    Rsa::public_key_from_pem(private.private_key_to_pem().unwrap().as_slice()).unwrap()
}

pub fn sign_bytes(key: &Rsa<Private>, msg: &[u8]) -> Sig {
    let pkey = PKey::from_rsa(key.clone()).unwrap();
    let mut signer = Signer::new(MessageDigest::sha256(), &pkey).unwrap();
    signer.update(msg).unwrap();
    signer.sign_to_vec().unwrap()
}

pub fn verify_bytes(key: &Rsa<Public>, msg: &[u8], sig: Sig) -> bool {
    let pkey = PKey::from_rsa(key.clone()).unwrap();
    let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey).unwrap();
    verifier.update(msg).unwrap();
    verifier.verify(&sig).unwrap()
}
