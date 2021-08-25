use crate::{block::Block, claim::Claim, reward::RewardState};
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use serde::{
    de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor},
    ser::{Serialize, SerializeMap, Serializer},
};
use sha256::digest_bytes;
use std::collections::HashMap;
use std::fmt::{self, Debug, Error, Formatter};

pub struct NetworkState {
    pub state: PickleDb,
}

impl NetworkState {
    pub fn init(&mut self, reward_state: RewardState, genesis: Block) {
        let mut credits: HashMap<String, u128> = HashMap::new();
        let debits: HashMap<String, u128> = HashMap::new();
        let mut block_archive: HashMap<u128, Block> = HashMap::new();
        credits.insert(genesis.clone().miner, genesis.clone().block_reward.amount);
        let claims = genesis.clone().owned_claims;
        block_archive.insert(1, genesis.clone());
        self.state.set("credits", &credits).unwrap();
        self.state.set("debits", &debits).unwrap();
        self.state.set("claims", &claims).unwrap();
        self.state.set("rewardstate", &reward_state).unwrap();
        self.state.set("lastblock", &genesis).unwrap();
        self.state.set("blockarchive", &block_archive).unwrap();
    }

    pub fn update(&mut self, block: Block) {
        // Ensure that all transactions in the block are valid
        // and have been included in the account state.
        // update the pending network state before hashing it to
        // test whether the proposed network state is up to date
        // and valid.
        let mut credits: HashMap<String, u128> = self.state.get("credits").unwrap();
        let mut debits: HashMap<String, u128> = self.state.get("debits").unwrap();
        let mut reward_state: RewardState = self.state.get("rewardstate").unwrap();
        let mut claims: HashMap<u128, Claim> = self.state.get("claims").unwrap();
        let mut block_archive: HashMap<u128, Block> = self.state.get("blockarchive").unwrap();

        println!("Updating network state -> Credits & Debits");

        block.data.iter().for_each(|(_id, txn)| {
            *credits.get_mut(&txn.clone().receiver_address).unwrap() += txn.txn_amount;
            *debits.get_mut(&txn.clone().sender_address).unwrap() += txn.txn_amount;
        });

        println!("Updating network state -> Claims");

        block.owned_claims.iter().for_each(|(claim_number, claim)| {
            claims.insert(*claim_number, claim.to_owned());
        });

        println!("Updating network state -> reward state");
        reward_state.update(block.block_reward.category);

        block_archive.insert(block.block_height, block.clone());

        self.state.set("credits", &credits).unwrap();
        self.state.set("debits", &debits).unwrap();
        self.state.set("claims", &claims).unwrap();
        self.state.set("rewardstate", &reward_state).unwrap();
        self.state.set("lastblock", &block).unwrap();
        self.state.set("blockarchive", &block_archive).unwrap();
    }

    pub fn restore(path: &str) -> NetworkState {
        let db = match PickleDb::load_bin(path, PickleDbDumpPolicy::DumpUponRequest) {
            Ok(nst) => nst,
            Err(_) => PickleDb::new(
                path,
                PickleDbDumpPolicy::DumpUponRequest,
                SerializationMethod::Bin,
            ),
        };

        NetworkState { state: db }
    }

    pub fn hash(&self, block: Block, uts: &[u8; 16]) -> String {
        let mut credits: HashMap<String, u128> = self.state.get("credits").unwrap();
        let mut debits: HashMap<String, u128> = self.state.get("debits").unwrap();
        let reward_state: RewardState = self.state.get("rewardstate").unwrap();
        let mut claims: HashMap<u128, Claim> = self.state.get("claims").unwrap();

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

        claims.extend(block.owned_claims);
        let mut credits_vec: Vec<_> = credits.iter().collect();
        let mut debits_vec: Vec<_> = debits.iter().collect();
        let mut claims_vec: Vec<_> = claims.iter().collect();

        credits_vec.sort_unstable_by(|a, b| a.0.cmp(b.0));
        debits_vec.sort_unstable_by(|a, b| a.0.cmp(b.0));
        claims_vec.sort_unstable_by(|a, b| a.0.cmp(b.0));

        let mut state_bytes = vec![];
        credits_vec.iter().for_each(|(k, v)| {
            k.clone().as_bytes().iter().for_each(|b| {
                state_bytes.push(*b);
            });
            v.clone().to_ne_bytes().iter().for_each(|b| {
                state_bytes.push(*b);
            })
        });
        debits_vec.iter().for_each(|(k, v)| {
            k.clone().as_bytes().iter().for_each(|b| {
                state_bytes.push(*b);
            });
            v.clone().to_ne_bytes().iter().for_each(|b| {
                state_bytes.push(*b);
            })
        });
        claims_vec.iter().for_each(|(k, v)| {
            k.clone().to_ne_bytes().iter().for_each(|b| {
                state_bytes.push(*b);
            });
            let mut v = v.clone().as_bytes();
            v.sort_unstable();
            v.iter().for_each(|b| {
                state_bytes.push(*b);
            });
        });
        let mut reward_bytes = reward_state.as_bytes();
        reward_bytes.sort_unstable();
        reward_bytes.iter().for_each(|b| {
            state_bytes.push(*b);
        });

        uts.iter().for_each(|b| {
            state_bytes.push(*b);
        });

        digest_bytes(&state_bytes)
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
}

impl Clone for NetworkState {
    fn clone(&self) -> NetworkState {
        let mut cloned_db = PickleDb::new(
            "temp.db",
            PickleDbDumpPolicy::NeverDump,
            SerializationMethod::Bin,
        );

        let credits: Option<HashMap<String, u128>> = self.state.get("credits");
        let debits: Option<HashMap<String, u128>> = self.state.get("debits");
        let claims: Option<HashMap<u128, Claim>> = self.state.get("claims");
        let reward_state: Option<RewardState> = self.state.get("rewardstate");
        let last_block: Option<Block> = self.state.get("lastblock");
        let block_archive: Option<HashMap<u128, Block>> = self.state.get("blockarchive");

        if let Some(credits) = credits {
            let cloned_result = cloned_db.set("credits", &credits);
            if let Err(e) = cloned_result {
                println!("Error setting to cloned_state: {}", e)
            }
        }

        if let Some(debits) = debits {
            let cloned_result = cloned_db.set("debits", &debits);

            if let Err(e) = cloned_result {
                println!("Error setting to cloned_state: {}", e)
            }
        }

        if let Some(claims) = claims {
            let cloned_result = cloned_db.set("claims", &claims);

            if let Err(e) = cloned_result {
                println!("Error setting to cloned_state: {}", e)
            }
        }

        if let Some(reward_state) = reward_state {
            let cloned_result = cloned_db.set("rewardstate", &reward_state);

            if let Err(e) = cloned_result {
                println!("Error setting to cloned_state: {}", e)
            }
        }

        if let Some(last_block) = last_block {
            let cloned_result = cloned_db.set("lastblock", &last_block);

            if let Err(e) = cloned_result {
                println!("Error setting to cloned_state: {}", e)
            }
        }

        if let Some(block_archive) = block_archive {
            let cloned_result = cloned_db.set("blockarchive", &block_archive);

            if let Err(e) = cloned_result {
                println!("Error setting to cloned_state: {}", e)
            }
        }

        NetworkState { state: cloned_db }
    }
}

impl Debug for NetworkState {
    fn fmt(&self, _f: &mut Formatter) -> Result<(), Error> {
        Ok(())
    }
}

impl Serialize for NetworkState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map: HashMap<&str, String> = HashMap::new();

        let credits: Option<HashMap<String, u128>> = self.state.get("credits");
        let debits: Option<HashMap<String, u128>> = self.state.get("debits");
        let claims: Option<HashMap<u128, Claim>> = self.state.get("claims");
        let reward_state: Option<RewardState> = self.state.get("rewardstate");
        let last_block: Option<Option<Block>> = self.state.get("lastblock");
        let block_archive: Option<HashMap<u128, Block>> = self.state.get("blockarchive");

        if let Some(creditmap) = credits {
            let credits = serde_json::to_string::<HashMap<String, u128>>(&creditmap).unwrap();
            map.insert("credits", credits);
        } else {
            let credits = serde_json::to_string::<HashMap<String, u128>>(&HashMap::new()).unwrap();
            map.insert("credits", credits);
        }

        if let Some(debits) = debits {
            let debits = serde_json::to_string::<HashMap<String, u128>>(&debits).unwrap();
            map.insert("debits", debits);
        } else {
            let debits = serde_json::to_string::<HashMap<String, u128>>(&HashMap::new()).unwrap();
            map.insert("debits", debits);
        }

        if let Some(reward_state) = reward_state {
            let reward_state = serde_json::to_string::<RewardState>(&reward_state).unwrap();
            map.insert("rewardstate", reward_state);
        } else {
            let reward_state = serde_json::to_string::<RewardState>(&RewardState::start()).unwrap();
            map.insert("rewardstate", reward_state);
        }

        if let Some(claims) = claims {
            let claims = serde_json::to_string::<HashMap<u128, Claim>>(&claims).unwrap();
            map.insert("claims", claims);
        } else {
            let claims = serde_json::to_string::<HashMap<u128, Claim>>(&HashMap::new()).unwrap();
            map.insert("claims", claims);
        }

        if let Some(block_option) = last_block {
            let last_block = serde_json::to_string::<Option<Block>>(&block_option).unwrap();
            map.insert("lastblock", last_block);
        } else {
            map.insert(
                "lastblock",
                serde_json::to_string::<Option<Block>>(&None).unwrap(),
            );
        }

        if let Some(block_archive) = block_archive {
            let block_archive =
                serde_json::to_string::<HashMap<u128, Block>>(&block_archive).unwrap();
            map.insert("blockarchive", block_archive);
        } else {
            map.insert(
                "blockarchive",
                serde_json::to_string::<HashMap<u128, Block>>(&HashMap::new()).unwrap(),
            );
        }

        let mut serialized_map = serializer.serialize_map(Some(map.len()))?;

        for (k, v) in map {
            serialized_map.serialize_entry(&k, &v)?;
        }

        serialized_map.end()
    }
}

impl<'de> Deserialize<'de> for NetworkState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Credits,
            Debits,
            RewardState,
            Claims,
            LastBlock,
            BlockArchive,
        }

        impl<'a, 'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`credits` or `debits` or `rewardstate` or `claims` or `lastblock` or `blockarchive`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "credits" => Ok(Field::Credits),
                            "debits" => Ok(Field::Debits),
                            "rewardstate" => Ok(Field::RewardState),
                            "claims" => Ok(Field::Claims),
                            "lastblock" => Ok(Field::LastBlock),
                            "blockarchive" => Ok(Field::BlockArchive),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct NetworkStateVisitor;

        impl<'de> Visitor<'de> for NetworkStateVisitor {
            type Value = NetworkState;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct NetworkState")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<NetworkState, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let credits = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let debits = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let reward_state = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let claims = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let last_block = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let block_archive = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;

                let mut state = PickleDb::new(
                    "temp.db",
                    PickleDbDumpPolicy::NeverDump,
                    SerializationMethod::Bin,
                );
                let credits_result = state.set("credits", &credits);
                let debits_result = state.set("debits", &debits);
                let reward_state_result = state.set("rewardstate", &reward_state);
                let claims_result = state.set("claims", &claims);
                let last_block_result = state.set("lastblock", &last_block);
                let block_archive = state.set("blockarchive", &block_archive);

                if let Err(e) = credits_result {
                    println!("Error setting to state: {}", e)
                }
                if let Err(e) = debits_result {
                    println!("Error setting to state: {}", e)
                }
                if let Err(e) = reward_state_result {
                    println!("Error setting to state: {}", e)
                }
                if let Err(e) = claims_result {
                    println!("Error setting to state: {}", e)
                }
                if let Err(e) = last_block_result {
                    println!("Error setting to state: {}", e)
                }
                if let Err(e) = block_archive {
                    println!("Error setting to state: {}", e)
                }

                Ok(NetworkState { state })
            }

            fn visit_map<V>(self, mut map: V) -> Result<NetworkState, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut credits = None;
                let mut debits = None;
                let mut reward_state = None;
                let mut claims = None;
                let mut last_block = None;
                let mut block_archive = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Credits => {
                            if credits.is_some() {
                                return Err(de::Error::duplicate_field("credits"));
                            }

                            credits = Some(map.next_value()?);
                        }
                        Field::Debits => {
                            if debits.is_some() {
                                return Err(de::Error::duplicate_field("debits"));
                            }

                            debits = Some(map.next_value()?);
                        }
                        Field::RewardState => {
                            if reward_state.is_some() {
                                return Err(de::Error::duplicate_field("rewardstate"));
                            }

                            reward_state = Some(map.next_value()?);
                        }
                        Field::Claims => {
                            if claims.is_some() {
                                return Err(de::Error::duplicate_field("claims"));
                            }

                            claims = Some(map.next_value()?);
                        }
                        Field::LastBlock => {
                            if last_block.is_some() {
                                return Err(de::Error::duplicate_field("lastblock"));
                            }

                            last_block = Some(map.next_value()?);
                        }
                        Field::BlockArchive => {
                            if block_archive.is_some() {
                                return Err(de::Error::duplicate_field("blockarchive"));
                            }

                            block_archive = Some(map.next_value()?);
                        }
                    }
                }
                let credits = credits.ok_or_else(|| de::Error::missing_field("credits"))?;
                let debits = debits.ok_or_else(|| de::Error::missing_field("debits"))?;
                let reward_state =
                    reward_state.ok_or_else(|| de::Error::missing_field("rewardstate"))?;
                let claims = claims.ok_or_else(|| de::Error::missing_field("claims"))?;
                let last_block = last_block.ok_or_else(|| de::Error::missing_field("lastblock"))?;
                let block_archive =
                    block_archive.ok_or_else(|| de::Error::missing_field("blockarchive"))?;

                let mut state = PickleDb::new(
                    "temp.db",
                    PickleDbDumpPolicy::NeverDump,
                    SerializationMethod::Bin,
                );
                let credits_result = state.set("credits", &credits);
                let debits_result = state.set("debits", &debits);
                let reward_state_result = state.set("rewardstate", &reward_state);
                let claims_result = state.set("claims", &claims);
                let last_block_result = state.set("lastblock", &last_block);
                let block_archive = state.set("blockarchive", &block_archive);

                if let Err(e) = credits_result {
                    println!("Error setting to state in deserializer: {}", e)
                }
                if let Err(e) = debits_result {
                    println!("Error setting to state in deserializer: {}", e)
                }
                if let Err(e) = reward_state_result {
                    println!("Error setting to state in deserializer: {}", e)
                }
                if let Err(e) = claims_result {
                    println!("Error setting to state in deserializer: {}", e)
                }
                if let Err(e) = last_block_result {
                    println!("Error setting to state in deserializer: {}", e)
                }
                if let Err(e) = block_archive {
                    println!("Error setting to state in deserializer: {}", e)
                }

                Ok(NetworkState { state })
            }
        }

        const FIELDS: &[&str] = &[
            "credits",
            "debits",
            "rewardstate",
            "claims",
            "lastblock",
            "blockarchive",
        ];
        deserializer.deserialize_struct("NetworkState", FIELDS, NetworkStateVisitor)
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
