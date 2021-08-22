use crate::validator::{Validator, ValidatorOptions};
use crate::wallet::{WalletAccount};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// use crate::validator::Validator;
use crate::{block::Block, claim::Claim, state::NetworkState, txn::Txn, verifiable::Verifiable};
use std::sync::{Arc, Mutex};

const STARTING_BALANCE: u128 = 1_000;

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
    pub fn update(&mut self, value: StateOption) {
        match value {
            // If the StateOption variant passed to the update method is a NewAccount
            // set the new account information into the account state, return the account state
            // and update the network state.
            StateOption::NewAccount(wallet) => {
                // Enter the wallet's secret key hash as the key and the wallet's public key as the value
                // if the secret key hash is not already in the hashmap
                let wallet = serde_json::from_str::<WalletAccount>(&wallet).unwrap();
                // Enter the wallet's public key string as the key and if it's not already in the HashMap
                // the wallet's address as the value.
                for address in wallet.addresses.values() {
                    self.accounts_pk.entry(address.to_string()).or_insert(wallet.pubkey.clone());
                }

                // Enter the wallet's public key string as the key and the STARTING_BALANCE const as
                // the value
                // TODO: 0 should be the starting value of ALL accounts on the live network
                let mut vrrb_starting_credit = HashMap::new();
                vrrb_starting_credit.insert("VRRB".to_string(), STARTING_BALANCE);
                let mut vrrb_starting_debit = HashMap::new();
                vrrb_starting_debit.insert("VRRB".to_string(), 0u128);

                self.credits.entry(wallet.pubkey.clone()).or_insert(vrrb_starting_credit);
                // Same thing as above since this is a new account
                self.debits.entry(wallet.pubkey.clone()).or_insert(vrrb_starting_debit);
                // The .update() method for the  network state sets a state object (struct)
                // either account_state, claim_state or reward state) into the pickle db
                // that represents the network state.
            }
            // If the StateOption variant passed is a NewTxn process the txn and either return
            // an error if there's obvious validity issues or set it to pending txns to be
            // fully validated by validators.
            StateOption::NewTxn(txn) => {
                // get the receiver's public key from the AccountState accounts_address field
                // which is a hashmap containing the address as the key and the public key as the value
                let txn = serde_json::from_str::<Txn>(&txn).unwrap();
                let receiver = self.accounts_pk.get(&txn.receiver_address);
                match receiver {
                    Some(receiver_pk) => {
                        let receiver_pk = receiver_pk;

                        // get the sender's public key from the AccountState accounts_address field
                        // which is a hashmap containing the address as the key and the public key as the value

                        // get the sender's coin balance as mutable, from the availabe_coin_balances field
                        // in the account_state object, which takes the public key as the key and the
                        // available balance as the value,

                        // TODO: Replace hard coded VRRB Ticker with txn.ticker
                        let sender = self.accounts_pk.get(&txn.sender_address);

                        match sender {
                            Some(sender_pk) => {
                                let sender_pk = sender_pk;
                                let sender_avail_bal = *self.pending_balances.get_mut(sender_pk)
                                                            .unwrap()
                                                            .get_mut("VRRB")
                                                            .unwrap();

                                let balance_check = sender_avail_bal.checked_sub(txn.txn_amount);

                                match balance_check {
                                    Some(_bal) => {
                                        // Add the amount to the receiver pending credits by getting
                                        *self.pending_credits.get_mut(receiver_pk)
                                            .unwrap()
                                            .get_mut("VRRB")
                                            .unwrap() += txn.txn_amount;

                                        // Update the pending debits of the sender
                                        *self.pending_debits.get_mut(sender_pk)
                                            .unwrap()
                                            .get_mut("VRRB")
                                            .unwrap() += txn.txn_amount;
                                                                                
                                        *self.pending_balances.get_mut(receiver_pk)
                                            .unwrap()
                                            .get_mut("VRRB")
                                            .unwrap() = self.pending_credits.get_mut(receiver_pk)
                                                            .unwrap()
                                                            .get_mut("VRRB")
                                                            .unwrap()
                                                            .checked_sub(
                                                                *self.pending_debits.get_mut(receiver_pk)
                                                                    .unwrap()
                                                                    .get_mut("VRRB")
                                                                    .unwrap()
                                                                ).unwrap();

                                        *self.pending_balances.get_mut(sender_pk)
                                            .unwrap()
                                            .get_mut("VRRB")
                                            .unwrap() = self.pending_credits.get_mut(sender_pk)
                                                            .unwrap()
                                                            .get_mut("VRRB")
                                                            .unwrap()
                                                            .checked_sub(
                                                                *self.pending_debits.get_mut(sender_pk)
                                                                    .unwrap()
                                                                    .get_mut("VRRB")
                                                                    .unwrap()
                                                                ).unwrap();

                                        // Push the transaction to pending transactions to be confirmed 
                                        // by validators.
                                        self.pending.entry(txn.clone().txn_id).or_insert(txn.clone());
                                        // Pending transactions do not update the network state, only confirmed
                                        // transactions update the network state. 
                                    }
                                    None => println!("Amount Exceeds Balance"),
                                }
                            }
                            None => println!("Sender is non-existent"),
                        }
                    }
                    None => println!("The receiver is non-existent"),
                }
            },

            // If the StateOption variant received by the update method is a ClaimAcquired
            // Update the account state by entering the relevant information into the
            // proper fields, return the updated account state and update the network state.
            StateOption::PendingClaimAcquired(claim) => {
                // Set a new entry (if it doesn't exist) into the AccountState
                // claim_state field's (which is a ClaimState Struct) owned_claims field
                // which is a HashMap consisting of the claim maturation time as the key and the claim
                // struct itself as the value.
                // TODO: break down PendingClaimAcquired and ConfirmedClaimAcquired as claim acquisition
                // has to be validated before it can be set into the account_state's claim_state.
                let claim = serde_json::from_str::<Claim>(&claim).unwrap();
                self.pending_claim_sales.insert(claim.claim_number, claim.current_owner.unwrap());
            },
            StateOption::ConfirmedClaimAcquired(_claim) => {

            },

            StateOption::GenesisBlock(_miner, block, _reward_state, _network_state) => {
                let _block = serde_json::from_str::<Block>(&block).unwrap();


            },

            // If the StateOption variant received by the update method is Miner
            // this means a new block has been mined, udpate the account state accordingly
            // TODO: mined blocks need to be validated by the network before they're confirmed
            // If it has not yet been confirmed there should be a PendingMiner variant as well
            // as a ConfirmedMiner variant. The logic in this block would be for a ConfirmedMiner
            StateOption::ProposedBlock(miner, block, reward_state, network_state) => {
                let block = serde_json::from_str::<Block>(&block).unwrap();
                let network_state = Arc::new(Mutex::new(serde_json::from_str::<NetworkState>(&network_state).unwrap()));
                
                // Confirm the block is valid and vote, if valid, and the network is in
                // consensus, a new option will do everything below.
                
                match block.is_valid(
                    Some(ValidatorOptions::NewBlock(
                        serde_json::to_string(&self.last_block.clone().unwrap()).unwrap(), 
                        serde_json::to_string(&block).unwrap(), 
                        miner.to_string(),
                        serde_json::to_string(&self.clone()).unwrap(),
                        serde_json::to_string(&network_state.clone().lock().unwrap().clone()).unwrap(),
                        reward_state,
                    ))) {
                        Some(true) => {
                            // Cast a true vote by pushing this into a queue to communicates with the
                            // node that can then publish your message
                        },
                        Some(false) => {
                            // Cast a false vote by pushing this into a queue that communicates with the
                            // node that can then publish the message
                        },
                        None => { 
                            println!("You are not a claim staker, to participate in network governance you must own and stake claims") 
                        }
                    }
            },
            StateOption::ConfirmedBlock(_miner, _block, _reward_state, _network_state) => {
                // If the block has been confirmed by the network, and there is consensus
                // around the state of the network at the given block.height
                // confirm the network state by replacing it with the temporary network state
                // object used to validate the new block. Replace any 
            }

            // If the StateOption is a confirmed transaction update the account state
            // accordingly (update balances of sender, receiver(s)) distribute the
            // fees to the trasnaction's validator.
            StateOption::ConfirmedTxn(txn, validator) => {
                //TODO: distribute txn fees among validators.
                let txn = serde_json::from_str::<Txn>(&txn).unwrap();
                let validator: Validator = serde_json::from_str::<Validator>(&validator).unwrap();
                self.pending.get(&txn.txn_id).unwrap().clone().validators.push(validator);

                let num_invalid = self.pending.get(&txn.txn_id)
                                        .unwrap()
                                        .clone().validators.iter()
                                        .filter(|&validator| !validator.to_owned().valid)
                                        .count();

                let len_of_validators = self.pending.get(&txn.txn_id).unwrap().clone().validators.len();
                println!("{}", &len_of_validators);
                if len_of_validators >= 3 {
                    if num_invalid as f32 / len_of_validators as f32 > 1.0 / 3.0 {
                        {}
                    } else {
                        self.mineable.insert(txn.clone().txn_id, txn);
                    }
                }
            }
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> AccountState {
        let mut buffer: Vec<u8> = vec![];
        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();
        serde_json::from_str::<AccountState>(&to_string).unwrap()
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

