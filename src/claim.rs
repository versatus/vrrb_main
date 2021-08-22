use crate::account::{AccountState, StateOption::PendingClaimAcquired};
use crate::wallet::WalletAccount;
use crate::validator::{ValidatorOptions};
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
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, Deserialize, Hash, Serialize, PartialEq)]
pub enum CustodianInfo {
    Homesteader(bool),
    AcquisitionTimestamp(u128),
    AcquisitionPrice(u32),
    AcquiredFrom(Option<String>),
    OwnerNumber(u32),
    SellerSignature(Option<String>),
    BuyerSignature(Option<String>),
}

#[derive(Debug, Eq, Deserialize, PartialEq, Serialize)]
pub struct Claim {
    pub claim_number: u128,
    pub maturation_time: u128,
    pub price: u32,
    pub available: bool,
    pub staked: bool,
    pub chain_of_custody: HashMap<String, HashMap<String, Option<CustodianInfo>>>,
    pub current_owner: Option<String>,
    pub claim_payload: Option<String>,
    pub acquisition_time: Option<u128>,
    pub validators: Vec<String>,
}

impl Claim {
    pub fn new(time: u128, claim_number: u128) -> Claim {
        Claim {
            claim_number,
            maturation_time: time,
            price: 0,
            available: false,
            staked: false,
            chain_of_custody: HashMap::new(),
            current_owner: None,
            claim_payload: None,
            acquisition_time: None,
            validators: vec![],
        }
    }

    pub fn acquire(&mut self, acquirer: Arc<Mutex<WalletAccount>>, account_state: Arc<Mutex<AccountState>>) {
        if self.available {
            let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            let seller_pk = self.clone().current_owner.unwrap();
            let serialized_chain_of_custody = serde_json::to_string(&self.chain_of_custody).unwrap();

            let payload = format!(
                "{},{},{},{}",
                &self.maturation_time.to_string(),
                &self.price.to_string(),
                &self.available.to_string(),
                &serialized_chain_of_custody
            );

            let signature = acquirer.lock().unwrap().sign(&payload).unwrap();

            let claim = self.update(
                0, 
                false, 
                acquirer.lock().unwrap().pubkey.clone(), 
                time.as_nanos(),
                Some(acquirer.lock().unwrap().pubkey.clone()),
                None,
                Some(signature.to_string()),
                Some(payload),
                Some(time.as_nanos()),
                Arc::clone(&account_state)
            ).unwrap();
            
            if let Some(bal) = account_state.lock().unwrap().pending_balances.get_mut(&acquirer.lock().unwrap().pubkey).unwrap().get_mut("VRRB") {
                *bal -= self.price as u128;
            };

            if let Some(bal) = account_state.lock().unwrap().pending_balances.get_mut(&seller_pk).unwrap().get_mut("VRRB") {
                *bal += self.price as u128;
            };

            acquirer.lock().unwrap().claims.insert(claim.claim_number, claim);
        }
    }

    pub fn valid_chain_of_custody(&self, current_owner: String) -> Option<bool> {
        let current_owner_custody = self.chain_of_custody.get(&current_owner);
        match current_owner_custody {
            Some(_map) => {
                let previous_owner = current_owner_custody.unwrap().get("acquired_from").unwrap();

                if previous_owner.clone().unwrap() == CustodianInfo::AcquiredFrom(None) {
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
                                    pubkey
                                ) => { self.valid_chain_of_custody(pubkey.clone().unwrap()) },
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
        current_owner: Option<String>,
        seller_signature: Option<String>,
        buyer_signature: Option<String>,
        claim_payload: Option<String>,
        acquisition_time: Option<u128>,
        account_state: Arc<Mutex<AccountState>>,
    ) -> Result<Self, Error> {

        let mut custodian_data = HashMap::new();

        let previous_owners = self.chain_of_custody.keys().len() as u32;

        if self.chain_of_custody.is_empty() {
            custodian_data.insert("homesteader".to_string(),Some(CustodianInfo::Homesteader(true)));
        } else {
            custodian_data.insert("homesteader".to_string(),Some(CustodianInfo::Homesteader(false)));
        }

        custodian_data.insert("acquisition_timestamp".to_string(), Some(CustodianInfo::AcquisitionTimestamp(acquisition_timestamp)));
        custodian_data.insert("acquired_from".to_string(), Some(CustodianInfo::AcquiredFrom(self.clone().current_owner)));
        custodian_data.insert("acquisition_price".to_string(), Some(CustodianInfo::AcquisitionPrice(self.clone().price)));
        custodian_data.insert("owner_number".to_string(), Some(CustodianInfo::OwnerNumber(previous_owners + 1)));
        custodian_data.insert("seller_signature".to_string(), Some(CustodianInfo::SellerSignature(seller_signature)));
        custodian_data.insert("buyer_signature".to_string(), Some(CustodianInfo::BuyerSignature(buyer_signature)));

        self.chain_of_custody.insert(acquirer, custodian_data);

        let updated_claim = Self {
            price,
            available,
            chain_of_custody: self.chain_of_custody.clone(),
            current_owner,
            claim_payload,
            acquisition_time,
            validators: self.validators.clone(),
            ..*self
        };

        account_state.lock().unwrap().update(PendingClaimAcquired(serde_json::to_string(&updated_claim).unwrap()));

        Ok(updated_claim)
    }

    pub fn stake(&self, _wallet: WalletAccount, account_state: &mut AccountState) {
        account_state.claims.get_mut(&self.claim_number).unwrap().staked = true;
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
            staked: self.staked,
            chain_of_custody: self.chain_of_custody.clone(),
            current_owner: self.current_owner.clone(),
            claim_payload: self.claim_payload.clone(),
            acquisition_time: self.acquisition_time,
            validators: self.validators.clone(),
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
                    ValidatorOptions::ClaimAcquire(account_state, acquirer_address) => {
                        let account_state = serde_json::from_str::<AccountState>(&account_state).unwrap();
                        let signature = self.chain_of_custody.get(&self.clone().current_owner.unwrap()).unwrap().get("seller_signature").unwrap().as_ref().unwrap();
                        let pk = PublicKey::from_str(&self.clone().current_owner.unwrap()).unwrap();

                        let signature = match signature {
                            CustodianInfo::SellerSignature(Some(signature)) => {
                                Signature::from_str(&signature).unwrap()
                            },
                            CustodianInfo::SellerSignature(None) => {
                                return Some(false);
                            },
                            _ => { return Some(false); }
                        };

                        let valid_signature = self.verify(&signature, &pk);
                        
                        match valid_signature {
                            Ok(false) => { println!("invalid_signature"); return Some(false) },
                            Err(_e) => { println!("invalid_signature"); return Some(false)},
                            _ => {println!("Signature Valid!")},
                        }
                        let acquirer_pk = account_state.accounts_pk.get(&acquirer_address).unwrap();
                        match account_state.clone().balances.get(acquirer_pk).unwrap().get("VRRB") {
                            Some(bal) => {
                                if *bal < self.price as u128 {
                                    return Some(false);
                                } else {
                                    println!("Valid balance!");
                                }
                            }
                            None => {println!("Buyer not found!"); return Some(false)},
                        }

                        let valid_timestamp_owned = account_state.claims.get(&self.claim_number);

                        match valid_timestamp_owned {
                            Some(claim) => {
                                if claim.current_owner != self.clone().current_owner {
                                    println!("Owner mismatch");
                                    return Some(false);
                                } else {
                                    println!("Valid owner!")
                                }

                                if claim.maturation_time >= SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() {
                                    println!("Claim Mature already");
                                    return Some(false);
                                } else {
                                    println!("Valid Timestamp!");
                                }

                                let previous_owner = self.chain_of_custody.get(&acquirer_address).unwrap().get("acquired_from").unwrap();
                                
                                let _previous_owner_pk = match previous_owner {
                                    Some(CustodianInfo::AcquiredFrom(prev_owner_pk)) => { prev_owner_pk.clone().unwrap() },
                                    None => { return Some(false) }
                                    _ => panic!("Incorrect Formatting of Chain of Custody")
                                };

                                let is_staked = account_state.claims.get(&self.claim_number).unwrap().staked;
                                    
                                match is_staked {
                                    true => { { println!("claim is staked"); return Some(false) } },
                                    false => { println!("Claim not staked!") }
                                }

                                let valid_chain_of_custody = self.valid_chain_of_custody(acquirer_address);

                                match valid_chain_of_custody {
                                    Some(false) => { println!("Invalid chain of custody"); return Some(false) },
                                    None => { println!("Chain of custody check returned None"); return Some(false) }
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

