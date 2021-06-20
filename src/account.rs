use bytebuffer::ByteBuffer;
use secp256k1::Error;
use secp256k1::{
    key::{PublicKey, SecretKey},
    Signature,
};
use std::str::{FromStr};
use secp256k1::{Message, Secp256k1};
use sha256::digest_bytes;
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use crate::validator::{InvalidMessageError, Validator};
// use crate::validator::Validator;
use crate::{claim::{Claim, ClaimState}, vrrbcoin::Token, txn::Txn, block::Block, state::NetworkState};

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
    NewTxn(Txn),
    NewAccount(WalletAccount),
    ClaimAcquired(Claim),
    ConfirmedTxn((Txn, Vec<Validator>)),
    Miner((String, Block)),
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

    // Map of account address to public key
    pub accounts_pk: HashMap<String, String>,

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
    pub token_balances: HashMap<String, Vec<(Option<Token>, Option<Token>)>>,

    /// The local claim state.
    pub claim_state: ClaimState,

    /// A HashMap of txn_id -> (txn, Vec<Validator>)
    pub pending: HashMap<String, (Txn, Vec<Validator>)>,

    /// A HashMap of confirmed Txn's (Txn's with 2/3 Validator's valid_field == true)
    pub mineable: HashMap<String, (Txn, Vec<Validator>)>,

    // TODO: Add a state hash, which will sha256 hash the entire state structure for
    // consensus purposes.
}

/// The WalletAccount struct is the user/node wallet in which coins, tokens and contracts
/// are held. The WalletAccount has a private/public keypair 
/// phrase are used to restore the Wallet. The private key is
/// also used to sign transactions, claims and mined blocks for network validation.
/// Private key signatures can be verified with the wallet's public key, the message that was
/// signed and the signature.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletAccount {
    private_key: String,
    pub address: String,
    pub public_key: String,
    pub balance: u128,
    pub available_balance: u128,
    pub tokens: Vec<(Option<Token>, Option<Token>)>,
    pub claims: Vec<Option<Claim>>,
    skhash: String,
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
            accounts_sk: HashMap::new(),
            accounts_address: HashMap::new(),
            accounts_pk: HashMap::new(),
            total_coin_balances: HashMap::new(),
            available_coin_balances: HashMap::new(),
            token_balances: HashMap::new(),
            claim_state: ClaimState::start(),
            pending: HashMap::new(),
            mineable: HashMap::new(),
        }
    }

    /// Update's the AccountState and NetworkState, takes a StateOption (for function routing)
    /// also requires the NetworkState to be provided in the function call.
    /// TODO: Provide Examples to Doc
    pub fn update(
        &mut self, 
        value: StateOption, 
        network_state: &mut NetworkState
    ) -> Result<Self, InvalidMessageError> 
    
    {
       match value {

            // If the StateOption variant passed to the update method is a NewAccount
            // set the new account information into the account state, return the account state
            // and update the network state.
            StateOption::NewAccount(wallet) => {

                // Enter the wallet's secret key hash as the key and the wallet's public key as the value
                // if the secret key hash is not already in the hashmap
                self.accounts_sk.entry(wallet.skhash.to_string())
                    .or_insert(wallet.public_key.to_string());

                // Enter the wallet's public key string as the key and if it's not already in the HashMap
                // the wallet's address as the value.
                self.accounts_pk.entry(wallet.public_key.to_string())
                    .or_insert(wallet.address.clone());
                
                // Enter the wallet's address as the key and if it's not already in the HashMap the wallet's
                // public key string as the value
                self.accounts_address.entry(wallet.address.clone())
                    .or_insert(wallet.public_key.to_string());
                
                // Enter the wallet's public key string as the key and the STARTING_BALANCE const as
                // the value 
                // TODO: 0 should be the starting value of ALL accounts on the live network
                self.total_coin_balances.entry(wallet.public_key.to_string())
                    .or_insert(STARTING_BALANCE);
                
                // Same thing as above since this is a new account
                self.available_coin_balances.entry(wallet.public_key.to_string())
                    .or_insert(STARTING_BALANCE);
                
                // The .update() method for the  network state sets a state object (struct)
                // either account_state, claim_state or reward state) into the pickle db
                // that represents the network state.
                network_state.update(self.clone(), "account_state");
                
                return Ok(self.to_owned());
            },

            // If the StateOption variant passed is a NewTxn process the txn and either return
            // an error if there's obvious validity issues or set it to pending txns to be
            // fully validated by validators.
            StateOption::NewTxn(txn) => {
                
                // get the receiver's public key from the AccountState accounts_address field
                // which is a hashmap containing the address as the key and the public key as the value
                let receiver = self.accounts_address.get(&txn.receiver_address);
                match receiver {
                    Some(receiver_pk) => {
                        let receiver_pk = receiver_pk;

                        // get the sender's public key from the AccountState accounts_address field
                        // which is a hashmap containing the address as the key and the public key as the value

                        // get the sender's coin balance as mutable, from the availabe_coin_balances field
                        // in the account_state object, which takes the public key as the key and the
                        // available balance as the value, 
                        let sender = self.accounts_address.get(&txn.sender_address);

                        match sender {
                            
                            Some(sender_pk) => {
                                let sender_pk = sender_pk;

                                let sender_avail_bal = *self.available_coin_balances
                                                                .get_mut(sender_pk)
                                                                .unwrap();
                                
                                let balance_check = sender_avail_bal.checked_sub(txn.txn_amount);

                                match balance_check {

                                    Some(bal) => {
                                        // Add the amount to the receiver balance by getting the entry in the
                                        // total_coin_balances field in the AccountState for the receiver
                                        // public key as mutable, add the transaction fee and set it to
                                        // the receiver total balance variable being initalized
                                        let receiver_total_bal = self.total_coin_balances
                                                                        .get_mut(receiver_pk)
                                                                        .unwrap()
                                                                        .checked_add(txn.txn_amount)
                                                                        .unwrap();

                                        // Update the available balance of the sender
                                        self.available_coin_balances
                                            .insert(sender_pk.to_owned(), bal);

                                        // update the total balance of the receiver
                                        self.total_coin_balances
                                            .insert(
                                                receiver_pk.to_owned(), 
                                                receiver_total_bal
                                            );
                                        
                                        // Push the transaction to pending transactions to be confirmed by validators.
                                        self.pending
                                            .entry(
                                                txn.clone().txn_id)
                                                    .or_insert(
                                                        (txn.clone(), vec![]));
                                        
                                        // Update the network state (the pickle db) with the updated account state information.
                                        network_state
                                            .update(
                                                self.clone(), 
                                                "account_state"
                                            );
                                        
                                        // Return the updated account state wrapped in an Ok() Result variant.
                                        return Ok(self.to_owned());
                                    },
                                None => {
                                    return Err(InvalidMessageError::InvalidTxnError("Amount Exceeds Balance".to_string()));
                                    }
                                }
                            },
                            None => {
                                return Err(InvalidMessageError::InvalidTxnError("Sender is non-existent".to_string()))
                            }
                        }
                    },
                    None => {
                        return Err(InvalidMessageError::InvalidTxnError("The receiver is non-existent".to_string()));
                    }
                }
            },

            // If the StateOption variant received by the update method is a ClaimAcquired
            // Update the account state by entering the relevant information into the 
            // proper fields, return the updated account state and update the network state.
            StateOption::ClaimAcquired(claim) => {

                // Set a new entry (if it doesn't exist) into the AccountState
                // claim_state field's (which is a ClaimState Struct) owned_claims field
                // which is a HashMap consisting of the claim maturation time as the key and the claim
                // struct itself as the value.
                // TODO: break down PendingClaimAcquired and ConfirmedClaimAcquired as claim acquisition
                // has to be validated before it can be set into the account_state's claim_state.
                self.claim_state.owned_claims.insert(claim.maturation_time.clone(),claim.clone());
                
                // Remove the claim from the account_state's claim_state's claims field since it is now owned
                self.claim_state.claims.remove_entry(&claim.maturation_time);
                
                // update the network state.
                network_state.update(self.clone(), "account_state");
                
                // return an Ok() variant of the Result enum with the account state in it.
                return Ok(self.to_owned());
            },
            
            // If the StateOption variant received by the update method is Miner
            // this means a new block has been mined, udpate the account state accordingly

            // TODO: mined blocks need to be validated by the network before they're confirmed
            // If it has not yet been confirmed there should be a PendingMiner variant as well
            // as a ConfirmedMiner variant. The logic in this block would be for a ConfirmedMiner
            StateOption::Miner((miner, block)) => {
                
                // get the public key of the miner from the address. This is set up
                // so that the entire wallet no longer gets passed through to this variant
                // but rather, just the wallet address get's passed through. This is much
                // safer than receiving the entire wallet account (which contains the secret key)

                // TODO: Change the Miner variant(s) to receive a wallet.address (String) not a wallet.
                // The assignment of the public key below COULD fail if the miner address (for whatever reason)
                // is not in the account_state, if it is not in the account state then error handling needs to occur
                // it is likely because this is a very very new account (unlikely that such a new account)
                // would be mining a block, but possible, especially if it acquired a claim or received a claim
                // This would mean, very likely that the current account state is out of consensus and would
                // need to request the latest confirmed account state from the network.
                let miner_pk = self.accounts_address.get(&miner).unwrap();
                
                // update the miner's total (confirmed) coin balance to include the reward
                self.total_coin_balances.insert(
                    miner_pk.to_owned(), 
                    self.total_coin_balances[miner_pk] + block.block_reward.amount);
                
                // update the miner's available coin balance to include the reward
                self.available_coin_balances.insert(
                    miner_pk.to_owned(), 
                    self.available_coin_balances[miner_pk] + block.block_reward.amount);
                
                // The block contains 20 new claims, set them in the claim state.
                for claim in block.clone().visible_blocks {
                    self.claim_state.claims.entry(claim.maturation_time).or_insert(claim);
                }
                
                // remove the claim used to mine the block from owned claims, it has been used and cannot
                // be used again.
                match self.claim_state.owned_claims.remove_entry(&block.clone().claim.maturation_time) {
                    Some(_) => println!("Mined block with claim. Removed claim from owned"),
                    None => println!("Couldn't find claim in owned"),
                }

                // Remove the claim from the miner's owned claims
                // TODO: this will only work if it's the current node's wallet
                // This can be removed, as the account state updates, if it's the current
                // wallet's claim being mined this can be handled in a method in the wallet.
                // miner.remove_mined_claims(&block);

                // Update the network's (confirmed) state to account for the mined block.
                network_state.update(self.clone(), "account_state");

                // Return the account state.                
                return Ok(self.to_owned());

            },

            // If the StateOption is a confirmed transaction update the account state
            // accordingly (update balances of sender, receiver(s)) distribute the
            // fees to the trasnaction's validator.
            StateOption::ConfirmedTxn((_txn, _validators)) => {
                //TODO: distribute txn fees among validators.
                return Ok(self.to_owned());
            },
        }
    }
        
}

impl WalletAccount {

    /// Initiate a new wallet.
    pub fn new(
        account_state: &mut AccountState,           // A new wallet must also receive the AccountState
        network_state: &mut NetworkState            // The network state as well, as it needs to be updated
    ) -> (Self, AccountState) 
    
    {
        // Initialize a new Secp256k1 context
        let secp = Secp256k1::new();

        // Generate a random number used to seed the new keypair for the wallet
        // TODO: Instead of using the rng, use a mnemonic seed.
        let mut rng  = rand::thread_rng();
        
        // Generate a new secret/public key pair using the random seed.
        let (secret_key, public_key) = secp.generate_keypair(&mut rng);
        
        // Generate an address by hashing a universally unique ID
        let uid_address = digest_bytes(Uuid::new_v4().to_string().as_bytes());
        
        // add the testnet prefix to the wallet address (TODO: add handling of testnet/mainnet)
        let mut address_prefix: String = "0x192".to_string();
        
        // push the hashed uuid string to the end of the address prefix
        address_prefix.push_str(&uid_address);

        // Print the private key string so that the user can save it.
        // TODO: require a confirmation the private key being saved by the user
        println!("DO NOT SHARE OR LOSE YOUR PRIVATE KEY:");
        println!("{:?}\n", &secret_key.to_string());

        // Generate a wallet struct by assigning the variables to the fields.
        let wallet = Self {
            private_key: secret_key.to_string(),
            public_key: public_key.to_string(),
            address: address_prefix,
            balance: STARTING_BALANCE,
            available_balance: STARTING_BALANCE,
            tokens: vec![],
            claims: vec![],
            skhash: digest_bytes(secret_key.to_string().as_bytes()),
        };
        
        // Update the account state and save it to a variable to return
        // this is required because this function consumes the account_state
        // TODO: Use Atomic Reference Counter for shared state concurrency
        // and prevent his from being consumed. 
        let updated_account_state = account_state.update(
            StateOption::NewAccount(
                wallet.clone()), 
                network_state
            );

        match updated_account_state {
            Ok(acccount_state) => {
                let updated_account_state = acccount_state;
                return (wallet, updated_account_state)
            },
            Err(invalid_txn) => {
                println!("{:?}", invalid_txn);
                return (wallet, account_state.clone());
            }
        }
        
        // Return the wallet and account state
        // TODO: Return a Result for error propagation and handling.

    }

    // method for restoring a wallet from the private key
    pub fn restore_from_private_key(
        private_key: String, 
        account_state: AccountState
    ) -> WalletAccount 
    
    {

        // TODO: Do signature verification on restoration to ensure someone isn't trying to simply
        // hack the account_state to increase their own wallet balances/token holdings, etc.

        // Hash the private key string as bytes
        let pk_hash = digest_bytes(&private_key.to_string().as_bytes());

        // pass the private key hash into the accounts_sk field (HashMap) as the key, and
        // get the public key associated with it
        // TODO: handle potential errors instead of just unwrapping and panicking if it's not found
        let public_key = account_state.accounts_sk.get(&pk_hash.to_owned()).unwrap();

        // Pass the public key string array into the accounts_pk hashmap to get the address in return
        // TODO: Handle potential errors instead of just unwrapping and panicking.
        let address = account_state.accounts_pk.get(&public_key[..]).unwrap();
        
        // Pass the public key string array into the total_coin_balances to get the wallet balance
        // TODO: Handle potential errors instead of just unwrapping and panicking if there's an error
        let balance = account_state.total_coin_balances.get(&public_key[..]).unwrap();
        
        // Pass the public key string array into the available_coin_balances to get the wallet balance
        // TODO: Handle potential errors instead of just unwrapping and panicking if there's an error.
        let available_balance = account_state.available_coin_balances.get(&public_key[..]).unwrap();
        
        // Pass the public key string array into the token balances to get the wallet tokens
        // TODO: Handle potential errors instead of just unwrapping and panicking if there's an error.
        let tokens = account_state.token_balances.get(&public_key[..]).unwrap();
        
        // TODO: Need to reorganize the claim_state so that the public key can be used.
        let claims = vec![];
        
        // Restore the private key from the private key string.
        let private_key = SecretKey::from_str(&private_key).unwrap().to_string();
        
        // Return a restored wallet.
        WalletAccount {
            private_key: private_key.clone(),
            public_key: public_key.clone(),
            address: address.to_owned(),
            balance: *balance,
            available_balance: *available_balance,
            tokens: tokens.to_owned(),
            claims: claims,
            skhash: pk_hash,
        }
    }

    /// Sign a message (transaction, claim, block, etc.)
    pub fn sign(&self, message: &String) -> Result<Signature, Error> {
        
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
        let sk = SecretKey::from_str(&self.private_key).unwrap();
        let sig = secp.sign(&message_hash, &sk);
        
        Ok(sig)
    }

    /// Verify a signature with the signers public key, the message payload and the signature.
    pub fn verify(
        message: String, 
        signature: Signature, 
        pk: PublicKey
    ) -> Result<bool, Error> 
    
    {
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
    pub fn get_balance(
        &mut self, 
        account_state: AccountState
    ) -> Result<Self, Error> 

    {
        let (
            balance, 
            available_balance
        ) = (
            account_state.total_coin_balances
                .get(&self.public_key.to_string()),
            
            account_state.available_coin_balances
                .get(&self.public_key.to_string()));

        Ok(
            Self {
                balance: balance.unwrap().clone(),
                
                available_balance: available_balance.unwrap().clone(),
                
                ..self.to_owned()
        })
    }

    pub fn remove_mined_claims(
        &mut self, 
        block: &Block
    ) -> Self 
    
    {
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
    ) -> Result<(Self, AccountState), Error> 
    
    {
        let txn = Txn::new(self.clone(), receivers.0, receivers.1);
    
        let updated_account_state = account_state.update(
            StateOption::NewTxn(txn), 
            network_state)
            .unwrap();
    
        Ok(
            (
                self.to_owned(), 
                updated_account_state.to_owned()
            )
        )
    }

    pub fn sell_claim(
        &mut self, 
        maturity_timestamp: u128, 
        account_state: &mut AccountState,
        price: u32,
    ) -> Option<(Claim, AccountState)> {

        let claim_to_sell = self.claims[
            self.claims.iter()
                .position(|x| x.clone().unwrap().maturation_time == maturity_timestamp).unwrap()]
                .clone();
        match claim_to_sell {
            Some(mut claim) => {
                claim.available = true;
                claim.price = price;
                // TODO: Add account state option to Update claim when claim goes up for sale.
                account_state.claim_state.owned_claims.insert(claim.clone().maturation_time, claim.clone());

                return Some((claim, account_state.clone()))
            },
            None => {
               return None
            }
        }
    }
}

unsafe impl Send for WalletAccount {}
unsafe impl Sync for WalletAccount {}
unsafe impl Send for AccountState {}
unsafe impl Sync for AccountState {}

impl fmt::Display for WalletAccount {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result 
    {
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

    fn clone(&self) -> WalletAccount 
    
    {
        WalletAccount {
            private_key: self.private_key.clone(),
            address: self.address.clone(),
            public_key: self.public_key.clone(),
            balance: self.balance,
            available_balance: self.available_balance,
            tokens: self.tokens.clone(),
            claims: self.claims.clone(),
            skhash: self.skhash.clone(),
        }
    }
}

impl Clone for AccountState {
    fn clone(&self) -> Self 
    
    {
        AccountState {
            accounts_sk: self.accounts_sk.clone(),
            accounts_pk: self.accounts_pk.clone(),
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::reward::RewardState;

    #[test]
    fn test_wallet_set_in_account_state() {

        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_state.db");
        let (wallet, updated_account_state) = WalletAccount::new(&mut account_state, &mut network_state);
        account_state = updated_account_state;
        let wallet_pk = account_state.accounts_sk.get(&wallet.skhash).unwrap();

        assert_eq!(wallet_pk.to_owned(), wallet.public_key.to_string());
    }

    #[test]
    fn test_restore_account_state_and_wallet() {
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_state.db");
        let mut wallet_vec: Vec<WalletAccount> = vec![];
        for _ in 0..=20 {
            let (
                new_wallet, 
                updated_account_state
            ) = WalletAccount::new(
                &mut account_state, 
                &mut network_state
            );
            
            wallet_vec.push(new_wallet);
            account_state = updated_account_state;
        }
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_state.db");
        for wallet in &wallet_vec
        {
            account_state = account_state.update(
                StateOption::NewAccount(wallet.clone()), 
                &mut network_state
            ).unwrap();
        }
        
        let wallet_to_restore = &wallet_vec[4];
        let secret_key_for_restoration = &wallet_to_restore.private_key;

        {
            let mut inner_scope_network_state = NetworkState::restore("test_state.db");
            let mut _reward_state = RewardState::start(&mut inner_scope_network_state);
            let db_iter = network_state.state.iter();
            for i in db_iter {
                match i.get_value::<AccountState>() {
                    Some(ast) => account_state = ast,
                    None => (),
                }
                match i.get_value::<RewardState>() {
                    Some(rst) => _reward_state = rst,
                    None => (),
                }
            }

            let wallet_to_restore_pk = account_state.accounts_sk.get(
                &digest_bytes(
                    secret_key_for_restoration
                        .to_string()
                        .as_bytes()
                        )
                    ).unwrap();
            
            // Assume no claims, no tokens for now.
            // TODO: Add claims and tokens
            let wallet_to_restore_address = account_state.accounts_pk.get(wallet_to_restore_pk).unwrap();
            let wallet_to_restore_balance = account_state.total_coin_balances.get(wallet_to_restore_pk).unwrap();
            let wallet_to_restore_available_balance = account_state.available_coin_balances.get(wallet_to_restore_pk).unwrap();
            let restored_wallet = WalletAccount {
                private_key: secret_key_for_restoration.clone(),
                public_key: wallet_to_restore_pk.clone(),
                address: wallet_to_restore_address.to_owned(),
                balance: wallet_to_restore_balance.to_owned(),
                available_balance: wallet_to_restore_available_balance.to_owned(),
                tokens: vec![],
                claims: vec![],
                skhash: digest_bytes(secret_key_for_restoration.to_string().as_bytes()),
            };

        assert_eq!(wallet_vec[4].skhash, restored_wallet.skhash);
        assert_eq!(wallet_vec[4].public_key.to_string(), restored_wallet.public_key.to_string());
        assert_eq!(wallet_vec[4].balance, restored_wallet.balance);
        assert_eq!(wallet_vec[4].available_balance, restored_wallet.available_balance);
        }
    }

    #[test]
    fn test_reward_received_by_miner() {

    }

    #[test]
    fn test_send_txn() {

    }

    #[test]
    fn test_recv_txn() {

    }

    #[test]
    fn test_valid_signature() {

    }
    #[test]
    fn test_invalid_signature() {

    }

    #[test]
    fn test_account_state_updated_after_claim_homesteaded() {

    }

    #[test]
    fn test_account_state_updated_after_new_block() {

    }
    
    #[test]
    fn test_account_state_updated_after_new_txn() {

    }

    #[test]
    fn test_account_state_updated_after_confirmed_txn() {

    }

}