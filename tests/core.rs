
use mercatoria_rust::blockdata::{
    AccountInit, Action, DataNode, MainBlock, MainBlockBody, MainOptions, PreSignedMainBlock,
    QuorumNode, QuorumNodeBody, QuorumNodeStats, RadixChildren,
};
use mercatoria_rust::construction::{best_super_node, genesis_block_body, next_main_block_body};
use mercatoria_rust::crypto::hash;
use mercatoria_rust::hashlookup::MapHashLookup;

use mercatoria_rust::state_machine::{genesis_state, get_main_state};

use mercatoria_rust::verification::verify_valid_main_block_body;
use proptest::prelude::*;

mod strategies;
use strategies::*;

// fn arb_init() -> impl Strategy<Value = AccountInit> {
//
// }

async fn test_genesis_block(
    inits: &Vec<AccountInit>,
    mut timestamp_ms: i64,
    opts: MainOptions,
) -> (MapHashLookup, MainBlockBody) {
    timestamp_ms =
        (timestamp_ms % (opts.timestamp_period_ms as i64)) * (opts.timestamp_period_ms as i64);
    let hash_opts = hash(&opts);
    let mut hl = MapHashLookup::new();
    let main = genesis_block_body(&mut hl, &inits, timestamp_ms, opts)
        .await
        .unwrap();
    assert_eq!(None, main.prev);
    assert_eq!(0, main.version);
    assert_eq!(timestamp_ms, main.timestamp_ms);
    assert_eq!(hash_opts, main.options);
    let expected_state = genesis_state(&inits).await;
    let actual_state = get_main_state(&hl, &main).await.unwrap();
    assert_eq!(expected_state, actual_state);
    verify_valid_main_block_body(&hl, &main).await.unwrap();
    (hl, main)
}

async fn test_send_and_receive(
    hl: &mut MapHashLookup,
    start_main: &MainBlockBody,
    _sender_ix: u32,
    _receiver_ix: u32,
    _amount: u64,
) {
    let _start_state = get_main_state(hl, start_main).await.unwrap();
    // let acct_states =
}

fn test_options() -> MainOptions {
    MainOptions {
        gas_cost: 1,
        gas_limit: u128::max_value(),
        timestamp_period_ms: 10,
        main_block_signers: 10,
        main_block_signatures_required: 10,
        random_seed_period: 10,
        quorum_period: 90,
        max_quorum_depth: 16,
        quorum_sizes_thresholds: vec![(3, 4)],
    }
}

proptest! {
    #[test]
    fn proptest_genesis_block(
        inits in account_inits(),
        timestamp_ms in prop::num::i32::ANY
    ) {
        smol::block_on(test_genesis_block(&inits, timestamp_ms as i64, test_options()));
    }
}
