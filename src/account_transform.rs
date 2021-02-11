use std::marker::PhantomData;
use std::collections::BTreeMap;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::blockdata::{DataNode, MainBlock, MainBlockBody, PreSignedMainBlock, QuorumNode, QuorumNodeBody, SendInfo};
use crate::crypto::{hash, path_to_hash_code, Hash, HashCode};
use crate::hashlookup::HashLookup;
use crate::hex_path::{bytes_to_path, HexPath};
use crate::queries::{lookup_account, lookup_data_in_account};

/// An typed account data field.
#[derive(Serialize, Deserialize, Debug)]
pub struct TypedDataField<T> {
    /// The path of the field in account data.
    pub path: HexPath,
    phantom: PhantomData<T>
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
    fn from_path(path: HexPath) -> TypedDataField<T> {
        TypedDataField { path: path, phantom: PhantomData }
    }
}

pub fn field_balance() -> TypedDataField<u128> {
    TypedDataField::from_path(bytes_to_path(b"balance"))
}
pub fn field_stake() -> TypedDataField<u128> {
    TypedDataField::from_path(bytes_to_path(b"balance"))
}
pub fn field_public_key() -> TypedDataField<Vec<u8>> {
    TypedDataField::from_path(bytes_to_path(b"public_key"))
}


/// A context providing operations related to transforming accounts (e.g.
/// running actions).
pub struct AccountTransform<'a, HL : HashLookup> {
    pub hl: &'a HL,
    pub is_initializing: bool,
    pub this_account: HashCode,
    pub hash_last_main: Hash<MainBlock>,
    pub fields_set: BTreeMap<HexPath, Vec<u8>>,
}


impl<'a, HL : HashLookup> HashLookup for AccountTransform<'a, HL> {
    fn lookup_bytes(&self, hash: HashCode) -> Result<Vec<u8>, String> {
        self.hl.lookup_bytes(hash)
    }
}

impl<'a, HL : HashLookup> AccountTransform<'a, HL> {

    fn new(hl: &'a HL, is_initializing: bool, this_account: HashCode, hash_last_main: Hash<MainBlock>) -> AccountTransform<'a, HL> {
        AccountTransform {
            hl, is_initializing, this_account, hash_last_main,
            fields_set: BTreeMap::new()
        }
    }

    /// Gets the value of a given data field.
    fn get_data_field_bytes(&self, acct: HashCode, field_name: &HexPath) -> Result<Option<Vec<u8>>, String> {
        if acct == self.this_account {
            match self.fields_set.get(field_name) {
                Some(x) => {
                    return Ok(Some(x.clone()));
                }
                None => {}
            }
        }
        let main = self.lookup(self.hash_last_main)?;
        let acct_node = lookup_account(self, &main.block.body, self.this_account)?;
        lookup_data_in_account(self, &acct_node, field_name)
    }

    /// Sets the value of a given data field.
    fn set_data_field_bytes(&mut self, field_name: &HexPath, value: Vec<u8>) -> Result<(), String> {
        self.fields_set.insert(field_name.clone(), value);
        Ok(())
    }

    /// Gets the value of a given typed data field.
    fn get_data_field<T : DeserializeOwned>(&self, acct: HashCode, field: &TypedDataField<T>) -> Result<Option<T>, String> {
        match self.get_data_field_bytes(acct, &field.path)? {
            None => Ok(None),
            Some(bs) => match rmp_serde::from_read(bs.as_slice()) {
                Ok(val) => Ok(Some(val)),
                Err(e) => Err(e.to_string())
            }
        }
    }

    /// Gets the value of a given typed data field, throwing an error if it is not found.
    fn get_data_field_or_error<T : DeserializeOwned>(&self, acct: HashCode, field: &TypedDataField<T>) -> Result<T, String> {
        match self.get_data_field(acct, field)? {
            None => Err(format!("data field not found: {:?}", field.path)),
            Some(x) => Ok(x)
        }
    }

    /// Sets the value of a given typed data field.
    fn set_data_field<T : Serialize>(&mut self, field: &TypedDataField<T>, value: &T) -> Result<(), String> {
        self.set_data_field_bytes(&field.path, rmp_serde::to_vec_named(value).unwrap())
    }
}

pub fn pay_fee<'a, HL : HashLookup>(at: &mut AccountTransform<'a, HL>, fee: u128) -> Result<(), String> {
    let bal = at.get_data_field_or_error(at.this_account, &field_balance())?;
    if bal < fee {
        return Err("not enough balance for fee".to_string());
    }
    at.set_data_field(&field_balance(), &(bal - fee))
}

// pub fn do_send<'a, HL : HashLookup>(at: &mut AccountTransform<'a, HL>, send: &SendInfo) -> Result<(), String> {
// 
// }
