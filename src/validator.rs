use std::collections::HashMap;
use crate::{
    account::{
        AccountState, 
        WalletAccount
    }, 
    block::Block, 
    claim::{
        Claim, 
        ClaimOption
    }, 
    txn::Txn, 
    verifiable::Verifiable
};

// Validator is the core unit of the network's consensus model. The validator
// checks if the validator owns a claim, is staking the claim, and then
// allocates it messages to process.
// Validator can only process messages that have a trait bound of Verifiable
// this means that they have a signature, essentially.
// The validator must go through the checks based on the kind of message it's validating
//
//          - Simple Transactions:
//              * Signature must be verified
//              * Amount + Txn Fee must be less than sender available balance
//              * Receiver must exist in the most recent CONFIRMED state of the network.
//
//          - New Block:
//              * Signature must be verified
//              * Reward must match last block next block reward
//              * State Hash must be valid
//              * Claim ownership must be verified
//              * Claim chain of custody must be valid
//              * All transactions must be validated
//
//         - Claim Homesteaded
//              * Signature must be verified
//              * Claim must have a valid maturity timestamp
//              * Claim must have been either not been owned before, or abandoned.
//              * If claim already has owner in local state, acquisition time must be compared
//              * If the claim acquisition time is a perfect tie (to the nanosecond), 
//                then tie handling must occur first
//
//         - Claim Acquired
//              * Claim availability must be valid
//              * Claim previous owner signature must be verified
//              * Claim acquirer signature must be verified.
//              * Claim chain of custody must be valid going back to original owner (homesteader)
//              * Claim must NOT be currently staked
//              * Claim must NOT be mature already.
//              
//         - Smart Contract Deployed
//              * TODO: ALL Contract functionality requires development of VVM.
#[derive(Clone, Debug)]
pub enum Message {
    ClaimAcquired((Claim, String, u8)),
    ClaimHomesteaded((Claim, String, u8)),
    NewBlock((Block, String, u8)),
    Txn((Txn, u8)),
}
#[derive(Clone, Debug)]
pub struct Validator {
    node_wallet: WalletAccount,
    staked_claims: HashMap<u128, Claim>,
    message: Message,
    valid: bool,
}


impl Validator {
    pub fn new(message: Message, wallet: WalletAccount, account_state: AccountState) -> Option<Validator> {
        let check_staked_claims = account_state.claim_state.staked_claims
            .get(&wallet.public_key.to_string());

        match check_staked_claims {
            Some(map) => {
                return Some(Validator {
                    node_wallet: wallet,
                    staked_claims: map.clone(),
                    message,
                    valid: false,
                })
            },
            None => {
                return None
            }
        }
    }

    pub fn validate(&self) -> Self {
        match self.message.clone() {
            Message::Txn((txn, confirmations)) => { 
                match txn.is_valid(None) { 
                    Some(true) => {

                        return Self {
                            valid: true,
                            message: Message::Txn((txn, confirmations + 1)),
                            ..self.clone()
                        };
                    },
                    Some(false) => {
                        return Self {
                            ..self.clone()
                        };
                    },
                    None => {
                        panic!("Invalid Transaction Message");
                    }
                }
            },
            Message::ClaimAcquired((claim, pubkey, confirmations)) => {
                match claim.is_valid(Some(ClaimOption::Acquire)) {
                    Some(true) => {
                        return Self {
                            valid: true,
                            message: Message::ClaimAcquired((claim, pubkey, confirmations + 1)),
                            ..self.clone()
                        }
                    }
                    Some(false) => {
                        return self.clone()
                    },
                    None => {
                        panic!("Invalid Claim Acquisition Message!")
                    }
                }               

            },
            Message::ClaimHomesteaded((claim, pubkey, confirmations)) => {
                match claim.is_valid(Some(ClaimOption::Homestead)) {
                    Some(true) => { 
                        return Self {
                            valid: true,
                            message: Message::ClaimHomesteaded((claim, pubkey, confirmations + 1)),
                            ..self.clone()
                        }
                    },
                    Some(false) => { 
                        return self.clone()
                    },
                    None => {
                        panic!("Invalid Claim Homesteading Message!")
                    }, 
                }
            },
            Message::NewBlock((block, pubkey, confirmations)) => {
                match block.is_valid(None) {
                    Some(true) => {
                        return Self {
                            valid: true,
                            message: Message::NewBlock((block, pubkey, confirmations + 1)),
                            ..self.clone()
                        }
                    },
                    Some(false) => {
                        return self.clone()
                    },
                    None => {
                        panic!("Invalid Block Message!")
                    }
                }

            }
            
            }
        }
    }

#[cfg(test)]
mod tests {
    
    #[test]
    fn test_valid_simple_transaction() {

    }

    #[test]
    fn test_invalid_simple_transaction_bad_signature() {

    }

    #[test]
    fn test_invalid_simple_transaction_amount_exceeds_balance() {

    }

    #[test]
    fn test_invalid_simple_transaction_double_spend_attack() {
    }

    #[test]
    fn test_invalid_simple_transaction_non_existent_receiver() {

    }

    #[test]
    fn test_invalid_simple_transaction_non_existent_sender_in_last_confirmed_state() {

    }

    #[test]
    fn test_valid_block() {

    }

    #[test]
    fn test_invalid_block_bad_signature() {

    }

    #[test]
    fn test_invalid_block_bad_claim_invalid_owner_signature() {

    }

    #[test]
    fn test_invalid_block_invalid_state_hash() {

    }

    #[test]
    fn test_invalid_block_bad_reward() {

    }

    #[test]
    fn test_invalid_block_bad_claim_invalid_chain_of_custody() {

    }

    #[test]
    fn test_invalid_block_contains_invalid_transactions() {

    }


}