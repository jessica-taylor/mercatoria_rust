use mercatoria_rust::{construction::*, crypto};
use proptest::prelude::*;

prop_compose! {
    fn account_init_strategy()
            (balance in prop::num::u64::ANY, stake in prop::num::u64::ANY)
            -> AccountInit {
        let keys = crypto::gen_private_key();
        let public_key = keys.public;
        AccountInit {
            public_key,
            balance: balance as u128,
            stake: stake as u128
        }
    }
}
