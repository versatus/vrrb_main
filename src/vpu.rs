use crate::{
    account::AccountState, 
    validator::{Validator, Message}, 
    verifiable::Verifiable, 
    claim::Claim,
    block::Block,
    txn::Txn,
};
use std::collections::HashMap;


pub struct ValidatorProcessor {
    pub validators: HashMap<String, Vec<Validator>>,
    pub confirmations: HashMap<String, u8>,
    pub confirmed: HashMap<String, Box<dyn Verifiable>>,
    pub slashed: Vec<String>,
}

impl ValidatorProcessor {

    pub fn start() -> ValidatorProcessor {

        ValidatorProcessor {
            validators: HashMap::new(),
            confirmations: HashMap::new(),
            confirmed: HashMap::new(),
            slashed: vec![],
        }
    }

    pub fn new_validator(&mut self, validator: Validator) {

        match validator.clone().message {
            Message::ClaimHomesteaded(
                claim, 
                _homesteader, 
                _account_state
            ) => {

                let claim_to_validate = serde_json::from_str::<Claim>(&claim).unwrap();

                if let Some(entry) = self.validators.get_mut(&claim_to_validate.claim_number.to_string()) {
                    entry.push(validator);
                } else {
                    self.validators.insert(claim_to_validate.claim_number.to_string(), vec![validator]);
                }
            },
            Message::ClaimAcquired(
                claim, 
                _seller_pubkey, 
                _account_state, 
                _buyer_pubkey
            ) => {

                let claim_to_validate = serde_json::from_str::<Claim>(&claim).unwrap();

                if let Some(entry) = self.validators.get_mut(&claim_to_validate.claim_number.to_string()) {
                    entry.push(validator);
                } else {
                    self.validators.insert(claim_to_validate.claim_number.to_string(), vec![validator]);
                }
            },
            Message::Txn(
                txn, 
                _account_state
            ) => {

                let txn_to_validate = serde_json::from_str::<Txn>(&txn).unwrap();

                if let Some(entry) = self.validators.get_mut(&txn_to_validate.txn_id) {
                    entry.push(validator)
                } else {
                    self.validators.insert(txn_to_validate.txn_id, vec![validator]);
                }
            },
            Message::NewBlock(
                _,
                block,
                _,
                _,
                _,
                _,
            ) => {

                let block_to_validate = serde_json::from_str::<Block>(&block).unwrap();

                if let Some(entry) = self.validators.get_mut(&block_to_validate.block_hash) {
                    entry.push(validator)
                } else {
                    self.validators.insert(block_to_validate.block_hash, vec![validator]);
                }
            }
        };
    }

    pub fn process_validators(&mut self) {

        for (key, value) in self.validators.iter() {
            let valid = value.iter().filter(|&v| v.valid).count();

            if let Some(entry) = self.confirmations.get_mut(key) {
                *entry += valid as u8;
            } else {
                self.confirmations.insert(key.to_owned(), valid as u8);
            }

            if valid as f64 / value.len() as f64 >= 2.0/3.0 {

                match value[0].clone().message {
                    Message::ClaimHomesteaded(claim, _, _) => {
                        self.confirmed.entry(key.to_owned()).or_insert_with(|| Box::new(serde_json::from_str::<Claim>(&claim).unwrap()));
                    },
                    Message::ClaimAcquired(claim, _, _, _) => {

                        self.confirmed.entry(key.to_owned()).or_insert_with(|| Box::new(serde_json::from_str::<Claim>(&claim).unwrap()));
                    },
                    Message::Txn(txn, _) => {
                        self.confirmed.entry(key.to_owned()).or_insert_with(|| Box::new(serde_json::from_str::<Txn>(&txn).unwrap()));
                    },
                    Message::NewBlock(_, block, _, _, _, _) => {
                        self.confirmed.entry(key.to_owned()).or_insert_with(|| Box::new(serde_json::from_str::<Block>(&block).unwrap()));
                    }
                }

            } else if value.len() > 10 {
                let malicious: Vec<String> = value
                    .iter()
                    .filter(|&v| v.valid)
                    .map(|v| v.clone().node_wallet).collect();

                for validator in malicious {
                    self.slashed.push(validator);
                }
            }
        }
    }

    pub fn slash_claims(&mut self, account_state: &mut AccountState) {
        
        self.slashed.iter().for_each(|slash| {
            account_state.claims.retain(|_k, v| v.current_owner.clone().0.unwrap() != *slash);
            account_state.staked_claims.retain(|_k, v| *v != *slash);
            account_state.owned_claims.retain(|_k, v| *v != *slash);
        });
    }
}
