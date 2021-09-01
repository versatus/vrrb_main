use crate::{block::Block, claim::Claim, reward::RewardState};
use ritelinked::LinkedHashMap;
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkState {
    pub path: String,
    pub claims: LinkedHashMap<u128, Claim>,
    pub credits: LinkedHashMap<String, u128>,
    pub debits: LinkedHashMap<String, u128>,
    pub reward_state: RewardState,
    pub block_archive: LinkedHashMap<u128, Block>,
    pub last_block: Option<Block>,
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

        let (credits, debits, claims, block_archive, reward_state, last_block) =
            NetworkState::restore_state_objects(db);

        NetworkState {
            path: path.to_string(),
            credits,
            debits,
            reward_state,
            claims,
            block_archive,
            last_block,
        }
    }

    pub fn hash(&mut self, block: Block, uts: &[u8; 16]) -> String {
        
        block.data.iter().for_each(|(_txn_id, txn)| {
            if let Some(entry) = self.credits.get_mut(&txn.receiver_address) {
                *entry += txn.clone().txn_amount
            } else {
                self.credits
                    .insert(txn.clone().receiver_address, txn.clone().txn_amount);
            }

            if let Some(entry) = self.debits.get_mut(&txn.clone().sender_address) {
                *entry += txn.txn_amount
            } else {
                self.debits
                    .insert(txn.clone().sender_address, txn.clone().txn_amount);
            }
        });

        self.claims.extend(block.owned_claims);
        let payload = format!("{:?},{:?},{:?},{:?},{:?}", self.credits, self.debits, self.claims, self.reward_state, uts);
        digest_bytes(payload.as_bytes())
    }

    pub fn restore_state_objects(
        db: PickleDb,
    ) -> (
        LinkedHashMap<String, u128>,
        LinkedHashMap<String, u128>,
        LinkedHashMap<u128, Claim>,
        LinkedHashMap<u128, Block>,
        RewardState,
        Option<Block>,
    ) {
        let credits: LinkedHashMap<String, u128> = if let Some(map) = db.get("credits") {
            map
        } else {
            LinkedHashMap::new()
        };

        let debits: LinkedHashMap<String, u128> = if let Some(map) = db.get("credits") {
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

        let block_archive: LinkedHashMap<u128, Block> = if let Some(map) = db.get("blockarchive") {
            map
        } else {
            LinkedHashMap::new()
        };
        let last_block: Option<Block> = if let Some(block) = db.get("lastblock") {
            block
        } else {
            None
        };

        (
            credits,
            debits,
            claims,
            block_archive,
            reward_state,
            last_block,
        )
    }

    pub fn dump(&self) {
        let mut db =
            match PickleDb::load_bin(self.path.clone(), PickleDbDumpPolicy::DumpUponRequest) {
                Ok(nst) => nst,
                Err(_) => PickleDb::new(
                    self.path.clone(),
                    PickleDbDumpPolicy::DumpUponRequest,
                    SerializationMethod::Bin,
                ),
            };

        if let Err(_) = db.set("credits", &self.credits) {
            println!("Error setting credits to state")
        }
        if let Err(_) = db.set("debits", &self.debits) {
            println!("Error setting debits to state")
        };
        if let Err(_) = db.set("claims", &self.claims) {
            println!("Error setting claims to state")
        };
        if let Err(_) = db.set("blockarchive", &self.block_archive) {
            println!("Error setting block archive to state")
        };
        if let Err(_) = db.set("rewardstate", &self.reward_state) {
            println!("Error setting reward state to state")
        };
        if let Err(_) = db.set("lastblock", &self.last_block) {
            println!("Error setting last block to state")
        };
        if let Err(_) = db.dump() {
            println!("Error dumping db to file")
        };
    }

    pub fn retrieve_balance(&self, address: String) -> Option<u128> {
        let address_credits = if let Some(amount) = self.credits.get(&address) {
            *amount
        } else {
            return None;
        };
        let address_debits = if let Some(amount) = self.debits.get(&address) {
            *amount
        } else {
            0u128
        };

        let address_balance = if let Some(balance) = address_credits.checked_sub(address_debits) {
            balance
        } else {
            panic!("Debits should never exceed credits");
        };

        Some(address_balance)
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
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_new_network_state() {}

    #[test]
    fn test_restored_network_state() {}

    #[test]
    fn test_valid_network_state() {}

    #[test]
    fn test_invalid_network_state() {}

    #[test]
    fn test_network_state_updated_locally() {}

    #[test]
    fn test_network_state_updated_via_gossip() {}
}
