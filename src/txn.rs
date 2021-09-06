use crate::validator::ValidatorOptions;
use crate::verifiable::Verifiable;
use crate::wallet::WalletAccount;
use secp256k1::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use std::collections::HashMap;

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
    pub validators: HashMap<String, bool>,
}

impl Txn {
    pub fn new(
        sender: Arc<Mutex<WalletAccount>>,
        sender_address: String,
        receiver: String,
        amount: u128,
    ) -> Txn {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let payload = format!(
            "{},{},{},{},{}",
            &time.as_nanos().to_string(),
            &sender_address,
            &sender.lock().unwrap().pubkey.clone(),
            &receiver,
            &amount.to_string()
        );
        let signature = sender.lock().unwrap().sign(&payload).unwrap();
        let uid_payload = format!(
            "{},{},{}",
            &payload,
            Uuid::new_v4().to_string(),
            &signature.to_string()
        );

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
            validators: HashMap::new(),
        }
    }

    // TODO: convert to_message into a function of the verifiable trait,
    // all verifiable objects need to be able to be converted to a message.
    pub fn to_string(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> Txn {
        serde_json::from_slice::<Txn>(data).unwrap()
    }

    pub fn from_string(string: &String) -> Txn {
        serde_json::from_str::<Txn>(string).unwrap()
    }

}

impl Verifiable for Txn {
    fn is_valid(&self, options: Option<ValidatorOptions>) -> Option<bool> {
        let message = self.txn_payload.clone();
        let signature = Signature::from_str(&self.txn_signature).unwrap();
        let pk = PublicKey::from_str(&self.sender_public_key).unwrap();

        if let Ok(false) | Err(_) = WalletAccount::verify(message, signature, pk) {
            println!("Invalid signature");
            return Some(false);
        }

        
        if let Some(ValidatorOptions::Transaction(account_state, network_state)) = options {
            let (_, pending_debits) = if let Some((credit_amount, debit_amount)) =
                account_state.pending_balance(self.sender_address.clone())
            {
                (credit_amount, debit_amount)
            } else {
                (0, 0)
            };

            let mut address_balance = network_state.get_balance(&self.sender_address);

            address_balance = if let Some(amount) = address_balance.checked_sub(pending_debits) {
                amount
            } else {
                panic!("Pending debits cannot exceed confirmed balance!")
            };
            
            if address_balance < self.txn_amount {
                println!("Invalid balance, not enough coins!");
                return Some(false);
            }

            if let Some(txn) = account_state.txn_pool.pending.get(&self.txn_id) {
                if txn.txn_id == self.txn_id && (txn.txn_amount != self.txn_amount || txn.receiver_address != self.receiver_address) {
                    println!("Attempted double spend");
                    return Some(false);
                }
            };
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
