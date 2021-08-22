use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt;
use secp256k1::{PublicKey, Signature};
use serde::{Serialize, Deserialize};
use crate::validator::ValidatorOptions;
use crate::verifiable::Verifiable;
use crate::{wallet::WalletAccount, account::AccountState, validator::Validator};
use uuid::Uuid;
use sha256::digest_bytes;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Txn {
    pub txn_id: String,
    pub txn_timestamp: u128,
    pub sender_address: String,
    pub sender_public_key: String,
    pub receiver_address: String,
    pub txn_token: Option<String>,
    pub txn_amount: u128,
    pub txn_payload: String,
    pub txn_signature: String,
    pub validators: Vec<Validator>,
}

impl Txn {

    pub fn new(
        sender: Arc<Mutex<WalletAccount>>,
        sender_address: String, 
        receiver: String, 
        amount: u128
    ) -> Txn {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    
        let payload = format!("{},{},{},{},{}",
            &time.as_nanos().to_string(),
            &sender_address, 
            &sender.lock().unwrap().pubkey.clone(), 
            &receiver, &amount.to_string()
        );
    
        let signature = sender.lock().unwrap().sign(&payload).unwrap();
        let uid_payload = format!("{},{},{}", &payload, Uuid::new_v4().to_string(), &signature.to_string());

        Txn {
            txn_id: digest_bytes(uid_payload.as_bytes()),
            txn_timestamp: time.as_nanos(),
            sender_address: sender_address,
            sender_public_key: sender.lock().unwrap().pubkey.clone(),
            receiver_address: receiver,
            txn_token: None,
            txn_amount: amount,
            txn_payload: payload,
            txn_signature: signature.to_string(),
            validators: vec![],
        }
    }

    // TODO: convert to_message into a function of the verifiable trait,
    // all verifiable objects need to be able to be converted to a message.
    pub fn to_message(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();

        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> Txn {

        let to_string = String::from_utf8_lossy(data).into_owned();
        serde_json::from_str::<Txn>(&to_string).unwrap()
    }
}

impl Verifiable for Txn {
    fn is_valid(
        &self, 
        options: Option<ValidatorOptions>,
    ) -> Option<bool> 
    {
        let message = self.txn_payload.clone();
        let signature = Signature::from_str(&self.txn_signature).unwrap();
        let pk = PublicKey::from_str(&self.sender_public_key).unwrap();

        match WalletAccount::verify(message, signature, pk) {
            Ok(true) => {},
            Ok(false) => { 
                println!("Invalid signature");
                return Some(false) 
            }
            Err(e) => { 
                println!("Signature verification resulted in an error: {}", e);
                return Some(false) 
            }
        }

        match options {
            Some(ValidatorOptions::Transaction(account_state)) => {
                let account_state = serde_json::from_str::<AccountState>(&account_state).unwrap();
                let balance = account_state.pending_balances.get(&self.sender_address).unwrap().get("VRRB");

                match balance {
                    Some(bal) => {
                        if bal < &self.txn_amount {
                            println!("Invalid balance, not enough coins!");
                            return Some(false)
                        }
                    },
                    None => {
                        println!("couldn't find balance in account_state");
                        return Some(false)
                    }
                }

                let receiver = account_state.accounts_pk.get(&self.receiver_address); 
                
                if receiver == None {
                    println!("couldn't find receiver");
                    return Some(false)
                }

                let check_double_spend = account_state.pending.get(&self.txn_id);

                match check_double_spend {
                    Some(txn) => {
                        if txn.txn_id == self.txn_id && (txn.txn_amount != self.txn_amount || 
                            txn.receiver_address != self.receiver_address) {
                            
                            println!("Attempted double spend");
                            return Some(false)
                        }
                    },
                    None => {
                        println!("Transaction not set in account state pending yet.")
                    }
                }
                
            },
            None => panic!("Message structure is invalid"),
            _ => panic!("Message Option is invalid")
        }
        Some(true)
    }
}

impl fmt::Display for Txn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        write!(
            f,
            "Txn(\n \
            txn_id: {},\n \
            txn_timestamp: {},\n \
            sender_address: {},\n \
            sender_public_key: {},\n \
            receiver_address: {},\n \
            txn_token: {:?},\n \
            txn_amount: {},\n \
            txn_signature: {}",
            self.txn_id, 
            self.txn_timestamp.to_string(), 
            self.sender_address, 
            self.sender_public_key,
            self.receiver_address,
            self.txn_token,
            self.txn_amount,
            self.txn_signature,
        )
    }
}
