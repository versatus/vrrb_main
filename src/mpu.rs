use crate::account::AccountState;
use crate::claim::Claim;
use crate::state::NetworkState;
use crate::txn::Txn;
use crate::validator::{Message, Validator, ValidatorOptions};
use crate::verifiable::Verifiable;

pub fn message_processor(validator: &mut Validator) {
    // Match the message type. Message type can be one of 4 options
    // Txn, NewBlock, ClaimAcquired or ClaimHomesteaded
    // If a message's subject (transaction, claim transaction, or new block)
    // the valid field in needs to be changed to true
    // the validator struct get's pushed to an external vector.
    match validator.message.clone() {
        Message::Txn(txn_string, account_state_string, network_state_string) => {
            // If the Message variant is a transaction
            // then it needs to be processed.
            // All of the variant inners implement the Verifiable trait
            // which has an is_valid method, which receives an option (for claims)
            // which is either None or Some(ClaimOption). For Txns and Blocks it should
            // always be None. For claims it should always be some.
            // Is valid returns an Option<bool> which can either be Some(true), Some(false) or None,
            // a None option is an error, and should propagate an invalid message error.
            let txn = Txn::from_string(&txn_string);
            let account_state = AccountState::from_string(&account_state_string);
            let network_state = NetworkState::from_string(&network_state_string);

            let options = Some(ValidatorOptions::Transaction(
                account_state.clone(),
                network_state.clone(),
            ));

            if let Some(validity) = txn.is_valid(options) { validator.valid = validity; } else { panic!("Invalid Transaction Message") };
        }
        Message::ClaimAcquired(
            claim_string,
            network_state_string,
            account_state_string,
            seller_pubkey,
            buyer_pubkey,
        ) => {
            // Claim acquisition is one of two types of claim messages that needs
            // to be validated. The claim.is_valid() method receives
            // a Some(ClaimOption::Acquire) option, so that it knows
            // that it is to validate the claim that is being acquired
            // not homestaeded.
            let claim = Claim::from_string(&claim_string.clone());
            let network_state = NetworkState::from_string(&network_state_string.clone());
            let account_state = AccountState::from_string(&account_state_string.clone());

            let options = Some(ValidatorOptions::ClaimAcquire(
                network_state,
                account_state,
                seller_pubkey.clone(),
                buyer_pubkey.clone(),
            ));

            if let Some(validity) = claim.is_valid(options) { validator.valid = validity } else { panic!("Invalid Claim Message") };
        }
        Message::NewBlock(
            last_block,
            block,
            reward_state,
            network_state,
        ) => {
            // If a message is a new block, then check that the block is
            // valid, by calling the block.is_valid() method and passing None
            // as the options, as only Claim validation requires an option
            match block.is_valid(Some(ValidatorOptions::NewBlock(
                last_block.clone(),
                reward_state.clone(),
                network_state.clone(),
            ))) {
                // If the is_valid() method returns Some(true)
                // then the block is valid, and the validator
                // should have it's valid field set to true
                Some(true) => { validator.valid = true; }
                // If the is_valid() method returns Some(false)
                // then return the validator as is.
                Some(false) => {}
                // If the is_valid() method returns None something has gone wrong
                // TODO: propagate error.
                None => { panic!("Invalid Block Message!") }
            }
        }
    }
}