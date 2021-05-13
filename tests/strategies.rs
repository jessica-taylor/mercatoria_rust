use ed25519_dalek::Keypair;
use mercatoria_rust::{blockdata::*, crypto};
use proptest::prelude::*;
use std::collections::BTreeMap;

prop_compose! {
    pub fn account_init_strategy()
            (balance in prop::num::u64::ANY, stake in prop::num::u64::ANY)
            -> (AccountInit, Keypair) {
        let keys = crypto::gen_private_key();
        let public_key = keys.public;
        (AccountInit {
            public_key,
            balance: balance as u128,
            stake: stake as u128
        }, keys)
    }
}

pub fn account_inits(
) -> impl Strategy<Value = (Vec<AccountInit>, BTreeMap<crypto::HashCode, Keypair>)> {
    prop::collection::vec(account_init_strategy(), 0..100).prop_map(|inits_keys| {
        let mut inits = Vec::new();
        let mut map = BTreeMap::new();
        for (init, key) in inits_keys {
            map.insert(crypto::hash(&init.public_key).code, key);
            inits.push(init);
        }
        (inits, map)
    })
}
