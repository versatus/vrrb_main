use std::collections::HashMap;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::state::NetworkState;
use crate::{
    account::{WalletAccount, AccountState, StateOption}, 
    claim::{Claim, ClaimState}, 
    txn::Txn, 
    reward::{RewardState, Reward},
};
use secp256k1::{Signature};
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;
use std::io::Error;
use std::fmt;
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
    pub visible_blocks: Vec<Claim>,
}

impl Block {
    pub fn genesis(
        reward_state: RewardState, 
        miner: &mut WalletAccount, 
        account_state: &mut AccountState,
        network_state: &mut NetworkState,
    ) -> Result<(Block, AccountState), Error> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let mut visible_blocks: Vec<Claim> = Vec::with_capacity(20);
        let mut next_time = now.as_nanos();
        for _ in 0..20 {
            visible_blocks.push(Claim::new(next_time));
            next_time = next_time + 5;
        }
        let genesis = Block {
            timestamp: now.as_nanos(),
            last_block_hash: digest_bytes("Genesis_Last_Block_Hash".to_string().as_bytes()),
            data: HashMap::new(),
            claim: Claim::new(now.as_nanos()),
            block_reward: Reward::genesis(Some(miner.address.clone())),
            block_signature: "Genesis_Signature".to_string(),
            block_hash: digest_bytes("Genesis_Block_Hash".to_string().as_bytes()),
            next_block_reward: Reward::new(None, &reward_state),
            miner: miner.address.clone(),
            visible_blocks,
        };

        let updated_account_state = account_state
                                                    .update(StateOption::Miner((miner.clone(), 
                                                        genesis.clone())), network_state)
                                                    .unwrap();

        Ok((genesis, updated_account_state))

    }
    pub fn mine(
        reward_state: &RewardState,
        claim: Claim,
        last_block: Block,
        data: HashMap<String, Txn>,
        miner: &mut WalletAccount,
        account_state: &mut AccountState,
        claim_state: &ClaimState,
        network_state: &mut NetworkState,
    ) -> Option<Result<(Block, AccountState), Error>> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let claim_signature: Signature = Signature::from_str(
            &claim.clone()
                .current_owner.2
                .unwrap())
                .ok()
                .unwrap();
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
            let mut visible_blocks: Vec<Claim> = Vec::with_capacity(20);
            let mut furthest_visible_block: u128 = Block::get_furthest_visible_block(&mut claim_state.clone());
            for _ in 0..=20 {
                furthest_visible_block += 5;
                visible_blocks.push(Claim::new(furthest_visible_block));
            }
            match WalletAccount::verify(
                claim
                .clone()
                .claim_payload.unwrap(), 
                claim_signature, 
                miner.public_key.clone()
            ) {
                Ok(_t) => {
                    let new_block = Block {
                        timestamp: now.as_nanos(),
                        last_block_hash: last_block.block_hash,
                        data: data,
                        claim: claim
                                .clone()
                                .to_owned(),
                        block_reward: Reward { miner: Some(
                            miner.address.clone()
                        ), ..last_block.next_block_reward },
                        block_hash: digest_bytes(block_payload.as_bytes()),
                        next_block_reward: Reward::new(None, reward_state),
                        miner: miner.address.clone(),
                        block_signature: claim_signature.to_string(),
                        visible_blocks,

                    };
                    let updated_account_state = account_state
                                                    .update(StateOption::Miner((miner.clone(), 
                                                        new_block.clone())), network_state)
                                                    .unwrap();
                    return Some(Ok((new_block, updated_account_state)));
                },
                Err(_e) => ()
            }
        }
        None   
    }

    fn get_furthest_visible_block(claim_state: &mut ClaimState) -> u128 {
        let (key, _value) = claim_state.claims
            .iter_mut()
            .max_by_key(|entry| entry.0)
            .unwrap();
        *key
    }
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

// TODO: Write tests for this module