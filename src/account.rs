use bytebuffer::ByteBuffer;
use secp256k1::rand::rngs::OsRng;
use secp256k1::Error;
use secp256k1::{
    key::{PublicKey, SecretKey},
    Signature,
};
use secp256k1::{Message, Secp256k1};
use sha256::digest_bytes;
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use crate::claim::Claim;

const STARTING_BALANCE: u128 = 1_000_000_000_000_000_000_000;

// TODO: Move to a different module
// Account State object is effectively the local
// database of all WalletAccounts. This is updated
// whenever transactions are sent/received
// and is "approved" by the network via consensus after
// each transaction and each block. It requires a hashmap
// with a vector of hashmaps that contains information for restoring a wallet.
// It re
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Token {
    Name(String),
    Units(i32),
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct WalletAccountState {
    accounts: HashMap<String, String>,
    coin_balances: HashMap<String, u128>,
    token_balances: HashMap<String, Vec<(Token, Token)>>,
    claims_owned: HashMap<String, Claim>,
}

#[derive(Debug, Clone)]
pub struct WalletAccount {
    private_key: SecretKey,
    // pub pk_hash: String,
    // pub mnemonic_hash: String,
    pub address: String,
    pub public_key: PublicKey,
    pub balance: u128,
    pub tokens: Vec<(Option<Token>, Option<Token>)>,
    pub claims: Vec<Option<Claim>>,
}

impl WalletAccount {
    pub fn new() -> Self {
        let secp = Secp256k1::new();
        let mut rng = OsRng::new().expect("OsRng");
        let (secret_key, public_key) = secp.generate_keypair(&mut rng);
        let uid_address = digest_bytes(Uuid::new_v4().to_string().as_bytes());
        let mut address_prefix: String = "0x192".to_string();
        address_prefix.push_str(&uid_address);
        Self {
            private_key: secret_key,
            public_key: public_key,
            address: address_prefix,
            balance: STARTING_BALANCE,
            tokens: vec![(None, None)],
            claims: vec![None],
        }
    }

    pub fn sign(&self, message: String) -> Result<Signature, Error> {
        let message_bytes = message.as_bytes().to_owned();
        let mut buffer = ByteBuffer::new();
        buffer.write_bytes(&message_bytes);
        while buffer.len() < 32 {
            buffer.write_u8(0);
        }
        let new_message = buffer.to_bytes();
        let message_hash = blake3::hash(&new_message);
        let message_hash = Message::from_slice(message_hash.as_bytes())?;
        let secp = Secp256k1::new();
        let sig = secp.sign(&message_hash, &self.private_key);
        Ok(sig)
    }

    pub fn verify(message: Message, signature: Signature, pk: PublicKey) -> Result<bool, Error> {
        let secp = Secp256k1::new();
        let valid = secp.verify(&message, &signature, &pk);
        match valid {
            Ok(()) => Ok(true),
            _ => Err(Error::IncorrectSignature),
        }
    }
}

impl fmt::Display for WalletAccount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let balance: String = self.balance.to_string();

        write!(
            f,
            "Wallet(\n \
            address: {:?},\n \
            balance: {},\n \
            tokens: {:?},\n \
            claims: {:?}",
            self.address, balance, self.tokens, self.claims
        )
    }
}
