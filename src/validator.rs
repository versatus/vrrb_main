#![allow(unused_imports)]
use crate::{
    account::AccountState, block::Block, claim::Claim, mpu, reward::RewardState,
    state::NetworkState, txn::Txn, wallet::WalletAccount,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvalidMessageError {
    InvalidTxnError(String),
    InvalidClaimAcquisition(String),
    InvalidBlock(String),
}

#[derive(Serialize, Deserialize)]
pub enum ValidatorOptions {
    ClaimAcquire(String, String),
    NewBlock(Block, Block, String, AccountState, RewardState, NetworkState),
    Transaction(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    ClaimAcquired(String, String, String, String),
    NewBlock(Block, Block, String, AccountState, RewardState, NetworkState),
    Txn(String, String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    pub node_wallet: String,
    pub staked_claims: HashMap<u128, Claim>,
    pub message: Message,
    pub valid: bool,
}

impl Validator {
    pub fn new(message: Message, pubkey: String, account_state: AccountState) -> Option<Validator> {
        let mut check_staked_claims: HashMap<u128, Claim> = HashMap::new();
        account_state
            .claims
            .iter()
            .filter(|(_claim_number, claim)| claim.current_owner.clone().unwrap() == pubkey)
            .for_each(
                |(claim_number, claim)| match account_state.claims.get(claim_number) {
                    Some(_) => {
                        check_staked_claims.insert(*claim_number, claim.clone());
                    }
                    None => {}
                },
            );
        // If there's no staked claims for the node wallet attempting to launch a validator
        // a validator cannot be launched. Claims must be staked to validate messages
        Some(check_staked_claims).map(|map| Validator {
            node_wallet: pubkey,
            staked_claims: map.clone(),
            message,
            valid: false,
        })
    }

    pub fn validate(&self) -> Self {
        mpu::message_processor(self.clone())
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> Validator {
        let mut buffer: Vec<u8> = vec![];

        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();

        serde_json::from_str::<Validator>(&to_string).unwrap()
    }
}

impl ValidatorOptions {
    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();

        as_string.as_bytes().iter().copied().collect()
    }
    pub fn from_bytes(data: &[u8]) -> ValidatorOptions {
        let mut buffer: Vec<u8> = vec![];
        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();

        serde_json::from_str::<ValidatorOptions>(&to_string).unwrap()
    }
}

impl Message {
    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> Message {
        let mut buffer: Vec<u8> = vec![];
        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();
        serde_json::from_str::<Message>(&to_string).unwrap()
    }
}
