#![allow(unused_imports)]
use crate::pool::Pool;
use crate::verifiable::Verifiable;
use crate::{
    account::AccountState, block::Block, claim::Claim, reward::RewardState, state::NetworkState,
    txn::Txn, wallet::WalletAccount,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxnValidator {
    pub pubkey: String,
    pub vote: bool,
    pub txn: Txn,
}

impl TxnValidator {
    pub fn new(
        pubkey: String,
        txn: Txn,
        network_state: &NetworkState,
        txn_pool: &Pool<String, Txn>,
    ) -> TxnValidator {
        TxnValidator {
            pubkey,
            vote: txn.clone().valid_txn(network_state, txn_pool),
            txn,
        }
    }
}
