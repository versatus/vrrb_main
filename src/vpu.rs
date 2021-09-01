use crate::{
    account::AccountState, 
    validator::{Validator, Message}, 
    verifiable::Verifiable, 
    claim::Claim,
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
            Message::ClaimAcquired(
                claim, 
                _network_state,
                _account_state, 
                _seller_pubkey, 
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
                txn_string, 
                _account_state,
                _network_state
            ) => {

                let txn = Txn::from_string(&txn_string);

                if let Some(entry) = self.validators.get_mut(&txn.txn_id) {
                    entry.push(validator)
                } else {
                    self.validators.insert(txn.txn_id, vec![validator]);
                }
            },
            Message::NewBlock(
                _,
                block,
                _,
                _,
            ) => {

                if let Some(entry) = self.validators.get_mut(&block.block_hash) {
                    entry.push(validator)
                } else {
                    self.validators.insert(block.block_hash, vec![validator]);
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
                    Message::ClaimAcquired(claim, _, _, _, _) => {

                        self.confirmed.entry(key.to_owned()).or_insert_with(|| Box::new(serde_json::from_str::<Claim>(&claim).unwrap()));
                    },
                    Message::Txn(txn, _, _) => {
                        self.confirmed.entry(key.to_owned()).or_insert_with(|| Box::new(serde_json::from_str::<Txn>(&txn).unwrap()));
                    },
                    Message::NewBlock(_, block, _, _) => {
                        self.confirmed.entry(key.to_owned()).or_insert_with(|| Box::new(block));
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
            account_state.claim_pool.confirmed.retain(|_k, v| v.current_owner.clone().unwrap() != *slash);
        });
    }
}
