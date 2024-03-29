//! A simplified state machine to represent the state of the blockchain over time.  This is used
//! in testing to check that the more complex blockchain transformation operations are consistent
//! with these simple semantics.
//!
//! Basically, we check that the following diagram commutes:
//!
//! main block 1  ------> main block 2
//!   |                     |
//!   v                     v
//! state 1       ------> state 2
use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

use futures_lite::FutureExt;
use serde::{Deserialize, Serialize};

use crate::account_transform::{
    field_balance, field_public_key, field_received, field_stake, run_action, AccountTransform,
};
use crate::blockdata::{
    AccountInit, Action, DataNode, MainBlock, MainBlockBody, QuorumNode, SendInfo,
};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::HashLookup;
use crate::hex_path::{bytes_to_path, HexPath};

/// The state of an account.
#[derive(Eq, PartialEq, Debug, Deserialize, Serialize, Clone)]
pub struct AccountState {
    /// A mapping from field to value.
    pub fields: BTreeMap<HexPath, Vec<u8>>,
}

impl AccountState {
    /// An account state with no fields.
    pub fn empty() -> AccountState {
        AccountState {
            fields: BTreeMap::new(),
        }
    }

    /// The sends sent by this account.
    pub fn sends(&self) -> Vec<SendInfo> {
        let mut res = Vec::new();
        for (path, value) in &self.fields {
            if path.len() >= 8 && path[0..8][..] == bytes_to_path(b"send")[..] {
                res.push(rmp_serde::from_read::<_, SendInfo>(value.as_slice()).unwrap());
            }
        }
        res
    }

    /// Whether this account has received a given send.
    pub fn has_received(&self, send: &SendInfo) -> bool {
        match self.fields.get(&field_received(hash(send)).path) {
            None => false,
            Some(value) => rmp_serde::from_read::<_, bool>(value.as_slice()).unwrap(),
        }
    }

    /// The balance of the account.
    pub fn balance(&self) -> u128 {
        rmp_serde::from_read::<_, u128>(self.fields.get(&field_balance().path).unwrap().as_slice())
            .unwrap()
    }

    /// The stake of the account.
    pub fn stake(&self) -> u128 {
        rmp_serde::from_read::<_, u128>(self.fields.get(&field_stake().path).unwrap().as_slice())
            .unwrap()
    }
}

impl fmt::Display for AccountState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccountState {{")?;
        for (path, value) in &self.fields {
            write!(f, "\n    {}: {:?}", path, value)?;
        }
        write!(f, "\n  }}")
    }
}

/// Collects field values in a given data tree into the `AccountState`.
fn add_data_tree_to_account_state<'a, HL: HashLookup>(
    hl: &'a HL,
    path: HexPath,
    node: Hash<DataNode>,
    state: &'a mut AccountState,
) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + 'a>> {
    async move {
        let node = hl.lookup(node).await?;
        match node.field {
            None => {}
            Some(value) => {
                state.fields.insert(path.clone(), value);
            }
        }
        for (suffix, child) in node.children.iter_entries() {
            let child_path = HexPath(vec![path.0.clone(), suffix.0.clone()].concat());
            add_data_tree_to_account_state(hl, child_path, *child, state).await?;
        }
        Ok(())
    }
    .boxed()
}

/// Gets an account state from a top `DataNode`.
pub async fn get_account_state<HL: HashLookup>(
    hl: &HL,
    node: Hash<DataNode>,
) -> Result<AccountState, anyhow::Error> {
    let mut state = AccountState::empty();
    add_data_tree_to_account_state(hl, HexPath(vec![]), node, &mut state).await?;
    Ok(state)
}

/// The state of the main block.
#[derive(Eq, PartialEq, Debug, Deserialize, Serialize, Clone)]
pub struct MainState {
    /// A mapping from account to the account's state.
    pub accounts: BTreeMap<HashCode, AccountState>,
}

impl MainState {
    /// An empty `MainState`.
    pub fn empty() -> MainState {
        MainState {
            accounts: BTreeMap::new(),
        }
    }
}

impl fmt::Display for MainState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MainState {{")?;
        for (path, acct) in &self.accounts {
            write!(f, "\n  {}: {}", bytes_to_path(path), acct)?;
        }
        write!(f, "\n}}")
    }
}

/// Collects account states under a given `QuorumNode` into a `MainState`.
fn get_account_states_under<'a, HL: HashLookup>(
    hl: &'a HL,
    node_hash: Hash<QuorumNode>,
    state: &'a mut MainState,
) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + 'a>> {
    async move {
        let node = hl.lookup(node_hash).await?;
        let depth = node.body.path.len();
        if depth == 64 {
            let acct = path_to_hash_code(node.body.path);
            let acct_state = get_account_state(hl, node.body.data_tree.unwrap()).await?;
            state.accounts.insert(acct, acct_state);
        } else {
            for (_, child) in node.body.children.iter_entries() {
                get_account_states_under(hl, *child, state).await?;
            }
        }
        Ok(())
    }
    .boxed()
}

/// Gets the `MainState` corresponding to a `MainBlockBody`.
pub async fn get_main_state<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
) -> Result<MainState, anyhow::Error> {
    let mut state = MainState::empty();
    get_account_states_under(hl, main.tree, &mut state).await?;
    Ok(state)
}

/// Gets the state of the genesis block.
pub async fn genesis_state(inits: &Vec<AccountInit>) -> MainState {
    let mut state = MainState::empty();
    for init in inits {
        let mut acct_state = AccountState::empty();
        acct_state.fields.insert(
            field_public_key().path,
            rmp_serde::to_vec_named(&init.public_key).unwrap(),
        );
        acct_state.fields.insert(
            field_balance().path,
            rmp_serde::to_vec_named(&init.balance).unwrap(),
        );
        acct_state.fields.insert(
            field_stake().path,
            rmp_serde::to_vec_named(&init.stake).unwrap(),
        );
        state
            .accounts
            .insert(hash(&init.public_key).code, acct_state);
    }
    state
}

/// Computes the next account state given a previous state and an action.
pub async fn get_next_account_state<HL: HashLookup>(
    hl: &HL,
    last_main: Hash<MainBlock>,
    this_account: HashCode,
    action: &Action,
    last_main_state: &MainState,
) -> Option<AccountState> {
    let (mut curr_state, is_init) = match last_main_state.accounts.get(&this_account) {
        None => (AccountState::empty(), true),
        Some(state) => ((*state).clone(), false),
    };
    let mut at = AccountTransform::new(hl, is_init, this_account, last_main);
    match run_action(&mut at, action).await {
        Ok(()) => {
            for (field, val) in at.fields_set {
                curr_state.fields.insert(field, val);
            }
            Some(curr_state)
        }
        Err(_) => None,
    }
}

/// Computes the next main state given a previous state and actions to run for some subset of
/// accounts.
pub async fn get_next_main_state<HL: HashLookup>(
    hl: &HL,
    last_main: Hash<MainBlock>,
    actions: BTreeMap<HashCode, Action>,
    main_state: &MainState,
) -> MainState {
    let mut next_state = main_state.clone();
    for (acct, action) in actions {
        match get_next_account_state(hl, last_main, acct, &action, main_state).await {
            None => {}
            Some(next_acct_state) => {
                next_state.accounts.insert(acct, next_acct_state);
            }
        }
    }
    next_state
}
