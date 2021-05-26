use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use crate::{account::{Token, WalletAccount}};
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
    pub txn_amount: i128,
    pub txn_signature: String,
}

impl Txn {

    pub fn new(wallet: WalletAccount, receiver: String, amount: i128) -> Txn {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let mut payload = String::new();
        payload.push_str(&wallet.address.to_string());
        payload.push(',');
        payload.push_str(&wallet.public_key.to_string());
        payload.push(',');
        payload.push_str(&receiver);
        payload.push(',');
        payload.push_str(&amount.to_string());
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
}