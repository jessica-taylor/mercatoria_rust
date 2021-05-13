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

#[derive(Eq, PartialEq, Debug, Deserialize, Serialize, Clone)]
pub struct AccountState {
    pub fields: BTreeMap<HexPath, Vec<u8>>,
}

impl AccountState {
    pub fn empty() -> AccountState {
        AccountState {
            fields: BTreeMap::new(),
        }
    }

    pub fn sends(&self) -> Vec<SendInfo> {
        let mut res = Vec::new();
        for (path, value) in &self.fields {
            if path.len() >= 8 && path[0..8][..] == bytes_to_path(b"send")[..] {
                res.push(rmp_serde::from_read::<_, SendInfo>(value.as_slice()).unwrap());
            }
        }
        res
    }

    pub fn has_received(&self, send: &SendInfo) -> bool {
        match self.fields.get(&field_received(hash(send)).path) {
            None => false,
            Some(value) => rmp_serde::from_read::<_, bool>(value.as_slice()).unwrap(),
        }
    }

    pub fn balance(&self) -> u128 {
        rmp_serde::from_read::<_, u128>(self.fields.get(&field_balance().path).unwrap().as_slice())
            .unwrap()
    }

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

pub async fn get_account_state<HL: HashLookup>(
    hl: &HL,
    node: Hash<DataNode>,
) -> Result<AccountState, anyhow::Error> {
    let mut state = AccountState::empty();
    add_data_tree_to_account_state(hl, HexPath(vec![]), node, &mut state).await?;
    Ok(state)
}

#[derive(Eq, PartialEq, Debug, Deserialize, Serialize, Clone)]
pub struct MainState {
    pub accounts: BTreeMap<HashCode, AccountState>,
}

impl MainState {
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

pub async fn get_main_state<HL: HashLookup>(
    hl: &HL,
    main: &MainBlockBody,
) -> Result<MainState, anyhow::Error> {
    let mut state = MainState::empty();
    get_account_states_under(hl, main.tree, &mut state).await?;
    Ok(state)
}

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
