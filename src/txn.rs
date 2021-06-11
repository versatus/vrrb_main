use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt;
use secp256k1::{PublicKey, Signature};
use serde::{Serialize, Deserialize};
use crate::validator::ValidatorOptions;
use crate::verifiable::Verifiable;
use crate::{account::WalletAccount, vrrbcoin::Token};
use uuid::Uuid;
use sha256::digest_bytes;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Txn {
    pub txn_id: String,
    pub txn_timestamp: u128,
    pub sender_address: String,
    pub sender_public_key: String,
    pub receiver_address: String,
    pub txn_token: Option<Token>,
    pub txn_amount: u128,
    pub txn_signature: String,
}

impl Txn {

    pub fn new(
        wallet: WalletAccount, 
        receiver: String, 
        amount: u128
    ) -> Txn {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    
        let payload = format!("{},{},{},{}", 
            &wallet.address.to_string(), 
            &wallet.public_key.to_string(), 
            &receiver, &amount.to_string()
        );
    
        let signature = wallet.sign(payload).unwrap();
    
        Txn {
            txn_id: digest_bytes(Uuid::new_v4().to_string().as_bytes()),
            txn_timestamp: time.as_nanos(),
            sender_address: wallet.address,
            sender_public_key: wallet.public_key.to_string(),
            receiver_address: receiver,
            txn_token: None,
            txn_amount: amount,
            txn_signature: signature.to_string(),
        }
    }

    pub fn to_message(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl Verifiable for Txn {
    fn is_valid(
        &self, 
        _options: Option<ValidatorOptions>,
    ) -> Option<bool> 
    {
        let message = self.to_message();
        let signature = Signature::from_str(&self.txn_signature).unwrap();
        let pk = PublicKey::from_str(&self.sender_public_key).unwrap();

        if WalletAccount::verify(message, signature, pk).unwrap() == false {
            return Some(false);
        }

        match _options {
            
            Some(ValidatorOptions::Transaction(account_state)) => {
                let balance = account_state.available_coin_balances.get(&self.sender_public_key);
                match balance {
                    Some(bal) => {
                        if bal < &self.txn_amount {
                            return Some(false)
                        }
                    },
                    None => {
                        return Some(false)
                    }
                }

                let receiver = account_state.accounts_address.get(&self.receiver_address); 
                
                if receiver == None {
                    return Some(false)
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
