use std::collections::HashMap;
use crate::{account::{
        AccountState, 
        WalletAccount
    }, block::Block, claim::{
        Claim
    }, reward::{RewardState}, state::NetworkState, txn::Txn, verifiable::Verifiable};
use serde::{Serialize, Deserialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvalidMessageError {
    InvalidTxnError(String),
    InvalidClaimAcquisition(String),
    InvalidClaimHomesteading(String),
    InvalidBlock(String),
}

pub enum ValidatorOptions {
    ClaimHomestead(AccountState),
    ClaimAcquire(AccountState, String),
    NewBlock(NetworkState, AccountState, RewardState),
    Transaction(AccountState)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    ClaimAcquired(Claim, String, AccountState, String),
    ClaimHomesteaded(Claim, String, AccountState),
    NewBlock(Block, String),
    Txn(Txn, AccountState),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
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

        // If there's no staked claims for the node wallet attempting to launch a validator
        // a validator cannot be launched. Claims must be staked to validate messages
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
                // TODO: Propagate a useful message to the user informing them they have no
                // claims staked.
                return None
            }
        }
    }

    pub fn validate(&self) -> Self {
        // Match the message type. Message type can be one of 4 options
        // Txn, NewBlock, ClaimAcquired or ClaimHomesteaded
        // If a message's subject (transaction, claim transaction, or new block)
        // the valid field in needs to be changed to true
        // the validator struct get's pushed to an external vector.
        match self.message.clone() {
            Message::Txn(txn, account_state) => { 
                // If the Message variant is a transaction
                // then it needs to be processed.
                // All of the variant inners implement the Verifiable trait
                // which has an is_valid method, which receives an option (for claims)
                // which is either None or Some(ClaimOption). For Txns and Blocks it should
                // always be None. For claims it should always be some.
                // Is valid returns an Option<bool> which can either be Some(true), Some(false) or None,
                // a None option is an error, and should propagate an invalid message error.
                match txn.is_valid(Some(ValidatorOptions::Transaction(account_state.clone()))) {
                    // If the transaction is valid
                    // return the validator structure
                    // with the valid field set to true
                    // and the message variant, the rest of the validator
                    // remains the same (node_wallet, and staked_claims).
                    Some(true) => {

                        return Self {
                            valid: true,
                            message: Message::Txn(txn, account_state.clone()),
                            ..self.clone()
                        };
                    },
                    // Validators default to invalid (valid field set to false)
                    // So if it is indeed invalid, then just return the validator struct as is
                    Some(false) => {
                        return Self {
                            ..self.clone()
                        };
                    },
                    // If None, there's an error, true or false should ALWAYS be returned
                    // by the is_valid() method.
                    // TODO: convert to error propagation to be handled by the thread calling the
                    // method.
                    None => {
                        panic!("Invalid Transaction Message");
                    }
                }
            },
            Message::ClaimAcquired(
                claim, 
                seller_pubkey, 
                account_state, 
                buyer_pubkey
            ) => {
                // Claim acquisition is one of two types of claim messages that needs
                // to be validated. The claim.is_valid() method receives
                // a Some(ClaimOption::Acquire) option, so that it knows
                // that it is to validate the claim that is being acquired
                // not homestaeded.
                match claim.is_valid(Some(ValidatorOptions::ClaimAcquire(account_state.clone(), buyer_pubkey.clone()))) {
                    Some(true) => {
                        return Self {
                            valid: true,
                            message: Message::ClaimAcquired(claim, seller_pubkey.clone(), account_state, buyer_pubkey.clone()),
                            ..self.clone()
                        }
                    }
                    // Validator defaults to invalid so if the message
                    // subject is invalid just return the validator as is
                    Some(false) => {
                        return self.clone()
                    },
                    // If the is_valid() method returns none, something has gone wrong
                    // TODO: propagate custom error for main to handle
                    None => {
                        panic!("Invalid Claim Acquisition Message!")
                    }
                }               

            },
            Message::ClaimHomesteaded(claim, pubkey, account_state) => {
                // If the message is a claim homesteading message
                // the message will contain a claim and the wallet which
                // is attempting to homestead the claim's public key
                // Pass the claim.is_valid() method Some(ClaimOption::Homestead)
                // so that the method knows to implement logic related to validating
                // a homesteaded claim not an acquired claim.
                match claim.is_valid(Some(ValidatorOptions::ClaimHomestead(account_state.clone()))) {
                    // If the claim is validly homesteaded, return 
                    // the validator with the valid field set to tru
                    // and the message.
                    Some(true) => { 
                        return Self {
                            valid: true,
                            message: Message::ClaimHomesteaded(claim, pubkey, account_state.clone()),
                            ..self.clone()
                        }
                    },
                    // If the claim is invalidly homesteaded
                    // then return the validator as is
                    Some(false) => { 
                        return self.clone()
                    },
                    // If the is_valid() method returns none, then something
                    // went wrong.
                    // TODO: propogate a custom error to provide a message to be handled
                    // by the main.
                    None => {
                        panic!("Invalid Claim Homesteading Message!")
                    }, 
                }
            },
            Message::NewBlock(block, pubkey) => {
                // If a message is a new block, then check that the block is
                // valid, by calling the block.is_valid() method and passing None
                // as the options, as only Claim validation requires an option
                match block.is_valid(None) {
                    // If the is_valid() method returns Some(true)
                    // then the block is valid, and the validator
                    // should have it's valid field set to true
                    Some(true) => {
                        return Self {
                            valid: true,
                            message: Message::NewBlock(block, pubkey),
                            ..self.clone()
                        }
                    },
                    // If the is_valid() method returns Some(false)
                    // then return the validator as is.
                    Some(false) => {
                        return self.clone()
                    },
                    // If the is_valid() method returns None something has gone wrong
                    // TODO: propagate error.
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
    use super::*;
    use crate::{
        account::{
            AccountState, 
            WalletAccount
        }, 
        state::NetworkState,
        reward::RewardState,
    };

    #[test]
    fn test_valid_simple_transaction() {
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_simple_valid_txn.db");
        let reward_state = RewardState::start(&mut network_state);
        
        let (
            mut wallet_1, 
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );
        
        account_state = updated_account_state;

        let (
            mut wallet_2,
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state,
            &mut network_state,
        );

        account_state = updated_account_state;

        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();
        wallet_2 = wallet_2.get_balance(account_state.clone()).unwrap();

        let result = wallet_1.send_txn(
            &mut account_state, 
            (wallet_2.address.clone(), 15 as u128), 
            &mut network_state);

        match result {
            Ok((updated_wallet, updated_account_state)) => {
                wallet_1 = updated_wallet;
                account_state = updated_account_state;

            }
            Err(e) => println!("Error attempting to send txn to receiver: {} -> {}", 
                wallet_2.address.clone(), 
                e
            )
        }

        let txn_id = account_state.clone().pending.keys().cloned().collect::<Vec<String>>()[0].clone();
        let txn = account_state.clone().pending.get(&txn_id).unwrap().0.clone();
        let mut validators_vec = account_state.clone().pending.get(&txn_id).unwrap().1.clone();

        let (_block, updated_account_state) = Block::genesis(
            reward_state, &mut wallet_1, &mut account_state, &mut network_state).unwrap();

        account_state = updated_account_state;


        let (
            mut validator_wallet, 
            mut validator_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = validator_account_state.clone();
        
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }

        let validator = Validator::new(
            Message::Txn(
                txn.clone(), 
                validator_account_state.clone()
            ), validator_wallet, validator_account_state);

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                if validators_vec.len() < 3 {
                    validators_vec.push(processed.clone());
                    account_state.pending.insert(txn_id, (txn.clone(), validators_vec));
                }
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_invalid_simple_transaction_bad_signature() {

        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_invalid_signature_transaction.db");
        let reward_state = RewardState::start(&mut network_state);
        
        let (
            mut wallet_1, 
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );
        
        account_state = updated_account_state;

        let (
            mut wallet_2,
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state,
            &mut network_state,
        );

        account_state = updated_account_state;

        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();
        wallet_2 = wallet_2.get_balance(account_state.clone()).unwrap();

        let result = wallet_1.send_txn(
            &mut account_state, 
            (wallet_2.address.clone(), 15 as u128), 
            &mut network_state);

        match result {
            Ok((updated_wallet, updated_account_state)) => {
                wallet_1 = updated_wallet;
                account_state = updated_account_state;
            }

            Err(e) => println!("Error attempting to send txn to receiver: {} -> {}", 
                wallet_2.address.clone(), 
                e
            )
        }

        let txn_id = account_state.clone().pending.keys().cloned().collect::<Vec<String>>()[0].clone();
        let mut txn = account_state.clone().pending.get(&txn_id).unwrap().0.clone();
        let mut validators_vec = account_state.clone().pending.get(&txn_id).unwrap().1.clone();
        txn.txn_signature = wallet_1.sign(&"Malicious_Signature".to_string()).unwrap().to_string();

        let (_block, updated_account_state) = Block::genesis(
            reward_state, &mut wallet_1, &mut account_state, &mut network_state).unwrap();

        account_state = updated_account_state;

        let (
            mut validator_wallet, 
            mut validator_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = validator_account_state.clone();
        
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }

        let validator = Validator::new(
            Message::Txn(
                txn.clone(), 
                validator_account_state.clone()
            ), validator_wallet, validator_account_state);

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                if validators_vec.len() < 3 {
                    validators_vec.push(processed.clone());
                    account_state.pending.insert(txn_id, (txn.clone(), validators_vec));
                }
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_invalid_simple_transaction_amount_exceeds_balance() {

        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_simple_invalid_amount_txn.db");
        let reward_state = RewardState::start(&mut network_state);
        
        let (
            mut wallet_1, 
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );
        
        account_state = updated_account_state;

        let (
            mut wallet_2,
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state,
            &mut network_state,
        );

        account_state = updated_account_state;

        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();
        wallet_2 = wallet_2.get_balance(account_state.clone()).unwrap();

        let result = wallet_1.send_txn(
            &mut account_state, 
            (wallet_2.address.clone(), 50 as u128), 
            &mut network_state);

        match result {
            Ok((updated_wallet, updated_account_state)) => {
                wallet_1 = updated_wallet;
                account_state = updated_account_state;
            }
            Err(e) => println!("Error attempting to send txn to receiver: {} -> {}", 
                wallet_2.address.clone(), 
                e
            )
        }
        let txn_id = account_state.pending.keys().cloned().collect::<Vec<String>>()[0].clone();
        let mut txn = account_state.clone().pending.get(&txn_id).unwrap().0.clone();
        let mut validators_vec = account_state.clone().pending.get(&txn_id).unwrap().1.clone();

        txn.txn_amount = 1005;

        let (_block, updated_account_state) = Block::genesis(
            reward_state, &mut wallet_1, &mut account_state, &mut network_state).unwrap();

        account_state = updated_account_state;

        let (
            mut validator_wallet, 
            mut validator_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = validator_account_state.clone();
        
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }

        let validator = Validator::new(
            Message::Txn(
                txn.clone(), 
                validator_account_state.clone()
            ), validator_wallet, validator_account_state);

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                if validators_vec.len() < 3 {
                    validators_vec.push(processed.clone());
                    account_state.pending.insert(txn_id, (txn.clone(), validators_vec));
                }
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_invalid_simple_transaction_double_spend_attack() {
        
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_invalid_double_spend_attack.db");
        let reward_state = RewardState::start(&mut network_state);
        
        let (
            mut wallet_1, 
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );
        
        account_state = updated_account_state;

        let (
            mut wallet_2,
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state,
            &mut network_state,
        );

        account_state = updated_account_state;

        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();
        wallet_2 = wallet_2.get_balance(account_state.clone()).unwrap();

        let result = wallet_1.send_txn(
            &mut account_state, 
            (wallet_2.address.clone(), 50 as u128), 
            &mut network_state);

        match result {
            Ok((updated_wallet, updated_account_state)) => {
                wallet_1 = updated_wallet;
                account_state = updated_account_state;
            }
            Err(e) => println!("Error attempting to send txn to receiver: {} -> {}", 
                wallet_2.address.clone(), 
                e
            )
        }
        let txn_id = account_state.pending.keys().cloned().collect::<Vec<String>>()[0].clone();
        let txn = account_state.clone().pending.get(&txn_id).unwrap().0.clone();
        let mut validators_vec = account_state.clone().pending.get(&txn_id).unwrap().1.clone();

        let (
            wallet_3,
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state,
            &mut network_state,
        );

        account_state = updated_account_state;

        let mut double_spend_txn = txn.clone();
        double_spend_txn.receiver_address = wallet_3.address.to_string();

        let (_block, updated_account_state) = Block::genesis(
            reward_state, &mut wallet_1, &mut account_state, &mut network_state).unwrap();

        account_state = updated_account_state;

        let (
            mut validator_wallet, 
            mut validator_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = validator_account_state.clone();

        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }

        let validator_1 = Validator::new(
            Message::Txn(
                txn.clone(),
                validator_account_state.clone()
            ), 
            validator_wallet.clone(), 
            validator_account_state.clone()
            );

        let validator_2 = Validator::new(
            Message::Txn(
                double_spend_txn.clone(),
                validator_account_state.clone(),
            ),
            validator_wallet.clone(),
            validator_account_state.clone(),
        );

        match validator_1 {
            Some(validator) => {
                let processed = validator.validate();
                if validators_vec.len() < 3 {
                    validators_vec.push(processed.clone());
                    account_state.pending.insert(txn_id.clone(), (txn.clone(), validators_vec.clone()));
                }
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none")
        }

        match validator_2 {
            Some(validator) => {
                let processed = validator.validate();
                if validators_vec.len() < 3 {
                    validators_vec.push(processed.clone());
                    account_state.pending.insert(
                        double_spend_txn.clone().txn_id, 
                        (double_spend_txn.clone(), 
                        validators_vec
                    ));
                }
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_invalid_simple_transaction_non_existent_receiver() {
        
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_invalid_receiver_simple_txn.db");
        let _reward_state = RewardState::start(&mut network_state);
        
        let (
            mut wallet_1, 
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );
        
        account_state = updated_account_state;

        let (
            mut wallet_2,
            _updated_account_state
        ) = WalletAccount::new(
            &mut account_state,
            &mut network_state,
        );

        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();
        wallet_2 = wallet_2.get_balance(account_state.clone()).unwrap();

        let result = wallet_1.send_txn(
            &mut account_state, 
            (wallet_2.address.clone(), 15 as u128), 
            &mut network_state);
        
        #[allow(unused_assignments)]
        match result {
            Ok((updated_wallet, updated_account_state)) => {
                wallet_1 = updated_wallet;
                account_state = updated_account_state;
            }
            Err(e) => println!("Error attempting to send txn to receiver: {} -> {}", 
                wallet_2.address.clone(), 
                e
            )
        }
        
        let txn_id = account_state.pending.keys().cloned().collect::<Vec<String>>()[0].clone();
        let txn = account_state.clone().pending.get(&txn_id).unwrap().0.clone();
        let mut validators_vec = account_state.clone().pending.get(&txn_id).unwrap().1.clone();

        let (
            mut validator_wallet, 
            mut validator_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = validator_account_state.clone();
        
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }

        let validator = Validator::new(
            Message::Txn(
                txn.clone(), 
                validator_account_state.clone()
            ), validator_wallet, validator_account_state);

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                if validators_vec.len() < 3 {
                    validators_vec.push(processed.clone());
                    account_state.pending.insert(txn_id, (txn.clone(), validators_vec));
                }
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_invalid_simple_transaction_non_existent_sender_in_last_confirmed_state() {

        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_invalid_receiver_simple_txn.db");
        let _reward_state = RewardState::start(&mut network_state);
        
        let (
            mut wallet_1, 
            _updated_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        let (
            mut wallet_2,
            updated_account_state
        ) = WalletAccount::new(
            &mut account_state,
            &mut network_state,
        );
        account_state = updated_account_state;

        wallet_2 = wallet_2.get_balance(account_state.clone()).unwrap();

        let result = wallet_1.send_txn(
            &mut account_state, 
            (wallet_2.address.clone(), 15 as u128), 
            &mut network_state);

        #[allow(unused_assignments)]
        match result {
            Ok((updated_wallet, updated_account_state)) => {
                wallet_1 = updated_wallet;
                account_state = updated_account_state;

            }
            Err(e) => println!("Error attempting to send txn to receiver: {} -> {}", 
                wallet_2.address.clone(), 
                e
            )
        }
        
        let txn_id = account_state.pending.keys().cloned().collect::<Vec<String>>()[0].clone();
        let txn = account_state.clone().pending.get(&txn_id).unwrap().0.clone();
        let mut validators_vec = account_state.clone().pending.get(&txn_id).unwrap().1.clone();

        let (
            mut validator_wallet, 
            mut validator_account_state
        ) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = validator_account_state.clone();
        
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }

        let validator = Validator::new(
            Message::Txn(
                txn.clone(), 
                validator_account_state.clone()
            ), validator_wallet, validator_account_state);

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                if validators_vec.len() < 3 {
                    validators_vec.push(processed.clone());
                    account_state.pending.insert(txn_id, (txn.clone(), validators_vec));
                }
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }
    }
    
    #[allow(unused_assignments)]  
    #[test]
    fn test_valid_homesteading_valid_claim_signature() {

        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_valid_homestead_signature.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader_wallet.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        account_state = updated_account_state;
        let claim_state = account_state.clone().claim_state;
        let (ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();
      
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;

        let (
            mut validator_wallet, 
            updated_account_state
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
            );
        
        account_state = updated_account_state.clone();

        let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
        let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();

        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            account_state = updated_account_state;
        }

        let validator = Validator::new(
                                            Message::ClaimHomesteaded(
                                                claim_to_validate.clone(), 
                                                current_owner_pub_key, account_state.clone()), 
                                                validator_wallet, 
                                                account_state.clone()
                                            );

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[allow(unused_assignments)]  
    #[test]
    fn test_valid_homesteading_valid_claim_maturity_timestamp() {
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_valid_homestead_maturity_timestamp.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader_wallet.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        account_state = updated_account_state;
        let claim_state = account_state.clone().claim_state;
        let (ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();
        
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;

        let (
            mut validator_wallet, 
            updated_account_state
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
            );
        
        account_state = updated_account_state;

        let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
        let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            account_state = updated_account_state;
        }

        let validator = Validator::new(
                                            Message::ClaimHomesteaded(
                                                claim_to_validate.clone(), 
                                                current_owner_pub_key, account_state.clone()), 
                                                validator_wallet, 
                                                account_state.clone()
                                            );

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none")
        }

    }

    #[allow(unused_assignments)]      
    #[test]
    fn test_valid_homesteading_claim_unowned() {
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_valid_homestead_claim_unowned.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader_wallet.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        account_state = updated_account_state;
        let claim_state = account_state.clone().claim_state;
        let (ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();
        
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;
        

        let (
            mut validator_wallet, 
            updated_account_state
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
            );
        
        account_state = updated_account_state;

        let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
        let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            account_state = updated_account_state;
        }

        let validator = Validator::new(
                                            Message::ClaimHomesteaded(
                                                claim_to_validate.clone(), 
                                                current_owner_pub_key, account_state.clone()), 
                                                validator_wallet, 
                                                account_state.clone()
                                            );

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none")
        }

    }

    #[allow(unused_assignments)] 
    #[test]
    fn test_valid_homesteading_claim_first_appropriator() {
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_valid_homestead_claim_unowned.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (mut homesteader_wallet_2, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);
        
        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader_wallet_1.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        account_state = updated_account_state;
        
        let claim_state = account_state.clone().claim_state;
        
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet_1, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();
        
        let new_account_state = updated_account_state;

        homesteader_wallet_1 = updated_wallet;

        let claim_state = account_state.clone().claim_state;
        
        let (ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet_2, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();

        account_state = updated_account_state;
        homesteader_wallet_2 = updated_wallet;

        let (
            mut validator_wallet, 
            updated_account_state
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
            );
        
        account_state = updated_account_state;

        let claim_to_validate = new_account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
        let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            account_state = updated_account_state;
        }

        let validator = Validator::new(
                                            Message::ClaimHomesteaded(
                                                claim_to_validate.clone(), 
                                                current_owner_pub_key, account_state.clone()), 
                                                validator_wallet, 
                                                account_state.clone()
                                            );

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[allow(unused_assignments)]      
    #[test]
    fn test_invalid_homesteading_invalid_claim_singature() {

        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_invalid_homestead_signature.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader_wallet.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        account_state = updated_account_state;
        let claim_state = account_state.clone().claim_state;
        let (ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();
        
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;

        let (
            mut validator_wallet, 
            updated_account_state
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
            );
        
        account_state = updated_account_state;

        let mut claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
        claim_to_validate.current_owner.2 = Some(homesteader_wallet.sign(&"Malicious_Signature".to_string()).unwrap().to_string());
        let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            account_state = updated_account_state;
        }

        let validator = Validator::new(
                                            Message::ClaimHomesteaded(
                                                claim_to_validate.clone(), 
                                                current_owner_pub_key, account_state.clone()), 
                                                validator_wallet, 
                                                account_state.clone()
                                            );

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }

    }

    #[allow(unused_assignments)]  
    #[test]
    fn test_invalid_homesteading_invalid_claim_maturity_timestamp() {
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_valid_homestead_signature.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader_wallet.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        account_state = updated_account_state;
        let claim_state = account_state.clone().claim_state;
        let (ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();
        
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;

        let (
            mut validator_wallet, 
            updated_account_state
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
            );
        
        account_state = updated_account_state;

        let mut claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
        claim_to_validate.maturation_time += 1000000000;

        let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            account_state = updated_account_state;
        }

        let validator = Validator::new(
                                            Message::ClaimHomesteaded(
                                                claim_to_validate.clone(), 
                                                current_owner_pub_key, 
                                                account_state.clone()), 
                                                validator_wallet, 
                                                account_state.clone()
                                            );

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[allow(unused_assignments)]  
    #[test]
    fn test_invalid_homesteading_claim_already_owned() {
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_invalid_homestead_claim_owned.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader1_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, _updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader1_wallet.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        let (mut homesteader2_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;
        let claim_state = account_state.clone().claim_state;
        let (ts, claim_to_homestead) = claim_state.claims
                                                        .iter()
                                                        .min_by_key(|x| x.0)
                                                        .unwrap();

        let (updated_wallet1, updated_account_state1) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader1_wallet, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();

        let (_updated_wallet2, updated_account_state2) = claim_to_homestead.clone().homestead(&mut homesteader2_wallet, &mut claim_state.clone(), &mut account_state, &mut network_state).unwrap();

        homesteader1_wallet = updated_wallet1;
        account_state = updated_account_state1;       

        let (
            mut validator_wallet, 
            updated_account_state1
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
            );
        
        account_state = updated_account_state1;

        let claim_to_validate = updated_account_state2.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();

        let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            account_state = updated_account_state;
        }

        let validator = Validator::new(
                                            Message::ClaimHomesteaded(
                                                claim_to_validate.clone(), 
                                                current_owner_pub_key, account_state.clone()), 
                                                validator_wallet, 
                                                account_state.clone()
                                            );

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_valid_claim_acquired() {
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_valid_claim_acquired.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader_wallet.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        account_state = updated_account_state;

        let claim_state = account_state.clone().claim_state;
        let (ts, claim_to_homestead) = claim_state.claims
                                                        .iter()
                                                        .min_by_key(|x| x.0)
                                                        .unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();
        
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;

        let (claim, updated_account_state) = match homesteader_wallet.sell_claim(
            *ts, 
            &mut account_state,
             10
            ) 
            {
                Some((claim, account_state)) => {
                    (Some(claim), account_state)
                },
                None => {(None, account_state)}
        };

        account_state = updated_account_state;

        let (mut acquirer_wallet, updated_account_state) = WalletAccount::new(&mut account_state, &mut network_state);

        account_state = updated_account_state;
        println!("{:?}", &account_state);
        println!("{}", &acquirer_wallet);

        if let Some(mut claim) = claim {
            let (updated_wallet, updated_account_state) = claim.acquire(
                &mut acquirer_wallet, &mut account_state, &mut network_state).unwrap();
            
            acquirer_wallet = updated_wallet;
            account_state = updated_account_state;
        }

        let (
            mut validator_wallet, 
            updated_account_state
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
            );
        
        account_state = updated_account_state;

        let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();

        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            account_state = updated_account_state;
        }

        let validator = Validator::new(
                                            Message::ClaimAcquired(
                                                claim_to_validate.clone(), 
                                                homesteader_wallet.public_key, 
                                                account_state.clone(), 
                                                acquirer_wallet.address), 
                                                validator_wallet, 
                                                account_state.clone()
                                            );

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[should_panic]
    #[test]
    fn test_invalid_transaction_unavailable_claim() {
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_invalid_claim_unavailable.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader_wallet.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        account_state = updated_account_state;

        let claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                                        .iter()
                                                        .min_by_key(|x| x.0)
                                                        .unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();
        
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;

        let (mut acquirer_wallet, updated_account_state) = WalletAccount::new(&mut account_state, &mut network_state);

        account_state = updated_account_state;
        
        let claim = homesteader_wallet.clone().claims.pop().unwrap();


        // Currently panics! Should change to ensure that a malicious acquirer can't get through
        // the validator.
        if let Some(mut claim) = claim {
            let (_updated_wallet, _updated_account_state) = claim.acquire(
                &mut acquirer_wallet, &mut account_state, &mut network_state).unwrap();
            
            // acquirer_wallet = updated_wallet;
            // account_state = updated_account_state;
        }

        // let (
        //     mut validator_wallet, 
        //     updated_account_state
        // ) = WalletAccount::new(
        //         &mut account_state, &mut network_state
        // );
        
        // account_state = updated_account_state;

        // let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();

        // for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
        //     let (new_wallet, updated_account_state) = claim.to_owned().homestead(
        //         &mut validator_wallet, 
        //         &mut account_state.clone().claim_state, 
        //         &mut account_state.clone(), 
        //         &mut network_state
        //     ).unwrap();

        //     validator_wallet = new_wallet;
        //     account_state = updated_account_state;
        // }

        // for claim in validator_wallet.clone().claims.iter() {
        //     let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
        //     account_state = updated_account_state;
        // }

        // let validator = Validator::new(
        //                                     Message::ClaimAcquired(
        //                                         claim_to_validate.clone(), 
        //                                         homesteader_wallet.public_key, 
        //                                         account_state.clone(), 
        //                                         acquirer_wallet.address), 
        //                                         validator_wallet, 
        //                                         account_state.clone()
        //                                     );

        // match validator {
        //     Some(validator) => {
        //         let processed = validator.validate();
        //         assert_eq!(processed.valid, true);
        //     },
        //     None => println!("Issue with validator, returned none")
        // }
    }

    #[test]
    fn test_invalid_claim_acquire_staked_claim() {

        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_invalid_claim_staked.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader_wallet.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        account_state = updated_account_state;

        let claim_state = account_state.clone().claim_state;
        let (ts, claim_to_homestead) = claim_state.claims
                                                        .iter()
                                                        .min_by_key(|x| x.0)
                                                        .unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();
        
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;
            
        let claim = homesteader_wallet.claims.get(0).unwrap().clone().unwrap();

        let updated_account_state = claim.stake(homesteader_wallet.clone(), &mut account_state);

        account_state = updated_account_state;
        println!("{:?}", account_state);

        let (claim, updated_account_state) = match homesteader_wallet.sell_claim(
            *ts, 
            &mut account_state,
             10
            ) 
            {
                Some((claim, account_state)) => {
                    (Some(claim), account_state)
                },
                None => {(None, account_state)}
        };

        account_state = updated_account_state;

        let (mut acquirer_wallet, updated_account_state) = WalletAccount::new(&mut account_state, &mut network_state);

        account_state = updated_account_state;

        if let Some(mut claim) = claim {
            let (updated_wallet, updated_account_state) = claim.acquire(
                &mut acquirer_wallet, &mut account_state, &mut network_state).unwrap();
            
            acquirer_wallet = updated_wallet;
            account_state = updated_account_state;
        }

        let (
            mut validator_wallet, 
            updated_account_state
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
            );
        
        account_state = updated_account_state;

        let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();

        for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
            let (new_wallet, updated_account_state) = claim.to_owned().homestead(
                &mut validator_wallet, 
                &mut account_state.clone().claim_state, 
                &mut account_state.clone(), 
                &mut network_state
            ).unwrap();

            validator_wallet = new_wallet;
            account_state = updated_account_state;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            account_state = updated_account_state;
        }

        let validator = Validator::new(
                                            Message::ClaimAcquired(
                                                claim_to_validate.clone(), 
                                                homesteader_wallet.public_key, 
                                                account_state.clone(), 
                                                acquirer_wallet.address), 
                                                validator_wallet, 
                                                account_state.clone()
                                            );

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }

    }

    #[test]
    fn test_invalid_claim_acquire_invalid_balance() {

    }

    #[test]
    fn test_invalid_claim_acquire_invalid_chain_of_custody() {

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