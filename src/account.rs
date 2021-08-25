use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// use crate::validator::Validator;
use crate::{block::Block, claim::Claim, txn::Txn};

// TODO: Move to a different module
// Account State object is effectively the local
// database of all WalletAccounts. This is updated
// whenever transactions are sent/received
// and is "approved" by the network via consensus after
// each transaction and each block. It requires a hashmap
// with a vector of hashmaps that contains information for restoring a wallet.
#[derive(Serialize, Deserialize)]
pub enum StateOption {
    // TODO: Change WalletAccount usage to tuples of types of
    // data from the Wallet needed. Using actual WalletAccount object
    // is unsafe.
    NewTxn(String),
    NewAccount(String),
    PendingClaimAcquired(String),
    ConfirmedClaimAcquired(String),
    ConfirmedTxn(String, String),
    ProposedBlock(String, String, String, String),
    ConfirmedBlock(String, String, String, String),
    GenesisBlock(String, String, String, String)
}

/// The State of all accounts. This is used to track balances
/// this is also used to track the state of the network in general
/// along with the ClaimState and RewardState. Will need to adjust
/// this to account for smart contracts at some point in the future.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct AccountState {
    // Map of account address to public key
    pub accounts_pk: HashMap<String, String>,   // K: address, V: pubkey
    pub credits: HashMap<String, HashMap<String, u128>>,    // K: address, V: Hashmap { K: ticker, V: amount }
    pub pending_credits: HashMap<String, HashMap<String, u128>>,    // K: address, V: Hashmap { K: ticker, V: amount }
    pub debits: HashMap<String, HashMap<String, u128>>, // K: address, V: Hashmap { K: ticker, V: amount }
    pub pending_debits: HashMap<String, HashMap<String, u128>>, // K: address, V: Hashmap { K: ticker, V: amount }
    pub balances: HashMap<String, HashMap<String, u128>>, // K: address, V: Hashmap { K: ticker, V: amount }
    pub pending_balances: HashMap<String, HashMap<String, u128>>, // K: address, V: Hashmap { K: ticker, V: amount }
    pub claims: HashMap<u128, Claim>, // K: claim_number, V: claim
    pub pending_claim_sales: HashMap<u128, String>, // K: claim_number, V: pubkey
    pub pending: HashMap<String, Txn>, // K: txn_id, V: Txn 
    pub mineable: HashMap<String, Txn>, // K: txn_id, V: Txn
    pub last_block: Option<Block>,
}

/// The state of all accounts in the network. This is one of the 3 core state objects
/// which ensures that the network maintains consensus amongst nodes. The account state
/// records accounts, along with their native token (VRRB) balances and smart contract
/// token balances. Also contains all pending and confirmed transactions. Pending
/// transactions are set into the pending vector and the confirmed transactions
/// are set in the mineable vector.
impl AccountState {
    /// Instantiates a new AccountState instance
    /// TODO: Add restoration functionality/optionality to restore an existing
    /// account state on a node that has previously operated but was stopped.
    pub fn start() -> AccountState {
        AccountState {
            accounts_pk: HashMap::new(),
            credits: HashMap::new(),
            pending_credits: HashMap::new(),
            debits: HashMap::new(),
            pending_debits: HashMap::new(),
            balances: HashMap::new(),
            pending_balances: HashMap::new(),
            claims: HashMap::new(),
            pending_claim_sales: HashMap::new(),
            pending: HashMap::new(),
            mineable: HashMap::new(),
            last_block: None,
        }
    }

    /// Update's the AccountState and NetworkState, takes a StateOption (for function routing)
    /// also requires the NetworkState to be provided in the function call.
    /// TODO: Provide Examples to Doc
    pub fn update(&mut self, _value: StateOption) {

    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.to_string().as_bytes().to_vec()
    }

    pub fn from_bytes(data: &[u8]) -> AccountState {
        let mut buffer: Vec<u8> = vec![];
        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();
        serde_json::from_str::<AccountState>(&to_string).unwrap()
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl StateOption {
    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> StateOption {
        let mut buffer: Vec<u8> = vec![];
        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();
        serde_json::from_str::<StateOption>(&to_string).unwrap()
    }
}

impl Clone for AccountState {
    fn clone(&self) -> Self {
        AccountState {
            accounts_pk: self.accounts_pk.clone(),
            credits: self.credits.clone(),
            pending_credits: self.pending_credits.clone(),
            debits: self.debits.clone(),
            pending_debits: self.pending_debits.clone(),
            balances: self.balances.clone(),
            pending_balances: self.pending_balances.clone(),
            claims: self.claims.clone(),
            pending_claim_sales: self.pending_claim_sales.clone(),
            pending: self.pending.clone(),
            mineable: self.mineable.clone(),
            last_block: self.last_block.clone(),
        }
    }
}

