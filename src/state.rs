use crate::utils::restore_db;
use crate::{block::Block, claim::Claim, reward::RewardState};
use log::info;
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use rand::Rng;
use ritelinked::LinkedHashMap;
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;

const ARCHIVE: &str = "ARCHIVE";
const FULL: &str = "FULL";
const LIGHT: &str = "LIGHT";
const ULTRALIGHT: &str = "ULTRALIGHT";
const BLOCK_ARCHIVE_PATH: &str = "blockarchive";

#[derive(Debug, Serialize, Deserialize)]
pub enum BlockArchive {
    // block_hash -> block
    Archive(String),
    // block_hash -> BlockHeight
    Full(String),
    // most recent block_hash, most recent block_height
    Light((Option<String>, Option<u128>)),
    // None
    UltraLight,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LastState(Option<String>);

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkState {
    // Path to database
    pub path: String,
    // hash of the state of the claims in the network
    pub claims: Option<String>,
    // hash of the state of credits in the network
    pub credits: Option<String>,
    // hash of the state of debits in the network
    pub debits: Option<String>,
    //reward state of the network
    pub reward_state: RewardState,
    // BlockArchive Object
    pub block_archive: BlockArchive,
    // the most recent block mined in the network and confirmed by the local node
    pub last_block: Option<Block>,
    // the last state hash -> sha256 hash of claims, credits, debits & reward state.
    pub state_hash: Option<String>,
    // the state of the network prior to the last block
    pub last_state: LastState,
}

impl BlockArchive {
    pub fn new(kind: &str) -> BlockArchive {
        match kind {
            ARCHIVE => {
                let mut rng = rand::thread_rng();
                let file_suffix: u32 = rng.gen();
                return BlockArchive::Archive(format!(
                    "{}_{}.db",
                    BLOCK_ARCHIVE_PATH.to_string(),
                    file_suffix
                ));
            }
            FULL => {
                let mut rng = rand::thread_rng();
                let file_suffix: u32 = rng.gen();
                return BlockArchive::Full(format!(
                    "{}_{}.db",
                    BLOCK_ARCHIVE_PATH.to_string(),
                    file_suffix
                ));
            }
            LIGHT => return BlockArchive::Light((None, None)),
            ULTRALIGHT => return BlockArchive::UltraLight,
            _ => {
                panic!("Must provide a type for block archive")
            }
        }
    }

    pub fn update(&mut self, block: &Block) {
        match self.clone() {
            Self::Archive(path) | Self::Full(path) => {
                let mut db = restore_db(&path);
                if let Err(_) = db.set(&block.clone().block_height.to_string(), &block.clone()) {
                    println!("Error setting block to block archive db");
                };
                if let Err(_) = db.dump() {
                    println!("Error dumping block archive db");
                };
            }
            Self::Light((mut hash_option, mut height_option)) => {
                hash_option = Some(block.clone().block_hash);
                height_option = Some(block.clone().block_height);

                Self::Light((hash_option, height_option));
            }
            _ => {}
        }
    }

    pub fn get_archive_db_snapshot(&self) -> Option<PickleDb> {
        if let Self::Archive(path) | Self::Full(path) = self.clone() {
            let db =
                if let Ok(nst) = PickleDb::load_read_only(path.clone(), SerializationMethod::Bin) {
                    nst
                } else {
                    PickleDb::new(
                        path.clone(),
                        PickleDbDumpPolicy::NeverDump,
                        SerializationMethod::Bin,
                    )
                };
            return Some(db);
        } else {
            None
        }
    }

    pub fn revert_db(&self, block: &Block) {
        match self.clone() {
            Self::Archive(path) | Self::Full(path) => {
                let mut db = restore_db(&path);
                if let Err(_) = db.rem(&block.clone().block_height.to_string()) {
                    println!("Error setting block to block archive db");
                };
                if let Err(_) = db.dump() {
                    println!("Error dumping block archive db");
                };
            }
            Self::Light((mut hash_option, mut height_option)) => {
                hash_option = Some(block.clone().block_hash);
                height_option = Some(block.clone().block_height);

                Self::Light((hash_option, height_option));
            }
            _ => {}
        }
    }
}

impl LastState {
    pub fn new(state_hash: Option<String>) -> LastState {
        LastState(state_hash)
    }

    pub fn update(self, state_hash: Option<String>) -> LastState {
        Self(state_hash)
    }

    pub fn get_state_hash(&self) -> Option<String> {
        self.0.clone()
    }

    // Return the previous last state from the network state ledger db
    // last state in network state ledger db is a LinkedHashMap<String, LastState>
    // wherein the key is the State hash of the current last_state.state_hash
    pub fn revert_one(&self, network_state: NetworkState) -> Option<NetworkState> {
        if let Some(state_hash) = self.get_state_hash() {
            let db = network_state.get_ledger_db();
            if let Some(map) = db.get::<LinkedHashMap<String, NetworkState>>("laststate") {
                if let Some(state) = map.get(&state_hash) {
                    Some(NetworkState { ..state.clone() })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_last_block(&self, network_state: NetworkState) -> Option<Block> {
        if let Some(state) = self.revert_one(network_state) {
            state.get_last_block()
        } else {
            None
        }
    }

    pub fn get_last_reward_state(&self, network_state: NetworkState) -> Option<RewardState> {
        if let Some(state) = self.revert_one(network_state) {
            Some(state.get_reward_state())
        } else {
            None
        }
    }
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

        let (
            credits_map,
            debits_map,
            claims_map,
            block_archive,
            reward_state,
            last_block,
            last_state,
        ) = NetworkState::restore_state_objects(&db);

        let credits = digest_bytes(format!("{:?}", &credits_map).as_bytes());
        let debits = digest_bytes(format!("{:?}", &credits_map).as_bytes());
        let claims = digest_bytes(format!("{:?}", &credits_map).as_bytes());

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
            claims: {
                if claims_map.is_empty() {
                    None
                } else {
                    Some(claims)
                }
            },
            block_archive,
            last_block,
            state_hash: None,
            last_state,
        }
    }

    pub fn get_block_archive_db(&self) -> Option<PickleDb> {
        self.block_archive.get_archive_db_snapshot()
    }

    pub fn revert_state(&mut self, block: &Block) {
        let db = self.get_ledger_db();
        let last_state = self.get_last_state().0;
        if let Some(hash) = last_state {
            if let Some(map) = db.get::<LinkedHashMap<String, NetworkState>>("laststate") {
                if let Some(state) = map.get(&hash) {
                    Self { ..state.clone() };
                }
            }
        }
        self.revert_db_credits(block);
        self.revert_db_debits(block);
        self.revert_db_claims(block);
        self.revert_db_reward_state();
        self.revert_db_last_block();
        self.revert_db_last_state();
    }

    pub fn get_last_state(&self) -> LastState {
        self.last_state.clone()
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

        block.data.iter().for_each(|(_txn_id, txn)| {
            if let Some(entry) = credits.get_mut(&txn.receiver_address) {
                *entry += txn.clone().txn_amount
            } else {
                credits.insert(txn.clone().receiver_address, txn.clone().txn_amount);
            }
        });

        if let Some(entry) = credits.get_mut(&block.block_reward.miner.clone().unwrap()) {
            *entry += block.block_reward.amount
        } else {
            credits.insert(
                block.block_reward.miner.clone().unwrap(),
                block.block_reward.amount,
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

        block.data.iter().for_each(|(_txn_id, txn)| {
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

    pub fn claim_hash(self, block: &Block) -> String {
        let mut claims = LinkedHashMap::new();
        claims.extend(block.owned_claims.clone());

        if let Some(chs) = self.claims {
            return digest_bytes(format!("{},{:?}", chs, claims).as_bytes());
        } else {
            return digest_bytes(format!("{:?},{:?}", self.claims, claims).as_bytes());
        }
    }

    pub fn hash(&mut self, block: Block, uts: &[u8; 16]) -> String {
        let reward_state = self.clone().get_reward_state();
        let credit_hash = self.clone().credit_hash(&block);
        let debit_hash = self.clone().debit_hash(&block);
        let claim_hash = self.clone().claim_hash(&block);
        let reward_state_hash =
            digest_bytes(format!("{:?},{:?}", self.reward_state, reward_state).as_bytes());
        let payload = format!(
            "{:?},{:?},{:?},{:?},{:?},{:?}",
            self.state_hash, credit_hash, debit_hash, claim_hash, reward_state_hash, uts
        );
        let new_state_hash = digest_bytes(payload.as_bytes());
        new_state_hash
    }

    pub fn restore_state_objects(
        db: &PickleDb,
    ) -> (
        LinkedHashMap<String, u128>,
        LinkedHashMap<String, u128>,
        LinkedHashMap<u128, Claim>,
        BlockArchive,
        RewardState,
        Option<Block>,
        LastState,
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

        let claims: LinkedHashMap<u128, Claim> = if let Some(map) = db.get("claims") {
            map
        } else {
            LinkedHashMap::new()
        };

        let block_archive: BlockArchive = if let Some(map) = db.get("blockarchive") {
            map
        } else {
            BlockArchive::new("FULL")
        };
        let last_block: Option<Block> = if let Some(block) = db.get("lastblock") {
            block
        } else {
            None
        };

        let last_state_map: LinkedHashMap<Option<String>, NetworkState> =
            if let Some(state) = db.get("laststate") {
                state
            } else {
                LinkedHashMap::new()
            };

        let last_state = {
            if !last_state_map.is_empty() {
                if let Some((_, state)) = last_state_map.back() {
                    LastState(state.state_hash.clone())
                } else {
                    LastState(None)
                }
            } else {
                LastState(None)
            }
        };

        (
            credits,
            debits,
            claims,
            block_archive,
            reward_state,
            last_block,
            last_state,
        )
    }

    pub fn dump_last_state(&self) {
        let mut db = self.get_ledger_db();
        if let Some(mut map) = db.get::<LinkedHashMap<Option<String>, NetworkState>>("laststate") {
            map.insert(self.last_state.clone().0, self.clone());
            if let Err(_) = db.set("laststate", &map) {
                println!("Error setting last state to db");
            }
        } else {
            let mut map: LinkedHashMap<Option<String>, NetworkState> = LinkedHashMap::new();
            map.insert(self.last_state.clone().0, self.clone());
            if let Err(_) = db.set("laststate", &map) {
                println!("Error setting last state to db");
            }
        }
    }

    pub fn dump(&mut self, block: Block) {
        let mut db = self.get_ledger_db();

        let (
            mut credits,
            mut debits,
            mut claims,
            mut block_archive,
            mut reward_state,
            _last_block,
            _last_state,
        ) = NetworkState::restore_state_objects(&db);
        

        block.data.iter().for_each(|(_txn_id, txn)| {
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

        if let Some(entry) = credits.get_mut(&block.block_reward.miner.clone().unwrap()) {
            *entry += block.block_reward.amount.clone()
        } else {
            credits.insert(
                block.block_reward.miner.clone().unwrap().clone(),
                block.block_reward.amount.clone(),
            );
        }

        info!(target: "credits", "{:?}", credits);
        info!(target: "debits", "{:?}", debits);

        claims.extend(block.owned_claims.clone());
        claims.remove(&block.claim.claim_number);

        block_archive.update(&block);
        reward_state.update(block.block_reward.category.clone());
        let last_block = Some(block);

        if let Err(_) = db.set("credits", &credits) {
            println!("Error setting credits to state")
        };
        if let Err(_) = db.set("debits", &debits) {
            println!("Error setting debits to state")
        };
        if let Err(_) = db.set("claims", &claims) {
            println!("Error setting claims to state")
        };
        if let Err(_) = db.set("blockarchive", &block_archive) {
            println!("Error setting block archive to state")
        };
        if let Err(_) = db.set("rewardstate", &reward_state) {
            println!("Error setting reward state to state")
        };
        if let Err(_) = db.set("lastblock", &last_block) {
            println!("Error setting last block to state")
        };
        if let Err(_) = db.dump() {
            println!("Error dumping state to file")
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

    pub fn update_claims(&mut self, block: &Block) {
        let chs = self.clone().claim_hash(block);
        self.claims = Some(chs);
    }

    pub fn update_reward_state(&mut self, block: &Block) {
        self.reward_state.update(block.block_reward.category);
    }

    pub fn update_last_block(&mut self, block: &Block) {
        self.last_block = Some(block.clone());
    }

    pub fn update_block_archive(&mut self, block: &Block) {
        self.block_archive.update(block)
    }

    pub fn update_state_hash(&mut self, block: &Block) {
        self.state_hash = Some(block.state_hash.clone());
        info!(target: "updating_state_hash", "new_state_hash: {:?}", self.state_hash)
    }

    pub fn update_last_state(&mut self) {
        self.last_state = self.last_state.clone().update(self.state_hash.clone());
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

    pub fn get_claims(&self) -> LinkedHashMap<u128, Claim> {
        let db = self.get_ledger_db();
        let claims: LinkedHashMap<u128, Claim> = if let Some(map) = db.get("claims") {
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

    pub fn get_account_claims(&self, pubkey: &str) -> LinkedHashMap<u128, Claim> {
        let mut claims = self.get_claims();
        claims.retain(|_, claim| claim.current_owner.clone().unwrap() == pubkey.to_string());

        claims
    }

    pub fn revert_db_credits(&self, block: &Block) {
        let mut credits = self.get_credits();
        block.data.iter().for_each(|(_, txn)| {
            if let Some(entry) = credits.get_mut(&txn.receiver_address) {
                *entry -= txn.txn_amount;
            }
        });

        if let Some(entry) = credits.get_mut(&block.block_reward.miner.clone().unwrap()) {
            *entry -= block.block_reward.amount;
        }

        let mut db = self.get_ledger_db();
        if let Err(_) = db.set("credits", &credits) {
            println!("Error reverting credits");
        }
        if let Err(_) = db.dump() {
            println!("Error dumping state after reversion");
        };
    }

    pub fn revert_db_debits(&self, block: &Block) {
        let mut debits = self.get_debits();
        block.data.iter().for_each(|(_, txn)| {
            if let Some(entry) = debits.get_mut(&txn.sender_address) {
                *entry -= txn.txn_amount;
            }
        });

        let mut db = self.get_ledger_db();
        if let Err(_) = db.set("debits", &debits) {
            println!("Error reverting debits");
        };
        if let Err(_) = db.dump() {
            println!("Error dumping state after reversion");
        };
    }

    pub fn revert_db_claims(&self, block: &Block) {
        let mut claims = self.get_claims();
        claims.retain(|k, _| !block.owned_claims.contains_key(&k));

        let mut db = self.get_ledger_db();
        if let Err(_) = db.set("claims", &claims) {
            println!("Error reverting claims");
        };
        if let Err(_) = db.dump() {
            println!("Error dumping state");
        };
    }
    pub fn revert_db_reward_state(&self) {
        let mut db = self.get_ledger_db();
        let last_state = self.get_last_state();
        let reward_state = last_state.get_last_reward_state(self.clone());
        if let Err(_) = db.set("rewardstate", &reward_state) {
            println!("Error setting last block");
        };
        if let Err(_) = db.dump() {
            println!("Error dumping last block to file");
        }
    }

    pub fn revert_db_last_block(&self) {
        let mut db = self.get_ledger_db();
        let last_state = self.get_last_state();
        let last_block = last_state.get_last_block(self.clone());
        if let Err(_) = db.set("lastblock", &last_block) {
            println!("Error setting last block");
        };
        if let Err(_) = db.dump() {
            println!("Error dumping last block to file");
        }
    }

    pub fn revert_db_last_state(&self) {
        let mut db = self.get_ledger_db();
        if let Some(mut map) = db.get::<LinkedHashMap<String, LastState>>("laststate") {
            map.pop_back();

            if let Err(_) = db.set("laststate", &map) {
                println!("error reverting db last state");
            };

            if let Err(_) = db.dump() {
                println!("Error dumping reverted db last state");
            }
        }
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

    pub fn last_block_as_bytes(&self) -> Vec<u8> {
        self.last_block.clone().unwrap().as_bytes()
    }

    pub fn last_block_to_string(&self) -> String {
        self.last_block.clone().unwrap().to_string()
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
}

impl Clone for NetworkState {
    fn clone(&self) -> NetworkState {
        NetworkState {
            path: self.path.clone(),
            credits: self.credits.clone(),
            debits: self.debits.clone(),
            claims: self.claims.clone(),
            block_archive: self.block_archive.clone(),
            reward_state: self.reward_state.clone(),
            last_block: self.last_block.clone(),
            state_hash: self.state_hash.clone(),
            last_state: self.last_state.clone(),
        }
    }
}

impl Clone for BlockArchive {
    fn clone(&self) -> BlockArchive {
        match self {
            Self::Archive(map) => return BlockArchive::Archive(map.clone()),
            Self::Full(map) => return BlockArchive::Full(map.clone()),
            Self::Light((hash_option, height_option)) => {
                BlockArchive::Light((hash_option.clone(), height_option.clone()))
            }
            Self::UltraLight => return BlockArchive::UltraLight,
        }
    }
}
