use crate::validator::{Validator, ValidatorOptions};
use bytebuffer::ByteBuffer;
use secp256k1::Error;
use secp256k1::{
    key::{PublicKey, SecretKey},
    Signature,
};
use secp256k1::{Message, Secp256k1};
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;
// use crate::validator::Validator;
use crate::{block::Block, claim::Claim, state::NetworkState, txn::Txn, verifiable::Verifiable};
use std::sync::{Arc, Mutex};

const STARTING_BALANCE: u128 = 1_000;

// TODO: Move to a different module
// Account State object is effectively the local
// database of all WalletAccounts. This is updated
// whenever transactions are sent/received
// and is "approved" by the network via consensus after
// each transaction and each block. It requires a hashmap
// with a vector of hashmaps that contains information for restoring a wallet.
#[derive(Serialize, Deserialize)]
pub enum StateOption {
    // TODO: Change WalletAccount usage to tuples of types of
    // data from the Wallet needed. Using actual WalletAccount object
    // is unsafe.
    NewTxn(String),
    NewAccount(String),
    PendingClaimAcquired(String),
    ConfirmedClaimAcquired(String),
    ConfirmedTxn(String, String),
    ProposedBlock(String, String, String, String),
    ConfirmedBlock(String, String, String, String),
}

/// The State of all accounts. This is used to track balances
/// this is also used to track the state of the network in general
/// along with the ClaimState and RewardState. Will need to adjust
/// this to account for smart contracts at some point in the future.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct AccountState {
    // Map of account address to public key
    pub accounts_pk: HashMap<String, String>,   // K: address, V: pubkey
    pub credits: HashMap<String, HashMap<String, u128>>,    // K: address, V: Hashmap { K: ticker, V: amount }
    pub pending_credits: HashMap<String, HashMap<String, u128>>,    // K: address, V: Hashmap { K: ticker, V: amount }
    pub debits: HashMap<String, HashMap<String, u128>>, // K: address, V: Hashmap { K: ticker, V: amount }
    pub pending_debits: HashMap<String, HashMap<String, u128>>, // K: address, V: Hashmap { K: ticker, V: amount }
    pub balances: HashMap<String, HashMap<String, u128>>, // K: address, V: Hashmap { K: ticker, V: amount }
    pub pending_balances: HashMap<String, HashMap<String, u128>>, // K: address, V: Hashmap { K: ticker, V: amount }
    pub claims: HashMap<u128, Claim>, // K: claim_number, V: claim
    pub pending_owned_claims: HashMap<u128, String>, // K: claim_number, V: pubkey
    pub owned_claims: HashMap<u128, String>, // K: claim_number, V: pubkey
    pub staked_claims: HashMap<u128, String>, // K: claim_number, V: pubkey
    pub pending: HashMap<String, Txn>, // K: txn_id, V: Txn 
    pub mineable: HashMap<String, Txn>, // K: txn_id, V: Txn
    pub last_block: Option<Block>,
}

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
    pub addresses: HashMap<u32, String>,
    pub total_balances: HashMap<String, HashMap<String, u128>>,
    pub available_balances: HashMap<String, HashMap<String, u128>>,
    pub claims: Vec<Option<Claim>>,
}

/// The state of all accounts in the network. This is one of the 3 core state objects
/// which ensures that the network maintains consensus amongst nodes. The account state
/// records accounts, along with their native token (VRRB) balances and smart contract
/// token balances. Also contains all pending and confirmed transactions. Pending
/// transactions are set into the pending vector and the confirmed transactions
/// are set in the mineable vector.
impl AccountState {
    /// Instantiates a new AccountState instance
    /// TODO: Add restoration functionality/optionality to restore an existing
    /// account state on a node that has previously operated but was stopped.
    pub fn start() -> AccountState {
        AccountState {
            accounts_pk: HashMap::new(),
            credits: HashMap::new(),
            pending_credits: HashMap::new(),
            debits: HashMap::new(),
            pending_debits: HashMap::new(),
            balances: HashMap::new(),
            pending_balances: HashMap::new(),
            claims: HashMap::new(),
            pending_owned_claims: HashMap::new(),
            owned_claims: HashMap::new(),
            staked_claims: HashMap::new(),
            pending: HashMap::new(),
            mineable: HashMap::new(),
            last_block: None,
        }
    }

    /// Update's the AccountState and NetworkState, takes a StateOption (for function routing)
    /// also requires the NetworkState to be provided in the function call.
    /// TODO: Provide Examples to Doc
    pub fn update(&mut self, value: StateOption) {
        match value {
            // If the StateOption variant passed to the update method is a NewAccount
            // set the new account information into the account state, return the account state
            // and update the network state.
            StateOption::NewAccount(wallet) => {
                // Enter the wallet's secret key hash as the key and the wallet's public key as the value
                // if the secret key hash is not already in the hashmap
                let wallet = serde_json::from_str::<WalletAccount>(&wallet).unwrap();
                // Enter the wallet's public key string as the key and if it's not already in the HashMap
                // the wallet's address as the value.
                for address in wallet.addresses.values() {
                    self.accounts_pk.entry(address.to_string()).or_insert(wallet.pubkey.clone());
                }

                // Enter the wallet's public key string as the key and the STARTING_BALANCE const as
                // the value
                // TODO: 0 should be the starting value of ALL accounts on the live network
                let mut vrrb_starting_credit = HashMap::new();
                vrrb_starting_credit.insert("VRRB".to_string(), STARTING_BALANCE);
                let mut vrrb_starting_debit = HashMap::new();
                vrrb_starting_debit.insert("VRRB".to_string(), 0u128);

                self.credits.entry(wallet.pubkey.clone()).or_insert(vrrb_starting_credit);
                // Same thing as above since this is a new account
                self.debits.entry(wallet.pubkey.clone()).or_insert(vrrb_starting_debit);
                // The .update() method for the  network state sets a state object (struct)
                // either account_state, claim_state or reward state) into the pickle db
                // that represents the network state.
            }
            // If the StateOption variant passed is a NewTxn process the txn and either return
            // an error if there's obvious validity issues or set it to pending txns to be
            // fully validated by validators.
            StateOption::NewTxn(txn) => {
                // get the receiver's public key from the AccountState accounts_address field
                // which is a hashmap containing the address as the key and the public key as the value
                let txn = serde_json::from_str::<Txn>(&txn).unwrap();
                let receiver = self.accounts_pk.get(&txn.receiver_address);
                match receiver {
                    Some(receiver_pk) => {
                        let receiver_pk = receiver_pk;

                        // get the sender's public key from the AccountState accounts_address field
                        // which is a hashmap containing the address as the key and the public key as the value

                        // get the sender's coin balance as mutable, from the availabe_coin_balances field
                        // in the account_state object, which takes the public key as the key and the
                        // available balance as the value,

                        // TODO: Replace hard coded VRRB Ticker with txn.ticker
                        let sender = self.accounts_pk.get(&txn.sender_address);

                        match sender {
                            Some(sender_pk) => {
                                let sender_pk = sender_pk;
                                let sender_avail_bal = *self.pending_balances.get_mut(sender_pk)
                                                            .unwrap()
                                                            .get_mut("VRRB")
                                                            .unwrap();

                                let balance_check = sender_avail_bal.checked_sub(txn.txn_amount);

                                match balance_check {
                                    Some(_bal) => {
                                        // Add the amount to the receiver pending credits by getting
                                        *self.pending_credits.get_mut(receiver_pk)
                                            .unwrap()
                                            .get_mut("VRRB")
                                            .unwrap() += txn.txn_amount;

                                        // Update the pending debits of the sender
                                        *self.pending_debits.get_mut(sender_pk)
                                            .unwrap()
                                            .get_mut("VRRB")
                                            .unwrap() += txn.txn_amount;
                                                                                
                                        *self.pending_balances.get_mut(receiver_pk)
                                            .unwrap()
                                            .get_mut("VRRB")
                                            .unwrap() = self.pending_credits.get_mut(receiver_pk)
                                                            .unwrap()
                                                            .get_mut("VRRB")
                                                            .unwrap()
                                                            .checked_sub(
                                                                *self.pending_debits.get_mut(receiver_pk)
                                                                    .unwrap()
                                                                    .get_mut("VRRB")
                                                                    .unwrap()
                                                                ).unwrap();

                                        *self.pending_balances.get_mut(sender_pk)
                                            .unwrap()
                                            .get_mut("VRRB")
                                            .unwrap() = self.pending_credits.get_mut(sender_pk)
                                                            .unwrap()
                                                            .get_mut("VRRB")
                                                            .unwrap()
                                                            .checked_sub(
                                                                *self.pending_debits.get_mut(sender_pk)
                                                                    .unwrap()
                                                                    .get_mut("VRRB")
                                                                    .unwrap()
                                                                ).unwrap();

                                        // Push the transaction to pending transactions to be confirmed 
                                        // by validators.
                                        self.pending.entry(txn.clone().txn_id).or_insert(txn.clone());
                                        // Pending transactions do not update the network state, only confirmed
                                        // transactions update the network state. 
                                    }
                                    None => println!("Amount Exceeds Balance"),
                                }
                            }
                            None => println!("Sender is non-existent"),
                        }
                    }
                    None => println!("The receiver is non-existent"),
                }
            },

            // If the StateOption variant received by the update method is a ClaimAcquired
            // Update the account state by entering the relevant information into the
            // proper fields, return the updated account state and update the network state.
            StateOption::PendingClaimAcquired(claim) => {
                // Set a new entry (if it doesn't exist) into the AccountState
                // claim_state field's (which is a ClaimState Struct) owned_claims field
                // which is a HashMap consisting of the claim maturation time as the key and the claim
                // struct itself as the value.
                // TODO: break down PendingClaimAcquired and ConfirmedClaimAcquired as claim acquisition
                // has to be validated before it can be set into the account_state's claim_state.
                let claim = serde_json::from_str::<Claim>(&claim).unwrap();
                self.pending_owned_claims.insert(claim.claim_number, claim.current_owner.0.unwrap());
            },
            StateOption::ConfirmedClaimAcquired(_claim) => {

            },

            // If the StateOption variant received by the update method is Miner
            // this means a new block has been mined, udpate the account state accordingly
            // TODO: mined blocks need to be validated by the network before they're confirmed
            // If it has not yet been confirmed there should be a PendingMiner variant as well
            // as a ConfirmedMiner variant. The logic in this block would be for a ConfirmedMiner
            StateOption::ProposedBlock(miner, block, reward_state, network_state) => {
                let block = serde_json::from_str::<Block>(&block).unwrap();
                let network_state = Arc::new(Mutex::new(serde_json::from_str::<NetworkState>(&network_state).unwrap()));
                
                // Confirm the block is valid and vote, if valid, and the network is in
                // consensus, a new option will do everything below.

                let miner_pk = self.accounts_pk.get(&miner).unwrap();
                
                match block.is_valid(
                    Some(ValidatorOptions::NewBlock(
                        serde_json::to_string(&self.last_block.clone().unwrap()).unwrap(), 
                        serde_json::to_string(&block).unwrap(), 
                        miner_pk.to_string(),
                        serde_json::to_string(&self.clone()).unwrap(),
                        serde_json::to_string(&network_state.clone().lock().unwrap().clone()).unwrap(),
                        reward_state,
                    ))) {
                        Some(true) => {
                            // Cast a true vote by pushing this into a queue to communicates with the
                            // node that can then publish your message
                        },
                        Some(false) => {
                            // Cast a false vote by pushing this into a queue that communicates with the
                            // node that can then publish the message
                        },
                        None => { 
                            println!("You are not a claim staker, to participate in network governance you must own and stake claims") 
                        }
                    }
            },
            StateOption::ConfirmedBlock(_miner, _block, _reward_state, _network_state) => {
                // If the block has been confirmed by the network, and there is consensus
                // around the state of the network at the given block.height
                // confirm the network state by replacing it with the temporary network state
                // object used to validate the new block. Replace any 
            }

            // If the StateOption is a confirmed transaction update the account state
            // accordingly (update balances of sender, receiver(s)) distribute the
            // fees to the trasnaction's validator.
            StateOption::ConfirmedTxn(txn, validator) => {
                //TODO: distribute txn fees among validators.
                let txn = serde_json::from_str::<Txn>(&txn).unwrap();
                let validator: Validator = serde_json::from_str::<Validator>(&validator).unwrap();
                self.pending.get(&txn.txn_id).unwrap().clone().validators.push(validator);

                let num_invalid = self.pending.get(&txn.txn_id)
                                        .unwrap()
                                        .clone().validators.iter()
                                        .filter(|&validator| !validator.to_owned().valid)
                                        .count();

                let len_of_validators = self.pending.get(&txn.txn_id).unwrap().clone().validators.len();
                println!("{}", &len_of_validators);
                if len_of_validators >= 3 {
                    if num_invalid as f32 / len_of_validators as f32 > 1.0 / 3.0 {
                        {}
                    } else {
                        self.mineable.insert(txn.clone().txn_id, txn);
                    }
                }
            }
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> AccountState {
        let mut buffer: Vec<u8> = vec![];
        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();
        serde_json::from_str::<AccountState>(&to_string).unwrap()
    }
}

impl WalletAccount {
    /// Initiate a new wallet.
    pub fn new(
        account_state: Arc<Mutex<AccountState>>, // A new wallet must also receive the AccountState
    ) -> WalletAccount {
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
        let mut addresses = HashMap::new();
        addresses.insert(1, address_prefix.clone());

        let mut total_balances = HashMap::new();
        let mut vrrb_balances = HashMap::new();
        vrrb_balances.insert("VRRB".to_string(), STARTING_BALANCE);
        total_balances.insert(address_prefix.clone(), vrrb_balances);

        // Generate a wallet struct by assigning the variables to the fields.
        let wallet = Self {
            secretkey: secret_key.to_string(),
            pubkey: public_key.to_string(),
            addresses,
            total_balances: total_balances.clone(),
            available_balances: total_balances,
            claims: vec![],
        };
        // Update the account state and save it to a variable to return
        // this is required because this function consumes the account_state
        // TODO: Use Atomic Reference Counter for shared state concurrency
        // and prevent his from being consumed.
        account_state.lock().unwrap().update(
            StateOption::NewAccount(serde_json::to_string(&wallet).unwrap()),
            );
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
    pub fn get_balance(&mut self, account_state: AccountState) {
        self.addresses.clone()
            .iter()
            .map(|x| x)
            .for_each(|(_i, x)| {
            self.total_balances
                .insert(x.clone(), account_state.balances.get(x).unwrap().clone());
            self.available_balances
                .insert(x.clone(), account_state.pending_balances.clone().get(x).unwrap().clone());
            });
    }

    pub fn remove_mined_claims(&mut self, block: &Block) -> Self {
        self.claims
            .iter()
            .position(|x| x.clone().unwrap() == block.clone().claim)
            .map(|e| self.claims.remove(e));
        self.clone()
    }

    pub fn send_txn(
        &mut self, address_number: u32, account_state: Arc<Mutex<AccountState>>, receiver: String, amount: u128
    ) -> Result<Txn, Error> {
        let txn = Txn::new(Arc::new(Mutex::new(self.clone())), self.addresses.get(&address_number).unwrap().clone(), receiver, amount);
        account_state.lock().unwrap().update(
            StateOption::NewTxn(serde_json::to_string(&txn).unwrap()),
        );
        Ok(txn.clone())
    }

    pub fn sell_claim(
        &mut self, maturity_timestamp: u128, account_state: &mut AccountState, price: u32
    ) -> Option<(Claim, AccountState)> 
    {
        let claim_to_sell = self.claims[self.claims.iter().position(|x| {
                x.clone().unwrap().maturation_time == maturity_timestamp
            }).unwrap()].clone();

        match claim_to_sell {
            Some(mut claim) => {
                claim.available = true;
                claim.price = price;
                // TODO: Add account state option to Update claim when claim goes up for sale.
                account_state
                    .owned_claims
                    .insert(claim.clone().claim_number, claim.clone().current_owner.0.unwrap());

                Some((claim, account_state.clone()))
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

impl StateOption {
    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> StateOption {
        let mut buffer: Vec<u8> = vec![];
        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();
        serde_json::from_str::<StateOption>(&to_string).unwrap()
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

impl Clone for AccountState {
    fn clone(&self) -> Self {
        AccountState {
            accounts_pk: self.accounts_pk.clone(),
            credits: self.credits.clone(),
            pending_credits: self.pending_credits.clone(),
            debits: self.debits.clone(),
            pending_debits: self.pending_debits.clone(),
            balances: self.balances.clone(),
            pending_balances: self.pending_balances.clone(),
            claims: self.claims.clone(),
            pending_owned_claims: self.pending_owned_claims.clone(),
            owned_claims: self.owned_claims.clone(),
            staked_claims: self.staked_claims.clone(),
            pending: self.pending.clone(),
            mineable: self.mineable.clone(),
            last_block: self.last_block.clone(),
        }
    }
}

