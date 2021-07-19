use std::collections::HashMap;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::reward;
use crate::state::NetworkState;
use crate::validator::ValidatorOptions;
use crate::verifiable::Verifiable;
use crate::{
    account::{WalletAccount, AccountState, StateOption::{Miner}}, 
    claim::{Claim}, 
    txn::Txn, 
    reward::{RewardState, Reward},
};
use secp256k1::{Signature};
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;
use std::io::Error;
use std::fmt;
use secp256k1::{
    key::{PublicKey}
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub block_height: u128,
    pub timestamp: u128,
    pub last_block_hash: String,
    pub data: HashMap<String, Txn>,
    pub claim: Claim,
    pub block_reward: Reward,
    pub block_signature: String,
    pub block_hash: String,
    pub next_block_reward: Reward,
    pub block_payload: String,
    pub miner: String,
    pub state_hash: String,
    pub visible_blocks: Vec<Claim>,
}

impl<'a> Block {

    /// The genesis method generates the genesis event. It needs to receive
    /// the reward state, the wallet of the node that initializes the network for the first time.
    /// the account state and the network state.
    ///
    /// ```
    /// use vrrb_lib::block::Block;
    /// use vrrb_lib::account::{AccountState, WalletAccount};
    /// use vrrb_lib::state::NetworkState;
    /// use vrrb_lib::reward::RewardState;
    /// use std::sync::{Arc, Mutex}
    /// let mut network_state = Arc::new(Mutex::new(NetworkState::restore("vrrb_doctest_state.db")));
    /// let mut reward_state = Arc::new(Mutex::new(RewardState::start(network_state)));
    /// let mut account_state = Arc::new(Mutex::new(AccountState::start()));
    /// let mut miner = WalletAccount::new(&mut account_state, &mut network_state);
    ///
    /// let (
    ///     genesis_block, 
    ///     updated_account_state
    /// ) = Block::genesis(
    ///         reward_state, 
    ///         miner.address.clone(), 
    ///         &mut account_state, 
    ///         &mut network_state
    ///     ).unwrap();
    ///
    /// println!("{:?}", genesis_block);
    /// ```
    ///

    pub fn genesis(
        reward_state: Arc<Mutex<RewardState>>,      // The reward state that needs to be updated when the genesis event occurs
        miner: String,      // the wallet that will receive the genesis reward (wallet attached to the node that initializes the network)
        account_state: Arc<Mutex<AccountState<'a>>>,   // the account state which needs to be updated when the genesis event occurs
        network_state: Arc<Mutex<NetworkState<'a>>>,   // the network state which needs to be updated with then genesis event occurs
    ) -> Result<Block, Error>   // Returns a result with either a tuple containing the genesis block and the updated account state (if successful) or an error (if unsuccessful) 
    
    {

        // Get the current time in a unix timestamp duration.
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        // initialize a vector to push claims created by this block to.
        let mut visible_blocks: Vec<Claim> = Vec::with_capacity(20);

        // initialize a variable to increment the maturity timstamp on claims.
        let mut next_time = now.as_nanos();

        // set 20 new claims into the vector initialized earlier incrementing each one
        // by 5 nano seconds
        // TODO: Change this to 1 second, 5 nano seconds is just for testing.
        for i in 1..=20 {
            visible_blocks.push(Claim::new(next_time, i as u128 + 1));
            next_time += 5;
        }

        // TODO: add path field to NetworkState so that this is not hardcoded

        let state_hash = network_state.lock().unwrap().hash(&now.as_nanos().to_ne_bytes());

        // Initialize a new block.
        let genesis = Block {

            block_height: 1,

            // set the timestamp to the now variable
            timestamp: now.as_nanos(),
            
            // set the last block hash to the hash result of the bytes of the string Genesis_Last_Block_Hash
            last_block_hash: digest_bytes("Genesis_Last_Block_Hash".to_string().as_bytes()),
            
            // set the value of data to an empty hashmap
            data: HashMap::new(),

            // set the value of the claim to a new claim with the maturity timestamp of now
            claim: Claim::new(now.as_nanos(), 1),

            // set the value of the block reward to the result of a call to the genesis associated
            // method in the Reward struct. This will generate the Genesis reward and send it to
            // the wallet of the node that initializes the network.
            block_reward: Reward::genesis(Some(miner.clone())),

            // Set the value of the block signature to the string "Genesis_Signature"
            block_signature: "Genesis_Signature".to_string(),

            // Set the value of the block hash to the result of hashing the bytes of the string "Genesis_Block_Hash"
            block_hash: digest_bytes("Genesis_Block_Hash".to_string().as_bytes()),

            // Set the value of the next block's reward to the result of calling the new() method from the Reward Struct.
            next_block_reward: Reward::new(None, Arc::clone(&reward_state)),

            // Set the value of block_payload field to the block payload

            block_payload: "Genesis_Block_Hash".to_string(),

            // Set the value of miner to the address of the wallet of the node that initializes the network.
            miner: miner.clone(),

            state_hash,

            // Set the value of visible blocks to the visible_blocks vector initializes at the top of this method.
            visible_blocks,
        };
        
        // Update the account state with the miner and new block, this will also set the values to the 
        // network state. Unwrap the result and assign it to the variable updated_account_state to
        // be returned by this method.
        account_state.lock().unwrap().update(Miner(miner, serde_json::to_string(&genesis).unwrap()), network_state);

        // Return an Ok() result with a tuple of the genesis block and the updated account state from the previous line
        Ok(genesis)

    }

    /// The mine method is used to generate a new block (and an updated account state with the reward set
    /// to the miner wallet's balance), this will also update the network state with a new confirmed state.
    pub fn mine(
        reward_state: Arc<Mutex<RewardState>>,     // The reward state which gets updated in place to reflect the reward for the currently mined block
        claim: Claim,                   // The claim entitling the miner to mine the block.
        last_block: Block,              // The last block, which contains the current block reward.
        data: HashMap<String, Txn>,     // A hashmap containing transaction IDs and confirmed transactions that will be made official with this block being mined
        account_state: Arc<Mutex<AccountState<'a>>>,   // The account state which will be updated and made official (set into the confirmed network state).
        network_state: Arc<Mutex<NetworkState<'a>>>,   // the network state, which the confirmed state of will be updated for the current block
    ) -> Option<Result<Block, Error>>       // Returns a result containing either a tuple of the new block and the updated account state or an error. 
    
    {
        // Initialize a timestamp of the current time.
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        // initialize a secp256k1 Signature struct from the signature string in the claim (this is to verify claim ownership)
        let claim_signature: Signature = Signature::from_str(
            &claim.clone()
                .current_owner.2
                .unwrap())
                .ok()
                .unwrap();

        // Generate the next block's reward by assigning the result of the Reward::new() method to a variable called "next block reward".
        let next_block_reward = Reward::new(None, Arc::clone(&reward_state));
        let miner = claim.clone().current_owner.0.unwrap();
        let pubkey = claim.clone().current_owner.1.unwrap();
        // Structure the block payload (to be signed by the miner's wallet for network verification).
        let block_payload = format!("{},{},{},{},{},{},{}", 
                        now.as_nanos().to_string(), 
                        last_block.block_hash, 
                        serde_json::to_string(&data).unwrap(), 
                        serde_json::to_string(&claim).unwrap(),
                        serde_json::to_string(&last_block.next_block_reward.clone()).unwrap(),
                        &miner,
                        serde_json::to_string(&next_block_reward).unwrap()
                    );
                    
        let state_hash = network_state.lock().unwrap().hash(&now.as_nanos().to_ne_bytes());

        // Ensure that the claim is mature
        if claim.maturation_time <= now.as_nanos() {

            // If the claim is mature, initialize a vector with the capacity to hold the new claims that will be created
            // by this new block being mined.
            let mut visible_blocks: Vec<Claim> = Vec::with_capacity(20);

            // Get the claim with the highest maturity timestamp that current exists.
            let mut furthest_visible_block: u128 = account_state.lock().unwrap().clone().claim_state.furthest_visible_block;
            
            let highest_claim_number: u128 = last_block.visible_blocks
                .iter()
                .map(|x| x.claim_number).max_by(|a, b| a.cmp(b)).unwrap();

            // Generate 20 new claims, increment each one's maturity timestamp by 5 nanoseconds
            // and push them to the visible_blocks vector.
            // TODO: Change this to 1 second, this is only for testing purposes.
            for _ in 0..20 {
                furthest_visible_block += 5;
                visible_blocks.push(Claim::new(furthest_visible_block, highest_claim_number));
                account_state.lock().unwrap().claim_state.furthest_visible_block = furthest_visible_block;
            }

            account_state.lock().unwrap().claim_state.furthest_visible_block = furthest_visible_block;

            // Verify that the claim is indeed owned by the miner attempting to mine this block.
            // Verify returns a result with either a boolean (true or false, but always true if Ok())
            // or an error.
            match WalletAccount::verify(
                claim
                .clone()
                .claim_payload.unwrap(), 
                claim_signature, 
                PublicKey::from_str(&&pubkey).unwrap(),
            ) {
                // if it is indeed owned by the miner attempting to mine this block
                Ok(_t) => {
                    // generate the new block and assign it to a variable new_block 
                    let new_block = Block {

                        block_height: last_block.block_height + 1,

                        // set the timestamp value as now (in nanoseconds)
                        timestamp: now.as_nanos(),
                        
                        // set the last_block_hash value to the block hash of the previous block.
                        last_block_hash: last_block.block_hash,

                        // Set the data value to data passed into this method in the signature
                        data,

                        // Set the claim to the claim that entitled the miner to mine this block
                        claim,
                        
                        // Set the block reward to the previous block reward but with the miner value 
                        // as Some() with the miner's wallet address inside.
                        block_reward: Reward { miner: Some(
                            miner.clone()
                        ), ..last_block.next_block_reward },

                        // Set the block hash to the resulting hash of the block payload string as bytes.
                        block_hash: digest_bytes(block_payload.as_bytes()),

                        // Generate a new reward for the next block.
                        next_block_reward: Reward::new(None, reward_state),
                        
                        block_payload,

                        // Set the miner to the miner wallet address.
                        miner: miner.clone(),

                        // Set the block signature to a string of the claim signature.
                        block_signature: claim_signature.to_string(),

                        state_hash,

                        // Set the visible blocks to the vector of new claims generated earlier in this method.
                        visible_blocks,

                    };

                    // Generate an updated account state by calling the .update() method
                    // on the account state that was passed through the function signature
                    // this returns a result with either an AccountState struct (the updated account state)
                    // if successful, or an error if not successful. Assign this to a variable to be
                    // included in the return expression.
                    account_state.lock().unwrap().update(Miner(miner, serde_json::to_string(&new_block).unwrap()), network_state);

                    // Return a Some() option variant with an Ok() result variant that wraps a tuple that contains
                    // the new block that was just mined and the updated account state.
                    return Some(Ok(new_block));
                },

                // If the claim is not valid then return a unit struct
                Err(_e) => ()
            }
        }

        // If the claim is not mature then return None.
        None   
    }

    pub fn as_bytes(&self) -> Vec<u8> {

        let as_string = serde_json::to_string(self).unwrap();

        as_string.as_bytes().iter().copied().collect()

    }

    pub fn from_bytes(data: &[u8]) -> Block {

        let mut buffer: Vec<u8> = vec![];

        data.iter().for_each(|x| buffer.push(*x));

        let to_string = String::from_utf8(buffer).unwrap();

        serde_json::from_str::<Block>(&to_string).unwrap()

    }
}

pub fn data_validator(data: HashMap<String, Txn>, account_state: AccountState) -> Option<bool> {
    for (id, _txn) in data {
        let valid = account_state.mineable.get(&id);

        match valid {
            Some((_txn, validator_vec)) => {
                let num_invalid = validator_vec
                    .iter()
                    .filter(|&validator| !validator.to_owned().valid)
                    .count();
                
                let len_of_validators = validator_vec.len();

                if len_of_validators < 3 {
                    return Some(false);
                }

                if num_invalid as f32 / len_of_validators as f32 > 1.0 / 3.0 {
                    return Some(false);
                } else {
                    println!("Txn is valid")
                }
            },
            None => { return Some(false) }
        }
    }

    Some(true)
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Block(\n \
            reward: {:?},\n \
            next_block_reward: {:?}\n \
            claim: {:?}",
            self.block_reward,
            self.next_block_reward,
            self.claim,
        )
    }
}

impl Verifiable for Block {
    fn is_valid(&self, options: Option<ValidatorOptions>) -> Option<bool> {
        match options {
            Some(block_options) => {
                match block_options {
                    ValidatorOptions::NewBlock(last_block, block, pubkey, account_state, reward_state, network_state) => {
                        
                        let block = serde_json::from_str::<Block>(&block).unwrap();
                        let last_block = serde_json::from_str::<Block>(&last_block).unwrap();
                        let network_state = serde_json::from_str::<NetworkState>(&network_state).unwrap();
                        let account_state = serde_json::from_str::<AccountState>(&account_state).unwrap();
                        let reward_state = serde_json::from_str::<RewardState>(&reward_state).unwrap();

                        let valid_signature = match block.clone().claim.current_owner.2 {
                            Some(sig) => {
                                let pubkey = match PublicKey::from_str(&pubkey) {
                                    Ok(pk) => {pk}
                                    Err(_e) => { println!("Invalid Public Key"); return Some(false) }
                                };
                                let signature = match Signature::from_str(&sig) {
                                    Ok(sig) => sig,
                                    Err(_e) => {println!("Invalid Signature Structure"); return Some(false)}
                                };

                                block.claim.verify(&signature, &pubkey)
                            },
                            None => {
                                println!("Signature verification returned None");
                                return Some(false)
                            }
                        };

                        match valid_signature {
                            Ok(true) => {},
                            Ok(false) => { println!("Invalid Signature"); return Some(false) },
                            Err(e) => { 
                                println!("Signature validation returned error: {}", e);
                                return Some(false) 
                            }
                        }

                        if block.last_block_hash != last_block.block_hash {
                            println!("Invalid last block hash");
                            return Some(false)
                        }
                        
                        if block.block_hash != digest_bytes(block.block_payload.as_bytes()) {
                            println!("Invalid block hash");
                            return Some(false)
                        }

                        let state_hash = network_state.hash(&block.timestamp.to_ne_bytes());
                        println!("{}", &state_hash);
                        if block.state_hash != state_hash {
                            println!("Invalid state hash");
                            return Some(false);
                        }
                        
                        let account_state_claim = {
                            if let Some(claim) = account_state.claim_state.owned_claims.get(
                                &block.claim.maturation_time
                            ) 
                            {
                                claim.to_owned()
                            } else {
                                println!("unable to find block claim");
                                return Some(false)
                            }
                        };

                        if account_state_claim != block.claim {
                            println!("invalid block claim");
                            return Some(false)
                        }

                        if last_block.next_block_reward.amount != block.block_reward.amount {
                            println!("invalid block reward doesn't match last block reward");
                            return Some(false)
                        }

                        match reward::valid_reward(block.block_reward.category, reward_state) {
                            Some(false) => {
                                println!("invalid block reward");
                                return Some(false)
                            },
                            None => {println!("reward validation returned None"); return Some(false)},
                            _ => {}
                        }

                        match reward::valid_reward(block.next_block_reward.category, reward_state) {
                            Some(false) => {
                                println!("invalid next block reward");
                                return Some(false)
                            },
                            None => {println!("reward validation returned None"); return Some(false)},
                            _ => {}
                        }

                        match data_validator(block.data, account_state) {
                            Some(false) => { 
                                println!("Invalid Data");
                                return Some(false) 
                            },
                            None => { println!("data validator returned none"); return Some(false) },
                            _ => {}
                        }

                        Some(true)
                    },    
                    _ => panic!("Invalid options for block"),
                }
            },
            None => {
                Some(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_genesis_block_creation() {

    }

    #[test]
    fn test_mine_block_with_immature_claim() {

    }

    #[test]
    fn test_mine_block_with_invalid_claim_signature() {

    }

    #[test]
    fn test_mine_block_with_unconfirmed_txns() {

    }

    #[test]
    fn test_mine_block_with_invalid_miner_signature() {

    }

    #[test]
    fn test_mine_block_with_invalid_reward() {

    }

    #[test]
    fn test_mine_block_with_all_valid_data() {

    }
}