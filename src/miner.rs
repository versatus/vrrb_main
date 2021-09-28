use crate::block::Block;
use crate::claim::Claim;
use crate::header::BlockHeader;
use crate::pool::{Pool, PoolKind};
use crate::reward::RewardState;
use crate::state::NetworkState;
use crate::txn::Txn;
use ritelinked::LinkedHashMap;
use std::error::Error;
use std::fmt;
use log::info;

#[derive(Debug)]
pub struct NoLowestPointerError(String);

#[derive(Debug)]
pub struct Miner {
    pub claim: Claim,
    pub mining: bool,
    pub claim_map: LinkedHashMap<String, Claim>,
    pub txn_pool: Pool<String, Txn>,
    pub last_block: Option<Block>,
    pub reward_state: RewardState,
    pub network_state: NetworkState,
    pub neighbors: Option<Vec<BlockHeader>>,
    pub init: bool,
}

impl Miner {
    pub fn start(
        pubkey: String,
        address: String,
        reward_state: RewardState,
        network_state: NetworkState,
    ) -> Miner {
        let mut miner = Miner {
            claim: Claim::new(pubkey.clone(), address, 1),
            mining: false,
            claim_map: LinkedHashMap::new(),
            txn_pool: Pool::new(PoolKind::Txn),
            last_block: None,
            reward_state,
            network_state,
            neighbors: None,
            init: false,
        };

        miner.claim_map.insert(pubkey.clone(), miner.claim.clone());

        miner
    }

    pub fn get_lowest_pointer(&mut self, nonce: u128) -> Option<(String, u128)> {
        let mut pointers = self
            .claim_map
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
        if let Some((hash, _)) = self.get_lowest_pointer(nonce) {
            return Ok(hash == self.claim.hash);
        } else {
            Err(
                Box::new(
                    NoLowestPointerError("There is no valid pointer, all claims in claim map must increment their nonce by 1".to_string())
                )
            )
        }
    }

    pub fn genesis(&self) -> Option<Block> {
        if self.mining {
            Block::genesis(&self.reward_state.clone(), self.claim.clone())
        } else {
            None
        }
    }

    pub fn mine(&mut self) -> Option<Block> {
        if !self.mining || !self.init {
            return None
        }

        if let Some(last_block) = self.last_block.clone() {
            match self.check_my_claim(last_block.header.next_block_nonce as u128) {
                Ok(true) => {
                    let block = Block::mine(
                        self.claim.clone(),
                        last_block.clone(),
                        self.txn_pool.confirmed.clone(),
                        &self.reward_state.clone(),
                        &self.network_state.clone(),
                        self.neighbors.clone(),
                    );

                    info!(target: "mine_block", "Discovered a new block");

                    return block;
                }
                Ok(false) => { // Nothing to do here, wait for other miner to propose block
                    info!(target: "mine_block", "not the lowest pointer, wait for block to be proposed");
                    self.mining = false;
                }
                Err(_) => {
                    info!(target: "mine_block", "no lowest pointer");
                    self.nonce_up();
                }
            }
        }

        None
    }

    pub fn nonce_up(&mut self) {
        let mut new_claim_map = LinkedHashMap::new();
        self.claim_map.clone().iter().for_each(|(pk, claim)| {
            let mut new_claim = claim.clone();
            // TODO If new_claim is exhausted don't nonce up, if not
            // nonce it up.
            new_claim.nonce_up();
            new_claim_map.insert(pk.clone(), new_claim.clone());
        });
        self.claim_map = new_claim_map;
        self.claim.nonce_up();
    }

    pub fn process_txn(&mut self, txn: Txn) {
        if let Some(txn) = self.txn_pool.confirmed.get(&txn.txn_id) {
            // Nothing really to do here
        } else if let Some(txn) = self.txn_pool.pending.get(&txn.txn_id) {
            // add validator
        } else {
            self.txn_pool.pending.insert(txn.txn_id.clone(), txn);
        }
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
