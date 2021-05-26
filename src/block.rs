// use secp256k1::{key::PublicKey, Signature};
// use std::time::Instant;
use std::collections::HashMap;
// use std::fmt;
use serde::{Serialize, Deserialize};
use crate::{txn::Txn, claim::Claim};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {

    pub timestamp: u128,
    pub last_block_hash: String,
    pub data: HashMap<String, Txn>,
    pub claim: Claim,
}