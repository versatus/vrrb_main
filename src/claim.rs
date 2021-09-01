use crate::account::AccountState;
use crate::validator::ValidatorOptions;
use crate::verifiable::Verifiable;
use crate::wallet::WalletAccount;
use bytebuffer::ByteBuffer;
use secp256k1::{Error, Message, Secp256k1};
use secp256k1::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use ritelinked::LinkedHashMap;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

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

#[derive(Clone, Debug, Eq, Serialize, Deserialize, PartialEq, Hash)]
pub enum CustodianOption {
    Homesteader,
    AcquisitionTime,
    Seller,
    Price,
    OwnerNumber,
    SellerSignature,
    BuyerSignature
}

#[derive(Debug, Eq, Deserialize, PartialEq, Serialize)]
pub struct Claim {
    pub claim_number: u128,
    pub claim_hash: Option<String>,
    pub expiration_time: u128,
    pub price: u32,
    pub available: bool,
    pub staked: bool,
    /// pubkey -> custodian_info 
    pub chain_of_custody: LinkedHashMap<String, LinkedHashMap<CustodianOption, Option<CustodianInfo>>>,
    pub current_owner: Option<String>,
    pub claim_payload: Option<String>,
    pub acquisition_time: Option<u128>,
    pub validators: Vec<String>,
}

impl Claim {
    pub fn new(time: u128, claim_number: u128) -> Claim {
        Claim {
            claim_number,
            claim_hash: None,
            expiration_time: time,
            price: 0,
            available: false,
            staked: false,
            chain_of_custody: LinkedHashMap::new(),
            current_owner: None,
            claim_payload: None,
            acquisition_time: None,
            validators: vec![],
        }
    }

    pub fn acquire(&mut self, acquirer: Arc<Mutex<WalletAccount>>) {
        if self.available {
            let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            let _seller_pk = self.clone().current_owner.unwrap();
            let serialized_chain_of_custody =
                serde_json::to_string(&self.chain_of_custody).unwrap();

            let payload = format!(
                "{},{},{},{}",
                &self.expiration_time.to_string(),
                &self.price.to_string(),
                &self.available.to_string(),
                &serialized_chain_of_custody
            );

            let signature = acquirer.lock().unwrap().sign(&payload).unwrap();

            self.update(
                0,
                false,
                acquirer.lock().unwrap().pubkey.clone(),
                time.as_nanos(),
                Some(acquirer.lock().unwrap().pubkey.clone()),
                None,
                Some(signature.to_string()),
                Some(payload),
                Some(time.as_nanos()),
            );
        }
    }

    pub fn valid_chain_of_custody(&self, current_owner: String) -> Option<bool> {
        let current_owner_custody = self.chain_of_custody.get(&current_owner);
        match current_owner_custody {
            Some(_map) => {
                let previous_owner = current_owner_custody.unwrap().get(&CustodianOption::Seller).unwrap();

                if previous_owner.clone().unwrap() == CustodianInfo::AcquiredFrom(None) {
                    match self
                        .chain_of_custody
                        .get(&current_owner)
                        .unwrap()
                        .get(&CustodianOption::Homesteader)
                        .unwrap()
                    {
                        Some(custodian_info) => match custodian_info {
                            CustodianInfo::Homesteader(true) => Some(true),
                            CustodianInfo::Homesteader(false) => Some(false),
                            _ => {
                                println!("Invalid Format!");
                                Some(false)
                            }
                        },
                        None => {
                            println!("Something went wrong!");
                            Some(false)
                        }
                    }
                } else {
                    match previous_owner {
                        Some(custodian_info) => match custodian_info {
                            CustodianInfo::AcquiredFrom(pubkey) => {
                                self.valid_chain_of_custody(pubkey.clone().unwrap())
                            }
                            _ => {
                                println!("Invalid format!");
                                None
                            }
                        },
                        None => {
                            println!("Something went wrong");
                            None
                        }
                    }
                }
            }
            None => Some(false),
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
    ) {
        let mut custodian_data = LinkedHashMap::new();

        let previous_owners = self.chain_of_custody.keys().len() as u32;

        if self.chain_of_custody.is_empty() {
            custodian_data.insert(
                CustodianOption::Homesteader,
                Some(CustodianInfo::Homesteader(true)),
            );
        } else {
            custodian_data.insert(
                CustodianOption::Homesteader,
                Some(CustodianInfo::Homesteader(false)),
            );
        }

        custodian_data.insert(
            CustodianOption::AcquisitionTime,
            Some(CustodianInfo::AcquisitionTimestamp(acquisition_timestamp)),
        );
        custodian_data.insert(
            CustodianOption::Seller,
            Some(CustodianInfo::AcquiredFrom(self.clone().current_owner)),
        );
        custodian_data.insert(
            CustodianOption::Price,
            Some(CustodianInfo::AcquisitionPrice(self.clone().price)),
        );
        custodian_data.insert(
            CustodianOption::OwnerNumber,
            Some(CustodianInfo::OwnerNumber(previous_owners + 1)),
        );
        custodian_data.insert(
            CustodianOption::SellerSignature,
            Some(CustodianInfo::SellerSignature(seller_signature)),
        );
        custodian_data.insert(
            CustodianOption::BuyerSignature,
            Some(CustodianInfo::BuyerSignature(buyer_signature)),
        );

        self.chain_of_custody.insert(acquirer, custodian_data);

        Self {
            price,
            available,
            claim_hash: self.claim_hash.clone(),
            chain_of_custody: self.chain_of_custody.clone(),
            current_owner,
            claim_payload,
            acquisition_time,
            validators: self.validators.clone(),
            ..*self
        };
    }

    pub fn stake(&self, wallet: &mut WalletAccount, account_state: &mut AccountState) {
        account_state
            .claim_pool
            .confirmed
            .get_mut(&self.claim_number)
            .unwrap()
            .staked = true;
        let mut wallet_claims = account_state.claim_pool.confirmed.clone();
        wallet_claims.retain(|_, v| v.current_owner == Some(wallet.pubkey.clone()));
        wallet.claims = wallet_claims;
    }

    pub fn hash(&self) {
        
    }

    pub fn is_expired(&self) -> bool {
        self.expiration_time < SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
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
        self.to_string().as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> Claim {
        let mut buffer: Vec<u8> = vec![];
        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();
        Claim::from_string(&to_string)
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn from_string(string: &String) -> Claim {
        serde_json::from_str::<Claim>(&string).unwrap()
    }
}

impl fmt::Display for Claim {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Claim(\n \
            expiration_time: {:?}\n \
            price: {}\n \
            available: {}\n \
            chain_of_custody: {:?}\n \
            current_owner: {:?}\n \
            claim_payload: {:?}",
            self.expiration_time,
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
            claim_hash: self.claim_hash.clone(),
            expiration_time: self.expiration_time,
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
        if let Some(claim_option) = options {
            if let ValidatorOptions::ClaimAcquire(
                network_state,
                account_state,
                _seller_address,
                acquirer_address,
            ) = claim_option
            {
                let signature = self
                    .chain_of_custody
                    .get(&self.clone().current_owner.unwrap())
                    .unwrap()
                    .get(&CustodianOption::SellerSignature)
                    .unwrap()
                    .as_ref()
                    .unwrap();

                let pk = PublicKey::from_str(&self.clone().current_owner.unwrap()).unwrap();

                let signature = match signature {
                    CustodianInfo::SellerSignature(Some(signature)) => {
                        Signature::from_str(&signature).unwrap()
                    }
                    CustodianInfo::SellerSignature(None) => {
                        return Some(false);
                    }
                    _ => {
                        return Some(false);
                    }
                };

                if let Ok(false) | Err(_) = self.verify(&signature, &pk) {
                    println!("Invalid Signature");
                    return Some(false);
                };

                let credits = if let Some(amount) = network_state.credits.get(&acquirer_address) {
                    *amount
                } else {
                    println!("Acquirer has 0 credits, cannot purchase");
                    return Some(false);
                };

                let debits = if let Some(amount) = network_state.debits.get(&acquirer_address) {
                    *amount
                } else {
                    0u128
                };

                if let Some(amount) = credits.checked_sub(debits) {
                    if amount < self.price as u128 {
                        return Some(false);
                    } else {
                        println!("Valid balance");
                    }
                } else {
                    panic!("Debits should never exceed credits");
                }

                if let Some(claim) = account_state.claim_pool.confirmed.get(&self.claim_number) {
                    if claim.current_owner != self.clone().current_owner {
                        println!("Owner mismatch");
                        return Some(false);
                    } else {
                        println!("Valid owner");
                    }

                    if claim.expiration_time
                        >= SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_nanos()
                    {
                        println!("Claim already mature, cannot sell or buy");
                        return Some(false);
                    } else {
                        println!("Valid timestamp");
                    }

                    if let true = account_state
                        .claim_pool
                        .confirmed
                        .get(&self.claim_number)
                        .unwrap()
                        .staked
                    {
                        println!("claim is staked, cannot be sold");
                        return Some(false);
                    };

                    if let Some(false) = self.valid_chain_of_custody(acquirer_address) {
                        println!("Invalid chain of custody");
                        return Some(false);
                    };
                }

                println!("All checks passed!");
            };

            return Some(true);
        } else {
            println!("No options found!");
            return Some(false);
        }
    }
}
