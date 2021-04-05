use mercatoria_rust::blockdata::{
    Action, DataNode, MainBlock, MainBlockBody, MainOptions, PreSignedMainBlock, QuorumNode,
    QuorumNodeBody, QuorumNodeStats, RadixChildren,
};
use mercatoria_rust::construction::{
    best_super_node, genesis_block_body, next_main_block_body, AccountInit,
};
use mercatoria_rust::crypto::hash;
use mercatoria_rust::hashlookup::MapHashLookup;

use mercatoria_rust::state_machine::{genesis_state, get_main_state};

use mercatoria_rust::verification::verify_valid_main_block_body;

async fn test_genesis_block(
    inits: &Vec<AccountInit>,
    timestamp_ms: i64,
    opts: MainOptions,
) -> (MapHashLookup, MainBlockBody) {
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
    verify_valid_main_block_body(&hl, &main).unwrap();
    (hl, main)
}

async fn test_send_and_receive(
    hl: &mut MapHashLookup,
    start_main: &MainBlockBody,
    sender_ix: u32,
    receiver_ix: u32,
    amount: u64,
) {
    let start_state = get_main_state(&hl, &main).await.unwrap();
    // let acct_states =
}
