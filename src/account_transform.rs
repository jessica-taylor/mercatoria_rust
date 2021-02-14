use std::{collections::BTreeMap, marker::PhantomData};

use anyhow::bail;
use async_trait::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::blockdata::{
    Action, DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody,
    SendInfo,
};
use crate::crypto::{hash, path_to_hash_code, verify_sig, Hash, HashCode, Signature};
use crate::hashlookup::HashLookup;
use crate::hex_path::{bytes_to_path, HexPath};
use crate::queries::{lookup_account, lookup_data_in_account};

/// An typed account data field.
#[derive(Serialize, Deserialize, Debug)]
pub struct TypedDataField<T> {
    /// The path of the field in account data.
    pub path: HexPath,
    phantom: PhantomData<T>,
}

impl<T> Clone for TypedDataField<T> {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            phantom: PhantomData,
        }
    }
}

impl<T> TypedDataField<T> {
    /// Creates a `TypedDataField` given a path.
    pub fn from_path(path: HexPath) -> TypedDataField<T> {
        TypedDataField {
            path: path,
            phantom: PhantomData,
        }
    }
}

/// Account balance field.
pub fn field_balance() -> TypedDataField<u128> {
    TypedDataField::from_path(bytes_to_path(b"balance"))
}

/// Account stake field.
pub fn field_stake() -> TypedDataField<u128> {
    TypedDataField::from_path(bytes_to_path(b"balance"))
}

/// Account public key field.
pub fn field_public_key() -> TypedDataField<ed25519_dalek::PublicKey> {
    TypedDataField::from_path(bytes_to_path(b"public_key"))
}

/// Field for a `SendInfo` stored in the sender's data.
pub fn field_send(send: Hash<SendInfo>) -> TypedDataField<SendInfo> {
    let mut path = bytes_to_path(b"send");
    path.extend(&bytes_to_path(&send.code));
    TypedDataField::from_path(path)
}

/// Field for tracking whether a `SendInfo` has been received in the receiver's
/// data.
pub fn field_received(send: Hash<SendInfo>) -> TypedDataField<bool> {
    let mut path = bytes_to_path(b"received");
    path.extend(&bytes_to_path(&send.code));
    TypedDataField::from_path(path)
}

/// A context providing operations related to transforming accounts (e.g.
/// running actions).
pub struct AccountTransform<'a, HL: HashLookup> {
    pub hl: &'a HL,
    pub is_initializing: bool,
    pub this_account: HashCode,
    pub last_main: Hash<MainBlock>,
    pub fields_set: BTreeMap<HexPath, Vec<u8>>,
}

#[async_trait]
impl<'a, HL: HashLookup> HashLookup for AccountTransform<'a, HL> {
    async fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, anyhow::Error> {
        self.hl.lookup_bytes(hash).await
    }
}

impl<'a, HL: HashLookup> AccountTransform<'a, HL> {
    /// Creates a new `AccountTransform`.
    pub fn new(
        hl: &'a HL,
        is_initializing: bool,
        this_account: HashCode,
        last_main: Hash<MainBlock>,
    ) -> AccountTransform<'a, HL> {
        AccountTransform {
            hl,
            is_initializing,
            this_account,
            last_main,
            fields_set: BTreeMap::new(),
        }
    }

    /// Gets the value of a given data field.
    async fn get_data_field_bytes(
        &self,
        acct: HashCode,
        field_name: &HexPath,
    ) -> Result<Option<Vec<u8>>, anyhow::Error> {
        if acct == self.this_account {
            match self.fields_set.get(field_name) {
                Some(x) => {
                    return Ok(Some(x.clone()));
                }
                None => {}
            }
        }
        let main = self.lookup(self.last_main).await?;
        if let Some(acct_node) = lookup_account(self, &main.block.body, self.this_account).await? {
            lookup_data_in_account(self, &acct_node, field_name).await
        } else {
            Ok(None)
        }
    }

    /// Sets the value of a given data field.
    fn set_data_field_bytes(
        &mut self,
        field_name: &HexPath,
        value: Vec<u8>,
    ) -> Result<(), anyhow::Error> {
        self.fields_set.insert(field_name.clone(), value);
        Ok(())
    }

    /// Gets the value of a given typed data field.
    async fn get_data_field<T: DeserializeOwned>(
        &self,
        acct: HashCode,
        field: &TypedDataField<T>,
    ) -> Result<Option<T>, anyhow::Error> {
        match self.get_data_field_bytes(acct, &field.path).await? {
            None => Ok(None),
            Some(bs) => Ok(Some(rmp_serde::from_read(bs.as_slice())?)),
        }
    }

    /// Gets the value of a given typed data field, throwing an error if it is not found.
    pub async fn get_data_field_or_error<T: DeserializeOwned>(
        &self,
        acct: HashCode,
        field: &TypedDataField<T>,
    ) -> Result<T, anyhow::Error> {
        match self.get_data_field(acct, field).await? {
            None => bail!("data field not found: {:?}", field.path),
            Some(x) => Ok(x),
        }
    }

    /// Sets the value of a given typed data field.
    fn set_data_field<T: Serialize>(
        &mut self,
        field: &TypedDataField<T>,
        value: &T,
    ) -> Result<(), anyhow::Error> {
        self.set_data_field_bytes(&field.path, rmp_serde::to_vec_named(value)?)
    }
}

// /// Causes the current account to pay a fee.
async fn pay_fee<'a, HL: HashLookup>(
    at: &mut AccountTransform<'a, HL>,
    fee: u128,
) -> Result<(), anyhow::Error> {
    let bal = at
        .get_data_field_or_error(at.this_account, &field_balance())
        .await?;
    if bal < fee {
        bail!("not enough balance for fee");
    }
    at.set_data_field(&field_balance(), &(bal - fee))
}

/// Causes the current account to send.
async fn do_send<'a, HL: HashLookup>(
    at: &mut AccountTransform<'a, HL>,
    send: &SendInfo,
) -> Result<(), anyhow::Error> {
    if send.sender != at.this_account {
        bail!("sender must be sent by this account");
    }
    if send.last_main != at.last_main {
        bail!("last main of send must be the current last main");
    }
    let bal = at
        .get_data_field_or_error(at.this_account, &field_balance())
        .await?;
    if bal < send.send_amount {
        bail!("not enough balance for send");
    }
    let send_df = field_send(hash(send));
    if at
        .get_data_field(at.this_account, &send_df)
        .await?
        .is_some()
    {
        bail!("that was already sent");
    }
    at.set_data_field(&field_balance(), &(bal - send.send_amount))?;
    at.set_data_field(&send_df, send)?;
    Ok(())
}

/// Causes the current account to receive.
async fn do_receive<'a, HL: HashLookup>(
    at: &mut AccountTransform<'a, HL>,
    sender: HashCode,
    send_hash: Hash<SendInfo>,
) -> Result<SendInfo, anyhow::Error> {
    let send = at
        .get_data_field_or_error(sender, &field_send(send_hash))
        .await?;
    if hash(&send) != send_hash {
        bail!("send hashes don't match");
    }
    if send.recipient != Some(at.this_account) {
        bail!("recipient of send doesn't match recipient");
    }
    let received_field = field_received(send_hash);
    let already_received = at.get_data_field(at.this_account, &received_field).await?;
    if already_received == Some(true) {
        bail!("tried to receive the same send twice");
    }
    let bal = at
        .get_data_field_or_error(at.this_account, &field_balance())
        .await?;
    at.set_data_field(&field_balance(), &(bal + send.send_amount))?;
    at.set_data_field(&received_field, &true)?;
    Ok(send)
}

/// Gets an argument out of action arguments.
fn get_arg<T: DeserializeOwned>(args: &Vec<Vec<u8>>, i: usize) -> Result<T, anyhow::Error> {
    if i >= args.len() {
        bail!("too few arguments");
    }
    Ok(rmp_serde::from_read(args[i].as_slice())?)
}

/// Verifies that the argument at a given index is a signature of a modified
/// version of the action where the signature itself is replaced with
/// an empty vector, and also that the signature's account matches the
/// given account.
fn verify_signature_argument(
    acct: HashCode,
    action: &Action,
    i: usize,
) -> Result<(), anyhow::Error> {
    let sig: Signature<Action> = get_arg(&action.args, i)?;
    if sig.account() != acct {
        bail!("signature account must equal current account");
    }
    let mut act2 = action.clone();
    act2.args[i] = Vec::new();
    if !verify_sig(&act2, &sig) {
        bail!("invalid signature");
    }
    Ok(())
}

/// Runs an action in a given `AccountTransform` context.
pub async fn run_action<'a, HL: HashLookup>(
    at: &mut AccountTransform<'a, HL>,
    action: &Action,
) -> Result<(), anyhow::Error> {
    if at.last_main != action.last_main {
        bail!("action last main must equal current last main");
    }
    if action.command == b"send" {
        if at.is_initializing {
            bail!("send can't initialize an account");
        }
        let recipient: HashCode = get_arg(&action.args, 0)?;
        let send_amount: u128 = get_arg(&action.args, 1)?;
        let initialize_spec: Option<Hash<Vec<u8>>> = get_arg(&action.args, 2)?;
        let message: Vec<u8> = get_arg(&action.args, 3)?;
        verify_signature_argument(at.this_account, action, 4)?;
        pay_fee(at, action.fee).await?;
        let send = SendInfo {
            last_main: action.last_main,
            sender: at.this_account,
            recipient: Some(recipient),
            send_amount,
            initialize_spec,
            message,
        };
        do_send(at, &send).await?;
    } else if action.command == b"receive" {
        let sender: HashCode = get_arg(&action.args, 0)?;
        let send_hash: Hash<SendInfo> = get_arg(&action.args, 1)?;
        let sig: Signature<Action> = get_arg(&action.args, 2)?;
        verify_signature_argument(at.this_account, action, 2)?;
        if at.is_initializing {
            at.set_data_field(&field_balance(), &0)?;
            at.set_data_field(&field_stake(), &0)?;
            at.set_data_field(&field_public_key(), &sig.key)?;
        }
        do_receive(at, sender, send_hash).await?;
        pay_fee(at, action.fee).await?;
    } else {
        bail!("unknown command {:?}", action.command);
    }
    Ok(())
}
