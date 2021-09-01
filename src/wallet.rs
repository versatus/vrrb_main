use crate::account::AccountState;
use crate::block::Block;
use crate::claim::Claim;
use crate::state::NetworkState;
use crate::txn::Txn;
use bytebuffer::ByteBuffer;
use secp256k1::Error;
use secp256k1::{
    key::{PublicKey, SecretKey},
    Signature,
};
use secp256k1::{Message, Secp256k1};
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;
use ritelinked::LinkedHashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const STARTING_BALANCE: u128 = 1000;

/// The WalletAccount struct is the user/node wallet in which coins, tokens and contracts
/// are held. The WalletAccount has a private/public keypair
/// phrase are used to restore the Wallet. The private key is
/// also used to sign transactions, claims and mined blocks for network validation.
/// Private key signatures can be verified with the wallet's public key, the message that was
/// signed and the signature.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletAccount {
    secretkey: String,
    pub pubkey: String,
    pub addresses: LinkedHashMap<u32, String>,
    pub total_balances: LinkedHashMap<String, LinkedHashMap<String, u128>>,
    pub available_balances: LinkedHashMap<String, LinkedHashMap<String, u128>>,
    pub claims: LinkedHashMap<u128, Claim>,
}

impl WalletAccount {
    /// Initiate a new wallet.
    pub fn new() -> WalletAccount {
        // Initialize a new Secp256k1 context
        let secp = Secp256k1::new();

        // Generate a random number used to seed the new keypair for the wallet
        // TODO: Instead of using the rng, use a mnemonic seed.
        let mut rng = rand::thread_rng();
        // Generate a new secret/public key pair using the random seed.
        let (secret_key, public_key) = secp.generate_keypair(&mut rng);
        // Generate 100 addresses by hashing a universally unique IDs + secret_key + public_key
        let uid_address = digest_bytes(Uuid::new_v4().to_string().as_bytes());
        // add the testnet prefix to the wallet address (TODO: add handling of testnet/mainnet)
        let mut address_prefix: String = "0x192".to_string();
        // push the hashed uuid string to the end of the address prefix
        address_prefix.push_str(&uid_address);

        // Print the private key string so that the user can save it.
        // TODO: require a confirmation the private key being saved by the user
        println!("DO NOT SHARE OR LOSE YOUR PRIVATE KEY:");
        println!("{:?}\n", &secret_key.to_string());
        let mut addresses = LinkedHashMap::new();
        addresses.insert(1, address_prefix.clone());

        let mut total_balances = LinkedHashMap::new();
        let mut vrrb_balances = LinkedHashMap::new();
        vrrb_balances.insert("VRRB".to_string(), STARTING_BALANCE);
        total_balances.insert(address_prefix.clone(), vrrb_balances);

        // Generate a wallet struct by assigning the variables to the fields.
        let wallet = Self {
            secretkey: secret_key.to_string(),
            pubkey: public_key.to_string(),
            addresses,
            total_balances: total_balances.clone(),
            available_balances: total_balances,
            claims: LinkedHashMap::new(),
        };

        wallet
    }
    // Return the wallet and account state
    // TODO: Return a Result for error propagation and handling.

    // method for restoring a wallet from the private key
    // pub fn restore_from_private_key(
    //     private_key: String,
    //     account_state: AccountState,
    // ) -> WalletAccount {
    // }

    /// Sign a message (transaction, claim, block, etc.)
    pub fn sign(&self, message: &str) -> Result<Signature, Error> {
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
        let sk = SecretKey::from_str(&self.secretkey).unwrap();
        let sig = secp.sign(&message_hash, &sk);
        Ok(sig)
    }

    /// Verify a signature with the signers public key, the message payload and the signature.
    pub fn verify(message: String, signature: Signature, pk: PublicKey) -> Result<bool, Error> {
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
        let valid = secp.verify(&message_hash, &signature, &pk);

        match valid {
            Ok(()) => Ok(true),
            _ => Err(Error::IncorrectSignature),
        }
    }

    /// get the current available and total balance of the current WalletAccount
    /// using the .get() method on the account state .total_coin_balances HashMap and
    /// the .available_coin_balances HashMap. The key for both is the WalletAccount's public key.
    /// TODO: Add signature verification to this method to ensure that the wallet requesting the
    /// balance update is the correct wallet.
    pub fn get_balances(&mut self, network_state: NetworkState, account_state: AccountState) {
        self.addresses
            .clone()
            .iter()
            .map(|x| x)
            .for_each(|(_i, x)| {
                let address_balance = if let Some(amount) = network_state.retrieve_balance(x.clone()) {
                    amount
                } else {
                    0u128
                };

                let mut vrrb_balance_map = LinkedHashMap::new();
                vrrb_balance_map.insert("VRRB".to_string(), address_balance);
                self.total_balances.insert(x.clone(), vrrb_balance_map);

                let (address_pending_credits, address_pending_debits) =
                    if let Some((credits_amount, debits_amount)) = account_state.pending_balance(x.clone())
                    {
                        (credits_amount, debits_amount)
                    } else {
                        (0, 0)
                    };

                let mut pending_balance =
                    if let Some(amount) = address_balance.checked_sub(address_pending_debits) {
                        amount
                    } else {
                        panic!("Pending debits cannot exceed confirmed balance");
                    };

                pending_balance += address_pending_credits;
                let mut pending_vrrb_balance_map = LinkedHashMap::new();
                pending_vrrb_balance_map.insert("VRRB".to_string(), pending_balance);

                self.available_balances
                    .insert(x.clone(), pending_vrrb_balance_map);
            });
    }

    pub fn remove_mined_claims(&mut self, block: &Block) {
        self.claims.remove(&block.claim.claim_number);
    }

    pub fn send_txn(
        self,
        address_number: u32,
        receiver: String,
        amount: u128,
    ) -> Result<Txn, Error> {
        let txn = Txn::new(
            Arc::new(Mutex::new(self.clone())),
            self.addresses.get(&address_number).unwrap().clone(),
            receiver,
            amount,
        );
        Ok(txn)
    }

    pub fn sell_claim(&mut self, claim_number: u128, price: u128) -> Option<Claim> {
        let claim_to_sell = self.claims.get_mut(&claim_number);

        match claim_to_sell {
            Some(mut claim) => {
                claim.available = true;
                claim.price = price as u32; // FIX CLAIM PRICE to be u128
                Some(claim.to_owned())
            }
            None => None,
        }
    }

    pub fn generate_new_address(&mut self) {
        let uid = Uuid::new_v4().to_string();
        let address_number: u32 = self.addresses.len() as u32 + 1u32;
        let payload = format!("{},{},{}", &address_number, &uid, &self.pubkey);
        let address = digest_bytes(payload.as_bytes());
        self.addresses.insert(address_number, address);
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> WalletAccount {
        let mut buffer: Vec<u8> = vec![];
        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();
        serde_json::from_str::<WalletAccount>(&to_string).unwrap()
    }
}

impl fmt::Display for WalletAccount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Wallet(\n \
            address: {:?},\n \
            balances: {:?},\n \
            available_balance: {:?},\n \
            claims_owned: {}",
            self.addresses,
            self.total_balances,
            self.available_balances,
            self.claims.len()
        )
    }
}

impl Clone for WalletAccount {
    fn clone(&self) -> WalletAccount {
        WalletAccount {
            secretkey: self.secretkey.clone(),
            pubkey: self.pubkey.clone(),
            addresses: self.addresses.clone(),
            total_balances: self.total_balances.clone(),
            available_balances: self.available_balances.clone(),
            claims: self.claims.clone(),
        }
    }
}
