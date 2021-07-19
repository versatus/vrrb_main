use crate::validator::{Message, Validator, ValidatorOptions};
use crate::verifiable::Verifiable;
use crate::txn::Txn;
use crate::block::Block;
use crate::claim::Claim;


pub fn message_processor(validator: Validator) -> Validator {
        // Match the message type. Message type can be one of 4 options
        // Txn, NewBlock, ClaimAcquired or ClaimHomesteaded
        // If a message's subject (transaction, claim transaction, or new block)
        // the valid field in needs to be changed to true
        // the validator struct get's pushed to an external vector.
        match validator.message.clone() {
            Message::Txn(txn, account_state) => { 
                // If the Message variant is a transaction
                // then it needs to be processed.
                // All of the variant inners implement the Verifiable trait
                // which has an is_valid method, which receives an option (for claims)
                // which is either None or Some(ClaimOption). For Txns and Blocks it should
                // always be None. For claims it should always be some.
                // Is valid returns an Option<bool> which can either be Some(true), Some(false) or None,
                // a None option is an error, and should propagate an invalid message error.
                let txn_to_validate = serde_json::from_str::<Txn>(&txn).unwrap();

                match txn_to_validate.is_valid(Some(ValidatorOptions::Transaction(account_state.clone()))) {
                    // If the transaction is valid
                    // return the validator structure
                    // with the valid field set to true
                    // and the message variant, the rest of the validator
                    // remains the same (node_wallet, and staked_claims).
                    Some(true) => {
                        Validator {
                            valid: true,
                            message: Message::Txn(txn, account_state),
                            ..validator
                        }
                    },
                    // Validators default to invalid (valid field set to false)
                    // So if it is indeed invalid, then just return the validator struct as is
                    Some(false) => {
                        Validator {
                            ..validator
                        }
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
                let claim_to_validate = serde_json::from_str::<Claim>(&claim).unwrap();

                match claim_to_validate.is_valid(Some(ValidatorOptions::ClaimAcquire(account_state.clone(), buyer_pubkey.clone()))) {
                    Some(true) => {
                        Validator {
                            valid: true,
                            message: Message::ClaimAcquired(
                                claim, seller_pubkey, account_state, buyer_pubkey
                            ),
                            ..validator
                        }
                    }
                    // Validator defaults to invalid so if the message
                    // subject is invalid just return the validator as is
                    Some(false) => {
                        validator
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
                let claim_to_validate = serde_json::from_str::<Claim>(&claim).unwrap();

                match claim_to_validate.is_valid(Some(ValidatorOptions::ClaimHomestead(account_state.clone()))) {
                    // If the claim is validly homesteaded, return 
                    // the validator with the valid field set to tru
                    // and the message.
                    Some(true) => { 
                        Validator {
                            valid: true,
                            message: Message::ClaimHomesteaded(claim, pubkey, account_state),
                            ..validator
                        }
                    },
                    // If the claim is invalidly homesteaded
                    // then return the validator as is
                    Some(false) => { 
                        validator
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
            Message::NewBlock(last_block, block, pubkey, network_state, account_state, reward_state) => {
                // If a message is a new block, then check that the block is
                // valid, by calling the block.is_valid() method and passing None
                // as the options, as only Claim validation requires an option

                let block_to_validate = serde_json::from_str::<Block>(&block).unwrap();

                match block_to_validate.is_valid(Some(ValidatorOptions::NewBlock(
                    last_block.clone(),
                    block.clone(), 
                    pubkey.clone(), 
                    account_state.clone(), 
                    reward_state.clone(),
                    network_state.clone())
                )) {
                    // If the is_valid() method returns Some(true)
                    // then the block is valid, and the validator
                    // should have it's valid field set to true
                    Some(true) => {
                        Validator {
                            valid: true,
                            message: Message::NewBlock(
                                last_block,
                                block, 
                                pubkey, 
                                network_state, 
                                account_state, 
                                reward_state),
                            ..validator
                        }
                    },
                    // If the is_valid() method returns Some(false)
                    // then return the validator as is.
                    Some(false) => {
                        validator
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