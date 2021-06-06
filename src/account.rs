use bytebuffer::ByteBuffer;
use secp256k1::Error;
use secp256k1::{
    key::{PublicKey, SecretKey},
    Signature,
};
use std::str::FromStr;
use secp256k1::{Message, Secp256k1};
use sha256::digest_bytes;
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
// use bip39::{Mnemonic, Language};
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

    /// A vector of pending txns that have not been validated
    /// consider changing this to a vec of (txn_id, txn_hash, signature) thruples
    /// may speed up txn validation/processing time and save memory.
    pub pending: Vec<Txn>,

    /// A vector of validated transactions that have not been included in a block.
    /// All of these transactions are eligible to be included in the next block.
    pub mineable: Vec<Txn>,

    // TODO: Add a state hash, which will sha256 hash the entire state structure for
    // consensus purposes.
}

/// The WalletAccount struct is the user/node wallet in which coins, tokens and contracts
/// are held. The WalletAccount has a private/public keypair 
/// phrase are used to restore the Wallet. The private key is
/// also used to sign transactions, claims and mined blocks for network validation.
/// Private key signatures can be verified with the wallet's public key, the message that was
/// signed and the signature.
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
            pending: vec![],
            mineable: vec![],
        }
    }

    /// Update's the AccountState and NetworkState, takes a StateOption (for function routing)
    /// also requires the NetworkState to be provided in the function call.
    /// TODO: Provide Examples to Doc
    pub fn update(&mut self, value: StateOption, network_state: &mut NetworkState) -> Result<Self, Error> {
       match value {
            StateOption::NewAccount(wallet) => {
                self.accounts_sk.entry(wallet.skhash.to_string())
                    .or_insert(wallet.public_key.to_string());
                self.accounts_pk.entry(wallet.public_key.to_string())
                    .or_insert(wallet.address.clone());
                self.accounts_address.entry(wallet.address.clone())
                    .or_insert(wallet.public_key.to_string());
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
                    Some(_) => println!("Mined block with claim. Removed claim from owned"),
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
        
}

impl WalletAccount {

    /// Initiate a new wallet.
    /// TODO: Set the wallet in the account state immediately, as opposed to how it is currently done.
    pub fn new(account_state: &mut AccountState, network_state: &mut NetworkState) -> (Self, AccountState) {
        let secp = Secp256k1::new();
        let mut rng = rand::thread_rng();
        let (secret_key, public_key) = secp.generate_keypair(&mut rng);
        let uid_address = digest_bytes(Uuid::new_v4().to_string().as_bytes());
        let mut address_prefix: String = "0x192".to_string();
        address_prefix.push_str(&uid_address);

        println!("DO NOT SHARE OR LOSE YOUR PRIVATE KEY:");
        println!("{:?}\n", &secret_key.to_string());
        // println!("{:?}\n", &secret_key.to_string().as_bytes()[0..(&secret_key.to_string().as_bytes().len() / 2)].len());

        let wallet = Self {
            private_key: secret_key,
            public_key: public_key,
            address: address_prefix,
            balance: STARTING_BALANCE,
            available_balance: STARTING_BALANCE,
            tokens: vec![],
            claims: vec![],
            skhash: digest_bytes(secret_key.to_string().as_bytes()),
        };
        
        let updated_account_state = account_state.update(StateOption::NewAccount(wallet.clone()), network_state).unwrap();

        (wallet, updated_account_state)
    }

    pub fn restore_from_private_key(private_key: String, account_state: AccountState) -> WalletAccount {
        let public_key = account_state.accounts_sk.get(&private_key.to_owned()).unwrap();
        let address = account_state.accounts_pk.get(&public_key[..]).unwrap();
        let balance = account_state.total_coin_balances.get(&public_key[..]).unwrap();
        let available_balance = account_state.available_coin_balances.get(&public_key[..]).unwrap();
        let tokens = account_state.token_balances.get(&public_key[..]).unwrap();
        let claims = vec![];
        let private_key = SecretKey::from_str(&private_key).unwrap();
        let sk_hash = digest_bytes(private_key.to_string().as_bytes());

        WalletAccount {
            private_key: private_key,
            public_key: PublicKey::from_str(&public_key).unwrap(),
            address: address.to_owned(),
            balance: *balance,
            available_balance: *available_balance,
            tokens: tokens.to_owned(),
            claims: claims,
            skhash: sk_hash,
        }
    }

    /// Sign a message (transaction, claim, block, etc.)
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
        }
    }
}

impl Clone for AccountState {
    fn clone(&self) -> Self {
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
                private_key: *secret_key_for_restoration,
                public_key: PublicKey::from_str(&wallet_to_restore_pk).unwrap(),
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