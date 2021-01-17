use sha2::{Sha256, Digest};
use std::convert::TryInto;

type HashCode = [u8; 32];

fn hash_of_bytes(bs: &[u8]) -> HashCode {
    let mut hasher = Sha256::new();
    hasher.update(bs);
    hasher.finalize().as_slice().try_into().expect("digest has wrong length")
}
