use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::hash::Hash;
use std::fmt;
use crate::account::WalletAccount;
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
#[derive(Clone, Debug, Eq, Deserialize, PartialEq, Serialize)]
pub struct ClaimState {
    pub claims: HashMap<u128, Claim>,
}

#[derive(Clone, Debug, Eq, Deserialize, PartialEq, Serialize)]
pub struct Claim {
    pub maturation_time: u128,
    pub price: i32,
    pub available: bool,
    pub chain_of_custody: HashMap<String, HashMap<String, Option<CustodianInfo>>>,
    pub current_owner: (Option<String>, Option<String>, Option<String>),
    pub claim_payload: Option<String>,
}

impl Claim {
    pub fn new(time: u128) -> Claim {
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
    ) -> Self {
        let mut new_custodian = HashMap::new();
        let mut custodian_data = HashMap::new();
        custodian_data
            .entry("homesteader".to_string())
            .or_insert(Some(CustodianInfo::Homesteader(true)));
        custodian_data
            .entry("acquisition_timestamp".to_string())
            .or_insert(Some(CustodianInfo::AcquisitionTimestamp(acquisition_timestamp)));
        new_custodian.entry(acquirer).or_insert(custodian_data);

        Self {
            price,
            available,
            chain_of_custody: new_custodian,
            current_owner: current_owner,
            claim_payload: claim_payload,
            ..*self
        }
    }

    pub fn homestead(
        &self, wallet: WalletAccount
    ) -> Self {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let serialized_chain_of_custody = serde_json::to_string(&self.chain_of_custody).unwrap();
        let payload = format!("{},{},{},{}", 
            &self.maturation_time.to_string(),
            &self.price.to_string(),
            &self.available.to_string(), 
            &serialized_chain_of_custody
        );
        let signature = wallet.sign(payload.clone()).unwrap();
        self.update(
            0, 
            false, 
            wallet.address.clone(), 
            time.as_nanos(),
            (Some(wallet.address), 
            Some(wallet.public_key.to_string()),
            Some(signature.to_string())),
            Some(payload)
        )
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
// TODO: Write tests for this module