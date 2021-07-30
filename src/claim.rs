use crate::account::{AccountState, StateOption::ClaimAcquired, WalletAccount};
use crate::arbiter::Arbiter;
use crate::state::NetworkState;
use crate::validator::ValidatorOptions;
use crate::verifiable::Verifiable;
use bytebuffer::ByteBuffer;
use secp256k1::{Error, Message, Secp256k1};
use secp256k1::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use std::cmp::Ordering;
use std::sync::{Arc, Mutex};

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
    AcquisitionPrice(u32),
    AcquiredFrom((Option<String>, Option<String>, Option<String>)),
    OwnerNumber(u32),
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
    pub claim_number: u128,
    pub maturation_time: u128,
    pub price: u32,
    pub available: bool,
    pub chain_of_custody: HashMap<String, HashMap<String, Option<CustodianInfo>>>,
    pub current_owner: (Option<String>, Option<String>, Option<String>),
    pub claim_payload: Option<String>,
    pub acquisition_time: Option<u128>,
}

impl ClaimState {
    pub fn start() -> ClaimState {
        ClaimState {
            claims: HashMap::new(),
            owned_claims: HashMap::new(),
            furthest_visible_block: 0_u128,
            staked_claims: HashMap::new(),
        }
    }

    pub fn update(&mut self, claim: &Claim, network_state: Arc<Mutex<NetworkState>>) {
        self.claims
            .insert(claim.clone().maturation_time, claim.clone());

        self.owned_claims
            .insert(claim.clone().maturation_time, claim.clone());

        let state_result = network_state.lock().unwrap().update(self.clone(), "claim_state");
        
        if let Err(e) =  state_result {println!("Error in updating network state: {:?}", e)}
    }
}

impl Claim {
    pub fn new(time: u128, claim_number: u128) -> Claim {
        Claim {
            claim_number,
            maturation_time: time,
            price: 0,
            available: true,
            chain_of_custody: HashMap::new(),
            current_owner: (None, None, None),
            claim_payload: None,
            acquisition_time: None,
        }
    }

    pub fn acquire(
        &mut self,
        acquirer: Arc<Mutex<WalletAccount>>,
        account_state: Arc<Mutex<AccountState>>,
        network_state: Arc<Mutex<NetworkState>>,
    )

    {
        if self.available {
            let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            let seller_pk = self.clone().current_owner.1.unwrap();
            let serialized_chain_of_custody =
                serde_json::to_string(&self.chain_of_custody).unwrap();

            let payload = format!(
                "{},{},{},{}",
                &self.maturation_time.to_string(),
                &self.price.to_string(),
                &self.available.to_string(),
                &serialized_chain_of_custody
            );

            let signature = acquirer.lock().unwrap().sign(&payload).unwrap();
            let claim_state = Arc::new(Mutex::new(account_state.lock().unwrap().claim_state.clone()));

            let claim = self
                .update(
                    0,
                    false,
                    acquirer.lock().unwrap().address.clone(),
                    time.as_nanos(),
                    (
                        Some(acquirer.lock().unwrap().address.clone()),
                        Some(acquirer.lock().unwrap().public_key.to_string()),
                        Some(signature.to_string()),
                    ),
                    Some(payload),
                    Some(time.as_nanos()),
                    Arc::clone(&claim_state),
                    Arc::clone(&account_state),
                    Arc::clone(&network_state),
                ).unwrap();
            
            if let Some(bal) = account_state.lock().unwrap().available_coin_balances.get_mut(&acquirer.lock().unwrap().public_key.to_string()) {
                *bal -= self.price as u128;
            };

            if let Some(bal) = account_state.lock().unwrap().total_coin_balances.get_mut(&seller_pk) {
                *bal += self.price as u128;
            }

            let state_result = network_state.lock().unwrap().update(claim_state.lock().unwrap().clone(), "claim_state");
            
            if let Err(e) = state_result {println!("Error in updating network state: {:?}", e)}

            acquirer.lock().unwrap().claims.push(Some(claim));

        }
    }

    pub fn valid_chain_of_custody(&self, current_owner: String) -> Option<bool> {
        let current_owner_custody = self.chain_of_custody
                                        .get(&current_owner);
        match current_owner_custody {
            Some(_map) => {
                let previous_owner = current_owner_custody.unwrap().get("acquired_from").unwrap();

                if previous_owner.clone().unwrap() == CustodianInfo::AcquiredFrom((None, None, None)) {
                    match self.chain_of_custody
                        .get(&current_owner)
                        .unwrap()
                        .get("homesteader")
                        .unwrap() {
                        
                        Some(custodian_info) => {
                            match custodian_info {
                                CustodianInfo::Homesteader(true) => { Some(true) },
                                CustodianInfo::Homesteader(false) => { Some(false) },
                                _ => { println!("Invalid Format!"); Some(false) }
                            }
                        }, None => { println!("Something went wrong!"); Some(false) }
                    }
                } else {
                    match previous_owner{
                        Some(custodian_info) => {
                            match custodian_info {
                                CustodianInfo::AcquiredFrom(
                                    (address, _pubkey, _signature)
                                ) => { self.valid_chain_of_custody(address.clone().unwrap()) },
                                _ => { println!("Invalid format!"); None }
                            }
                        }, None => { println!("Something went wrong"); None }
                    }
                }
            },
            None => { Some(false) }
        }
    }

    // TODO: Group some parameters into a new type
    pub fn update(
        &mut self,
        price: u32,
        available: bool,
        acquirer: String,
        acquisition_timestamp: u128,
        current_owner: (Option<String>, Option<String>, Option<String>),
        claim_payload: Option<String>,
        acquisition_time: Option<u128>,
        claim_state: Arc<Mutex<ClaimState>>,
        account_state: Arc<Mutex<AccountState>>,
        network_state: Arc<Mutex<NetworkState>>,
    ) -> Result<Self, Error> {

        let mut custodian_data = HashMap::new();

        let previous_owners = self.chain_of_custody.keys().len() as u32;

        if self.chain_of_custody.is_empty() {
            custodian_data
                .insert("homesteader".to_string(),Some(CustodianInfo::Homesteader(true)));
        } else {
            custodian_data
                .insert("homesteader".to_string(),Some(CustodianInfo::Homesteader(false)));
        }

        custodian_data
            .insert("acquisition_timestamp".to_string(), Some(
                CustodianInfo::AcquisitionTimestamp(acquisition_timestamp)));

        custodian_data
            .insert("acquired_from".to_string(),Some(CustodianInfo::AcquiredFrom(
                self.clone().current_owner,
            )));

        custodian_data
            .insert("acquisition_price".to_string(),Some(CustodianInfo::AcquisitionPrice(self.clone().price)));

        custodian_data
            .insert("owner_number".to_string(), Some(CustodianInfo::OwnerNumber(previous_owners + 1)));

        self.chain_of_custody
            .insert(acquirer, custodian_data);

        let updated_claim = Self {
            price,
            available,
            chain_of_custody: self.chain_of_custody.clone(),
            current_owner,
            claim_payload,
            acquisition_time,
            ..*self
        };

        claim_state.lock().unwrap().update(&updated_claim, Arc::clone(&network_state));

        account_state.lock().unwrap().update(ClaimAcquired(serde_json::to_string(&updated_claim).unwrap()), network_state);

        Ok(updated_claim)
    }

    pub fn homestead(
        &mut self,
        wallet: Arc<Mutex<WalletAccount>>,
        claim_state: Arc<Mutex<ClaimState>>,
        account_state: Arc<Mutex<AccountState>>,
        network_state: Arc<Mutex<NetworkState>>,
    ) {
        if self.available {
            let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

            let serialized_chain_of_custody =
                serde_json::to_string(&self.chain_of_custody).unwrap();

            let payload = format!(
                "{},{},{},{}",
                &self.maturation_time.to_string(),
                &self.price.to_string(),
                &self.available.to_string(),
                &serialized_chain_of_custody
            );

            let signature = wallet.lock().unwrap().sign(&payload).unwrap();

            let claim = self
                .update(
                    0,
                    false,
                    wallet.lock().unwrap().address.clone(),
                    time.as_nanos(),
                    (
                        Some(wallet.lock().unwrap().address.clone()),
                        Some(wallet.lock().unwrap().public_key.to_string()),
                        Some(signature.to_string()),
                    ),
                    Some(payload),
                    Some(time.as_nanos()),
                    Arc::clone(&claim_state),
                    Arc::clone(&account_state),
                    Arc::clone(&network_state),
                )
                .unwrap();

            network_state.lock().unwrap().update(claim_state.lock().unwrap().clone(), "claim_state").unwrap();

            wallet.lock().unwrap().claims.push(Some(claim));

        }
    }

    pub fn stake(&self, wallet: WalletAccount, account_state: &mut AccountState) -> AccountState {
        account_state
            .claim_state
            .staked_claims
            .entry(wallet.public_key.to_string())
            .or_insert_with( HashMap::new);
        let mut staked_claims =
            account_state.claim_state.staked_claims[&wallet.public_key].clone();
        staked_claims
            .entry(self.maturation_time)
            .or_insert_with(|| self.clone());
        account_state
            .claim_state
            .staked_claims
            .insert(wallet.public_key, staked_claims);

        // TODO: Convert this to an account_state.update() need to add a matching arm
        // on the account state, and a StateOption::ClaimStaked.
        account_state.to_owned()
    }

    pub fn to_message(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }

    pub fn verify(&self, signature: &Signature, pk: &PublicKey) -> Result<bool, Error> {
        let message_bytes = self.claim_payload.clone().unwrap().as_bytes().to_owned();

        let mut buffer = ByteBuffer::new();

        buffer.write_bytes(&message_bytes);

        while buffer.len() < 32 {
            buffer.write_u8(0);
        }

        let new_message = buffer.to_bytes();

        let message_hash = blake3::hash(&new_message);

        let message_hash = Message::from_slice(message_hash.as_bytes())?;

        let secp = Secp256k1::new();

        let valid = secp.verify(&message_hash, signature, pk);

        match valid {
            Ok(()) => Ok(true),
            _ => Err(Error::IncorrectSignature),
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();

        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> Claim {
        let mut buffer: Vec<u8> = vec![];

        data.iter().for_each(|x| buffer.push(*x));

        let to_string = String::from_utf8(buffer).unwrap();

        serde_json::from_str::<Claim>(&to_string).unwrap()
    }
}

impl fmt::Display for Claim {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
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
            self.claim_payload
        )
    }
}

impl Clone for Claim {
    fn clone(&self) -> Claim {
        Claim {
            claim_number: self.claim_number,
            maturation_time: self.maturation_time,
            price: self.price,
            available: self.available,
            chain_of_custody: self.chain_of_custody.clone(),
            current_owner: self.current_owner.clone(),
            claim_payload: self.claim_payload.clone(),
            acquisition_time: self.acquisition_time,
        }
    }
}

impl Clone for ClaimState {
    fn clone(&self) -> ClaimState {
        ClaimState {
            claims: self.claims.clone(),
            owned_claims: self.owned_claims.clone(),
            furthest_visible_block: self.furthest_visible_block,
            staked_claims: self.staked_claims.clone(),
        }
    }
}

impl Verifiable for Claim {
    // Default method for Verifiable, receives the subject of the trait (Claim in this case)
    // and an Option containing one of the ValidatorOptions enum variants, matching arms
    // are used to handle the different variants and the resulting functionality required.
    // and returns an Option containing a bool. If it doesn't return anything the receiving
    // process should propagate an error.
    fn is_valid(&self, options: Option<ValidatorOptions>) -> Option<bool> {
        // match the options, should contain a ValidatorOption enum
        match options {
            Some(claim_option) => {
                // match the ValidatorOptions variant
                match claim_option {
                    // If it's a ClaimHomestead variant
                    ValidatorOptions::ClaimHomestead(account_state) => {
                        // convert the signature string found in the current owner tuple to a
                        // signature struct
                        // if self.claim_number < 1000000 {
                        //     return Some(true)
                        // }
                        let account_state = serde_json::from_str::<AccountState>(&account_state).unwrap();
                        let signature = Signature::from_str(&self.clone().current_owner.2.unwrap()).unwrap();

                        // conver the pubkey string found in the current owner tuple to a PublicKey
                        // struct
                        let pk =
                            PublicKey::from_str(&self.clone().current_owner.1.unwrap()).unwrap();

                        // check the validity of the signature (verify method gets the payload
                        // from the subject (in this case the claim).
                        let valid_signature = self.verify(&signature, &pk);

                        // If the signature is invalid, the validator will return Some(false)
                        match valid_signature {
                            Ok(false) => { return Some(false) },
                            Err(_e) => { return Some(false)},
                            _ => {},
                        }

                        // Extract the subject's maturation time from the account state's claim state field's own claims field
                        let valid_timestamp_unowned = account_state
                            .claim_state
                            .owned_claims
                            .get(&self.maturation_time);

                        // match the option returned by the .get() method called above
                        match valid_timestamp_unowned {
                            Some(claim) => {
                                // If the option is Some() check that the current owner in the subject claim
                                // matches the current owner in the retrieved claim. If not then check if
                                // the subject claim's current owner acquired it first.
                                if self.current_owner != claim.current_owner {
                                    // If the subject's current owner doesn't match the record
                                    // in the account state, and the acquisition time is later
                                    // than the record in the account state, return Some(false)
                                    match self.acquisition_time.cmp(&claim.acquisition_time) {
                                        Ordering::Greater => { return Some(false) },
                                        Ordering::Equal => {
                                            let addresses = vec![
                                            self.clone().current_owner.1.unwrap(),
                                            claim.clone().current_owner.1.unwrap(),
                                            ];

                                            let mut arbiter = Arbiter::new(addresses);

                                            arbiter.tie_handler();

                                            // match the value returned by the arbiter's winner field
                                            match arbiter.winner {
                                                Some((pubkey, _coin_flip)) => {
                                                    // If it's the subject's owner, then they are the winner
                                                    // of the tie handling process and now own the claim
                                                    // return true. Otherwise the owner in the record
                                                    // retrieved from the account state remains the owner
                                                    // TODO: Propagate tie handling winner message to network
                                                    // to ensure that account state's get updated and have the
                                                    // correct owner of the claim for the given maturity timestamp.
                                                    if pubkey == self.clone().current_owner.1.unwrap() {
                                                        return Some(true);
                                                    }
                                                }
                                                None => {
                                                    // If the winner field in the arbiter struct is None, the the program should panic
                                                    // this should never occur.
                                                    // TODO: Propagate an error message so the entire program doesn't shut down, but the
                                                    // user is informed that something is wrong with their validator instance and needs
                                                    // to be reset/updated, restarted or something.
                                                    panic!("Something went wrong. The arbiter should always contain a winner!");
                                                }
                                            }
                                        },
                                        Ordering::Less => {}
                                    }

                                } else {
                                    let claims: Vec<_> = account_state.claim_state.owned_claims
                                        .iter()
                                        .filter(|(_ts, claim)| claim.current_owner.0.clone().unwrap() == self.current_owner.0.clone().unwrap())
                                        .collect();
                                    if claims.len() >= 20 {
                                        return Some(false)
                                    }
                                }
                            }
                            None => { return Some(false) }
                        }
                        // If nothing else returns false, then return true.
                        Some(true)
                    }
                    ValidatorOptions::ClaimAcquire(account_state, acquirer_address) => {
                        let account_state = serde_json::from_str::<AccountState>(&account_state).unwrap();

                        let signature =
                            Signature::from_str(&self.clone().current_owner.2.unwrap()).unwrap();

                        let pk =
                            PublicKey::from_str(&self.clone().current_owner.1.unwrap()).unwrap();

                        let valid_signature = self.verify(&signature, &pk);
                        
                        match valid_signature {
                            Ok(false) => { println!("invalid_signature"); return Some(false) },
                            Err(_e) => { println!("invalid_signature"); return Some(false)},
                            _ => {println!("Signature Valid!")},
                        }
                        let acquirer_pk = account_state.accounts_address.get(&acquirer_address).unwrap();
                        match account_state
                            .clone()
                            .total_coin_balances
                            .get(acquirer_pk)
                        {
                            Some(bal) => {
                                if *bal < self.price as u128 {
                                    return Some(false);
                                } else {
                                    println!("Valid balance!");
                                }
                            }
                            None => {println!("Buyer not found!"); return Some(false)},
                        }

                        let valid_timestamp_owned = account_state
                            .claim_state
                            .owned_claims
                            .get(&self.maturation_time);

                        match valid_timestamp_owned {
                            Some(claim) => {
                                if claim.current_owner != self.clone().current_owner {
                                    println!("Owner mismatch");
                                    return Some(false);
                                } else {
                                    println!("Valid owner!")
                                }

                                if claim.maturation_time
                                    >= SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_nanos()
                                {
                                    println!("Claim Mature already");
                                    return Some(false);
                                } else {
                                    println!("Valid Timestamp!");
                                }

                                let previous_owner = self.chain_of_custody
                                    .get(&acquirer_address)
                                    .unwrap().get("acquired_from").unwrap();
                                
                                let previous_owner_pk = match previous_owner {
                                    Some(CustodianInfo::AcquiredFrom(
                                            (
                                            _prev_owner_address, 
                                            prev_owner_pk, 
                                            _prev_owner_sig
                                        ))) => { prev_owner_pk.clone().unwrap() },
                                    None => { return Some(false) }
                                    _ => panic!("Incorrect Formatting of Chain of Custody")
                                };

                                let is_staked =
                                    account_state.claim_state.staked_claims.get(&previous_owner_pk);
                                    
                                match is_staked {
                                    Some(map) => {
                                        let matched_claim = map.get(&self.maturation_time);
                                        match matched_claim {
                                            Some(_claim) => {println!("claim is staked"); return Some(false)},
                                            None => { println!("Claim not staked!")}
                                        }
                                    }
                                    None => {println!("Claim not staked!")}
                                }

                                let valid_chain_of_custody = self.valid_chain_of_custody(acquirer_address);

                                match valid_chain_of_custody {
                                    Some(false) => {println!("Invalid chain of custody"); return Some(false)},
                                    None => {println!("Chain of custody check returned None"); return Some(false)}
                                    _ => {}
                                }
                            }
                            None => { println!("Claim is unowned"); return Some(false)},
                        }
                        println!("All checks passed!");
                        Some(true)
                    },
                    _ => panic!("Allocated to the wrong process")
                }
            }
            None => {
                panic!("No Claim Option Found!");
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
}
