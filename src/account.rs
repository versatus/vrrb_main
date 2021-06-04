use bytebuffer::ByteBuffer;
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
use bip39::{Mnemonic, Language};
use crate::{claim::{Claim, ClaimState}, vrrbcoin::Token, txn::Txn, block::Block, state::NetworkState};

const STARTING_BALANCE: u128 = 1_000;

// TODO: Move to a different module
// Account State object is effectively the local
// database of all WalletAccounts. This is updated
// whenever transactions are sent/received
// and is "approved" by the network via consensus after
// each transaction and each block. It requires a hashmap
// with a vector of hashmaps that contains information for restoring a wallet.
pub enum StateOption {
    // TODO: Change WalletAccount usage to tuples of types of 
    // data from the Wallet needed. Using actual WalletAccount object
    // is unsafe.
    NewTxn(Txn),
    NewAccount(WalletAccount),
    ClaimAcquired(Claim),
    ConfirmedTxn((Txn, Vec<WalletAccount>)),
    Miner((WalletAccount, Block)),
}

/// The State of all accounts. This is used to track balances
/// this is also used to track the state of the network in general
/// along with the ClaimState and RewardState. Will need to adjust
/// this to account for smart contracts at some point in the future.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct AccountState {
    /// Map of account (secret key, mnemoic) hashes to public keys
    /// This is used to allow users to restore their account.
    pub accounts_sk: HashMap<String, String>,
    pub accounts_mk: HashMap<String, String>,

    /// Map of public keys to total balances. This is updated as 
    /// transactions occur and then are confirmed. 
    /// Users may only transfer balances less than or
    /// equal to their available balance.
    pub total_coin_balances: HashMap<String, u128>,

    /// Map of public keys to avaialbe balances. This is updated
    /// as transactions occur and then are confirmed.
    pub available_coin_balances: HashMap<String, u128>,

    /// Map of address to public keys to be able to access public keys from address
    /// as transactions are mostly conducted via an address, not via the public
    /// key.
    pub accounts_address: HashMap<String, String>,

    /// Map of public key to vector of (Token(Ticker), Token(Units)) tuples
    /// This is effectively a placeholder for non-native tokens, and will be
    /// adjusted according to Smart Contract token protocols in the future,
    /// This is currently primarly for test purposes.
    pub token_balances: HashMap<String, Vec<(Token, Token)>>,

    /// The local claim state.
    pub claim_state: ClaimState,

    /// A vector of pending txns that have not been validated
    /// consider changing this to a vec of (txn_id, txn_hash, signature) thruples
    /// may speed up txn validation/processing time and save memory.
    pub pending: Vec<Txn>,

    pub mineable: Vec<Txn>,

    // TODO: Add a state hash, which will sha256 hash the entire state structure for
    // consensus purposes.
}

#[derive(Debug)]
pub struct WalletAccount {
    private_key: SecretKey,
    pub address: String,
    pub public_key: PublicKey,
    pub balance: u128,
    pub available_balance: u128,
    pub tokens: Vec<(Option<Token>, Option<Token>)>,
    pub claims: Vec<Option<Claim>>,
    skhash: String,
    mnemonic_hash: String,

    // TODO: Add a secret key hash and mnemonic hash to the struct to be able to identify rightful
    // owners of a given account when a user is attempting to restore their account.
}

impl AccountState {
    pub fn start() -> AccountState {

        AccountState {
            accounts_sk: HashMap::new(),
            accounts_mk: HashMap::new(),
            accounts_address: HashMap::new(),
            total_coin_balances: HashMap::new(),
            available_coin_balances: HashMap::new(),
            token_balances: HashMap::new(),
            claim_state: ClaimState::start(),
            pending: vec![],
            mineable: vec![],
        }
    }

    pub fn update(&mut self, value: StateOption, network_state: &mut NetworkState) -> Result<Self, Error> {
        
        // Read the purpose and match the purpose to different update types
        // If the purpose is "new_txn", then StateOption should contain NewTxn(Txn)
        // Unwrap the Txn, and update the available balance of the account
        // the txn was sent from, the total balance of the account the txn was
        // sent to, and place it in the pending vector. If the purpose is
        // "confirmed_txn", update the total balance of the sender, the
        // available balance of the receiver and remove it from the pending
        // vector and place it in the mineable vector.
        // if the purpose is "new_account", update all relevant fields
        // if purpose is "claim_acquired" update the claim state (and balance if
        // the claim was purchased and not homesteaded. If the purpose is "miner",
        // update the coin balance of the miner account with the block reward.
       match value {
            StateOption::NewAccount(wallet) => {
                self.accounts_sk.entry(wallet.skhash)
                    .or_insert(wallet.public_key
                        .to_string());
                self.accounts_mk.entry(wallet.mnemonic_hash)
                    .or_insert(wallet.public_key
                        .to_string());
                self.accounts_address.entry(wallet.address)
                    .or_insert(wallet.public_key
                        .to_string());
                self.total_coin_balances.entry(wallet.public_key.to_string())
                    .or_insert(STARTING_BALANCE);
                self.available_coin_balances.entry(wallet.public_key.to_string())
                    .or_insert(STARTING_BALANCE);
                network_state.update(self.clone(), "account_state");
                return Ok(self.to_owned());
            },
            StateOption::NewTxn(txn) => {
                let receiver_pk = self.accounts_address.get(&txn.receiver_address).unwrap();
                let sender_pk = self.accounts_address.get(&txn.sender_address).unwrap();

                let sender_avail_bal = *self.available_coin_balances
                                                .get_mut(sender_pk)
                                                .unwrap() - txn.txn_amount;
                if sender_avail_bal < txn.txn_amount {
                    return Err(Error::InvalidMessage);
                }
                let receiver_total_bal = *self.total_coin_balances
                                                .get_mut(receiver_pk)
                                                .unwrap() + txn.txn_amount;
                self.available_coin_balances.insert(sender_pk.to_owned(), sender_avail_bal);
                self.total_coin_balances.insert(receiver_pk.to_owned(), receiver_total_bal);
                self.pending.push(txn);
                network_state.update(self.clone(), "account_state");
                return Ok(self.to_owned());
            },
            StateOption::ClaimAcquired(claim) => {
                self.claim_state.owned_claims.entry(claim.maturation_time).or_insert(claim);
                network_state.update(self.clone(), "account_state");
                return Ok(self.to_owned());
            },
            StateOption::Miner((miner, block)) => {
                let reward = block.block_reward.amount;
                let miner_pk = self.accounts_address.get(&miner.address).unwrap();
                self.total_coin_balances.insert(
                    miner_pk.to_owned(), 
                    self.total_coin_balances[miner_pk] + reward);
                
                self.available_coin_balances.insert(
                    miner_pk.to_owned(), 
                    self.available_coin_balances[miner_pk] + reward);

                for claim in block.clone().visible_blocks {
                    self.claim_state.claims.entry(claim.maturation_time).or_insert(claim);
                }

                self.claim_state.claims.remove_entry(&block.claim.maturation_time).unwrap();
                match self.claim_state.owned_claims.remove_entry(&block.clone().claim.maturation_time) {
                    Some(_) => println!("Removed claim from owned"),
                    None => println!("Couldn't find claim in owned"),
                }
                network_state.update(self.clone(), "account_state");
                
                return Ok(self.to_owned());

            },
            StateOption::ConfirmedTxn((_txn, _validators)) => {
                //TODO: distribute txn fees among validators.
                return Ok(self.to_owned());
            },
        }
    }

    // pub fn stream(&self, _file: Option<File>) {
    //     // TODO stream the account state to a file for maintenance and for restoration.
    // }
        
}

impl WalletAccount {
    pub fn new() -> Self {
        let secp = Secp256k1::new();
        let mut rng = rand::thread_rng();
        let mut mrng = rand::thread_rng();
        let (secret_key, public_key) = secp.generate_keypair(&mut rng);
        let uid_address = digest_bytes(Uuid::new_v4().to_string().as_bytes());
        let mut address_prefix: String = "0x192".to_string();
        let mnemonic = Mnemonic::generate_in_with(&mut mrng, Language::English, 24)
                            .unwrap()
                            .to_string();
        address_prefix.push_str(&uid_address);
        Self {
            private_key: secret_key,
            public_key: public_key,
            address: address_prefix,
            balance: STARTING_BALANCE,
            available_balance: STARTING_BALANCE,
            tokens: vec![],
            claims: vec![],
            skhash: digest_bytes(secret_key.to_string().as_bytes()),
            mnemonic_hash: digest_bytes(mnemonic.as_bytes()),
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

    pub fn get_balance(&mut self, account_state: AccountState) -> Result<Self, Error> {
        let (balance, available_balance) = (
            account_state.total_coin_balances.get(&self.public_key.to_string()),
            account_state.available_coin_balances.get(&self.public_key.to_string()));
        Ok(Self {
            balance: balance.unwrap().clone(),
            available_balance: available_balance.unwrap().clone(),
            ..self.to_owned()
        })
    }

    pub fn remove_mined_claims(&mut self, block: &Block) -> Self {
        self.claims.iter()
            .position(|x| x.clone().unwrap() == block.clone().claim)
            .map(|e| self.claims.remove(e));
        
        self.clone()

    }

    pub fn send_txn(
        &mut self, 
        account_state: &mut AccountState, 
        receivers: (String, u128),
        network_state: &mut NetworkState
    ) -> Result<(Self, AccountState), Error> {
        let txn = Txn::new(self.clone(), receivers.0, receivers.1);
        let updated_account_state = account_state.update(StateOption::NewTxn(txn), network_state).unwrap();
        Ok((self.to_owned(), updated_account_state.to_owned()))
    }
}

impl fmt::Display for WalletAccount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let balance: String = self.balance.to_string();
        let available_balance: String = self.available_balance.to_string();
        write!(
            f,
            "Wallet(\n \
            address: {:?},\n \
            balance: {},\n \
            available_balance: {},\n \
            tokens: {:?},\n \
            claims: {}",
            self.address, balance, available_balance, self.tokens, self.claims.len()
        )
    }
}

impl Clone for WalletAccount {
    fn clone(&self) -> WalletAccount {
        WalletAccount {
            private_key: self.private_key,
            address: self.address.clone(),
            public_key: self.public_key,
            balance: self.balance,
            available_balance: self.available_balance,
            tokens: self.tokens.clone(),
            claims: self.claims.clone(),
            skhash: self.skhash.clone(),
            mnemonic_hash: self.mnemonic_hash.clone(),
        }
    }
}

impl Clone for AccountState {
    fn clone(&self) -> Self {
        AccountState {
            accounts_sk: self.accounts_sk.clone(),
            accounts_mk: self.accounts_mk.clone(),
            total_coin_balances: self.total_coin_balances.clone(),
            available_coin_balances: self.available_coin_balances.clone(),
            accounts_address: self.accounts_address.clone(),
            token_balances: self.token_balances.clone(),
            claim_state: self.claim_state.clone(),
            pending: self.pending.clone(),
            mineable: self.mineable.clone(),
        }
    }
}

// TODO: Write tests for this module