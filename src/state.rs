use crate::network::chunkable::Chunkable;
use crate::network::node::MAX_TRANSMIT_SIZE;
use crate::pool::Pool;
use crate::txn::Txn;
use crate::{block::Block, claim::Claim, reward::RewardState};
use log::info;
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use ritelinked::LinkedHashMap;
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Ledger {
    pub credits: LinkedHashMap<String, u128>,
    pub debits: LinkedHashMap<String, u128>,
    pub claims: LinkedHashMap<String, Claim>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Components {
    pub genesis: Option<Vec<u8>>,
    pub child: Option<Vec<u8>>,
    pub parent: Option<Vec<u8>>,
    pub blockchain: Option<Vec<u8>>,
    pub ledger: Option<Vec<u8>>,
    pub network_state: Option<Vec<u8>>,
    pub archive: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkState {
    // Path to database
    pub path: String,
    // hash of the state of credits in the network
    pub credits: Option<String>,
    // hash of the state of debits in the network
    pub debits: Option<String>,
    //reward state of the network
    pub reward_state: RewardState,
    // the last state hash -> sha256 hash of credits, debits & reward state.
    pub state_hash: Option<String>,
}

impl NetworkState {
    pub fn restore(path: &str) -> NetworkState {
        let db = match PickleDb::load_bin(path, PickleDbDumpPolicy::DumpUponRequest) {
            Ok(nst) => nst,
            Err(_) => PickleDb::new(
                path,
                PickleDbDumpPolicy::DumpUponRequest,
                SerializationMethod::Bin,
            ),
        };

        let (credits_map, debits_map, reward_state, _claims) =
            NetworkState::restore_state_objects(&db);

        let credits = digest_bytes(format!("{:?}", &credits_map).as_bytes());
        let debits = digest_bytes(format!("{:?}", &credits_map).as_bytes());

        NetworkState {
            path: path.to_string(),
            credits: {
                if credits_map.is_empty() {
                    None
                } else {
                    Some(credits)
                }
            },
            debits: {
                if debits_map.is_empty() {
                    None
                } else {
                    Some(debits)
                }
            },
            reward_state,
            state_hash: None,
        }
    }

    pub fn get_balance(&self, address: &str) -> u128 {
        let credits = self.get_account_credits(address);
        let debits = self.get_account_debits(address);

        if let Some(balance) = credits.checked_sub(debits) {
            return balance;
        } else {
            return 0u128;
        }
    }

    pub fn credit_hash(self, block: &Block) -> String {
        let mut credits = LinkedHashMap::new();

        block.txns.iter().for_each(|(_txn_id, txn)| {
            if let Some(entry) = credits.get_mut(&txn.receiver_address) {
                *entry += txn.clone().txn_amount
            } else {
                credits.insert(txn.clone().receiver_address, txn.clone().txn_amount);
            }
        });

        if let Some(entry) = credits.get_mut(&block.header.block_reward.miner.clone().unwrap()) {
            *entry += block.header.block_reward.amount
        } else {
            credits.insert(
                block.header.block_reward.miner.clone().unwrap(),
                block.header.block_reward.amount,
            );
        }

        if let Some(chs) = self.credits {
            return digest_bytes(format!("{},{:?}", chs, credits).as_bytes());
        } else {
            return digest_bytes(format!("{:?},{:?}", self.credits, credits).as_bytes());
        }
    }

    pub fn debit_hash(self, block: &Block) -> String {
        let mut debits = LinkedHashMap::new();

        block.txns.iter().for_each(|(_txn_id, txn)| {
            if let Some(entry) = debits.get_mut(&txn.sender_address) {
                *entry += txn.clone().txn_amount
            } else {
                debits.insert(txn.clone().sender_address, txn.clone().txn_amount);
            }
        });

        if let Some(dhs) = self.debits {
            return digest_bytes(format!("{},{:?}", dhs, debits).as_bytes());
        } else {
            return digest_bytes(format!("{:?},{:?}", self.debits, debits).as_bytes());
        }
    }

    pub fn hash(&mut self, block: Block) -> String {
        let credit_hash = self.clone().credit_hash(&block);
        let debit_hash = self.clone().debit_hash(&block);
        let reward_state_hash = digest_bytes(format!("{:?}", self.reward_state).as_bytes());
        let payload = format!(
            "{:?},{:?},{:?},{:?}",
            self.state_hash, credit_hash, debit_hash, reward_state_hash
        );
        let new_state_hash = digest_bytes(payload.as_bytes());
        new_state_hash
    }

    pub fn restore_state_objects(
        db: &PickleDb,
    ) -> (
        LinkedHashMap<String, u128>,
        LinkedHashMap<String, u128>,
        RewardState,
        LinkedHashMap<String, Claim>,
    ) {
        let credits: LinkedHashMap<String, u128> = if let Some(map) = db.get("credits") {
            map
        } else {
            LinkedHashMap::new()
        };

        let debits: LinkedHashMap<String, u128> = if let Some(map) = db.get("debits") {
            map
        } else {
            LinkedHashMap::new()
        };

        let reward_state: RewardState = if let Some(reward_state) = db.get("rewardstate") {
            reward_state
        } else {
            RewardState::start()
        };

        let claims: LinkedHashMap<String, Claim> = if let Some(claims) = db.get("claims") {
            claims
        } else {
            LinkedHashMap::new()
        };

        (credits, debits, reward_state, claims)
    }

    pub fn dump(&mut self, block: &Block) {
        let mut db = self.get_ledger_db();
        let (mut credits, mut debits, mut reward_state, mut claims) =
            NetworkState::restore_state_objects(&db);

        block.txns.iter().for_each(|(_txn_id, txn)| {
            if let Some(entry) = credits.get_mut(&txn.receiver_address) {
                *entry += txn.clone().txn_amount
            } else {
                credits.insert(txn.clone().receiver_address, txn.clone().txn_amount);
            }

            if let Some(entry) = debits.get_mut(&txn.clone().sender_address) {
                *entry += txn.txn_amount
            } else {
                debits.insert(txn.clone().sender_address, txn.clone().txn_amount);
            }
        });

        block.claims.iter().for_each(|(k, v)| {
            claims.insert(k.clone(), v.clone());
        });

        claims.insert(
            block.header.claim.clone().pubkey,
            block.header.claim.clone(),
        );

        if let Some(entry) = credits.get_mut(&block.header.block_reward.miner.clone().unwrap()) {
            *entry += block.header.block_reward.amount.clone()
        } else {
            credits.insert(
                block.header.block_reward.miner.clone().unwrap().clone(),
                block.header.block_reward.amount.clone(),
            );
        }

        reward_state.update(block.header.block_reward.category.clone());
        self.update_state_hash(&block);
        self.update_reward_state(&block);
        self.update_credits_and_debits(&block);

        if let Err(_) = db.set("credits", &credits) {
            println!("Error setting credits to state")
        };
        if let Err(_) = db.set("debits", &debits) {
            println!("Error setting debits to state")
        };
        if let Err(_) = db.set("rewardstate", &reward_state) {
            println!("Error setting reward state to state")
        };
        if let Err(_) = db.set("claims", &claims) {
            println!("Error setting claims to state");
        };
        if let Err(e) = db.dump() {
            info!("Error dumping state to file: {:?}", e)
        }
    }

    pub fn nonce_up(&mut self) {
        let mut new_claim_map = LinkedHashMap::new();
        self.get_claims().clone().iter().for_each(|(pk, claim)| {
            let mut new_claim = claim.clone();
            new_claim.nonce_up();
            new_claim_map.insert(pk.clone(), new_claim.clone());
        });
        let mut db = self.get_ledger_db();
        if let Err(e) = db.set("claims", &new_claim_map) {
            println!("Error setting nonced up claims to database: {:?}", e);
        }
    }

    pub fn abandoned_claim(&mut self, hash: String) {
        let mut db = self.get_ledger_db();
        let (_, _, _, mut claims) = NetworkState::restore_state_objects(&db);

        claims.retain(|_, v| v.hash != hash);

        if let Err(_) = db.set("claims", &claims) {
            println!("Error setting claims to state")
        };

        if let Err(e) = db.dump() {
            info!("Error dumping state to file: {:?}", e)
        }
    }

    pub fn get_ledger_db(&self) -> PickleDb {
        match PickleDb::load_bin(self.path.clone(), PickleDbDumpPolicy::DumpUponRequest) {
            Ok(nst) => return nst,
            Err(_) => {
                return PickleDb::new(
                    self.path.clone(),
                    PickleDbDumpPolicy::DumpUponRequest,
                    SerializationMethod::Bin,
                )
            }
        };
    }

    pub fn update_credits_and_debits(&mut self, block: &Block) {
        let chs = self.clone().credit_hash(block);
        let dhs = self.clone().debit_hash(block);
        self.credits = Some(chs);
        self.debits = Some(dhs);
    }

    pub fn update_reward_state(&mut self, block: &Block) {
        self.reward_state.update(block.header.block_reward.category);
    }

    pub fn update_state_hash(&mut self, block: &Block) {
        self.state_hash = Some(block.hash.clone());
    }

    pub fn get_credits(&self) -> LinkedHashMap<String, u128> {
        let db = self.get_ledger_db();
        let credits: LinkedHashMap<String, u128> = if let Some(map) = db.get("credits") {
            map
        } else {
            LinkedHashMap::new()
        };

        credits
    }

    pub fn get_debits(&self) -> LinkedHashMap<String, u128> {
        let db = self.get_ledger_db();
        let debits: LinkedHashMap<String, u128> = if let Some(map) = db.get("debits") {
            map
        } else {
            LinkedHashMap::new()
        };

        debits
    }

    pub fn get_claims(&self) -> LinkedHashMap<String, Claim> {
        let db = self.get_ledger_db();
        let claims: LinkedHashMap<String, Claim> = if let Some(map) = db.get("claims") {
            map
        } else {
            LinkedHashMap::new()
        };

        claims
    }

    pub fn get_reward_state(&self) -> RewardState {
        let db = self.get_ledger_db();
        if let Some(reward_state) = db.get("rewardstate") {
            return reward_state;
        } else {
            RewardState::start()
        }
    }

    pub fn get_last_block(&self) -> Option<Block> {
        let db = self.get_ledger_db();
        if let Some(last_block) = db.get("lastblock") {
            return last_block;
        } else {
            None
        }
    }

    pub fn get_block_archive(&self) -> LinkedHashMap<u128, Block> {
        let db = self.get_ledger_db();
        if let Some(block_archive) = db.get("blockarchive") {
            return block_archive;
        } else {
            return LinkedHashMap::new();
        }
    }

    pub fn get_account_credits(&self, address: &str) -> u128 {
        let credits = self.get_credits();
        if let Some(amount) = credits.get(address) {
            return *amount;
        } else {
            return 0u128;
        }
    }

    pub fn get_account_debits(&self, address: &str) -> u128 {
        let debits = self.get_debits();
        if let Some(amount) = debits.get(address) {
            return *amount;
        } else {
            return 0u128;
        }
    }
    pub fn update_ledger(&mut self, ledger: Ledger, reward_state: RewardState) {
        let mut db = self.get_ledger_db();
        if let Err(_) = db.set("credits", &ledger.credits) {
            println!("Error setting credits to ledger");
        }
        if let Err(_) = db.set("debits", &ledger.debits) {
            println!("Error setting debits to ledger");
        }
        if let Err(_) = db.set("rewardstate", &reward_state) {
            println!("Error setting reward state to ledger");
        }
        if let Err(_) = db.set("claims", &ledger.claims) {
            println!("Error setting claims to ledger");
        }
        if let Err(_) = db.dump() {
            info!("Error dumping ledger to db");
        }
    }

    pub fn get_lowest_pointer(&self, nonce: u128) -> Option<(String, u128)> {
        let claim_map = self.get_claims();
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

    pub fn slash_claims(&mut self, bad_validators: Vec<String>) {
        let mut db = self.get_ledger_db();
        let (_, _, _, mut claims) = NetworkState::restore_state_objects(&db);

        bad_validators.iter().for_each(|k| {
            if let Some(claim) = claims.get_mut(&k.to_string()) {
                claim.eligible = false;
            }
        });

        if let Err(_) = db.set("claims", &claims) {
            println!("Error setting claims to state")
        };

        if let Err(e) = db.dump() {
            info!("Error dumping state to file: {:?}", e)
        }
    }

    pub fn pending_balance(
        &self,
        _address: String,
        _txn_pool: &Pool<String, Txn>,
    ) -> Option<(u128, u128)> {
        None
    }

    pub fn credits_as_bytes(credits: &LinkedHashMap<String, u128>) -> Vec<u8> {
        NetworkState::credits_to_string(credits).as_bytes().to_vec()
    }

    pub fn credits_to_string(credits: &LinkedHashMap<String, u128>) -> String {
        serde_json::to_string(credits).unwrap()
    }

    pub fn credits_from_bytes(data: &[u8]) -> LinkedHashMap<String, u128> {
        serde_json::from_slice::<LinkedHashMap<String, u128>>(data).unwrap()
    }

    pub fn debits_as_bytes(debits: &LinkedHashMap<String, u128>) -> Vec<u8> {
        NetworkState::debits_to_string(debits).as_bytes().to_vec()
    }

    pub fn debits_to_string(debits: &LinkedHashMap<String, u128>) -> String {
        serde_json::to_string(debits).unwrap()
    }

    pub fn debits_from_bytes(data: &[u8]) -> LinkedHashMap<String, u128> {
        serde_json::from_slice::<LinkedHashMap<String, u128>>(data).unwrap()
    }

    pub fn claims_as_bytes(claims: &LinkedHashMap<u128, Claim>) -> Vec<u8> {
        NetworkState::claims_to_string(claims).as_bytes().to_vec()
    }

    pub fn claims_to_string(claims: &LinkedHashMap<u128, Claim>) -> String {
        serde_json::to_string(claims).unwrap()
    }

    pub fn claims_from_bytes(data: &[u8]) -> LinkedHashMap<u128, Claim> {
        serde_json::from_slice::<LinkedHashMap<u128, Claim>>(data).unwrap()
    }

    pub fn last_block_from_bytes(data: &[u8]) -> Block {
        serde_json::from_slice::<Block>(data).unwrap()
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.to_string().as_bytes().to_vec()
    }

    pub fn from_bytes(data: &[u8]) -> NetworkState {
        serde_json::from_slice::<NetworkState>(data).unwrap()
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn from_string(string: &String) -> NetworkState {
        serde_json::from_str::<NetworkState>(&string).unwrap()
    }

    pub fn db_to_ledger(&self) -> Ledger {
        let credits = self.get_credits();
        let debits = self.get_debits();
        let claims = self.get_claims();

        Ledger {
            credits,
            debits,
            claims,
        }
    }
}

impl Ledger {
    pub fn as_bytes(&self) -> Vec<u8> {
        self.to_string().as_bytes().to_vec()
    }

    pub fn from_bytes(data: &[u8]) -> Ledger {
        serde_json::from_slice::<Ledger>(data).unwrap()
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn from_string(string: &String) -> Ledger {
        serde_json::from_str::<Ledger>(&string).unwrap()
    }
}

impl Components {
    pub fn as_bytes(&self) -> Vec<u8> {
        self.to_string().as_bytes().to_vec()
    }

    pub fn from_bytes(data: &[u8]) -> Components {
        serde_json::from_slice::<Components>(data).unwrap()
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn from_string(string: &String) -> Components {
        serde_json::from_str::<Components>(&string).unwrap()
    }
}

impl Chunkable for Components {
    fn chunk(&self) -> Option<Vec<Vec<u8>>> {
        let bytes_len = self.as_bytes().len();
        if bytes_len > MAX_TRANSMIT_SIZE {
            let mut n_chunks = bytes_len / MAX_TRANSMIT_SIZE;
            if bytes_len % MAX_TRANSMIT_SIZE != 0 {
                n_chunks += 1;
            }
            let mut chunks_vec = vec![];
            let mut last_slice_end = 0;
            (1..=n_chunks)
                .map(|n| n * MAX_TRANSMIT_SIZE)
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

impl Clone for NetworkState {
    fn clone(&self) -> NetworkState {
        NetworkState {
            path: self.path.clone(),
            credits: self.credits.clone(),
            debits: self.debits.clone(),
            reward_state: self.reward_state.clone(),
            state_hash: self.state_hash.clone(),
        }
    }
}
