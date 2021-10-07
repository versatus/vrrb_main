use crate::blockchain::{InvalidBlockError, InvalidBlockErrorReason};
use crate::header::BlockHeader;
use crate::network::chunkable::Chunkable;
use crate::network::node::MAX_TRANSMIT_SIZE;
use crate::state::NetworkState;
use crate::verifiable::Verifiable;
use crate::{claim::Claim, reward::RewardState, txn::Txn};
use ritelinked::LinkedHashMap;
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;
use std::fmt;

pub const NANO: u128 = 1;
pub const MICRO: u128 = NANO * 1000;
pub const MILLI: u128 = MICRO * 1000;
pub const SECOND: u128 = MILLI * 1000;

const VALIDATOR_THRESHOLD: f64 = 0.60;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct Block {
    pub header: BlockHeader,
    pub neighbors: Option<Vec<BlockHeader>>,
    pub height: u128,
    pub txns: LinkedHashMap<String, Txn>,
    pub claims: LinkedHashMap<String, Claim>,
    pub hash: String,
    pub received_at: Option<u128>,
    pub received_from: Option<String>,
    pub abandoned_claim: Option<Claim>,
}

impl Block {
    // Returns a result with either a tuple containing the genesis block and the
    // updated account state (if successful) or an error (if unsuccessful)
    pub fn genesis(reward_state: &RewardState, claim: Claim) -> Option<Block> {
        let header = BlockHeader::genesis(0, reward_state, claim.clone());
        let state_hash = digest_bytes(
            format!(
                "{},{}",
                header.last_hash,
                digest_bytes("Genesis_State_Hash".as_bytes())
            )
            .as_bytes(),
        );

        let mut claims = LinkedHashMap::new();
        claims.insert(claim.clone().pubkey.clone(), claim);

        let genesis = Block {
            header,
            neighbors: None,
            height: 0,
            txns: LinkedHashMap::new(),
            claims,
            hash: state_hash,
            received_at: None,
            received_from: None,
            abandoned_claim: None,
        };

        // Update the account state with the miner and new block, this will also set the values to the
        // network state. Unwrap the result and assign it to the variable updated_account_state to
        // be returned by this method.

        Some(genesis)
    }

    /// The mine method is used to generate a new block (and an updated account state with the reward set
    /// to the miner wallet's balance), this will also update the network state with a new confirmed state.
    pub fn mine(
        claim: Claim,      // The claim entitling the miner to mine the block.
        last_block: Block, // The last block, which contains the current block reward.
        txns: LinkedHashMap<String, Txn>,
        claims: LinkedHashMap<String, Claim>,
        claim_map_hash: Option<String>,
        reward_state: &RewardState,
        network_state: &NetworkState,
        neighbors: Option<Vec<BlockHeader>>,
        abandoned_claim: Option<Claim>,
    ) -> Option<Block> {
        let txn_hash = {
            let mut txn_vec = vec![];
            txns.iter().for_each(|(_, v)| {
                txn_vec.extend(v.as_bytes());
            });
            digest_bytes(&txn_vec)
        };

        let header = BlockHeader::new(last_block.clone(), reward_state, claim, txn_hash, claim_map_hash);
        let height = last_block.height.clone() + 1;
        if let Some(time) = header.timestamp.checked_sub(last_block.header.timestamp) {
            if time / SECOND < 1 {
                return None 
            }
        } else {
            return None
        }
        let mut block = Block {
            header: header.clone(),
            neighbors,
            height,
            txns,
            claims,
            hash: header.last_hash.clone(),
            received_at: None,
            received_from: None,
            abandoned_claim,
        };

        let mut hashable_state = network_state.clone();

        let hash = hashable_state.hash(block.clone());
        block.hash = hash;
        Some(block)
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

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Block(\n \
            header: {:?},\n",
            self.header
        )
    }
}

impl Verifiable for Block {
    fn verifiable(&self) -> bool {
        true
    }

    fn valid_genesis(&self, _network_state: &NetworkState, _reward_state: &RewardState) -> bool {
        true
    }

    fn valid_block(
        &self,
        last_block: &Block,
        network_state: &NetworkState,
        reward_state: &RewardState,
    ) -> Result<(), InvalidBlockError> {
        if !self.valid_last_hash(last_block) {
            return Err(InvalidBlockError {
                details: InvalidBlockErrorReason::InvalidLastHash,
            });
        }

        if !self.valid_block_nonce(last_block) {
            return Err(InvalidBlockError {
                details: InvalidBlockErrorReason::InvalidBlockNonce,
            });
        }

        if !self.valid_state_hash(network_state) {
            return Err(InvalidBlockError {
                details: InvalidBlockErrorReason::InvalidStateHash,
            });
        }

        if !self.valid_block_reward(reward_state) {
            return Err(InvalidBlockError {
                details: InvalidBlockErrorReason::InvalidBlockReward,
            });
        }

        if !self.valid_next_block_reward(reward_state) {
            return Err(InvalidBlockError {
                details: InvalidBlockErrorReason::InvalidBlockReward,
            });
        }

        if !self.valid_txns() {
            return Err(InvalidBlockError {
                details: InvalidBlockErrorReason::InvalidTxns,
            });
        }

        Ok(())
    }

    fn valid_last_hash(&self, last_block: &Block) -> bool {
        self.header.last_hash == last_block.hash
    }

    fn valid_state_hash(&self, network_state: &NetworkState) -> bool {
        let mut hashable_state = network_state.clone();
        let hash = hashable_state.hash(self.clone());
        self.hash == hash
    }

    fn valid_block_reward(&self, reward_state: &RewardState) -> bool {
        if let Some(true) = reward_state.valid_reward(self.header.block_reward.category) {
            return true;
        }

        false
    }

    fn valid_next_block_reward(&self, reward_state: &RewardState) -> bool {
        if let Some(true) = reward_state.valid_reward(self.header.next_block_reward.category) {
            return true;
        }

        false
    }

    fn valid_txns(&self) -> bool {
        let mut valid_data: bool = true;

        self.txns.iter().for_each(|(_, txn)| {
            let n_valid = txn.validators.iter().filter(|(_, &valid)| valid).count();
            if (n_valid as f64 / txn.validators.len() as f64) < VALIDATOR_THRESHOLD {
                valid_data = false
            }
        });

        valid_data
    }

    fn valid_block_nonce(&self, last_block: &Block) -> bool {
        self.header.block_nonce == last_block.header.next_block_nonce
    }
}

impl Chunkable for Block {
    fn chunk(&self) -> Option<Vec<Vec<u8>>> {
        let bytes_len = self.as_bytes().len();
        if bytes_len > MAX_TRANSMIT_SIZE {
            let mut n_chunks = bytes_len / MAX_TRANSMIT_SIZE;
            if bytes_len % MAX_TRANSMIT_SIZE != 0 {
                n_chunks += 1;
            }
            let mut chunks_vec = vec![];
            let mut last_slice_end = 0;
            (1..=bytes_len)
                .map(|n| n * (MAX_TRANSMIT_SIZE))
                .enumerate()
                .for_each(|(index, slice_end)| {
                    if index + 1 == n_chunks {
                        chunks_vec.push(self.clone().as_bytes()[last_slice_end..].to_vec());
                    } else {
                        chunks_vec
                            .push(self.clone().as_bytes()[last_slice_end..slice_end].to_vec());
                        last_slice_end = slice_end;
                    }
                });
            Some(chunks_vec)
        } else {
            Some(vec![self.clone().as_bytes()])
        }
    }
}
