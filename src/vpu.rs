use crate::{
    account::AccountState, 
    validator::{Validator, Message}, 
    verifiable::Verifiable, 
    claim::Claim,
    block::Block,
    txn::Txn,
};
use std::collections::HashMap;


pub struct ValidatorProcessor<'a> {
    pub validators: HashMap<String, Vec<Validator<'a>>>,
    pub confirmations: HashMap<String, u8>,
    pub confirmed: HashMap<String, Box<dyn Verifiable>>,
    pub slashed: Vec<String>,
}

impl<'a> ValidatorProcessor<'a> {

    pub fn start() -> ValidatorProcessor<'a> {

        ValidatorProcessor {
            validators: HashMap::new(),
            confirmations: HashMap::new(),
            confirmed: HashMap::new(),
            slashed: vec![],
        }
    }

    pub fn new_validator(&mut self, validator: Validator<'a>) {

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
                last_block,
                block,
                pubkey,
                account_state,
                reward_state,
                network_state,
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
                    .map(|v| v.clone().node_wallet.address).collect();

                for validator in malicious {
                    self.slashed.push(validator);
                }
            }
        }
    }

    pub fn slash_claims(&mut self, account_state: &mut AccountState) {
        
        self.slashed.iter().for_each(|slash| {
            let claim_state = account_state.clone().claim_state;
            let staked = claim_state.staked_claims.get(slash).unwrap();
            account_state.claim_state.staked_claims.remove(slash);
            let staked_vec: Vec<u128> = staked.iter().map(|(k, _v)| *k).collect();
            staked_vec.iter().for_each( |time| {
                let claim_number = account_state.claim_state.owned_claims.get(time).unwrap().claim_number;
                account_state.claim_state.owned_claims.remove(&time);
                account_state.claim_state.claims.insert(*time, Claim::new(*time, claim_number));
            })
        });
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_validator_setting_by_message_id() {

    }

    #[test]
    fn test_vpu_updates_state_when_confirmed_valid() {

    }

    #[test]
    fn test_vpu_updates_state_when_confirmed_invalid() {

    }

    #[test]
    fn test_vpu_slashes_claims() {
        
    }


}