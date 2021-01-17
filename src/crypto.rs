use std::convert::TryInto;
use rand::rngs::ThreadRng;

use rsa::{PublicKey, RSAPrivateKey, RSAPublicKey, PaddingScheme};
use sha2::{Sha256, Digest};

type HashCode = [u8; 32];

fn hash_of_bytes(bs: &[u8]) -> HashCode {
    let mut hasher = Sha256::new();
    hasher.update(bs);
    hasher.finalize().as_slice().try_into().expect("digest has wrong length")
}

fn gen_private_key() -> RSAPrivateKey {
    let mut rng = ThreadRng::default();
    let bits = 2048;
    RSAPrivateKey::new(&mut rng, bits).expect("failed to generate private key")
}

fn private_to_public_key(k: &RSAPrivateKey) -> RSAPublicKey {
    RSAPublicKey::from(k)
}
