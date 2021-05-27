use std::collections::HashMap;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::{account::WalletAccount, claim::Claim, txn::Txn, reward::{RewardState, Reward}};
use secp256k1::{key::PublicKey, Signature};
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub timestamp: u128,
    pub last_block_hash: String,
    pub data: HashMap<String, Txn>,
    pub claim: Claim,
    pub block_reward: Reward,
    pub block_signature: String,
    pub block_hash: String,
    pub next_block_reward: Reward,
    pub miner: String,
}

impl Block {
    pub fn genesis(reward_state: &RewardState, miner: Option<String>) -> Block {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        Block {
            timestamp: now.as_nanos(),
            last_block_hash: digest_bytes("Genesis_Last_Block_Hash".to_string().as_bytes()),
            data: HashMap::new(),
            claim: Claim::new(now.as_nanos()),
            block_reward: Reward::new(miner.clone(), reward_state),
            block_signature: "Genesis_Signature".to_string(),
            block_hash: digest_bytes("Genesis_Block_Hash".to_string().as_bytes()),
            next_block_reward: Reward::new(miner.clone(), reward_state),
            miner: miner.clone().unwrap()
        }
    }
    pub fn mine(
        reward_state: &RewardState,
        claim: Claim,
        last_block: Block,
        data: HashMap<String, Txn>,
        miner: String,
    ) -> Option<Block> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let claim_signature: Signature = Signature::from_str(
            &claim.clone()
                .current_owner.2
                .unwrap())
                .ok()
                .unwrap();
        let public_key = PublicKey::from_str(&miner)
                                        .ok().unwrap();
        let next_block_reward = Reward::new(None, reward_state);
        let block_payload = format!("{},{},{},{},{},{},{}", 
                        now.as_nanos().to_string(), 
                        last_block.block_hash, 
                        serde_json::to_string(&data).unwrap(), 
                        serde_json::to_string(&claim).unwrap(),
                        serde_json::to_string(&last_block.next_block_reward.clone()).unwrap(),
                        miner.clone(), 
                        serde_json::to_string(&next_block_reward.clone()).unwrap()
                    );
        if claim.maturation_time <= now.as_nanos() {
            match WalletAccount::verify(
                claim
                .clone()
                .claim_payload.unwrap(), 
                claim_signature, 
                public_key
            ) {
                Ok(_t) => {
                    return Some(Block {
                        timestamp: now.as_nanos(),
                        last_block_hash: last_block.block_hash,
                        data: data,
                        claim: claim
                                .clone()
                                .to_owned(),
                        block_reward: Reward { miner: Some(miner.clone()), ..last_block.next_block_reward },
                        block_hash: digest_bytes(block_payload.as_bytes()),
                        next_block_reward: Reward::new(None, reward_state),
                        miner: miner,
                        block_signature: claim_signature.to_string(),
                    })
                },
                Err(e) => println!("Claim is not valid {}", e),
            }
        }
        None
    }
}
