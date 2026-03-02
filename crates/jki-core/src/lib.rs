use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

pub mod import;

#[derive(Archive, Deserialize, Serialize, SerdeDeserialize, SerdeSerialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct Account {
    pub name: String,
    pub issuer: Option<String>,
    pub secret: String,
    pub digits: u32,
    pub algorithm: String,
    pub account_type: AccountType,
}

#[derive(Archive, Deserialize, Serialize, SerdeDeserialize, SerdeSerialize, Debug, Clone, PartialEq)]
#[archive(check_bytes)]
pub enum AccountType {
    Standard,
    Steam,
    Blizzard,
}

#[derive(Archive, Deserialize, Serialize, SerdeDeserialize, SerdeSerialize, Debug, Clone)]
#[archive(check_bytes)]
pub struct Vault {
    pub accounts: Vec<Account>,
    pub version: u32,
}

impl Vault {
    pub fn new() -> Self {
        Self {
            accounts: Vec::new(),
            version: 1,
        }
    }
}
