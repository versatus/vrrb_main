use crate::reward;
use crate::state::NetworkState;
use crate::validator::ValidatorOptions;
use crate::verifiable::Verifiable;
use crate::{
    account::AccountState,
    allocator::allocate_claims,
    claim::{Claim, CustodianInfo, CustodianOption},
    reward::{Reward, RewardState},
    txn::Txn,
    wallet::WalletAccount,
};
use crate::network::node::MAX_TRANSMIT_SIZE;
use crate::network::chunkable::Chunkable;
use log::{info, warn};
use secp256k1::key::PublicKey;
use secp256k1::Signature;
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;
use ritelinked::LinkedHashMap;
use std::fmt;
use std::io::Error;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
const NANO: u128 = 1;
const MICRO: u128 = NANO * 1000;
const MILLI: u128 = MICRO * 1000;
const SECOND: u128 = MILLI * 1000;
const VALIDATOR_THRESHOLD: f64 = 0.60;

pub enum InvalidBlockReason {
    InvalidStateHash,
    InvalidBlockHeight,
    InvalidLastBlockHash,
    InvalidData(String),
    InvalidClaim,
    InvalidBlockHash,
    InvalidNextBlockReward,
    InvalidBlockReward,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct Block {
    pub block_height: u128,
    pub timestamp: u128,
    pub last_block_hash: String,
    pub data: LinkedHashMap<String, Txn>,
    pub claim: Claim,
    pub block_reward: Reward,
    pub block_signature: String,
    pub block_hash: String,
    pub next_block_reward: Reward,
    pub block_payload: String,
    pub miner: String,
    pub state_hash: String,
    pub owned_claims: LinkedHashMap<u128, Claim>,
}

impl Block {
    pub fn genesis(
        miner: Arc<Mutex<WalletAccount>>, // the wallet that will receive the genesis reward (wallet attached to the node that initializes the network)
        reward_state: Arc<Mutex<RewardState>>,
        account_state: Arc<Mutex<AccountState>>,
    ) -> Result<Block, Error> // Returns a result with either a tuple containing the genesis block and the updated account state (if successful) or an error (if unsuccessful)
    {
        // Get the current time in a unix timestamp duration.
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        // initialize a vector to push claims created by this block to.
        let mut visible_blocks: Vec<Claim> = Vec::with_capacity(20);
        let n_peers = account_state.lock().unwrap().claim_counter.len().clone();
        let n_claims = n_peers;
        // initialize a variable to increment the maturity timstamp on claims.
        let next_time = now.as_nanos();
        // set 20 new claims into the vector initialized earlier incrementing each one
        // by 5 nano seconds
        // TODO: Change this to 1 second, 5 nano seconds is just for testing.
        let mut expiration = next_time.clone();
        let mut maturity = next_time.clone();
        if n_peers < 20 {
            for i in 0..n_claims {
                expiration += 30 * SECOND;
                maturity += 100 * MILLI;
                let claim = Claim::new(expiration, maturity, i as u128 + 1);
                visible_blocks.push(claim);
           }
        } else {
            for i in 0..20 {
                expiration += 30 * SECOND;
                maturity += 100 * MILLI;
                let claim = Claim::new(next_time, maturity, i as u128 + 1);
                visible_blocks.push(claim);
            }
        }

        let mut owned_claims = allocate_claims(
            visible_blocks,
            Arc::clone(&miner),
            0u128,
            Arc::clone(&account_state),
        );
        owned_claims = owned_claims
            .iter_mut()
            .map(|(_, claim)| {
                claim.staked = true;
                return (claim.claim_number.clone(), claim.clone());
            })
            .collect::<LinkedHashMap<u128, Claim>>();

        let state_hash = digest_bytes("Genesis_State_Hash".to_string().as_bytes());
        let miner_address = miner.lock().unwrap().addresses[&1].clone();
        // Initialize a new block.
        let genesis = Block {
            block_height: 0,

            // set the timestamp to the now variable
            timestamp: now.as_nanos(),
            // set the last block hash to the hash result of the bytes of the string Genesis_Last_Block_Hash
            last_block_hash: digest_bytes("Genesis_Last_Block_Hash".to_string().as_bytes()),
            // set the value of data to an empty hashmap
            data: LinkedHashMap::new(),

            // set the value of the claim to a new claim with the maturity timestamp of now
            claim: Claim::new(now.as_nanos() + 30 * SECOND, now.as_nanos() + 1 * SECOND, 0),

            // set the value of the block reward to the result of a call to the genesis associated
            // method in the Reward struct. This will generate the Genesis reward and send it to
            // the wallet of the node that initializes the network.
            block_reward: Reward::genesis(Some(miner_address.clone())),

            // Set the value of the block signature to the string "Genesis_Signature"
            block_signature: "Genesis_Signature".to_string(),

            // Set the value of the block hash to the result of hashing the bytes of the string "Genesis_Block_Hash"
            block_hash: digest_bytes("Genesis_Block_Hash".to_string().as_bytes()),

            // Set the value of the next block's reward to the result of calling the new() method from the Reward Struct.
            next_block_reward: Reward::new(None, Arc::clone(&reward_state)),

            // Set the value of block_payload field to the block payload
            block_payload: "Genesis_Block_Hash".to_string(),

            // Set the value of miner to the address of the wallet of the node that initializes the network.
            miner: miner_address.clone(),

            state_hash,

            owned_claims,
        };

        // Update the account state with the miner and new block, this will also set the values to the
        // network state. Unwrap the result and assign it to the variable updated_account_state to
        // be returned by this method.

        Ok(genesis)
    }

    /// The mine method is used to generate a new block (and an updated account state with the reward set
    /// to the miner wallet's balance), this will also update the network state with a new confirmed state.
    pub fn mine(
        claim: Claim,      // The claim entitling the miner to mine the block.
        last_block: Block, // The last block, which contains the current block reward.
        account_state: Arc<Mutex<AccountState>>,
        reward_state: Arc<Mutex<RewardState>>,
        network_state: Arc<Mutex<NetworkState>>,
        wallet: Arc<Mutex<WalletAccount>>,
    ) -> Option<Result<Block, Error>> // Returns a result containing either a tuple of the new block and the updated account state or an error.
    {
        // Initialize a timestamp of the current time.
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        // initialize a secp256k1 Signature struct from the signature string in the claim (this is to verify claim ownership)
        let signature_string = match claim
            .clone()
            .chain_of_custody
            .get(&claim.clone().current_owner.unwrap())
            .unwrap()
            .get(&CustodianOption::BuyerSignature)
            .unwrap()
            .clone()
            .unwrap()
        {
            CustodianInfo::BuyerSignature(Some(signature_string)) => signature_string,
            _ => {
                panic!("No buyer signature")
            }
        };

        let signature = Signature::from_str(&signature_string).unwrap();

        // Generate the next block's reward by assigning the result of the Reward::new() method to a variable called "next block reward".
        let next_block_reward = Reward::new(None, Arc::clone(&reward_state));
        let miner = wallet.lock().unwrap().addresses[&1].clone();
        let pubkey = claim.clone().current_owner.unwrap();
        let data = account_state
            .clone()
            .lock()
            .unwrap()
            .txn_pool
            .confirmed
            .clone();
        // Structure the block payload (to be signed by the miner's wallet for network verification).
        let block_payload = format!(
            "{},{},{},{},{},{},{}",
            now.as_nanos().to_string(),
            last_block.block_hash,
            serde_json::to_string(&data).unwrap(),
            serde_json::to_string(&claim).unwrap(),
            serde_json::to_string(&last_block.next_block_reward.clone()).unwrap(),
            &miner,
            serde_json::to_string(&next_block_reward).unwrap()
        );

        // Ensure that the claim is mature
        if claim.maturation_time <= now.clone().as_nanos() {
            // If the claim is mature, initialize a vector with the capacity to hold the new claims that will be created
            // by this new block being mined.
            let mut visible_blocks: Vec<Claim> = Vec::with_capacity(20);

            // Get the claim with the highest maturity timestamp that current exists.
            let mut furthest_expiration: u128 = if let Some((_, claim)) = account_state.lock().unwrap().clone().claim_pool.confirmed.back() {
                claim.expiration_time
            } else {
                now.clone().as_nanos()
            };
            
            let mut furthest_maturity: u128 = if let Some((_, claim)) = account_state.lock().unwrap().clone().claim_pool.confirmed.back() {
                claim.maturation_time
            } else {
                now.clone().as_nanos()
            };

            let mut highest_claim_number: u128 = if let Some((claim_number, _)) = last_block.owned_claims.back() {
                *claim_number
            } else {
                0
            };
            // Generate 20 new claims, increment each one's maturity timestamp by 5 nanoseconds
            // and push them to the visible_blocks vector.
            // TODO: Add capacity feature to max number of claims available to 1 million.
            let n_peers = account_state.lock().unwrap().claim_counter.len().clone();
            let n_claims = n_peers;

            if n_peers < 20 {
                (0..n_claims).for_each(|_| {
                    furthest_expiration += 30 * SECOND;
                    furthest_maturity += 2500 * MILLI;
                    highest_claim_number += 1;
                    visible_blocks.push(Claim::new(furthest_expiration, furthest_maturity, highest_claim_number));
                });
            } else {
                (0..20).for_each(|_| {
                    furthest_expiration += 30 * SECOND;
                    furthest_maturity += 2500 * MILLI;
                    highest_claim_number += 1;
                    visible_blocks.push(Claim::new(furthest_expiration, furthest_maturity, highest_claim_number));
                });
            }

            let owned_claims = allocate_claims(
                visible_blocks,
                Arc::clone(&wallet),
                last_block.block_height + 1,
                Arc::clone(&account_state),
            );
            // Verify that the claim is indeed owned by the miner attempting to mine this block.
            // Verify returns a result with either a boolean (true or false, but always true if Ok())
            // or an error.
            match WalletAccount::verify(
                claim.clone().claim_payload.unwrap(),
                signature,
                PublicKey::from_str(&&pubkey).unwrap(),
            ) {
                // if it is indeed owned by the miner attempting to mine this block
                Ok(_t) => {
                    // generate the new block and assign it to a variable new_block
                    let mut new_block = Block {
                        block_height: last_block.block_height + 1,

                        // set the timestamp value as now (in nanoseconds)
                        timestamp: now.as_nanos(),
                        // set the last_block_hash value to the block hash of the previous block.
                        last_block_hash: last_block.block_hash.clone(),

                        // Set the data value to data passed into this method in the signature
                        data,

                        // Set the claim to the claim that entitled the miner to mine this block
                        claim: claim.clone(),

                        // Set the block reward to the previous block reward but with the miner value
                        // as Some() with the miner's wallet address inside.
                        block_reward: Reward {
                            miner: Some(miner.clone()),
                            ..last_block.next_block_reward
                        },

                        // Set the block hash to the resulting hash of the block payload string as bytes.
                        block_hash: digest_bytes(block_payload.as_bytes()),

                        // Generate a new reward for the next block.
                        next_block_reward: Reward::new(None, Arc::clone(&reward_state)),
                        block_payload,

                        // Set the miner to the miner wallet address.
                        miner: miner.clone(),

                        // Set the block signature to a string of the claim signature.
                        block_signature: signature.to_string(),

                        state_hash: last_block.block_hash.clone(),

                        owned_claims,
                    };

                    // Generate an updated account state by calling the .update() method
                    // on the account state that was passed through the function signature
                    // this returns a result with either an AccountState struct (the updated account state)
                    // if successful, or an error if not successful. Assign this to a variable to be
                    // included in the return expression.
                    // Return a Some() option variant with an Ok() result variant that wraps a tuple that contains
                    // the new block that was just mined and the updated account state.
                    let mut hashable_state = network_state.lock().unwrap().clone();
                    let state_hash =
                        hashable_state.hash(new_block.clone(), &now.as_nanos().to_ne_bytes());
                    new_block = Block {
                        state_hash,
                        ..new_block.clone()
                    };

                    return Some(Ok(new_block));
                }

                // If the claim is not valid then return a unit struct
                Err(_e) => (),
            }
        }

        // If the claim is not mature then return None.
        None
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.to_string().as_bytes().to_vec()
    }

    pub fn from_bytes(data: &[u8]) -> Block {
        let mut buffer: Vec<u8> = vec![];

        data.iter().for_each(|x| buffer.push(*x));

        let to_string = String::from_utf8(buffer).unwrap();

        serde_json::from_str::<Block>(&to_string).unwrap()
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

pub fn data_validator(data: LinkedHashMap<String, Txn>) -> Option<bool> {
    let mut valid_data: bool = true;

    data.iter().for_each(|(_, txn)| {
        let n_valid = txn.validators.iter().filter(|(_, &valid)| valid).count();
        if (n_valid as f64 / txn.validators.len() as f64) < VALIDATOR_THRESHOLD {
            valid_data = false
        }
    });

    Some(valid_data)
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Block(\n \
            reward: {:?},\n \
            next_block_reward: {:?}\n \
            claim: {:?}",
            self.block_reward, self.next_block_reward, self.claim,
        )
    }
}

impl Verifiable for Block {
    fn is_valid(&self, options: Option<ValidatorOptions>) -> Option<bool> {
        match options {
            Some(block_options) => {
                match block_options {
                    ValidatorOptions::NewBlock(last_block, reward_state, network_state) => {
                        
                        let valid_signature = match self
                            .clone()
                            .claim
                            .chain_of_custody
                            .get(&self.clone().claim.current_owner.unwrap())
                        {
                            Some(map) => {
                                match map.get(&CustodianOption::BuyerSignature) {
                                    Some(Some(CustodianInfo::BuyerSignature(Some(sig)))) => {
                                        let pubkey_string =
                                            &self.clone().claim.current_owner.unwrap().clone();
                                        let pubkey = match PublicKey::from_str(&pubkey_string) {
                                            Ok(pk) => pk,
                                            Err(_e) => {
                                                println!("Invalid Public Key");
                                                // Cast false vote with proper structure and
                                                // reason for false vote.
                                                return Some(false);
                                            }
                                        };
                                        let signature = match Signature::from_str(&sig) {
                                            Ok(sig) => sig,
                                            Err(_e) => {
                                                println!("Invalid Signature Structure");
                                                // Cast false vote with proper structure and reason
                                                // for false vote.
                                                return Some(false);
                                            }
                                        };

                                        self.claim.verify(&signature, &pubkey)
                                    }
                                    _ => {
                                        println!("Buyer never signed claim");
                                        return Some(false);
                                    }
                                }
                            }
                            None => {
                                println!("Signature verification returned None");
                                // Cast false vote with proper structure and reason for false vote
                                return Some(false);
                            }
                        };

                        match valid_signature {
                            Ok(true) => {}
                            Ok(false) => {
                                println!("Invalid Signature");
                                return Some(false);
                            }
                            Err(e) => {
                                println!("Signature validation returned error: {}", e);
                                return Some(false);
                            }
                        }

                        if self.last_block_hash != last_block.block_hash {
                            info!(target: "block_height","{} - 1 == {}", &self.block_height, last_block.block_height);
                            info!(target: "block_hash", "{} == {}", &self.last_block_hash, last_block.block_hash);
                            warn!(target: "invalid_block_hash", "Invalid last block hash");
                            return Some(false);
                        }
                        if self.block_hash != digest_bytes(self.block_payload.as_bytes()) {
                            println!("Invalid block hash");
                            return Some(false);
                        }

                        let mut hashable_state = network_state.clone();

                        let state_hash =
                            hashable_state.hash(self.clone(), &self.timestamp.to_ne_bytes());

                        if self.state_hash != state_hash {
                            println!("Invalid state hash");
                            // If state hash is invalid cast false vote with the reason why.
                            return Some(false);
                        }

                        if last_block.next_block_reward.amount != self.block_reward.amount {
                            println!("invalid block reward doesn't match last block reward");
                            return Some(false);
                        }

                        match reward::valid_reward(self.block_reward.category, reward_state) {
                            Some(false) => {
                                println!("invalid block reward");
                                return Some(false);
                            }
                            None => {
                                println!("reward validation returned None");
                                return Some(false);
                            }
                            _ => {}
                        }

                        match reward::valid_reward(self.next_block_reward.category, reward_state) {
                            Some(false) => {
                                println!("invalid next block reward");
                                return Some(false);
                            }
                            None => {
                                println!("reward validation returned None");
                                return Some(false);
                            }
                            _ => {}
                        }

                        match data_validator(self.data.clone()) {
                            Some(false) => {
                                println!("Invalid Data");
                                return Some(false);
                            }
                            None => {
                                println!("data validator returned none");
                                return Some(false);
                            }
                            _ => {}
                        }
                        //TODO: If block is valid cast true vote.
                        Some(true)
                    }
                    _ => panic!("Invalid options for block"),
                }
            }
            None => Some(false),
        }
    }
}


impl Chunkable for Block {
    fn chunk(&self) -> Option<Vec<Vec<u8>>> {
        let bytes_len = self.as_bytes().len();
        if bytes_len > MAX_TRANSMIT_SIZE {
            let mut n_chunks = bytes_len / ( MAX_TRANSMIT_SIZE / 10 );
            if bytes_len % MAX_TRANSMIT_SIZE != 0 {
                n_chunks += 1;
            }
            let mut chunks_vec = vec![];
            let mut last_slice_end = 0;
            (1..=bytes_len).map(|n| n * (MAX_TRANSMIT_SIZE / 10)).enumerate().for_each(|(index, slice_end)| {
                if index + 1 == n_chunks {
                    chunks_vec.push(self.clone().as_bytes()[last_slice_end..].to_vec());
                } else {
                    chunks_vec.push(self.clone().as_bytes()[last_slice_end..slice_end].to_vec());
                    last_slice_end = slice_end;
                }
            });
            Some(chunks_vec)
        } else {
            None
        }
    }
}