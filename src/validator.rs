use std::collections::HashMap;
use crate::{account::{
        AccountState, 
        WalletAccount
    }, block::Block, claim::{
        Claim
    }, mpu, reward::{RewardState}, state::NetworkState, txn::Txn};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvalidMessageError {
    InvalidTxnError(String),
    InvalidClaimAcquisition(String),
    InvalidClaimHomesteading(String),
    InvalidBlock(String),
}

#[derive(Serialize, Deserialize)]
pub enum ValidatorOptions {
    ClaimHomestead(AccountState),
    ClaimAcquire(AccountState, String),
    NewBlock(Box<(Block, Block, String, AccountState, RewardState, NetworkState)>),
    Transaction(AccountState)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    ClaimAcquired(Claim, String, AccountState, String),
    ClaimHomesteaded(Claim, String, AccountState),
    NewBlock(Box<(Block, Block, String, NetworkState, AccountState, RewardState)>),
    Txn(Txn, AccountState),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    pub node_wallet: WalletAccount,
    pub staked_claims: HashMap<u128, Claim>,
    pub message: Message,
    pub valid: bool,
}

impl Validator {
    pub fn new(message: Message, wallet: WalletAccount, account_state: AccountState) -> Option<Validator> {
        let check_staked_claims = account_state.claim_state.staked_claims
            .get(&wallet.public_key);

        // If there's no staked claims for the node wallet attempting to launch a validator
        // a validator cannot be launched. Claims must be staked to validate messages
        check_staked_claims.map(|map| Validator {
            node_wallet: wallet, staked_claims: map.clone(), message, valid: false
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


#[cfg(test)]
mod tests {
    use std::{time::{SystemTime, UNIX_EPOCH}};
    use super::*;
    use crate::{account::{
            AccountState,
            StateOption, 
            WalletAccount
        }, 
        claim::CustodianInfo, 
        reward::{RewardState}, 
        state::{NetworkState}, utils::{txn_test_setup}};
    use sha256::digest_bytes;

    #[test]
    fn test_valid_simple_transaction() {
        let state_path = "test_valid_simple_txn.db";
        let (
            mut _wallet_1, 
            mut _wallet_2,
            mut account_state, 
            mut network_state,
            mut _reward_state,
            txn,
            mut validators_vec, 
        ) = txn_test_setup(state_path).unwrap();

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
                    account_state.pending.insert(txn.clone().txn_id, (txn.clone(), validators_vec));
                }
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_invalid_simple_transaction_bad_signature() {
        let state_path = "test_invalid_simple_txn_bad_sig.db";
        let (
            wallet_1, 
            _wallet_2,
            mut account_state, 
            mut network_state,
            reward_state,
            mut txn,
            mut validators_vec, 
        ) = txn_test_setup(state_path).unwrap();

        txn.txn_signature = wallet_1.sign(&"Malicious_Signature".to_string()).unwrap().to_string();

        let (_block, updated_account_state) = Block::genesis(
            reward_state, 
            wallet_1.address.clone(), 
            &mut account_state, 
            &mut network_state).unwrap();

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
                    account_state.pending.insert(txn.clone().txn_id, (txn.clone(), validators_vec));
                }
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_invalid_simple_transaction_amount_exceeds_balance() {
        let state_path = "test_invalid_simple_txn_amt_exceeds_balance.db";

        let (
            wallet_1, 
            _wallet_2,
            mut account_state, 
            mut network_state,
            reward_state,
            mut txn,
            mut validators_vec, 
        ) = txn_test_setup(state_path).unwrap();

        txn.txn_amount = 1005;

        let (_block, updated_account_state) = Block::genesis(
            reward_state, 
            wallet_1.address.clone(), 
            &mut account_state, 
            &mut network_state,
        ).unwrap();

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
                    account_state.pending.insert(txn.clone().txn_id, (txn.clone(), validators_vec));
                }
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_invalid_simple_transaction_double_spend_attack() {
        
        let state_path = "test_invalid_double_spend_attack.db";

        let (
            wallet_1, 
            _wallet_2,
            mut account_state, 
            mut network_state,
            reward_state,
            txn,
            mut validators_vec, 
        ) = txn_test_setup(state_path).unwrap();

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
            reward_state, 
            wallet_1.address.clone(), 
            &mut account_state, 
            &mut network_state,
        ).unwrap();

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
                    account_state.pending.insert(txn.txn_id.clone(), (txn.clone(), validators_vec.clone()));
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
        let state_path = "test_invalid_receiver_simple_txn.db";

        let (_wallet_1, 
            _wallet_2,
            mut account_state,
            mut network_state,
            _reward_state,
            mut txn,
            mut validators_vec,) = txn_test_setup(state_path).unwrap();

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

        txn.receiver_address = "unknown_receiver".to_string();

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
                    account_state.pending.insert(txn.clone().txn_id, (txn.clone(), validators_vec));
                }
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_invalid_simple_transaction_non_existent_sender_in_last_confirmed_state() {

        let state_path = "test_invalid_receiver_simple_txn.db";

        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
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
        let state_path = "test_valid_homestead_signature.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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
        let state_path = "test_valid_homestead_maturity_timestamp.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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
        let state_path = "test_valid_homestead_claim_unowned.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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
        let state_path = "test_valid_homestead_claim_unowned.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (mut homesteader_wallet_2, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);
        
        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet_1.address.clone(), 
            &mut account_state, 
            &mut network_state,
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
        let state_path = "test_invalid_homestead_signature.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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
        let state_path = "test_valid_homestead_signature.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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
        let state_path = "test_invalid_homestead_claim_owned.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader1_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, _updated_account_state) = Block::genesis(
            reward_state, 
            homesteader1_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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
        let state_path = "test_valid_claim_acquired.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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
        let state_path = "test_invalid_claim_unavailable.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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

        if let Some(mut claim) = claim {
            let (_updated_wallet, _updated_account_state) = claim.acquire(
                &mut acquirer_wallet, &mut account_state, &mut network_state).unwrap();

        }
    }

    #[test]
    fn test_invalid_claim_acquire_staked_claim() {

        let state_path = "test_invalid_claim_staked.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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
        let state_path = "test_invalid_claim_staked.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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
        println!("{:?}", &account_state);

        let (mut acquirer_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = updated_account_state;

        if let Some(mut claim) = claim {
            let (updated_wallet, _updated_account_state) = claim.acquire(
                &mut acquirer_wallet, &mut account_state, &mut network_state).unwrap();
            
            acquirer_wallet = updated_wallet;
        }

        let (
            mut validator_wallet, 
            updated_account_state
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
            );
        
        account_state = updated_account_state;

        let mut claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
        
        claim_to_validate.price = 15000;

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

    #[allow(unused_assignments)]
    #[test]
    fn test_invalid_claim_acquire_invalid_chain_of_custody() {
        let state_path = "test_valid_claim_acquired.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
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

        let mut claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
        
        let mut coc = claim_to_validate.clone().chain_of_custody;
        
        let (malicious_acquirer_wallet, updated_account_state) = WalletAccount::new(&mut account_state, &mut network_state);
        
        account_state = updated_account_state;

        let mut malicious_acquirer = HashMap::new();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        malicious_acquirer
            .insert("acquisition_timestamp".to_string(), Some(
                CustodianInfo::AcquisitionTimestamp(now.as_nanos())));

        malicious_acquirer
            .insert("acquired_from".to_string(),Some(CustodianInfo::AcquiredFrom(
                (Some("seller".to_string()), Some("seller_pubkey".to_string()), Some("Seller_signature".to_string())),
            )));

        malicious_acquirer
            .insert("acquisition_price".to_string(),Some(CustodianInfo::AcquisitionPrice(claim_to_validate.clone().price)));

        malicious_acquirer
            .insert("owner_number".to_string(), Some(CustodianInfo::OwnerNumber(2 + 1)));

        coc.insert(malicious_acquirer_wallet.address.clone(), malicious_acquirer);
        
        claim_to_validate.chain_of_custody = coc;

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
                                                malicious_acquirer_wallet.address.clone()), 
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
    #[allow(unused_assignments, dead_code)]
    fn test_valid_block() {
        let state_path = "test_valid_block.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);

        let (mut wallet_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;

        let (genesis, updated_account_state) = Block::genesis(
            reward_state, 
            wallet_1.clone().address, 
            &mut account_state, 
            &mut network_state).unwrap();
        
        account_state = updated_account_state;

        let mut claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        
        let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut wallet_1, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        wallet_1 = updated_wallet;
        account_state = updated_account_state;

        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

        let claim = wallet_1.clone().claims[0].clone().unwrap();

        let (mut validator_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = updated_account_state;

        let mut validator_account_state = account_state.clone();

        for (_ts, claim) in validator_account_state.clone().claim_state.claims {
            let (updated_wallet, updated_account_state) = claim.clone().homestead(
                &mut validator_wallet, 
                &mut claim_state, 
                &mut account_state.clone(), 
                &mut network_state).unwrap();
            
            validator_account_state = updated_account_state;
            validator_wallet = updated_wallet;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }

        let (block, updated_account_state) = Block::mine(
            &reward_state, 
            claim, 
            genesis.clone(), 
            HashMap::new(),
            &mut account_state.clone(), 
            &mut network_state.clone()
        ).unwrap().unwrap();

        account_state = updated_account_state.clone();

        let validator = Validator::new(
            Message::NewBlock(Box::new((
                genesis, 
                block, 
                wallet_1.public_key, 
                network_state, validator_account_state.clone(), reward_state
            ))), validator_wallet, validator_account_state.clone()
        );
        
        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none")
        }
    }

    #[test]
    fn test_invalid_block_bad_signature() {
        let state_path = "test_invalid_block_bad_signature.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);

        let (mut wallet_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;

        let (genesis, updated_account_state) = Block::genesis(
            reward_state, 
            wallet_1.clone().address, 
            &mut account_state, 
            &mut network_state).unwrap();
        
        account_state = updated_account_state;

        let mut claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        
        let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut wallet_1, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        wallet_1 = updated_wallet;
        account_state = updated_account_state;

        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

        let claim = wallet_1.clone().claims[0].clone().unwrap();

        let (mut validator_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = updated_account_state;

        let mut validator_account_state = account_state.clone();

        for (_ts, claim) in validator_account_state.clone().claim_state.claims {
            let (updated_wallet, updated_account_state) = claim.clone().homestead(
                &mut validator_wallet, 
                &mut claim_state, 
                &mut account_state.clone(), 
                &mut network_state).unwrap();
            
            validator_account_state = updated_account_state;
            validator_wallet = updated_wallet;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }

        let (mut block, updated_account_state) = Block::mine(
            &reward_state, 
            claim, 
            genesis.clone(), 
            HashMap::new(),
            &mut account_state.clone(), 
            &mut network_state.clone()
        ).unwrap().unwrap();

        account_state = updated_account_state.clone();

        block.claim.current_owner.2 = Some(wallet_1.sign(&"malicious_signature".to_string())
            .unwrap()
            .to_string());

        let validator = Validator::new(
            Message::NewBlock(Box::new((
                genesis, 
                block, 
                wallet_1.public_key, 
                network_state, validator_account_state.clone(), reward_state
            ))), validator_wallet, validator_account_state.clone()
        );
        
        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none: {:?}", account_state)
        }
    }

    #[test]
    fn test_invalid_block_invalid_state_hash() {
        let state_path = "test_invalid_block_bad_signature.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);

        let (mut wallet_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;

        let (genesis, updated_account_state) = Block::genesis(
            reward_state, 
            wallet_1.clone().address, 
            &mut account_state, 
            &mut network_state).unwrap();
        
        account_state = updated_account_state;

        let mut claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        
        let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut wallet_1, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        wallet_1 = updated_wallet;
        account_state = updated_account_state;

        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

        let claim = wallet_1.clone().claims[0].clone().unwrap();

        let (block, updated_account_state) = Block::mine(
            &reward_state, 
            claim, 
            genesis.clone(), 
            HashMap::new(),
            &mut account_state.clone(), 
            &mut network_state.clone()
        ).unwrap().unwrap();

        account_state = updated_account_state.clone();
    
        let (mut validator_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = updated_account_state;

        let mut validator_account_state = account_state.clone();

        for (_ts, claim) in validator_account_state.clone().claim_state.claims {
            let (updated_wallet, updated_account_state) = claim.clone().homestead(
                &mut validator_wallet, 
                &mut claim_state, 
                &mut account_state.clone(), 
                &mut network_state).unwrap();
            
            validator_account_state = updated_account_state;
            validator_wallet = updated_wallet;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }


        let validator = Validator::new(
            Message::NewBlock(Box::new((
                genesis, 
                block, 
                wallet_1.public_key, 
                network_state, validator_account_state.clone(), reward_state
            ))), validator_wallet, validator_account_state.clone()
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
    fn test_invalid_block_bad_last_block_hash() {
        let state_path = "test_invalid_block_bad_signature.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);

        let (mut wallet_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;

        let (genesis, updated_account_state) = Block::genesis(
            reward_state, 
            wallet_1.clone().address, 
            &mut account_state, 
            &mut network_state).unwrap();
        
        account_state = updated_account_state;

        let mut claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        
        let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut wallet_1, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        wallet_1 = updated_wallet;
        account_state = updated_account_state;

        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

        let claim = wallet_1.clone().claims[0].clone().unwrap();

        let (mut validator_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = updated_account_state;

        let mut validator_account_state = account_state.clone();

        for (_ts, claim) in validator_account_state.clone().claim_state.claims {
            let (updated_wallet, updated_account_state) = claim.clone().homestead(
                &mut validator_wallet, 
                &mut claim_state, 
                &mut account_state.clone(), 
                &mut network_state).unwrap();
            
            validator_account_state = updated_account_state;
            validator_wallet = updated_wallet;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }

        let (mut block, updated_account_state) = Block::mine(
            &reward_state, 
            claim, 
            genesis.clone(), 
            HashMap::new(),
            &mut account_state.clone(), 
            &mut network_state.clone()
        ).unwrap().unwrap();

        account_state = updated_account_state.clone();

        block.last_block_hash = digest_bytes("malicious_last_block_hash".as_bytes());

        let validator = Validator::new(
            Message::NewBlock(Box::new((
                genesis, 
                block, 
                wallet_1.public_key, 
                network_state, validator_account_state.clone(), reward_state
            ))), validator_wallet, validator_account_state.clone()
        );
        
        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none: {:?}", account_state)
        }
    }

    #[test]
    fn test_invalid_block_bad_reward() {
        let state_path = "test_invalid_block_bad_signature.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);

        let (mut wallet_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;

        let (genesis, updated_account_state) = Block::genesis(
            reward_state, 
            wallet_1.clone().address, 
            &mut account_state, 
            &mut network_state).unwrap();
        
        account_state = updated_account_state;

        let mut claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        
        let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut wallet_1, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        wallet_1 = updated_wallet;
        account_state = updated_account_state;

        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

        let claim = wallet_1.clone().claims[0].clone().unwrap();

        let (mut validator_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, 
            &mut network_state
        );

        account_state = updated_account_state;

        let mut validator_account_state = account_state.clone();

        for (_ts, claim) in validator_account_state.clone().claim_state.claims {
            let (updated_wallet, updated_account_state) = claim.clone().homestead(
                &mut validator_wallet, 
                &mut claim_state, 
                &mut account_state.clone(), 
                &mut network_state).unwrap();
            
            validator_account_state = updated_account_state;
            validator_wallet = updated_wallet;
        }

        for claim in validator_wallet.clone().claims.iter() {
            let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
            validator_account_state = updated_account_state;
        }

        let (mut block, updated_account_state) = Block::mine(
            &reward_state, 
            claim, 
            genesis.clone(), 
            HashMap::new(),
            &mut account_state.clone(), 
            &mut network_state.clone()
        ).unwrap().unwrap();

        account_state = updated_account_state.clone();

        block.block_reward.amount = 90;
        
        let validator = Validator::new(
            Message::NewBlock(Box::new((
                genesis, 
                block, 
                wallet_1.public_key, 
                network_state, validator_account_state.clone(), reward_state
            ))), validator_wallet, validator_account_state.clone()
        );
        
        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none: {:?}", account_state)
        }
    }

    #[test]
    fn test_valid_block_valid_txns() {
        let state_path = "test_invalid_block_bad_signature.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);

        let (mut wallet_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;

        let (wallet_2, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;

        let (mut validator_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;
        
        let (mut validator_2, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );
        
        account_state = updated_account_state;

        let (mut validator_3, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;
        
        let (genesis, updated_account_state) = Block::genesis(
            reward_state, 
            wallet_1.clone().address, 
            &mut account_state, 
            &mut network_state).unwrap();
        
        account_state = updated_account_state;
        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

        let mut claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        
        let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut wallet_1, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        wallet_1 = updated_wallet;
        account_state = updated_account_state;

        claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        
        let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut validator_1, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        validator_1 = updated_wallet;
        account_state = updated_account_state;

        account_state = validator_1.claims[0].clone().unwrap().stake(validator_1.clone(), &mut account_state);

        claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut validator_2, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        validator_2 = updated_wallet;
        account_state = updated_account_state;
        
        account_state = validator_2.claims[0].clone().unwrap().stake(validator_2.clone(), &mut account_state);
        
        claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut validator_3, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        validator_3 = updated_wallet;
        account_state = updated_account_state;

        account_state = validator_3.claims[0].clone().unwrap().stake(validator_3.clone(), &mut account_state);
        
        claim_state = account_state.clone().claim_state;
        let (updated_wallet, updated_account_state) = wallet_1.send_txn(
            &mut account_state, 
            (wallet_2.address, 15), 
            &mut network_state).unwrap();

        wallet_1 = updated_wallet;
        account_state = updated_account_state;

        let pending = account_state.clone().pending;

        let wallet_1_txn = pending
            .iter().filter(|&x| x.1.0.sender_address == wallet_1.address)
            .map(|x| x.clone()).collect::<Vec<_>>()[0];
            
        let first_validator = Validator::new(
            Message::Txn(
                wallet_1_txn.1.0.clone(), 
                account_state.clone()
            ), validator_1.clone(), account_state.clone()
        ).unwrap().validate();

        account_state = account_state
            .update(StateOption::ConfirmedTxn(Box::new((wallet_1_txn.1.0.clone(), first_validator))), &mut network_state).unwrap();

        let second_validator = Validator::new(
            Message::Txn(
                wallet_1_txn.1.0.clone(), 
                account_state.clone()
            ), validator_2, account_state.clone()
        ).unwrap().validate();

        account_state = account_state.clone()
            .update(StateOption::ConfirmedTxn(Box::new((wallet_1_txn.1.0.clone(), second_validator))), &mut network_state).unwrap();

        let third_validator = Validator::new(
            Message::Txn(
                wallet_1_txn.1.0.clone(), 
                account_state.clone()
            ), validator_3, account_state.clone()
        ).unwrap().validate();
        
        account_state = account_state.clone()
            .update(StateOption::ConfirmedTxn(Box::new((wallet_1_txn.1.0.clone(), third_validator))), &mut network_state).unwrap();

        let claim: Vec<Claim> = account_state.clone().claim_state.clone().owned_claims
            .iter().filter(|&claim| claim.1.clone().current_owner.clone().0.unwrap() == wallet_1.address
        ).map(|claim| claim.1.clone()).collect::<Vec<_>>();

        let data: HashMap<String, Txn> = account_state.mineable.iter().map(|(key, value)| {
            return (key.clone(), value.0.clone())
        }).collect();

        let validator_account_state = account_state.clone();

        let (block, updated_account_state) = Block::mine(
            &reward_state, 
            claim[0].clone(), 
            genesis.clone(), 
            data,
            &mut account_state.clone(), 
            &mut network_state.clone()
        ).unwrap().unwrap();

        account_state = updated_account_state.clone();
        
        let block_validator = Validator::new(
            Message::NewBlock(Box::new((
                genesis, 
                block, 
                wallet_1.public_key, 
                network_state, validator_account_state.clone(), reward_state
            ))), validator_1, validator_account_state.clone()
        );
        
        match block_validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none: {:?} -> {:?}", account_state, claim_state)
        }     
    }

    #[test]
    fn test_invalid_block_contains_invalid_transactions() {
        let state_path = "test_invalid_block_bad_signature.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);

        let (mut wallet_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;

        let (wallet_2, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;

        let (mut validator_1, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;
        
        let (mut validator_2, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );
        
        account_state = updated_account_state;

        let (mut validator_3, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state
        );

        account_state = updated_account_state;
        
        let (genesis, updated_account_state) = Block::genesis(
            reward_state, 
            wallet_1.clone().address, 
            &mut account_state, 
            &mut network_state).unwrap();
        
        account_state = updated_account_state;
        wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

        let mut claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        
        let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut wallet_1, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        wallet_1 = updated_wallet;
        account_state = updated_account_state;

        claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();
        
        let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut validator_1, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        validator_1 = updated_wallet;
        account_state = updated_account_state;

        account_state = validator_1.claims[0].clone().unwrap().stake(validator_1.clone(), &mut account_state);

        claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut validator_2, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        validator_2 = updated_wallet;
        account_state = updated_account_state;
        
        account_state = validator_2.claims[0].clone().unwrap().stake(validator_2.clone(), &mut account_state);
        
        claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                            .iter()
                                            .min_by_key(|x| x.0).unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut validator_3, 
            &mut account_state.clone().claim_state, 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        validator_3 = updated_wallet;
        account_state = updated_account_state;

        account_state = validator_3.claims[0].clone().unwrap().stake(validator_3.clone(), &mut account_state);
        
        claim_state = account_state.clone().claim_state;

        let (updated_wallet, updated_account_state) = wallet_1.send_txn(
            &mut account_state, 
            (wallet_2.address, 15), 
            &mut network_state).unwrap();

        wallet_1 = updated_wallet;
        account_state = updated_account_state;

        let pending = account_state.clone().pending;

        let wallet_1_txn = pending
            .iter().filter(|&x| x.1.0.sender_address == wallet_1.address)
            .map(|x| x.clone()).collect::<Vec<_>>()[0];

        let mut txn_to_validate: Txn = wallet_1_txn.1.0.clone();
        txn_to_validate.txn_signature = wallet_1.sign(&"malicious_signature".to_string()).unwrap().to_string();

        let first_validator = Validator::new(
            Message::Txn(
                txn_to_validate.clone(), 
                account_state.clone()
            ), validator_1.clone(), account_state.clone()
        ).unwrap();

        account_state = account_state
            .update(StateOption::ConfirmedTxn(Box::new((wallet_1_txn.1.0.clone(), first_validator))), &mut network_state).unwrap();

        let second_validator = Validator::new(
            Message::Txn(
                txn_to_validate.clone(), 
                account_state.clone()
            ), validator_2, account_state.clone()
        ).unwrap();

        account_state = account_state.clone()
            .update(StateOption::ConfirmedTxn(Box::new((wallet_1_txn.1.0.clone(), second_validator))), &mut network_state).unwrap();

        let third_validator = Validator::new(
            Message::Txn(
                txn_to_validate.clone(), 
                account_state.clone()
            ), validator_3, account_state.clone()
        ).unwrap();
        
        let updated_account_state = match account_state.clone()
            .update(StateOption::ConfirmedTxn(Box::new((wallet_1_txn.1.0.clone(), third_validator))), &mut network_state) {
                Ok(account_state) => { account_state },
                _ => { account_state }
            };
        
        account_state = updated_account_state;

        let claim: Vec<Claim> = account_state.clone().claim_state.clone().owned_claims
            .iter().filter(|&claim| claim.1.clone().current_owner.clone().0.unwrap() == wallet_1.address
        ).map(|claim| claim.1.clone()).collect::<Vec<_>>();

        let data: HashMap<String, Txn> = account_state.mineable.iter().map(|(key, value)| {
            return (key.clone(), value.0.clone())
        }).collect();

        let validator_account_state = account_state.clone();

        let (block, updated_account_state) = Block::mine(
            &reward_state, 
            claim[0].clone(), 
            genesis.clone(), 
            data,
            &mut account_state.clone(), 
            &mut network_state.clone()
        ).unwrap().unwrap();

        account_state = updated_account_state.clone();
        assert!(block.data.is_empty());

        let block_validator = Validator::new(
            Message::NewBlock(Box::new((
                genesis, 
                block, 
                wallet_1.public_key, 
                network_state, validator_account_state.clone(), reward_state
            ))), validator_1, validator_account_state.clone()
        );
        
        match block_validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, true);
            },
            None => println!("Issue with validator, returned none: {:?} -> {:?}", account_state, claim_state)
        }     
    }

    #[test]
    fn test_max_number_of_claims() {
        let state_path = "test_valid_homestead_signature.db";
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore(state_path);
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            homesteader_wallet.address.clone(), 
            &mut account_state, 
            &mut network_state,
        ).unwrap();

        account_state = updated_account_state;

        for (_ts, claim) in account_state.clone().claim_state.claims {

            let (updated_wallet, updated_account_state) = claim.clone().homestead(
                                                            &mut homesteader_wallet, 
                                                            &mut account_state.clone().claim_state, 
                                                            &mut account_state.clone(), 
                                                            &mut network_state).unwrap();
            homesteader_wallet = updated_wallet;
            account_state = updated_account_state;
        }

        let (
            mut validator_wallet, 
            updated_account_state
        ) = WalletAccount::new(
                &mut account_state, &mut network_state
        );
        
        account_state = updated_account_state.clone();

        let maturation_time = homesteader_wallet.claims
            .iter()
            .map(|claim| claim.clone().unwrap().maturation_time)
            .min_by(|a, b| a.cmp(&b)).unwrap();

        let claim = account_state.claim_state.owned_claims.get(&maturation_time).unwrap().to_owned();

        let (_block, updated_account_state) = Block::mine(
            &reward_state, 
            claim, 
            _genesis_block, 
            HashMap::new(), 
            &mut account_state,
            &mut network_state
        ).unwrap().unwrap();

        account_state = updated_account_state;

        let claim_state = account_state.clone().claim_state;

        let (_ts, claim_to_homestead) = claim_state.claims.iter()
            .min_by_key(|entry| entry.0).unwrap();

        let mut claim_state = account_state.clone().claim_state;

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
            &mut homesteader_wallet, &mut claim_state, &mut account_state, &mut network_state).unwrap();
        
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;

        let claim_state = account_state.clone().claim_state;

        let (ts, claim_to_homestead) = claim_state.claims.iter()
            .min_by_key(|entry| entry.0).unwrap();
        
        let mut claim_state = account_state.clone().claim_state;

        let (updated_wallet, updated_account_state) = claim_to_homestead
            .clone().homestead(&mut homesteader_wallet, &mut claim_state, &mut account_state, &mut network_state).unwrap();
        
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;

        let claim_to_validate = claim_state.owned_claims.get(ts).unwrap();

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
                                                account_state.clone()
                                            ), 
                                                validator_wallet, 
                                                account_state.clone()
                                            );

        match validator {
            Some(validator) => {
                let processed = validator.validate();
                assert_eq!(processed.valid, false);
            },
            None => println!("Issue with validator, returned none: {:?}", homesteader_wallet)
        }        
    }
}
