// use secp256k1::{key::PublicKey, Signature};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
// use std::fmt;
use crate::{account::WalletAccount, claim::Claim, txn::Txn};
use secp256k1::{key::PublicKey, Signature};
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub timestamp: u128,
    pub last_block_hash: String,
    pub data: HashMap<String, Txn>,
    pub claim: Claim,
    pub block_reward: (String, i128),
    pub block_signature: String,
    pub block_hash: String,
    pub next_block_reward: (String, i128),
    pub miner: String,
}

impl Block {
    pub fn genesis() -> Block {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        Block {
            timestamp: now.as_nanos(),
            last_block_hash: "Genesis_Block_Hash".to_string(),
            data: HashMap::new(),
            claim: Claim::new(now.as_nanos()),
            block_reward: ("Genesis Reward".to_string(), 500000000000000000i128),
            block_signature: "Genesis_Signature".to_string(),
            block_hash: digest_bytes("Genesis_Hash".to_string().as_bytes()),
            next_block_reward: ("Motherload".to_string(), 50000000000000i128),
            miner: "Genesis_Miner".to_string(),
        }
    }
    pub fn mine(
        claim: Claim,
        last_block: Block,
        data: HashMap<String, Txn>,
        miner: String,
    ) -> Option<Block> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let mut block_payload = String::new();
        block_payload.push_str("block_id");
        let claim_signature: Signature = Signature::from_str(&claim.clone().current_owner.2.unwrap()).ok().unwrap();
        println!("{:?}", claim_signature);
        let public_key = PublicKey::from_str(&miner).ok().unwrap();
        if claim.maturation_time <= now.as_nanos() {
            match WalletAccount::verify(claim.clone().claim_payload.unwrap(), claim_signature, public_key) {
                Ok(_t) => {
                    return Some(Block {
                        timestamp: now.as_nanos(),
                        last_block_hash: last_block.block_hash,
                        data: data,
                        claim: claim.clone().to_owned(),
                        block_reward: last_block.next_block_reward,
                        block_hash: digest_bytes(block_payload.as_bytes()),
                        next_block_reward: ("Motherload".to_string(), 50000000000000i128),
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
