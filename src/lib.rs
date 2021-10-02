pub mod account;
pub mod block;
pub mod blockchain;
pub mod claim;
pub mod handler;
pub mod header;
pub mod miner;
pub mod network;
pub mod pool;
pub mod reward;
pub mod state;
pub mod txn;
pub mod utils;
pub mod validator;
pub mod verifiable;
pub mod wallet;

// #[cfg(test)]
// mod tests {
//     use std::{time::{SystemTime, UNIX_EPOCH}};
//     use super::*;
//     use crate::{account::{
//             AccountState,
//             StateOption,
//             WalletAccount
//         },
//         claim::CustodianInfo,
//         reward::{RewardState},
//         state::{NetworkState}, utils::{txn_test_setup}};
//     use sha256::digest_bytes;

//     #[test]
//     fn test_valid_simple_transaction() {
//         let state_path = "test_valid_simple_txn.db";
//         let (
//             mut _wallet_1,
//             mut _wallet_2,
//             mut account_state,
//             mut network_state,
//             mut _reward_state,
//             txn,
//             mut validators_vec,
//         ) = txn_test_setup(state_path).unwrap();

//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );
//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         let mut validator_account_state = account_state.clone();

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&txn).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//             ), validator_wallet, validator_account_state);

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 if validators_vec.len() < 3 {
//                     validators_vec.push(processed.clone());
//                     account_state.pending.insert(txn.clone().txn_id, (txn.clone(), validators_vec));
//                 }
//                 assert_eq!(processed.valid, true);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[test]
//     fn test_invalid_simple_transaction_bad_signature() {
//         let state_path = "test_invalid_simple_txn_bad_sig.db";
//         let (
//             wallet_1,
//             _wallet_2,
//             mut account_state,
//             mut network_state,
//             reward_state,
//             mut txn,
//             mut validators_vec,
//         ) = txn_test_setup(state_path).unwrap();

//         txn.txn_signature = wallet_1.sign(&"Malicious_Signature".to_string()).unwrap().to_string();

//         let (_block, updated_account_state) = Block::genesis(
//             reward_state,
//             wallet_1.address.clone(),
//             &mut account_state,
//             &mut network_state).unwrap();

//         account_state = updated_account_state;

//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );
//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         let mut validator_account_state = account_state.clone();

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&txn).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//             ), validator_wallet, validator_account_state);

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 if validators_vec.len() < 3 {
//                     validators_vec.push(processed.clone());
//                     account_state.pending.insert(txn.clone().txn_id, (txn.clone(), validators_vec));
//                 }
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[test]
//     fn test_invalid_simple_transaction_amount_exceeds_balance() {
//         let state_path = "test_invalid_simple_txn_amt_exceeds_balance.db";

//         let (
//             wallet_1,
//             _wallet_2,
//             mut account_state,
//             mut network_state,
//             reward_state,
//             mut txn,
//             mut validators_vec,
//         ) = txn_test_setup(state_path).unwrap();

//         txn.txn_amount = 1005;

//         let (_block, updated_account_state) = Block::genesis(
//             reward_state,
//             wallet_1.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;

//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );
//         let mut validator_account_state = account_state.clone();

//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&txn).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//             ), validator_wallet, validator_account_state);

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 if validators_vec.len() < 3 {
//                     validators_vec.push(processed.clone());
//                     account_state.pending.insert(txn.clone().txn_id, (txn.clone(), validators_vec));
//                 }
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[test]
//     fn test_invalid_simple_transaction_double_spend_attack() {
//         let state_path = "test_invalid_double_spend_attack.db";

//         let (
//             wallet_1,
//             _wallet_2,
//             mut account_state,
//             mut network_state,
//             reward_state,
//             txn,
//             mut validators_vec,
//         ) = txn_test_setup(state_path).unwrap();

//         let wallet_3 = WalletAccount::new(
//             &mut account_state,
//             &mut network_state,
//         );

//         let mut double_spend_txn = txn.clone();
//         double_spend_txn.receiver_address = wallet_3.address.to_string();

//         let (_block, updated_account_state) = Block::genesis(
//             reward_state,
//             wallet_1.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;

//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );

//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         let mut validator_account_state = account_state.clone();

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         let validator_1 = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&txn).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap()
//             ),
//             validator_wallet.clone(),
//             validator_account_state.clone()
//             );

//         let validator_2 = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&double_spend_txn).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//             ),
//             validator_wallet.clone(),
//             validator_account_state.clone(),
//         );

//         match validator_1 {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 if validators_vec.len() < 3 {
//                     validators_vec.push(processed.clone());
//                     account_state.pending.insert(txn.txn_id.clone(), (txn.clone(), validators_vec.clone()));
//                 }
//                 assert_eq!(processed.valid, true);
//             },
//             None => println!("Issue with validator, returned none")
//         }

//         match validator_2 {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 if validators_vec.len() < 3 {
//                     validators_vec.push(processed.clone());
//                     account_state.pending.insert(
//                         double_spend_txn.clone().txn_id,
//                         (double_spend_txn.clone(),
//                         validators_vec
//                     ));
//                 }
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }
//     #[test]
//     fn test_invalid_simple_transaction_non_existent_receiver() {
//         let state_path = "test_invalid_receiver_simple_txn.db";

//         let (_wallet_1,
//             _wallet_2,
//             mut account_state,
//             mut network_state,
//             _reward_state,
//             mut txn,
//             mut validators_vec,) = txn_test_setup(state_path).unwrap();

//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );

//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         let mut validator_account_state = account_state.clone();

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         txn.receiver_address = "unknown_receiver".to_string();

//         let validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&txn).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap()
//             ), validator_wallet, validator_account_state);

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 if validators_vec.len() < 3 {
//                     validators_vec.push(processed.clone());
//                     account_state.pending.insert(txn.clone().txn_id, (txn.clone(), validators_vec));
//                 }
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[test]
//     fn test_invalid_simple_transaction_non_existent_sender_in_last_confirmed_state() {

//         let state_path = "test_invalid_receiver_simple_txn.db";

//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let _reward_state = RewardState::start(&mut network_state);
//         let mut wallet_1 = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );

//         let mut wallet_2 = WalletAccount::new(
//             &mut account_state,
//             &mut network_state,
//         );

//         wallet_2 = wallet_2.get_balance(account_state.clone()).unwrap();

//         let _result = wallet_1.send_txn(
//             &mut account_state,
//             (wallet_2.address.clone(), 15 as u128),
//             &mut network_state);
//         let txn_id = account_state.pending.keys().cloned().collect::<Vec<String>>()[0].clone();
//         let txn = account_state.clone().pending.get(&txn_id).unwrap().0.clone();
//         let mut validators_vec = account_state.clone().pending.get(&txn_id).unwrap().1.clone();

//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );
//         let mut validator_account_state = account_state.clone();
//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&txn).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap()
//             ), validator_wallet, validator_account_state);

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 if validators_vec.len() < 3 {
//                     validators_vec.push(processed.clone());
//                     account_state.pending.insert(txn_id, (txn.clone(), validators_vec));
//                 }
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[allow(unused_assignments)]
//     #[test]
//     fn test_valid_homesteading_valid_claim_signature() {
//         let state_path = "test_valid_homestead_signature.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(
//             &mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;
//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let mut validator_wallet = WalletAccount::new(
//                 &mut account_state, &mut network_state
//             );

//         let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
//         let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();

//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimHomesteaded(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 serde_json::to_string(&current_owner_pub_key).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_wallet, account_state.clone()
//         );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, true);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[allow(unused_assignments)]
//     #[test]
//     fn test_valid_homesteading_valid_claim_maturity_timestamp() {
//         let state_path = "test_valid_homestead_maturity_timestamp.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(
//             &mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;
//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0)
//                                             .unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let mut validator_wallet = WalletAccount::new(&mut account_state, &mut network_state);
//         let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
//         let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimHomesteaded(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 serde_json::to_string(&current_owner_pub_key).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_wallet, account_state.clone()
//         );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, true);
//             },
//             None => println!("Issue with validator, returned none")
//         }

//     }

//     #[allow(unused_assignments)]
//     #[test]
//     fn test_valid_homesteading_claim_unowned() {
//         let state_path = "test_valid_homestead_claim_unowned.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(
//             &mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;
//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let mut validator_wallet = WalletAccount::new(
//                 &mut account_state, &mut network_state
//             );

//         let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
//         let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimHomesteaded(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 serde_json::to_string(&current_owner_pub_key).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_wallet, account_state.clone()
//         );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, true);
//             },
//             None => println!("Issue with validator, returned none")
//         }

//     }

//     #[allow(unused_assignments)]
//     #[test]
//     fn test_valid_homesteading_claim_first_appropriator() {
//         let state_path = "test_valid_homestead_claim_unowned.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet_1 = WalletAccount::new(
//             &mut account_state, &mut network_state);

//         let mut homesteader_wallet_2 = WalletAccount::new(
//             &mut account_state, &mut network_state);
//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet_1.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;
//         let claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet_1,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         let new_account_state = updated_account_state;

//         homesteader_wallet_1 = updated_wallet;

//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet_2,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();

//         account_state = updated_account_state;
//         homesteader_wallet_2 = updated_wallet;

//         let mut validator_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let claim_to_validate = new_account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
//         let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimHomesteaded(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 serde_json::to_string(&current_owner_pub_key).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_wallet, account_state.clone()
//         );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, true);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[allow(unused_assignments)]
//     #[test]
//     fn test_invalid_homesteading_invalid_claim_singature() {
//         let state_path = "test_invalid_homestead_signature.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(
//             &mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;
//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let mut validator_wallet = WalletAccount::new(&mut account_state, &mut network_state);
//         let mut claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
//         claim_to_validate.current_owner.2 = Some(homesteader_wallet.sign(&"Malicious_Signature".to_string()).unwrap().to_string());
//         let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimHomesteaded(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 serde_json::to_string(&current_owner_pub_key).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_wallet, account_state.clone()
//             );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }

//     }

//     #[allow(unused_assignments)]
//     #[test]
//     fn test_invalid_homesteading_invalid_claim_maturity_timestamp() {
//         let state_path = "test_valid_homestead_signature.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;
//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let mut validator_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let mut claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
//         claim_to_validate.maturation_time += 1000000000;

//         let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimHomesteaded(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 serde_json::to_string(&current_owner_pub_key).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_wallet, account_state.clone()
//         );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[allow(unused_assignments)]
//     #[test]
//     fn test_invalid_homesteading_claim_already_owned() {
//         let state_path = "test_invalid_homestead_claim_owned.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader1_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let (_genesis_block, _updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader1_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         let mut homesteader2_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims.iter()
//                                             .min_by_key(|x| x.0)
//                                             .unwrap();

//         let (updated_wallet1, updated_account_state1) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader1_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();

//         let (_updated_wallet2, updated_account_state2) = claim_to_homestead.clone().homestead(&mut homesteader2_wallet, &mut claim_state.clone(), &mut account_state, &mut network_state).unwrap();

//         homesteader1_wallet = updated_wallet1;
//         account_state = updated_account_state1;

//         let mut validator_wallet = WalletAccount::new(&mut account_state, &mut network_state);
//         let claim_to_validate = updated_account_state2.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();

//         let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();
//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimHomesteaded(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 serde_json::to_string(&current_owner_pub_key).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_wallet, account_state.clone()
//         );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[test]
//     fn test_valid_claim_acquired() {
//         let state_path = "test_valid_claim_acquired.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;

//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims
//                                                         .iter()
//                                                         .min_by_key(|x| x.0)
//                                                         .unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let (claim, updated_account_state) = match homesteader_wallet.sell_claim(
//             *ts,
//             &mut account_state,
//              10
//             )
//             {
//                 Some((claim, account_state)) => {
//                     (Some(claim), account_state)
//                 },
//                 None => {(None, account_state)}
//         };

//         account_state = updated_account_state;

//         let mut acquirer_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         if let Some(mut claim) = claim {
//             let (updated_wallet, updated_account_state) = claim.acquire(
//                 &mut acquirer_wallet, &mut account_state, &mut network_state).unwrap();
//             acquirer_wallet = updated_wallet;
//             account_state = updated_account_state;
//         }

//         let mut validator_wallet = WalletAccount::new(&mut account_state, &mut network_state);
//         let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();

//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimAcquired(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 serde_json::to_string(&homesteader_wallet.public_key).unwrap(),
//                 serde_json::to_string(&account_state).unwrap(),
//                 serde_json::to_string(&acquirer_wallet.address).unwrap(),
//             ), validator_wallet, account_state.clone()
//         );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, true);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[should_panic]
//     #[test]
//     fn test_invalid_transaction_unavailable_claim() {
//         let state_path = "test_invalid_claim_unavailable.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;

//         let claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                                         .iter()
//                                                         .min_by_key(|x| x.0)
//                                                         .unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let mut acquirer_wallet = WalletAccount::new(&mut account_state, &mut network_state);
//         let claim = homesteader_wallet.clone().claims.pop().unwrap();

//         if let Some(mut claim) = claim {
//             let (_updated_wallet, _updated_account_state) = claim.acquire(
//                 &mut acquirer_wallet, &mut account_state, &mut network_state).unwrap();

//         }
//     }

//     #[test]
//     fn test_invalid_claim_acquire_staked_claim() {

//         let state_path = "test_invalid_claim_staked.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;

//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims
//                                                         .iter()
//                                                         .min_by_key(|x| x.0)
//                                                         .unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;
//         let claim = homesteader_wallet.claims.get(0).unwrap().clone().unwrap();

//         let updated_account_state = claim.stake(homesteader_wallet.clone(), &mut account_state);

//         account_state = updated_account_state;
//         println!("{:?}", account_state);

//         let (claim, updated_account_state) = match homesteader_wallet.sell_claim(
//             *ts,
//             &mut account_state,
//              10
//             )
//             {
//                 Some((claim, account_state)) => {
//                     (Some(claim), account_state)
//                 },
//                 None => {(None, account_state)}
//         };

//         account_state = updated_account_state;

//         let mut acquirer_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         if let Some(mut claim) = claim {
//             let (updated_wallet, updated_account_state) = claim.acquire(
//                 &mut acquirer_wallet, &mut account_state, &mut network_state).unwrap();
//             acquirer_wallet = updated_wallet;
//             account_state = updated_account_state;
//         }

//         let mut validator_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();

//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimAcquired(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 homesteader_wallet.public_key,
//                 serde_json::to_string(&account_state).unwrap(),
//                 acquirer_wallet.address),
//                 validator_wallet,
//                 account_state.clone()
//         );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }
//     #[test]
//     fn test_invalid_claim_acquire_invalid_balance() {
//         let state_path = "test_invalid_claim_staked.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;

//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims
//                                                         .iter()
//                                                         .min_by_key(|x| x.0)
//                                                         .unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let (claim, updated_account_state) = match homesteader_wallet.sell_claim(
//             *ts,
//             &mut account_state,
//              10
//             )
//             {
//                 Some((claim, account_state)) => {
//                     (Some(claim), account_state)
//                 },
//                 None => {(None, account_state)}
//         };

//         account_state = updated_account_state;
//         println!("{:?}", &account_state);

//         let mut acquirer_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         if let Some(mut claim) = claim {
//             let (updated_wallet, _updated_account_state) = claim.acquire(
//                 &mut acquirer_wallet, &mut account_state, &mut network_state).unwrap();
//             acquirer_wallet = updated_wallet;
//         }

//         let mut validator_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let mut claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
//         claim_to_validate.price = 15000;

//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimAcquired(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 homesteader_wallet.public_key,
//                 serde_json::to_string(&account_state).unwrap(),
//                 acquirer_wallet.address
//             ), validator_wallet, account_state.clone()
//         );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }

//     }

//     #[allow(unused_assignments)]
//     #[test]
//     fn test_invalid_claim_acquire_invalid_chain_of_custody() {
//         let state_path = "test_valid_claim_acquired.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(
//             &mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;

//         let claim_state = account_state.clone().claim_state;
//         let (ts, claim_to_homestead) = claim_state.claims
//                                                         .iter()
//                                                         .min_by_key(|x| x.0)
//                                                         .unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                         &mut homesteader_wallet,
//                                                         &mut account_state.clone().claim_state,
//                                                         &mut account_state.clone(),
//                                                         &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let (claim, updated_account_state) = match homesteader_wallet.sell_claim(
//             *ts,
//             &mut account_state,
//              10
//             )
//             {
//                 Some((claim, account_state)) => {
//                     (Some(claim), account_state)
//                 },
//                 None => {(None, account_state)}
//         };

//         account_state = updated_account_state;

//         let mut acquirer_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         if let Some(mut claim) = claim {
//             let (updated_wallet, updated_account_state) = claim.acquire(
//                 &mut acquirer_wallet, &mut account_state, &mut network_state).unwrap();
//             acquirer_wallet = updated_wallet;
//             account_state = updated_account_state;
//         }

//         let mut validator_wallet = WalletAccount::new(
//                 &mut account_state, &mut network_state
//             );
//         let mut claim_to_validate = account_state.clone().claim_state.owned_claims.get(ts).unwrap().to_owned();
//         let mut coc = claim_to_validate.clone().chain_of_custody;
//         let malicious_acquirer_wallet = WalletAccount::new(&mut account_state, &mut network_state);
//         let mut malicious_acquirer = HashMap::new();
//         let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

//         malicious_acquirer
//             .insert("acquisition_timestamp".to_string(), Some(
//                 CustodianInfo::AcquisitionTimestamp(now.as_nanos())));

//         malicious_acquirer
//             .insert("acquired_from".to_string(),Some(CustodianInfo::AcquiredFrom(
//                 (Some("seller".to_string()), Some("seller_pubkey".to_string()), Some("Seller_signature".to_string())),
//             )));

//         malicious_acquirer
//             .insert("acquisition_price".to_string(),Some(CustodianInfo::AcquisitionPrice(claim_to_validate.clone().price)));

//         malicious_acquirer
//             .insert("owner_number".to_string(), Some(CustodianInfo::OwnerNumber(2 + 1)));

//         coc.insert(malicious_acquirer_wallet.address.clone(), malicious_acquirer);
//         claim_to_validate.chain_of_custody = coc;

//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimAcquired(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 homesteader_wallet.public_key,
//                 serde_json::to_string(&account_state).unwrap(),
//                 malicious_acquirer_wallet.address.clone()
//             ), validator_wallet, account_state.clone()
//             );

//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[test]
//     #[allow(unused_assignments, dead_code)]
//     fn test_valid_block() {
//         let state_path = "test_valid_block.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut wallet_1 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let (genesis, updated_account_state) = Block::genesis(
//             reward_state,
//             wallet_1.clone().address,
//             &mut account_state,
//             &mut network_state).unwrap();
//         account_state = updated_account_state;

//         let mut claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut wallet_1,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         wallet_1 = updated_wallet;
//         account_state = updated_account_state;

//         wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

//         let claim = wallet_1.clone().claims[0].clone().unwrap();

//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );

//         let mut validator_account_state = account_state.clone();

//         for (_ts, claim) in validator_account_state.clone().claim_state.claims {
//             let (updated_wallet, updated_account_state) = claim.clone().homestead(
//                 &mut validator_wallet,
//                 &mut claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state).unwrap();
//             validator_account_state = updated_account_state;
//             validator_wallet = updated_wallet;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         let (block, updated_account_state) = Block::mine(
//             &reward_state,
//             claim,
//             genesis.clone(),
//             HashMap::new(),
//             &mut account_state.clone(),
//             &mut network_state.clone()
//         ).unwrap().unwrap();

//         account_state = updated_account_state.clone();

//         let validator = Validator::new(
//             Message::NewBlock(
//                 serde_json::to_string(&genesis).unwrap(),
//                 serde_json::to_string(&block).unwrap(),
//                 wallet_1.public_key,
//                 serde_json::to_string(&network_state).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//                 serde_json::to_string(&reward_state).unwrap(),
//             ), validator_wallet, validator_account_state.clone()
//         );
//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, true);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[test]
//     fn test_invalid_block_bad_signature() {
//         let state_path = "test_invalid_block_bad_signature.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut wallet_1 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let (genesis, updated_account_state) = Block::genesis(
//             reward_state,
//             wallet_1.clone().address,
//             &mut account_state,
//             &mut network_state).unwrap();
//         account_state = updated_account_state;

//         let mut claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut wallet_1,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         wallet_1 = updated_wallet;
//         account_state = updated_account_state;

//         wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

//         let claim = wallet_1.clone().claims[0].clone().unwrap();

//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );

//         let mut validator_account_state = account_state.clone();

//         for (_ts, claim) in validator_account_state.clone().claim_state.claims {
//             let (updated_wallet, updated_account_state) = claim.clone().homestead(
//                 &mut validator_wallet,
//                 &mut claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state).unwrap();
//             validator_account_state = updated_account_state;
//             validator_wallet = updated_wallet;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         let (mut block, updated_account_state) = Block::mine(
//             &reward_state,
//             claim,
//             genesis.clone(),
//             HashMap::new(),
//             &mut account_state.clone(),
//             &mut network_state.clone()
//         ).unwrap().unwrap();

//         account_state = updated_account_state.clone();

//         block.claim.current_owner.2 = Some(wallet_1.sign(&"malicious_signature".to_string())
//             .unwrap()
//             .to_string());

//         let validator = Validator::new(
//             Message::NewBlock(
//                 serde_json::to_string(&genesis).unwrap(),
//                 serde_json::to_string(&block).unwrap(),
//                 wallet_1.public_key,
//                 serde_json::to_string(&network_state).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//                 serde_json::to_string(&reward_state).unwrap(),
//             ), validator_wallet, validator_account_state.clone()
//         );
//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none: {:?}", account_state)
//         }
//     }

//     #[test]
//     fn test_invalid_block_invalid_state_hash() {
//         let state_path = "test_invalid_block_bad_signature.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut wallet_1 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let (genesis, updated_account_state) = Block::genesis(
//             reward_state,
//             wallet_1.clone().address,
//             &mut account_state,
//             &mut network_state).unwrap();
//         account_state = updated_account_state;

//         let mut claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut wallet_1,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         wallet_1 = updated_wallet;
//         account_state = updated_account_state;

//         wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

//         let claim = wallet_1.clone().claims[0].clone().unwrap();

//         let (block, updated_account_state) = Block::mine(
//             &reward_state,
//             claim,
//             genesis.clone(),
//             HashMap::new(),
//             &mut account_state.clone(),
//             &mut network_state.clone()
//         ).unwrap().unwrap();

//         account_state = updated_account_state.clone();
//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );

//         let mut validator_account_state = account_state.clone();

//         for (_ts, claim) in validator_account_state.clone().claim_state.claims {
//             let (updated_wallet, updated_account_state) = claim.clone().homestead(
//                 &mut validator_wallet,
//                 &mut claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state).unwrap();
//             validator_account_state = updated_account_state;
//             validator_wallet = updated_wallet;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::NewBlock(
//                 serde_json::to_string(&genesis).unwrap(),
//                 serde_json::to_string(&block).unwrap(),
//                 wallet_1.public_key,
//                 serde_json::to_string(&network_state).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//                 serde_json::to_string(&reward_state).unwrap(),
//             ), validator_wallet, validator_account_state.clone()
//         );
//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none")
//         }
//     }

//     #[test]
//     fn test_invalid_block_bad_last_block_hash() {
//         let state_path = "test_invalid_block_bad_signature.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut wallet_1 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let (genesis, updated_account_state) = Block::genesis(
//             reward_state,
//             wallet_1.clone().address,
//             &mut account_state,
//             &mut network_state).unwrap();
//         account_state = updated_account_state;

//         let mut claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut wallet_1,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         wallet_1 = updated_wallet;
//         account_state = updated_account_state;

//         wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

//         let claim = wallet_1.clone().claims[0].clone().unwrap();

//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );

//         let mut validator_account_state = account_state.clone();

//         for (_ts, claim) in validator_account_state.clone().claim_state.claims {
//             let (updated_wallet, updated_account_state) = claim.clone().homestead(
//                 &mut validator_wallet,
//                 &mut claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state).unwrap();
//             validator_account_state = updated_account_state;
//             validator_wallet = updated_wallet;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         let (mut block, updated_account_state) = Block::mine(
//             &reward_state,
//             claim,
//             genesis.clone(),
//             HashMap::new(),
//             &mut account_state.clone(),
//             &mut network_state.clone()
//         ).unwrap().unwrap();

//         account_state = updated_account_state.clone();

//         block.last_block_hash = digest_bytes("malicious_last_block_hash".as_bytes());

//         let validator = Validator::new(
//             Message::NewBlock(
//                 serde_json::to_string(&genesis).unwrap(),
//                 serde_json::to_string(&block).unwrap(),
//                 wallet_1.public_key,
//                 serde_json::to_string(&network_state).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//                 serde_json::to_string(&reward_state).unwrap(),
//             ), validator_wallet, validator_account_state.clone()
//         );
//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none: {:?}", account_state)
//         }
//     }

//     #[test]
//     fn test_invalid_block_bad_reward() {
//         let state_path = "test_invalid_block_bad_signature.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut wallet_1 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let (genesis, updated_account_state) = Block::genesis(
//             reward_state,
//             wallet_1.clone().address,
//             &mut account_state,
//             &mut network_state).unwrap();
//         account_state = updated_account_state;

//         let mut claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut wallet_1,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         wallet_1 = updated_wallet;
//         account_state = updated_account_state;

//         wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

//         let claim = wallet_1.clone().claims[0].clone().unwrap();

//         let mut validator_wallet = WalletAccount::new(
//             &mut account_state,
//             &mut network_state
//         );

//         let mut validator_account_state = account_state.clone();

//         for (_ts, claim) in validator_account_state.clone().claim_state.claims {
//             let (updated_wallet, updated_account_state) = claim.clone().homestead(
//                 &mut validator_wallet,
//                 &mut claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state).unwrap();
//             validator_account_state = updated_account_state;
//             validator_wallet = updated_wallet;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             validator_account_state = updated_account_state;
//         }

//         let (mut block, updated_account_state) = Block::mine(
//             &reward_state,
//             claim,
//             genesis.clone(),
//             HashMap::new(),
//             &mut account_state.clone(),
//             &mut network_state.clone()
//         ).unwrap().unwrap();

//         account_state = updated_account_state.clone();

//         block.block_reward.amount = 90;
//         let validator = Validator::new(
//             Message::NewBlock(
//                 serde_json::to_string(&genesis).unwrap(),
//                 serde_json::to_string(&block).unwrap(),
//                 wallet_1.public_key,
//                 serde_json::to_string(&network_state).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//                 serde_json::to_string(&reward_state).unwrap(),
//             ), validator_wallet, validator_account_state.clone()
//         );
//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none: {:?}", account_state)
//         }
//     }

//     #[test]
//     fn test_valid_block_valid_txns() {
//         let state_path = "test_invalid_block_bad_signature.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut wallet_1 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let wallet_2 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let mut validator_1 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );
//         let mut validator_2 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let mut validator_3 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );
//         let (genesis, updated_account_state) = Block::genesis(
//             reward_state,
//             wallet_1.clone().address,
//             &mut account_state,
//             &mut network_state).unwrap();
//         account_state = updated_account_state;
//         wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

//         let mut claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut wallet_1,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         wallet_1 = updated_wallet;
//         account_state = updated_account_state;

//         claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut validator_1,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         validator_1 = updated_wallet;
//         account_state = updated_account_state;

//         account_state = validator_1.claims[0].clone().unwrap().stake(validator_1.clone(), &mut account_state);

//         claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut validator_2,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         validator_2 = updated_wallet;
//         account_state = updated_account_state;
//         account_state = validator_2.claims[0].clone().unwrap().stake(validator_2.clone(), &mut account_state);
//         claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut validator_3,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         validator_3 = updated_wallet;
//         account_state = updated_account_state;

//         account_state = validator_3.claims[0].clone().unwrap().stake(validator_3.clone(), &mut account_state);
//         claim_state = account_state.clone().claim_state;
//         let _txn = wallet_1.send_txn(
//             &mut account_state,
//             (wallet_2.address, 15),
//             &mut network_state).unwrap();

//         let pending = account_state.clone().pending;

//         let wallet_1_txn = pending
//             .iter().filter(|&x| x.1.0.sender_address == wallet_1.address)
//             .map(|x| x.clone()).collect::<Vec<_>>()[0];
//         let first_validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&wallet_1_txn.1.0).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_1.clone(), account_state.clone()
//         ).unwrap().validate();

//         account_state
//             .update(StateOption::ConfirmedTxn(
//                 serde_json::to_string(&wallet_1_txn.1.0).unwrap(),
//                 serde_json::to_string(&first_validator).unwrap()), &mut network_state);

//         let second_validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&wallet_1_txn.1.0).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_2, account_state.clone()
//         ).unwrap().validate();

//         account_state.clone()
//             .update(
//                 StateOption::ConfirmedTxn(
//                     serde_json::to_string(&wallet_1_txn.1.0).unwrap(),
//                     serde_json::to_string(&second_validator).unwrap()
//                 ), &mut network_state
//             );

//         let third_validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&wallet_1_txn.1.0).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_3, account_state.clone()
//         ).unwrap().validate();
//         account_state.clone()
//             .update(
//                 StateOption::ConfirmedTxn(
//                     serde_json::to_string(&wallet_1_txn.1.0).unwrap(),
//                     serde_json::to_string(&third_validator).unwrap(),
//                 ), &mut network_state
//             );

//         let claim: Vec<Claim> = account_state.clone().claim_state.clone().owned_claims
//             .iter().filter(|&claim| claim.1.clone().current_owner.clone().0.unwrap() == wallet_1.address
//         ).map(|claim| claim.1.clone()).collect::<Vec<_>>();

//         let data: HashMap<String, Txn> = account_state.mineable.iter().map(|(key, value)| {
//             return (key.clone(), value.0.clone())
//         }).collect();

//         let validator_account_state = account_state.clone();

//         let (block, updated_account_state) = Block::mine(
//             &reward_state,
//             claim[0].clone(),
//             genesis.clone(),
//             data,
//             &mut account_state.clone(),
//             &mut network_state.clone()
//         ).unwrap().unwrap();

//         account_state = updated_account_state.clone();
//         let block_validator = Validator::new(
//             Message::NewBlock(
//                 serde_json::to_string(&genesis).unwrap(),
//                 serde_json::to_string(&block).unwrap(),
//                 wallet_1.public_key,
//                 serde_json::to_string(&network_state).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//                 serde_json::to_string(&reward_state).unwrap()
//             ), validator_1, validator_account_state.clone()
//         );
//         match block_validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, true);
//             },
//             None => println!("Issue with validator, returned none: {:?} -> {:?}", account_state, claim_state)
//         }
//     }

//     #[test]
//     fn test_invalid_block_contains_invalid_transactions() {
//         let state_path = "test_invalid_block_bad_signature.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut wallet_1 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let wallet_2 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let mut validator_1 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );
//         let mut validator_2 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );
//         let mut validator_3 = WalletAccount::new(
//             &mut account_state, &mut network_state
//         );

//         let (genesis, updated_account_state) = Block::genesis(
//             reward_state,
//             wallet_1.clone().address,
//             &mut account_state,
//             &mut network_state).unwrap();
//         account_state = updated_account_state;
//         wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();

//         let mut claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut wallet_1,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         wallet_1 = updated_wallet;
//         account_state = updated_account_state;

//         claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();
//         let(updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut validator_1,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         validator_1 = updated_wallet;
//         account_state = updated_account_state;

//         account_state = validator_1.claims[0].clone().unwrap().stake(validator_1.clone(), &mut account_state);

//         claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut validator_2,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         validator_2 = updated_wallet;
//         account_state = updated_account_state;
//         account_state = validator_2.claims[0].clone().unwrap().stake(validator_2.clone(), &mut account_state);
//         claim_state = account_state.clone().claim_state;
//         let (_ts, claim_to_homestead) = claim_state.claims
//                                             .iter()
//                                             .min_by_key(|x| x.0).unwrap();

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut validator_3,
//             &mut account_state.clone().claim_state,
//             &mut account_state,
//             &mut network_state
//         ).unwrap();

//         validator_3 = updated_wallet;
//         account_state = updated_account_state;

//         account_state = validator_3.claims[0].clone().unwrap().stake(validator_3.clone(), &mut account_state);
//         claim_state = account_state.clone().claim_state;

//         let _txn = wallet_1.send_txn(
//             &mut account_state,
//             (wallet_2.address, 15),
//             &mut network_state).unwrap();

//         let pending = account_state.clone().pending;

//         let wallet_1_txn = pending
//             .iter()
//             .filter(|&x| x.1.0.sender_address == wallet_1.address)
//             .map(|x| x.clone())
//             .collect::<Vec<_>>()[0];

//         let mut txn_to_validate: Txn = wallet_1_txn.1.0.clone();
//         txn_to_validate.txn_signature = wallet_1.sign(&"malicious_signature".to_string()).unwrap().to_string();

//         let first_validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&txn_to_validate).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_1.clone(), account_state.clone()
//         ).unwrap();

//         account_state
//             .update(StateOption::ConfirmedTxn(
//                 serde_json::to_string(&wallet_1_txn.1.0).unwrap(),
//                 serde_json::to_string(&first_validator).unwrap()
//             ), &mut network_state
//         );

//         let second_validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&txn_to_validate).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_2, account_state.clone()
//         ).unwrap();

//         account_state.clone()
//             .update(StateOption::ConfirmedTxn(
//                 serde_json::to_string(&wallet_1_txn.1.0).unwrap(),
//                 serde_json::to_string(&second_validator).unwrap()
//             ), &mut network_state);

//         let third_validator = Validator::new(
//             Message::Txn(
//                 serde_json::to_string(&txn_to_validate).unwrap(),
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_3, account_state.clone()
//         ).unwrap();
//         account_state
//             .clone()
//             .update(
//                 StateOption::ConfirmedTxn(
//                     serde_json::to_string(&wallet_1_txn.1.0).unwrap(),
//                     serde_json::to_string(&third_validator).unwrap()
//                 ), &mut network_state
//             );

//         let claim: Vec<Claim> = account_state.clone().claim_state.clone().owned_claims
//             .iter().filter(|&claim| claim.1.clone().current_owner.clone().0.unwrap() == wallet_1.address
//         ).map(|claim| claim.1.clone()).collect::<Vec<_>>();

//         let data: HashMap<String, Txn> = account_state.mineable.iter().map(|(key, value)| {
//             return (key.clone(), value.0.clone())
//         }).collect();

//         let validator_account_state = account_state.clone();

//         let (block, updated_account_state) = Block::mine(
//             &reward_state,
//             claim[0].clone(),
//             genesis.clone(),
//             data,
//             &mut account_state.clone(),
//             &mut network_state.clone()
//         ).unwrap().unwrap();

//         account_state = updated_account_state.clone();
//         assert!(block.data.is_empty());

//         let block_validator = Validator::new(
//             Message::NewBlock(
//                 serde_json::to_string(&genesis).unwrap(),
//                 serde_json::to_string(&block).unwrap(),
//                 wallet_1.public_key,
//                 serde_json::to_string(&network_state).unwrap(),
//                 serde_json::to_string(&validator_account_state).unwrap(),
//                 serde_json::to_string(&reward_state).unwrap()
//             ), validator_1, validator_account_state.clone()
//         );
//         match block_validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, true);
//             },
//             None => println!("Issue with validator, returned none: {:?} -> {:?}", account_state, claim_state)
//         }
//     }

//     #[test]
//     fn test_max_number_of_claims() {
//         let state_path = "test_valid_homestead_signature.db";
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);

//         let mut homesteader_wallet = WalletAccount::new(&mut account_state, &mut network_state);

//         let (_genesis_block, updated_account_state) = Block::genesis(
//             reward_state,
//             homesteader_wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         ).unwrap();

//         account_state = updated_account_state;

//         for (_ts, claim) in account_state.clone().claim_state.claims {

//             let (updated_wallet, updated_account_state) = claim.clone().homestead(
//                                                             &mut homesteader_wallet,
//                                                             &mut account_state.clone().claim_state,
//                                                             &mut account_state.clone(),
//                                                             &mut network_state).unwrap();
//             homesteader_wallet = updated_wallet;
//             account_state = updated_account_state;
//         }

//         let mut validator_wallet = WalletAccount::new(&mut account_state, &mut network_state);
//         let maturation_time = homesteader_wallet.claims
//             .iter()
//             .map(|claim| claim.clone().unwrap().maturation_time)
//             .min_by(|a, b| a.cmp(&b)).unwrap();

//         let claim = account_state.claim_state.owned_claims.get(&maturation_time).unwrap().to_owned();

//         let (_block, updated_account_state) = Block::mine(
//             &reward_state,
//             claim,
//             _genesis_block,
//             HashMap::new(),
//             &mut account_state,
//             &mut network_state
//         ).unwrap().unwrap();

//         account_state = updated_account_state;

//         let claim_state = account_state.clone().claim_state;

//         let (_ts, claim_to_homestead) = claim_state.claims.iter()
//             .min_by_key(|entry| entry.0).unwrap();

//         let mut claim_state = account_state.clone().claim_state;

//         let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//             &mut homesteader_wallet, &mut claim_state, &mut account_state, &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let claim_state = account_state.clone().claim_state;

//         let (ts, claim_to_homestead) = claim_state.claims.iter()
//             .min_by_key(|entry| entry.0).unwrap();
//         let mut claim_state = account_state.clone().claim_state;

//         let (updated_wallet, updated_account_state) = claim_to_homestead
//             .clone().homestead(&mut homesteader_wallet, &mut claim_state, &mut account_state, &mut network_state).unwrap();
//         homesteader_wallet = updated_wallet;
//         account_state = updated_account_state;

//         let claim_to_validate = claim_state.owned_claims.get(ts).unwrap();

//         let current_owner_pub_key = claim_to_validate.clone().current_owner.1.unwrap();

//         for (_ts, claim) in account_state.clone().claim_state.claims.iter() {
//             let (new_wallet, updated_account_state) = claim.to_owned().homestead(
//                 &mut validator_wallet,
//                 &mut account_state.clone().claim_state,
//                 &mut account_state.clone(),
//                 &mut network_state
//             ).unwrap();

//             validator_wallet = new_wallet;
//             account_state = updated_account_state;
//         }

//         for claim in validator_wallet.clone().claims.iter() {
//             let updated_account_state = claim.clone().unwrap().stake(validator_wallet.clone(), &mut account_state);
//             account_state = updated_account_state;
//         }

//         let validator = Validator::new(
//             Message::ClaimHomesteaded(
//                 serde_json::to_string(&claim_to_validate).unwrap(),
//                 current_owner_pub_key,
//                 serde_json::to_string(&account_state).unwrap()
//             ), validator_wallet, account_state.clone()
//         );
//
//         match validator {
//             Some(validator) => {
//                 let processed = validator.validate();
//                 assert_eq!(processed.valid, false);
//             },
//             None => println!("Issue with validator, returned none: {:?}", homesteader_wallet)
//         }
//     }
//      #[test]
//      fn test_validator_setting_by_message_id() {
//
//      }
//
//      #[test]
//      fn test_vpu_updates_state_when_confirmed_valid() {
//
//      }
//
//      #[test]
//      fn test_vpu_updates_state_when_confirmed_invalid() {
//
//      }
//
//      #[test]
//      fn test_vpu_slashes_claims() {
//
//      }
//  }
// #[cfg(test)]
// mod test {
//     use super::*;
//     use crate::reward::RewardState;

//     #[test]
//     fn test_wallet_set_in_account_state() {
//         let mut account_state = AccountState::start();
//         let mut network_state = NetworkState::restore("test_state.db");
//         let wallet = WalletAccount::new(Arc::new(Mutex::new(account_state)), Arc::new(Mutex::new(network_state)));
//         let wallet_pk = account_state.accounts_sk.get(&wallet.skhash).unwrap();

//         assert_eq!(wallet_pk.to_owned(), wallet.public_key.to_string());
//     }

//     #[test]
//     fn test_restore_account_state_and_wallet() {
//         let account_state = Arc::new(Mutex::new(AccountState::start()));
//         let network_state = Arc::new(Mutex::new(NetworkState::restore("test_state.db")));
//         let mut wallet_vec: Vec<WalletAccount> = vec![];
//         for _ in 0..=20 {
//             let new_wallet = WalletAccount::new(Arc::clone(&account_state), Arc::clone(&network_state));
//             wallet_vec.push(new_wallet);
//         }
//         let mut account_state = AccountState::start();
//         let mut network_state = Arc::new(Mutex::new(NetworkState::restore("test_state.db")));
//         for wallet in &wallet_vec {
//             account_state.update(StateOption::NewAccount(serde_json::to_string(&wallet).unwrap()), Arc::clone(&network_state));
//         }

//         let wallet_to_restore = &wallet_vec[4];
//         let secret_key_for_restoration = &wallet_to_restore.private_key;

//         {
//             let mut inner_scope_network_state = NetworkState::restore("test_state.db");
//             let mut _reward_state = RewardState::start(&mut inner_scope_network_state);
//             let db_iter = network_state.lock().unwrap().state.iter();
//             for i in db_iter {
//                 match i.get_value::<AccountState>() {
//                     Some(ast) => account_state = ast,
//                     None => (),
//                 }
//                 match i.get_value::<RewardState>() {
//                     Some(rst) => _reward_state = rst,
//                     None => (),
//                 }
//             }

//             let wallet_to_restore_pk = account_state
//                 .accounts_sk
//                 .get(&digest_bytes(
//                     secret_key_for_restoration.to_string().as_bytes(),
//                 ))
//                 .unwrap();
//             // Assume no claims, no tokens for now.
//             // TODO: Add claims and tokens
//             let wallet_to_restore_address =
//                 account_state.accounts_pk.get(wallet_to_restore_pk).unwrap();
//             let wallet_to_restore_balance = account_state
//                 .total_coin_balances
//                 .get(wallet_to_restore_pk)
//                 .unwrap();
//             let wallet_to_restore_available_balance = account_state
//                 .available_coin_balances
//                 .get(wallet_to_restore_pk)
//                 .unwrap();
//             let restored_wallet = WalletAccount {
//                 private_key: secret_key_for_restoration.clone(),
//                 public_key: wallet_to_restore_pk.clone(),
//                 address: wallet_to_restore_address.to_owned(),
//                 balance: wallet_to_restore_balance.to_owned(),
//                 available_balance: wallet_to_restore_available_balance.to_owned(),
//                 tokens: vec![],
//                 claims: vec![],
//                 skhash: digest_bytes(secret_key_for_restoration.to_string().as_bytes()),
//                 marker: PhantomData
//             };

//             assert_eq!(wallet_vec[4].skhash, restored_wallet.skhash);
//             assert_eq!(
//                 wallet_vec[4].public_key.to_string(),
//                 restored_wallet.public_key.to_string()
//             );
//             assert_eq!(wallet_vec[4].balance, restored_wallet.balance);
//             assert_eq!(
//                 wallet_vec[4].available_balance,
//                 restored_wallet.available_balance
//             );
//         }
//     }

//     #[test]
//     fn test_reward_received_by_miner() {}

//     #[test]
//     fn test_send_txn() {}

//     #[test]
//     fn test_recv_txn() {}

//     #[test]
//     fn test_valid_signature() {}
//     #[test]
//     fn test_invalid_signature() {}

//     #[test]
//     fn test_account_state_updated_after_claim_homesteaded() {}

//     #[test]
//     fn test_account_state_updated_after_new_block() {}

//     #[test]
//     fn test_account_state_updated_after_new_txn() {}

//     #[test]
//     fn test_account_state_updated_after_confirmed_txn() {}
//
// use super::*;
// use crate::{block::Block, reward::RewardState};

//     #[test]
//     fn test_claim_creation_with_new_block() {
//         let state_path = "claim_test1_state.db";
//         let mut network_state = NetworkState::restore(state_path);
//         let reward_state = RewardState::start(&mut network_state);
//         let mut account_state = AccountState::start();
//         let mut claim_state = ClaimState::start();
//         let new_wallet = WalletAccount::new(&mut account_state, &mut network_state);
//         let mut wallet = new_wallet;

//         let genesis = Block::genesis(
//             reward_state,
//             wallet.address.clone(),
//             &mut account_state,
//             &mut network_state,
//         )
//         .unwrap();

//         account_state = genesis.1;
//         let mut last_block = genesis.0;

//         for claim in &account_state.clone().claim_state.claims {
//             let _ts = claim.0;
//             let mut claim_obj = claim.1.to_owned();

//             let (new_wallet, new_account_state) = claim_obj
//                 .homestead(
//                     &mut wallet,
//                     &mut claim_state,
//                     &mut account_state,
//                     &mut network_state,
//                 )
//                 .unwrap();

//             wallet = new_wallet;
//             account_state = new_account_state;
//         }

//         for claim in &wallet.clone().claims {
//             let claim_obj = claim.clone().unwrap();
//             let (next_block, new_account_state) = Block::mine(
//                 &reward_state,
//                 claim_obj,
//                 last_block,
//                 HashMap::new(),
//                 &mut account_state,
//                 &mut network_state,
//             )
//             .unwrap()
//             .unwrap();

//             last_block = next_block;
//             account_state = new_account_state;
//         }

//         assert_eq!(account_state.claim_state.claims.len(), 400);
//     }

//     #[test]
//     fn test_claim_update_after_homestead() {}

//     #[test]
//     fn test_mature_claim_valid_signature_mines_block() {}

//     #[test]
//     fn test_immature_claim_valid_signature_doesnt_mine_block() {}

//     #[test]
//     fn test_mature_claim_invalid_signature_doesnt_mine_block() {}

//     #[test]
//     fn test_claim_for_sale() {}

//     #[test]
//     fn test_claim_sold() {}
//}
