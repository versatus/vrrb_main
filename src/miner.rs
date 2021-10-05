use crate::block::Block;
use crate::claim::Claim;
use crate::header::BlockHeader;
use crate::pool::{Pool, PoolKind};
use crate::reward::RewardState;
use crate::state::NetworkState;
use crate::txn::Txn;
use crate::validator::TxnValidator;
use crate::verifiable::Verifiable;
use ritelinked::LinkedHashMap;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct NoLowestPointerError(String);

#[derive(Debug, Clone)]
pub struct Miner {
    pub claim: Claim,
    pub mining: bool,
    pub claim_map: LinkedHashMap<String, Claim>,
    pub txn_pool: Pool<String, Txn>,
    pub claim_pool: Pool<String, Claim>,
    pub last_block: Option<Block>,
    pub reward_state: RewardState,
    pub network_state: NetworkState,
    pub neighbors: Option<Vec<BlockHeader>>,
    pub current_nonce_counter: u128,
    pub n_miners: u128,
    pub init: bool,
}

impl Miner {
    pub fn start(
        pubkey: String,
        address: String,
        reward_state: RewardState,
        network_state: NetworkState,
        n_miners: u128,
    ) -> Miner {
        let miner = Miner {
            claim: Claim::new(pubkey.clone(), address, 1),
            mining: false,
            claim_map: LinkedHashMap::new(),
            txn_pool: Pool::new(PoolKind::Txn),
            claim_pool: Pool::new(PoolKind::Claim),
            last_block: None,
            reward_state,
            network_state,
            neighbors: None,
            current_nonce_counter: 0,
            n_miners,
            init: false,
        };

        miner
    }

    pub fn get_lowest_pointer(&mut self, nonce: u128) -> Option<(String, u128)> {
        let claim_map = self.claim_map.clone();
        let mut pointers = claim_map
            .iter()
            .map(|(_, claim)| return (claim.clone().hash, claim.clone().get_pointer(nonce)))
            .collect::<Vec<_>>();

        pointers.retain(|(_, v)| !v.is_none());

        let mut raw_pointers = pointers
            .iter()
            .map(|(k, v)| {
                return (k.clone(), v.unwrap());
            })
            .collect::<Vec<_>>();

        if let Some(min) = raw_pointers.clone().iter().min_by_key(|(_, v)| v) {
            raw_pointers.retain(|(_, v)| *v == min.1);
            Some(raw_pointers[0].clone())
        } else {
            None
        }
    }

    pub fn check_my_claim(&mut self, nonce: u128) -> Result<bool, Box<dyn Error>> {
        if let Some((hash, _)) = self.clone().get_lowest_pointer(nonce) {
            return Ok(hash == self.clone().claim.hash);
        } else {
            Err(
                Box::new(
                    NoLowestPointerError("There is no valid pointer, all claims in claim map must increment their nonce by 1".to_string())
                )
            )
        }
    }

    pub fn genesis(&mut self) -> Option<Block> {
        self.claim.eligible = true;
        self.claim_map
            .insert(self.claim.pubkey.clone(), self.claim.clone());
        Block::genesis(&self.reward_state.clone(), self.claim.clone())
    }

    pub fn mine(&mut self) -> Option<Block> {
        if let Some(last_block) = self.last_block.clone() {
            return Block::mine(
                self.clone().claim,
                last_block.clone(),
                self.clone().txn_pool.confirmed.clone(),
                self.clone().claim_pool.confirmed.clone(),
                &self.clone().reward_state.clone(),
                &self.clone().network_state.clone(),
                self.clone().neighbors.clone(),
            )
        }

        None
    }

    pub fn nonce_up(&mut self) {
        let mut new_claim_map = LinkedHashMap::new();
        self.claim_map.clone().iter().for_each(|(pk, claim)| {
            let mut new_claim = claim.clone();
            new_claim.nonce_up();
            new_claim_map.insert(pk.clone(), new_claim.clone());
        });
        self.claim_map = new_claim_map;
    }

    pub fn process_txn(&mut self, mut txn: Txn) -> TxnValidator {
        if let Some(_txn) = self.txn_pool.confirmed.get(&txn.txn_id) {
            // Nothing really to do here
        } else if let Some(txn) = self.txn_pool.pending.get(&txn.txn_id) {
            // add validator if you have not validated already
            if let None = txn.validators.clone().get(&self.claim.pubkey) {
                let mut txn = txn.clone();
                txn.validators.insert(
                    self.claim.pubkey.clone(),
                    txn.valid_txn(&self.network_state, &self.txn_pool),
                );
                self.txn_pool
                    .pending
                    .insert(txn.txn_id.clone(), txn.clone());
            }
        } else {
            // add validator
            txn.validators.insert(
                self.claim.pubkey.clone(),
                txn.valid_txn(&self.network_state, &self.txn_pool),
            );
            self.txn_pool
                .pending
                .insert(txn.txn_id.clone(), txn.clone());
        }

        return TxnValidator::new(
            self.claim.pubkey.clone(),
            txn.clone(),
            &self.network_state,
            &self.txn_pool,
        );
    }

    pub fn process_txn_validator(&mut self, txn_validator: TxnValidator) {
        if let Some(_txn) = self.txn_pool.confirmed.get(&txn_validator.txn.txn_id) {
        } else if let Some(txn) = self.txn_pool.pending.get_mut(&txn_validator.txn.txn_id) {
            txn.validators
                .entry(txn_validator.pubkey)
                .or_insert(txn_validator.vote);
        } else {
            let mut txn = txn_validator.txn.clone();
            txn.validators
                .insert(txn_validator.pubkey, txn_validator.vote);
            self.txn_pool.pending.insert(txn.txn_id.clone(), txn);
        }
    }

    pub fn check_confirmed(&mut self, txn_id: String) {
        let mut validators = {
            if let Some(txn) = self.txn_pool.pending.get(&txn_id) {
                txn.validators.clone()
            } else {
                HashMap::new()
            }
        };

        validators.retain(|_, v| *v);
        if validators.len() > self.claim_map.len() / 3 {
            if let Some((k, v)) = self.txn_pool.pending.remove_entry(&txn_id) {
                self.txn_pool.confirmed.insert(k, v);
            }
        }
    }
    pub fn abandoned_claim(&mut self, hash: String) {
        self.claim_map.retain(|_, v| {
            v.hash != hash
        });
    }
}

impl fmt::Display for NoLowestPointerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for NoLowestPointerError {
    fn description(&self) -> &str {
        &self.0
    }
}
