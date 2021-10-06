use crate::block::Block;
use crate::claim::Claim;
use crate::reward::{Reward, RewardState};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;
use std::time::{SystemTime, UNIX_EPOCH};
use std::u32::MAX as u32MAX;
use std::u64::MAX as u64MAX;
pub const NANO: u128 = 1;
pub const MICRO: u128 = NANO * 1000;
pub const MILLI: u128 = MICRO * 1000;
pub const SECOND: u128 = MILLI * 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub last_hash: String,
    pub block_nonce: u64,
    pub next_block_nonce: u64,
    pub block_height: u128,
    pub timestamp: u128,
    pub txn_hash: String,
    pub claim: Claim,
    pub claim_map_hash: Option<String>,
    pub block_reward: Reward,
    pub next_block_reward: Reward,
    pub neighbor_hash: Option<String>,
}

impl BlockHeader {
    pub fn genesis(nonce: u64, reward_state: &RewardState, claim: Claim) -> BlockHeader {
        let mut rng = rand::thread_rng();
        let last_hash = digest_bytes("Genesis_Last_Hash".as_bytes());
        let block_nonce = nonce;
        let next_block_nonce: u64 = rng.gen_range(u32MAX as u64, u64MAX);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let txn_hash = digest_bytes("Genesis_Txn_Hash".as_bytes());
        let block_reward = Reward::genesis(Some(claim.address.clone()));
        let next_block_reward = Reward::new(None, reward_state);

        BlockHeader {
            last_hash,
            block_nonce,
            next_block_nonce,
            block_height: 0,
            timestamp,
            txn_hash,
            claim,
            claim_map_hash: None,
            block_reward,
            next_block_reward,
            neighbor_hash: None,
        }
    }

    pub fn new(
        last_block: Block,
        reward_state: &RewardState,
        claim: Claim,
        txn_hash: String,
        claim_map_hash: Option<String>,
    ) -> BlockHeader {
        let mut rng = rand::thread_rng();
        let last_hash = last_block.hash;
        let block_nonce = last_block.header.next_block_nonce.clone();
        let next_block_nonce: u64 = rng.gen_range(0, u64MAX);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut block_reward = last_block.header.next_block_reward;
        block_reward.miner = Some(claim.clone().address);
        let next_block_reward = Reward::new(None, reward_state);

        BlockHeader {
            last_hash,
            block_nonce,
            next_block_nonce,
            block_height: last_block.header.block_height + 1,
            timestamp,
            txn_hash,
            claim,
            claim_map_hash,
            block_reward,
            next_block_reward,
            neighbor_hash: None,
        }
    }
}
