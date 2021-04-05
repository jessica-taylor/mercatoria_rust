use mercatoria_rust::account_construction::{initialize_account_node, insert_into_rh_tree};
use mercatoria_rust::blockdata::{
    Action, DataNode, MainBlock, MainBlockBody, MainOptions, PreSignedMainBlock, QuorumNode,
    QuorumNodeBody, QuorumNodeStats, RadixChildren,
};
use mercatoria_rust::construction::{
    best_super_node, genesis_block_body, next_main_block_body, AccountInit,
};
use mercatoria_rust::crypto::{hash, path_to_hash_code, verify_sig, Hash, HashCode};
use mercatoria_rust::hashlookup::{HashLookup, HashPut, MapHashLookup};
use mercatoria_rust::hex_path::{is_prefix, HexPath};
use mercatoria_rust::queries::{
    longest_prefix_length, lookup_account, lookup_quorum_node, quorums_by_prev_block,
};
use mercatoria_rust::state_machine::{
    genesis_state, get_main_state, get_next_main_state, AccountState, MainState,
};
use mercatoria_rust::verification::{quorum_node_body_score, verify_endorsed_quorum_node};

// fn arb_init() -> impl Strategy<Value = AccountInit> {
//
// }

async fn test_genesis_block(inits: Vec<AccountInit>, timestamp_ms: i64, opts: MainOptions) {
    let hash_opts = hash(&opts);
    let mut hl = MapHashLookup::new();
    let main = genesis_block_body(&mut hl, inits.clone(), timestamp_ms, opts)
        .await
        .unwrap();
    assert_eq!(None, main.prev);
    assert_eq!(0, main.version);
    assert_eq!(timestamp_ms, main.timestamp_ms);
    assert_eq!(hash_opts, main.options);
    let expected_state = genesis_state(inits).await;
    let actual_state = get_main_state(&hl, &main).await.unwrap();
    assert_eq!(expected_state, actual_state);
}
