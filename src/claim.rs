use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::hash::Hash;
use std::fmt;
use std::io::Error;
use crate::account::{WalletAccount, AccountState, StateOption::ClaimAcquired};
use crate::state::NetworkState;
use crate::validator::ValidatorOptions;
use crate::verifiable::Verifiable;

//  Claim receives
//      - a maturation time (UNIX Timestamp in nanoseconds)
//      - a price (starts at 0, i32)
//      - availability (bool)
//      - a chain of custody which is a hashmap of hash maps
//      - a current owner which is a thruple contining the acquirer address,
//        the acquirer wallet public key, and the wallet signature on the payload
//        the payload is a hashmap in json format with the key of the maturation time
//        and a value of the chain of custody (which is also a hashmap converted to json format)

#[derive(Clone, Debug, Eq, Deserialize, Hash, Serialize, PartialEq)]
pub enum CustodianInfo {
    Homesteader(bool),
    AcquisitionTimestamp(u128),
    AcquisitionPrice(i32),
    Address(String),
    PublicKey(String),
    Signature(String),
}

// Claim state is a structure that contains
// all the relevant information about the 
// currently outstanding (unmined) claims.
#[derive(Debug, Eq, Deserialize, PartialEq, Serialize)]
pub struct ClaimState {
    pub claims: HashMap<u128, Claim>,
    pub owned_claims: HashMap<u128, Claim>,
    pub staked_claims: HashMap<String, HashMap<u128, Claim>>,
    pub furthest_visible_block: u128,

}

#[derive(Debug, Eq, Deserialize, PartialEq, Serialize)]
pub struct Claim {
    pub maturation_time: u128,
    pub price: i32,
    pub available: bool,
    pub chain_of_custody: HashMap<String, HashMap<String, Option<CustodianInfo>>>,
    pub current_owner: (Option<String>, Option<String>, Option<String>),
    pub claim_payload: Option<String>,
}

impl ClaimState {
    pub fn start() -> ClaimState {
        ClaimState {
            claims: HashMap::new(),
            owned_claims: HashMap::new(),
            furthest_visible_block: 0 as u128,
            staked_claims: HashMap::new(),
        }
    }

    pub fn update(
        &mut self, 
        claim: &Claim, 
        network_state: &mut NetworkState
    ) 
    
    {
        self.claims.insert(claim.clone().maturation_time, claim.clone());

        self.owned_claims.insert(claim.clone().maturation_time, claim.clone());
        
        network_state.update(self.clone(), "claim_state");
    }
}

impl Claim {
    pub fn new(time: u128) -> Claim 
    
    {
        Claim {
            maturation_time: time,
            price: 0,
            available: true,
            chain_of_custody: HashMap::new(),
            current_owner: (None, None, None),
            claim_payload: None,
        }
    }

    pub fn update(
        &self,
        price: i32,
        available: bool,
        acquirer: String,
        acquisition_timestamp: u128,
        current_owner: (Option<String>, Option<String>, Option<String>),
        claim_payload: Option<String>,
        claim_state: &mut ClaimState,
        account_state: &mut AccountState,
        network_state: &mut NetworkState,
    ) -> Result<(Self, ClaimState, AccountState), Error> 
    
    {
        let mut new_custodian = HashMap::new();

        let mut custodian_data = HashMap::new();
        
        custodian_data
            .entry("homesteader".to_string())
            .or_insert(Some(CustodianInfo::Homesteader(true)));
        
        custodian_data
            .entry("acquisition_timestamp".to_string())
            .or_insert(Some(CustodianInfo::AcquisitionTimestamp(acquisition_timestamp)));
        
        new_custodian.entry(acquirer).or_insert(custodian_data);

        let updated_claim = Self {
            price,
            available,
            chain_of_custody: new_custodian,
            current_owner: current_owner,
            claim_payload: claim_payload,
            ..*self
        };
        
        claim_state.update(&updated_claim.clone(), network_state);
        
        account_state.update(ClaimAcquired(updated_claim.clone()), network_state).unwrap();
        
        Ok((updated_claim, claim_state.to_owned(), account_state.to_owned()))

    }

    pub fn homestead(
        &self, 
        wallet: &mut WalletAccount, 
        claim_state: &mut ClaimState, 
        account_state: &mut AccountState, 
        network_state: &mut NetworkState,
    ) -> Option<(WalletAccount, AccountState)> 
    
    {
        if self.available {
            
            let time = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap();

            let serialized_chain_of_custody = serde_json::to_string(
                &self.chain_of_custody
            ).unwrap();

            let payload = format!("{},{},{},{}", 
                &self.maturation_time.to_string(),
                &self.price.to_string(),
                &self.available.to_string(), 
                &serialized_chain_of_custody
            );

            let mut cloned_wallet = wallet.clone();
            
            let signature = wallet.sign(payload.clone()).unwrap();
            
            let (
                claim, 
                claim_state, 
                account_state) = self.update(
                0, 
                false, 
                wallet.address.clone(), 
                time.as_nanos(),
                (Some(wallet.address.clone()), 
                Some(wallet.public_key.to_string()),
                Some(signature.to_string())),
                Some(payload),
                claim_state,
                account_state,
                network_state,
            ).unwrap();

            network_state.update(claim_state, "claim_state");

            cloned_wallet.claims.push(Some(claim.clone()));
            
            return Some(
                ( 
                    cloned_wallet.to_owned(), 
                    account_state.to_owned(), 
                )
            );

        } else {
            return None;
        }
    }

    pub fn stake(
        &self, 
        wallet: WalletAccount, 
        account_state: &mut AccountState
    ) -> AccountState

    {
        account_state.claim_state.staked_claims.entry(
            wallet.public_key.to_string()).or_insert(HashMap::new());
        let mut staked_claims = account_state
            .claim_state
            .staked_claims[&wallet.public_key.to_string()]
            .clone();
        staked_claims.entry(self.maturation_time).or_insert(self.clone());
        account_state.claim_state.staked_claims.insert(wallet.public_key.to_string(), staked_claims);
        
        // TODO: Convert this to an account_state.update() need to add a matching arm
        // on the account state, and a StateOption::ClaimStaked.
        account_state.to_owned()
    }
}

impl fmt::Display for Claim {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
            "Claim(\n \
            maturation_time: {:?}\n \
            price: {}\n \
            available: {}\n \
            chain_of_custody: {:?}\n \
            current_owner: {:?}\n \
            claim_payload: {:?}", 
            self.maturation_time, 
            self.price, 
            self.available, 
            self.chain_of_custody, 
            self.current_owner,
            self.claim_payload)
    }
}

impl Clone for Claim {
    fn clone(&self) -> Claim 
    
    {
        Claim {
            maturation_time: self.maturation_time,
            price: self.price,
            available: self.available,
            chain_of_custody: self.chain_of_custody.clone(),
            current_owner: self.current_owner.clone(),
            claim_payload: self.claim_payload.clone(),
        }
    }
}

impl Clone for ClaimState {
    fn clone(&self) -> ClaimState 
    
    {
        ClaimState {
            claims: self.claims.clone(),
            owned_claims: self.owned_claims.clone(),
            furthest_visible_block: self.furthest_visible_block.clone(),
            staked_claims: self.staked_claims.clone(),
        }
    }
}

impl Verifiable for Claim {
    fn is_valid(&self, options: Option<ValidatorOptions>) -> Option<bool> {
        match options {
            Some(claim_option) => {
                match claim_option {

                    ValidatorOptions::ClaimHomestead => { 
                        return Some(true) 
                    },
                    ValidatorOptions::ClaimAcquire => { 
                        return Some(true) 
                    },
                    ValidatorOptions::ClaimSell => { 
                        return Some(true) 
                    },
                    ValidatorOptions::ClaimStake => { 
                        return Some(true) 
                    },
                    _ => panic!("Message allocated to wrong process")

                }
            },
            None => {
                panic!("No Claim Option Found!");
            }
        }
    }
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::{reward::RewardState, block::Block};

    #[test]
    fn test_claim_creation_with_new_block() {
        let mut network_state = NetworkState::restore("claim_test1_state.db");
        let reward_state = RewardState::start(&mut network_state);
        let mut account_state = AccountState::start();
        let mut claim_state = ClaimState::start();

        let new_wallet = WalletAccount::new(&mut account_state, &mut network_state);
        account_state = new_wallet.1;
        let mut wallet = new_wallet.0;

        let genesis = Block::genesis(
            reward_state,
            &mut wallet, 
            &mut account_state, 
            &mut network_state).unwrap();

        account_state = genesis.1;
        let mut last_block = genesis.0;

        for claim in &account_state.clone().claim_state.claims {
            let _ts = claim.0;
            let claim_obj = claim.1;

            let (new_wallet, new_account_state) = claim_obj.homestead(
                &mut wallet, &mut claim_state, &mut account_state, &mut network_state).unwrap();
            
            wallet = new_wallet;
            account_state = new_account_state;
        }

        for claim in &wallet.clone().claims {
            let claim_obj = claim.clone().unwrap();
            let (next_block, new_account_state) = Block::mine(
                &reward_state,
                claim_obj, 
                last_block, 
                HashMap::new(), 
                &mut wallet, 
                &mut account_state, 
                &mut network_state
            ).unwrap().unwrap();
            
            last_block = next_block;
            account_state = new_account_state;            

        }

        assert_eq!(account_state.claim_state.claims.len(), 400);
    }

    #[test]
    fn test_claim_update_after_homestead() {
    }

    #[test]
    fn test_mature_claim_valid_signature_mines_block() {

    }

    #[test]
    fn test_immature_claim_valid_signature_doesnt_mine_block() {

    }

    #[test]
    fn test_mature_claim_invalid_signature_doesnt_mine_block() {

    }

    #[test]
    fn test_claim_for_sale() {

    }

    #[test]
    fn test_claim_sold() {

    }
}