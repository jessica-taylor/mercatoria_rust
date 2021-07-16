use ed25519_dalek::Keypair;
use std::collections::BTreeMap;
use std::iter::FromIterator;

use mercatoria_rust::account_construction::*;
use mercatoria_rust::account_transform::*;
use mercatoria_rust::blockdata::*;
use mercatoria_rust::construction::*;
use mercatoria_rust::crypto::*;
use mercatoria_rust::hashlookup::*;
use mercatoria_rust::hex_path::*;

use mercatoria_rust::state_machine::{genesis_state, get_account_state, get_main_state};

use mercatoria_rust::verification::verify_valid_main_block_body;
use proptest::prelude::*;

use mercatoria_rust::queries;

mod strategies;
use strategies::*;

// fn arb_init() -> impl Strategy<Value = AccountInit> {
//
// }

async fn test_insert_into_data_tree(
    entries: &Vec<(HexPath, Vec<u8>)>,
) -> (MapHashLookup, Hash<DataNode>) {
    let mut hl = MapHashLookup::new();
    let mut hash_node = hl
        .put(&DataNode {
            field: None,
            children: RadixChildren::default(),
        })
        .await
        .unwrap();
    let mut node_count: usize = 0;
    for (key, val) in entries {
        hash_node =
            insert_into_data_tree(&mut hl, &mut node_count, &key[..], val.clone(), hash_node)
                .await
                .unwrap();
    }
    let actual_state = get_account_state(&hl, hash_node).await.unwrap();
    let expected_state = BTreeMap::from_iter(entries.clone().into_iter());
    assert_eq!(
        expected_state, actual_state.fields,
        "insert_into_data_tree should produce expected result"
    );
    (hl, hash_node)
}

async fn test_genesis_block(
    inits: &Vec<AccountInit>,
    _keys: &BTreeMap<HashCode, Keypair>,
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
    assert_eq!(None, main.prev, "genesis main.prev");
    assert_eq!(0, main.version, "genesis main.version");
    assert_eq!(timestamp_ms, main.timestamp_ms, "genesis timestamp_ms");
    assert_eq!(hash_opts, main.options, "genesis options");
    let expected_state = genesis_state(&inits).await;
    let actual_state = get_main_state(&hl, &main).await.unwrap();
    assert_eq!(
        expected_state, actual_state,
        "genesis state: {} versus {}",
        expected_state, actual_state
    );
    verify_valid_main_block_body(&hl, &main).await.unwrap();
    for init in inits.iter() {
        let h = hash(&init.public_key).code;
        let acc = queries::lookup_account(&hl, &main, h).await;
        assert!(acc.is_ok(), "account data not accessed");
        let acc = acc.ok().unwrap();
        assert!(acc.is_some(), "account data not stored");
        let acc_data = acc.unwrap().body.data_tree;
        assert!(acc_data.is_some(), "missing data tree");
        let _acc_data = acc_data.unwrap();
        // TODO test account data contents
    }
    (hl, main)
}

async fn test_send_and_receive(
    hl: &mut MapHashLookup,
    keys: &BTreeMap<HashCode, Keypair>,
    start_main: &MainBlock,
    sender_ix: usize,
    receiver_ix: usize,
    fee: u128,
    amount: u128,
) -> Result<(), anyhow::Error> {
    let start_state = get_main_state(hl, &start_main.block.body).await.unwrap();
    let accts: Vec<HashCode> = start_state.accounts.keys().cloned().collect();
    let sender = accts[sender_ix % accts.len()];
    let receiver = accts[receiver_ix % accts.len()];
    let (send_act, _send_info) = mk_send(
        hash(start_main),
        fee,
        receiver,
        amount,
        None,
        vec![],
        keys.get(&sender).unwrap(),
    );
    let sender_new_node = add_action_to_account(hl, start_main, sender, &send_act, 0)
        .await?
        .into_unsigned();
    let _sender_new_hash = hl.put(&sender_new_node).await?;

    // TODO finish the test by constructing two new main blocks, one with sends and one with receives

    Ok(())
    // recipient: HashCode,
    // send_amount: u128,
    // initialize_spec: Option<Hash<Vec<u8>>>,
    // message: Vec<u8>,
    // key: ed25519_dalek::Keypair,

    // pub last_main: Hash<MainBlock>,
    // pub fee: u128,
    // pub command: Vec<u8>,
    // pub args: Vec<Vec<u8>>,
    // let send_act = Action {
    //     last_main: hash(start_main),
    //     fee,
    //     command: b"send",
    //     args: vec![
    // pub last_main: Hash<MainBlock>,
    // pub fee: u128,
    // pub command: Vec<u8>,
    // pub args: Vec<Vec<u8>>,
    // };
    // let new_sender_acct = add_action_to_account(hl, start_main, sender,
    // hl: &mut HL,
    // last_main: &MainBlock,
    // account: HashCode,
    // action: &Action,
    // prize: u128,
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
    #![proptest_config(ProptestConfig {
        cases: 10, .. ProptestConfig::default()
    })]
    #[test]
    fn proptest_insert_into_data_tree(entries: Vec<(HexPath, Vec<u8>)>) {
        smol::block_on(test_insert_into_data_tree(&entries));
    }
    #[test]
    fn proptest_genesis_block(
        inits in account_inits(),
        timestamp_ms in prop::num::i32::ANY
    ) {
        smol::block_on(test_genesis_block(&inits.0, &inits.1, timestamp_ms as i64, test_options()));
    }
    #[test]
    fn proptest_transfer(
        inits in account_inits(),
        timestamp_ms in prop::num::i32::ANY
    ) {
        let account_inits = inits.0;
        let account_keys = inits.1;
        let (mut hl, genesis_block_body) = smol::block_on(test_genesis_block(&account_inits, &account_keys, timestamp_ms as i64, test_options()));
        if account_inits.len() < 2 {
            // Impossible to test transfer without at least two accounts, TODO generate in a way that avoids this
            return Ok(());
        }
        let miner_keys = account_keys.get(&hash(&account_inits[0].public_key).code).unwrap();
        let presigned_genesis_block = PreSignedMainBlock{
            body: genesis_block_body.clone(),
            signatures: account_inits.iter().map(|init| sign(account_keys.get(&hash(&init.public_key).code).unwrap(), genesis_block_body.clone()).clone()).collect::<Vec<_>>(),
        };
        let genesis_block = MainBlock{
            block: presigned_genesis_block.clone(),
            signature: sign(miner_keys, presigned_genesis_block),
        };
        let mut sender_ix: Option<usize> = None;
        // TODO use non-deterministic index
        for (i, acc) in account_inits.iter().enumerate() {
            if acc.balance > 0 {
                sender_ix = Some(i);
            }
        }
        // TODO change indices to hashes since order of accounts need not be persisted
        let sender_ix = match sender_ix{
            None => return Ok(()),
            Some(ix) => ix,
        };
        let receiver_ix = (sender_ix + 1) % account_inits.len();
        let sender_balance = account_inits[sender_ix as usize].balance;
        let to_send_amount = (sender_balance + 1)/2;
        let fee = (sender_balance - to_send_amount)/2;
        let res = smol::block_on(test_send_and_receive(&mut hl, &account_keys, &genesis_block, sender_ix, receiver_ix, fee, to_send_amount));
        assert!(res.is_ok(), "got error: {}", res.err().unwrap())
    }
}
