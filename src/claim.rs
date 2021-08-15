use crate::account::{AccountState, StateOption::PendingClaimAcquired, WalletAccount};
use crate::arbiter::Arbiter;
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
    AcquiredFrom((Option<String>, Option<String>)),
    OwnerNumber(u32),
}

#[derive(Debug, Eq, Deserialize, PartialEq, Serialize)]
pub struct Claim {
    pub claim_number: u128,
    pub maturation_time: u128,
    pub price: u32,
    pub available: bool,
    pub chain_of_custody: HashMap<String, HashMap<String, Option<CustodianInfo>>>,
    pub current_owner: (Option<String>, Option<String>),
    pub claim_payload: Option<String>,
    pub acquisition_time: Option<u128>,
}

impl Claim {
    pub fn new(time: u128, claim_number: u128) -> Claim {
        Claim {
            claim_number,
            maturation_time: time,
            price: 0,
            available: true,
            chain_of_custody: HashMap::new(),
            current_owner: (None, None),
            claim_payload: None,
            acquisition_time: None,
        }
    }

    pub fn acquire(
        &mut self,
        acquirer: Arc<Mutex<WalletAccount>>,
        account_state: Arc<Mutex<AccountState>>,
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

            let claim = self
                .update(
                    0,
                    false,
                    acquirer.lock().unwrap().pubkey.clone(),
                    time.as_nanos(),
                    (Some(acquirer.lock().unwrap().pubkey.clone()), Some(signature.to_string())),
                    Some(payload),
                    Some(time.as_nanos()),
                    Arc::clone(&account_state),
                ).unwrap();
            
            if let Some(bal) = account_state.lock()
                                .unwrap().pending_balances
                                .get_mut(&acquirer.lock().unwrap().pubkey)
                                .unwrap()
                                .get_mut("VRRB") 
            {
                *bal -= self.price as u128;
            };

            if let Some(bal) = account_state.lock()
                                .unwrap().pending_balances
                                .get_mut(&seller_pk)
                                .unwrap()
                                .get_mut("VRRB") 
            {
                *bal += self.price as u128;
            };

            acquirer.lock().unwrap().claims.push(Some(claim));

        }
    }

    pub fn valid_chain_of_custody(&self, current_owner: String) -> Option<bool> {
        let current_owner_custody = self.chain_of_custody
                                        .get(&current_owner);
        match current_owner_custody {
            Some(_map) => {
                let previous_owner = current_owner_custody.unwrap().get("acquired_from").unwrap();

                if previous_owner.clone().unwrap() == CustodianInfo::AcquiredFrom((None, None)) {
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
                                    (pubkey, _signature)
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
        current_owner: (Option<String>, Option<String>),
        claim_payload: Option<String>,
        acquisition_time: Option<u128>,
        account_state: Arc<Mutex<AccountState>>,
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

        account_state.lock().unwrap().update(PendingClaimAcquired(serde_json::to_string(&updated_claim).unwrap()));

        Ok(updated_claim)
    }

    pub fn homestead(
        &mut self,
        wallet: Arc<Mutex<WalletAccount>>,
        account_state: Arc<Mutex<AccountState>>,
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
                    wallet.lock().unwrap().pubkey.clone(),
                    time.as_nanos(),
                    (Some(wallet.lock().unwrap().clone().pubkey), Some(signature.to_string())),
                    Some(payload),
                    Some(time.as_nanos()),
                    Arc::clone(&account_state),
                )
                .unwrap();

            wallet.lock().unwrap().claims.push(Some(claim));

        }
    }

    pub fn stake(&self, wallet: WalletAccount, account_state: &mut AccountState) {
        account_state.staked_claims.insert(self.claim_number, wallet.pubkey);
        // TODO: Convert this to an account_state.update() need to add a matching arm
        // on the account state, and a StateOption::ClaimStaked.
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
                        let signature = Signature::from_str(&self.clone().current_owner.1.unwrap()).unwrap();

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
                        let valid_timestamp_unowned = account_state.owned_claims.get(&self.maturation_time);

                        // match the option returned by the .get() method called above
                        match valid_timestamp_unowned {
                            Some(owner) => {

                                let claim = account_state.claims
                                                        .iter()
                                                        .find(|(&_n, claim)| {
                                                            claim.current_owner.clone().0.unwrap() == *owner
                                                        })
                                                        .unwrap().1;

                                // If the option is Some() check that the current owner in the subject claim
                                // matches the current owner in the retrieved claim. If not then check if
                                // the subject claim's current owner acquired it first.
                                if self.current_owner.clone().0.unwrap() != *owner {
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
                                    let claims: Vec<_> = account_state.claims
                                        .iter()
                                        .filter(|(_n, claim)| {
                                            claim.current_owner.0.clone().unwrap() == self.current_owner.0.clone().unwrap()
                                        })
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
                            Signature::from_str(&self.clone().current_owner.1.unwrap()).unwrap();

                        let pk =
                            PublicKey::from_str(&self.clone().current_owner.0.unwrap()).unwrap();

                        let valid_signature = self.verify(&signature, &pk);
                        
                        match valid_signature {
                            Ok(false) => { println!("invalid_signature"); return Some(false) },
                            Err(_e) => { println!("invalid_signature"); return Some(false)},
                            _ => {println!("Signature Valid!")},
                        }
                        let acquirer_pk = account_state.accounts_pk.get(&acquirer_address).unwrap();
                        match account_state
                            .clone()
                            .balances
                            .get(acquirer_pk)
                            .unwrap()
                            .get("VRRB")
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

                        let valid_timestamp_owned = account_state.claims
                            .get(&self.claim_number);

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
                                
                                let _previous_owner_pk = match previous_owner {
                                    Some(CustodianInfo::AcquiredFrom(
                                            (
                                            prev_owner_pk, 
                                            _prev_owner_sig
                                        ))) => { prev_owner_pk.clone().unwrap() },
                                    None => { return Some(false) }
                                    _ => panic!("Incorrect Formatting of Chain of Custody")
                                };

                                let is_staked =
                                    account_state.staked_claims.get(&self.claim_number);
                                    
                                match is_staked {
                                    Some(_staker) => {
                                        {println!("claim is staked"); return Some(false)}
                                    },
                                    None => {
                                        println!("Claim not staked!")
                                    }
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

