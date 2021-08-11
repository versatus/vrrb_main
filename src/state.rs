use std::fmt::{self, Debug, Error, Formatter};
use crate::{block::Block, reward::RewardState, claim::Claim};
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use pickledb::error;
use serde::{
    de::{
        self,
        Deserialize, 
        Deserializer, 
        Visitor, 
        MapAccess,
        SeqAccess
    }, 
    ser::{
        Serialize, 
        SerializeMap, 
        Serializer,
    }};
use sha256::digest_bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct PendingNetworkState {
    pub state: PickleDb,
}

pub struct NetworkState {
    pub state: PickleDb,
}

impl PendingNetworkState {
    pub fn temp(
        network_state: Arc<Mutex<NetworkState>>,
        block: Block
    ) -> PendingNetworkState {
        
        let mut temp = PendingNetworkState { state: network_state.lock().unwrap().clone().state };
        temp.update(block);
        temp
    }

    pub fn update(&mut self, block: Block) {
        // Ensure that all transactions in the block are valid
        // and have been included in the account state.
        // update the pending network state before hashing it to
        // test whether the proposed network state is up to date
        // and valid.
        let mut credits: HashMap<String, u128> = self.state.get("credits").unwrap();
        let mut debits: HashMap<String, u128> = self.state.get("debits").unwrap();
        let mut reward_state: RewardState = self.state.get("reward_state").unwrap();
        let mut claims: HashMap<u128, Claim> = self.state.get("claims").unwrap();
        let last_block: Block = self.state.get("last_block").unwrap();
        let block_archive: HashMap<u128, Block> = self.state.get("block_archive").unwrap();

        block.data.iter().for_each(|(_id, txn)| {
            *credits.get_mut(&txn.clone().receiver_address).unwrap() += txn.txn_amount;
            *debits.get_mut(&txn.clone().sender_address).unwrap() += txn.txn_amount;
        });

        block.confirmed_owned_claims.iter().for_each(|claim| {
            claims.insert(claim.claim_number, claim.to_owned());
        });

        block.visible_blocks.iter().for_each(|claim| {
            claims.insert(claim.claim_number, claim.to_owned());
        });

        reward_state.update(block.block_reward.category);

        self.state.set("credits", &credits).unwrap();
        self.state.set("debits", &debits).unwrap();
        self.state.set("claims", &claims).unwrap();
        self.state.set("reward_state", &reward_state).unwrap();
        self.state.set("last_block", &last_block).unwrap();
        self.state.set("block_archive", &block_archive).unwrap();

    }

    pub fn hash(&self, uts: &[u8; 16]) -> String {
        let credits = self.state.get::<HashMap<String,u128>>("credits").unwrap();
        let mut credits: Vec<_> = credits.iter().collect();
        let debits = self.state.get::<HashMap<String, u128>>("debits").unwrap();
        let mut debits: Vec<_> = debits.iter().collect();
        let reward_state = serde_json::to_string::<RewardState>(&self.state.get::<RewardState>("reward_state").unwrap()).unwrap();
        let claims = self.state.get::<HashMap<u128, Claim>>("claims").unwrap();
        let mut claims: Vec<_> = claims.iter().collect();
        let last_block = serde_json::to_string::<Block>(&self.state.get("last_block").unwrap()).unwrap();
        
        let mut reward_state_bytes = reward_state.as_bytes().to_vec();
        let mut block_bytes = last_block.as_bytes().to_vec();
        reward_state_bytes.sort_unstable();
        block_bytes.sort_unstable();

        credits.sort_by_key(|x| x.0);
        debits.sort_by_key(|x| x.0);
        claims.sort_by_key(|x| x.0);

        let mut network_state_bytes = vec![];

        credits.iter().for_each(|(x, y)| {
            network_state_bytes.extend(x.as_bytes().to_vec()); 
            network_state_bytes.extend(y.to_ne_bytes().to_vec());
        });
        debits.iter().for_each(|(x, y)| {
            network_state_bytes.extend(x.as_bytes().to_vec());
            network_state_bytes.extend(y.to_ne_bytes().to_vec());
        });
        claims.iter().for_each(|(x, y)| {
            network_state_bytes.extend(x.to_ne_bytes().to_vec());
            network_state_bytes.extend(serde_json::to_string::<Claim>(y).unwrap().to_string().as_bytes().to_vec());
        });

        network_state_bytes.extend(reward_state_bytes);
        network_state_bytes.extend(block_bytes);

        let ts_hash = digest_bytes(uts);

        network_state_bytes.extend(ts_hash.as_bytes().to_vec());

        let state_bytes: &[u8] = &network_state_bytes;

        digest_bytes(state_bytes)
    }
}

impl NetworkState {

    pub fn update<T: Serialize>(
        &mut self, 
        state_obj: T, 
        state_obj_type: &str
    ) -> Result<(), error::ErrorType> 
    {
        let result = self.state.set(state_obj_type, &state_obj);
        if let Err(e) = result {println!("Error setting to state"); return Err(error::Error::get_type(&e))}

        Ok(())
    }

    pub fn restore(path: &str) -> NetworkState {
        let db = match PickleDb::load_bin(
            path, 
            PickleDbDumpPolicy::AutoDump
            ) {

                Ok(nst) => nst,
                Err(_) => PickleDb::new(
                path, 
                PickleDbDumpPolicy::AutoDump, 
                SerializationMethod::Bin)
        };
        
        NetworkState {
            state: db,
        }    
    }

    pub fn hash(&self, uts: &[u8; 16]) -> String {
        let mut network_state_bytes = serde_json::to_string(&self).unwrap().into_bytes();
        network_state_bytes.sort_unstable();

        let ts_hash = digest_bytes(uts);
        for byte in ts_hash.as_bytes().iter() {
            network_state_bytes.push(*byte);
        }
        let state_bytes: &[u8] = &network_state_bytes;

        digest_bytes(state_bytes)
    }
    
    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();

        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> NetworkState {
        serde_json::from_slice::<NetworkState>(data).unwrap()
    }
}

impl Clone for NetworkState {
    fn clone(&self) -> NetworkState {
        let mut cloned_db = PickleDb::new("temp.db", PickleDbDumpPolicy::NeverDump, SerializationMethod::Bin);

        let credits: Option<HashMap<String, u128>> = self.state.get("credits");
        let debits: Option<HashMap<String, u128>> = self.state.get("debits");
        let claims: Option<HashMap<u128, Claim>> = self.state.get("claims");
        let reward_state: Option<RewardState> = self.state.get("reward_state");
        let last_block: Option<Block> = self.state.get("last_block");

        if let Some(credits) = credits {
            let cloned_result = cloned_db.set("credits", &credits);
            if let Err(e) = cloned_result { println!("Error setting to cloned_state: {}", e) }
        }

        if let Some(debits) = debits {
            let cloned_result = cloned_db.set("debits", &debits);

            if let Err(e) = cloned_result { println!("Error setting to cloned_state: {}", e) }
        }

        if let Some(claims) = claims {
            let cloned_result = cloned_db.set("claims", &claims);

            if let Err(e) = cloned_result { println!("Error setting to cloned_state: {}", e) }
        }

        if let Some(reward_state) = reward_state {
            let cloned_result = cloned_db.set("reward_state", &reward_state);

            if let Err(e) = cloned_result { println!("Error setting to cloned_state: {}", e) }
        }

        if let Some(last_block) = last_block {
            let cloned_result = cloned_db.set("last_block", &last_block);

            if let Err(e) = cloned_result { println!("Error setting to cloned_state: {}", e) }
        }

        NetworkState {
            state: cloned_db
        }
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

        let credits = serde_json::to_string::<HashMap<String, HashMap<String, u128>>>(
            &self.state.get("credits").unwrap()
        ).unwrap();

        let debits = serde_json::to_string::<HashMap<String, HashMap<String, u128>>>(
            &self.state.get("debits").unwrap()
        ).unwrap();

        let reward_state = serde_json::to_string::<RewardState>(
            &self.state.get("reward_state").unwrap()
        ).unwrap();

        let claims = serde_json::to_string::<Claim>(
            &self.state.get("claims").unwrap()
        ).unwrap();

        let last_block = serde_json::to_string::<Block>(
            &self.state.get("last_block").unwrap()
        ).unwrap();

        let block_archive = serde_json::to_string::<HashMap<String, HashMap<u128, Block>>>(
            &self.state.get("block_archive").unwrap()
        ).unwrap();

        map.insert("credits", credits);
        map.insert("debits", debits);
        map.insert("reward_state", reward_state);
        map.insert("claims", claims);
        map.insert("last_block", last_block);
        map.insert("block_archive", block_archive);
        
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
        D: Deserializer<'de> 
    {

        enum Field { Credits, Debits, RewardState, Claims, LastBlock, BlockArchive }

        impl<'a, 'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de> 
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
                            _ => Err(de::Error::unknown_field(value, FIELDS))
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
                let credits = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let debits = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let reward_state = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let claims = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let last_block = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let block_archive = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                
                let mut state = PickleDb::new("temp.db", PickleDbDumpPolicy::NeverDump, SerializationMethod::Bin);
                let credits_result = state.set("credits", &credits);
                let debits_result = state.set("debits", &debits);
                let reward_state_result = state.set("reward_state", &reward_state);
                let claims_result = state.set("claims", &claims);
                let last_block_result = state.set("last_block", &last_block);
                let block_archive = state.set("block_archive", &block_archive);

                if let Err(e) = credits_result {println!("Error setting to state: {}", e) }
                if let Err(e) = debits_result {println!("Error setting to state: {}", e) }
                if let Err(e) = reward_state_result { println!("Error setting to state: {}", e) }
                if let Err(e) = claims_result { println!("Error setting to state: {}", e) }
                if let Err(e) = last_block_result { println!("Error setting to state: {}", e) }
                if let Err(e) = block_archive { println!("Error setting to state: {}", e) }

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
                                return Err(de::Error::duplicate_field("credits"))
                            }

                            credits = Some(map.next_value()?);
                        },
                        Field::Debits => {
                            if debits.is_some() {
                                return Err(de::Error::duplicate_field("debits"))
                            }

                            debits = Some(map.next_value()?);
                        },
                        Field::RewardState => {
                            if reward_state.is_some() {
                                return Err(de::Error::duplicate_field("rewardstate"))
                            }

                            reward_state = Some(map.next_value()?);
                        },
                        Field::Claims => {
                            if claims.is_some() {
                                return Err(de::Error::duplicate_field("claims"))
                            }

                            claims = Some(map.next_value()?);
                        },
                        Field::LastBlock => {
                            if last_block.is_some() {
                                return Err(de::Error::duplicate_field("lastblock"))
                            }

                            last_block = Some(map.next_value()?);
                        },
                        Field::BlockArchive => {
                            if block_archive.is_some() {
                                return Err(de::Error::duplicate_field("blockarchive"))
                            }

                            block_archive = Some(map.next_value()?);
                        }
                    }
                }
                let credits = credits.ok_or_else(|| de::Error::missing_field("credits"))?;
                let debits = debits.ok_or_else(|| de::Error::missing_field("debits"))?;
                let reward_state = reward_state.ok_or_else(|| de::Error::missing_field("rewardstate"))?;
                let claims = claims.ok_or_else(|| de::Error::missing_field("claims"))?;
                let last_block = last_block.ok_or_else(|| de::Error::missing_field("lastblock"))?;
                let block_archive = block_archive.ok_or_else(|| de::Error::missing_field("blockarchive"))?;

                let mut state = PickleDb::new("temp.db", PickleDbDumpPolicy::NeverDump, SerializationMethod::Bin);
                
                let credits_result = state.set("credits", &credits);
                let debits_result = state.set("debits", &debits);
                let reward_state_result = state.set("reward_state", &reward_state);
                let claims_result = state.set("claims", &claims);
                let last_block_result = state.set("last_block", &last_block);
                let block_archive = state.set("block_archive", &block_archive);

                if let Err(e) = credits_result { println!("Error setting to state in deserializer: {}", e) }
                if let Err(e) = debits_result { println!("Error setting to state in deserializer: {}", e) }
                if let Err(e) = reward_state_result { println!("Error setting to state in deserializer: {}", e) }
                if let Err(e) = claims_result { println!("Error setting to state in deserializer: {}", e) }
                if let Err(e) = last_block_result { println!("Error setting to state in deserializer: {}", e) }
                if let Err(e) = block_archive { println!("Error setting to state in deserializer: {}", e) }

                Ok(NetworkState { state })
            }
        }
    
        const FIELDS: &[&str] = &["credits", "debits", "rewardstate", "claims", "lastblock", "blockarchive"];
        deserializer.deserialize_struct("NetworkState", FIELDS, NetworkStateVisitor)
    
    }
}

impl Serialize for PendingNetworkState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map: HashMap<&str, String> = HashMap::new();

        let credits = serde_json::to_string::<HashMap<String, HashMap<String, u128>>>(
            &self.state.get("credits").unwrap()
        ).unwrap();

        let debits = serde_json::to_string::<HashMap<String, HashMap<String, u128>>>(
            &self.state.get("debits").unwrap()
        ).unwrap();

        let reward_state = serde_json::to_string::<RewardState>(
            &self.state.get("reward_state").unwrap()
        ).unwrap();

        let claims = serde_json::to_string::<HashMap<String, HashMap<u128, Claim>>>(
            &self.state.get("claims").unwrap()
        ).unwrap();

        let last_block = serde_json::to_string::<Block>(
            &self.state.get("last_block").unwrap()
        ).unwrap();
        
        let block_archive = serde_json::to_string::<HashMap<String, HashMap<u128, Block>>>(
            &self.state.get("block_archive").unwrap()
        ).unwrap();

        map.insert("credits", credits);
        map.insert("debits", debits);
        map.insert("reward_state", reward_state);
        map.insert("claims", claims);
        map.insert("last_block", last_block);
        map.insert("block_archive", block_archive);
        
        let mut serialized_map = serializer.serialize_map(Some(map.len()))?;

        for (k, v) in map {
            serialized_map.serialize_entry(&k, &v)?;
        }

        serialized_map.end()
    }
}

impl<'de> Deserialize<'de> for PendingNetworkState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de> 
    {

        enum Field { Credits, Debits, RewardState, Claims, LastBlock }

        impl<'a, 'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de> 
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`accountstate` or `rewardstate")
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
                            _ => Err(de::Error::unknown_field(value, FIELDS))
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct NetworkStateVisitor;

        impl<'de> Visitor<'de> for NetworkStateVisitor {
            type Value = PendingNetworkState;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct NetworkState")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<PendingNetworkState, V::Error>
            where
                V: SeqAccess<'de>,
            
            {
                let credits = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let debits = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let reward_state = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let claims = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let last_block = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                
                let mut state = PickleDb::new("temp.db", PickleDbDumpPolicy::NeverDump, SerializationMethod::Bin);
                let credits_result = state.set("credits", &credits);
                let debits_result = state.set("debits", &debits);
                let reward_state_result = state.set("reward_state", &reward_state);
                let claims_result = state.set("claims", &claims);
                let last_block_result = state.set("last_block", &last_block);

                if let Err(e) = credits_result {println!("Error setting to state: {}", e) }
                if let Err(e) = debits_result {println!("Error setting to state: {}", e) }
                if let Err(e) = reward_state_result { println!("Error setting to state: {}", e) }
                if let Err(e) = claims_result { println!("Error setting to state: {}", e) }
                if let Err(e) = last_block_result { println!("Error setting to state: {}", e) }

                Ok(PendingNetworkState { state })
            }

            fn visit_map<V>(self, mut map: V) -> Result<PendingNetworkState, V::Error>
            where
                V: MapAccess<'de>,

            {
                let mut credits = None;
                let mut debits = None;
                let mut reward_state = None;
                let mut claims = None;
                let mut last_block = None;


                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Credits => {
                            if credits.is_some() {
                                return Err(de::Error::duplicate_field("credits"))
                            }

                            credits = Some(map.next_value()?);
                        },
                        Field::Debits => {
                            if debits.is_some() {
                                return Err(de::Error::duplicate_field("debits"))
                            }

                            debits = Some(map.next_value()?);
                        },
                        Field::RewardState => {
                            if reward_state.is_some() {
                                return Err(de::Error::duplicate_field("rewardstate"))
                            }

                            reward_state = Some(map.next_value()?);
                        },
                        Field::Claims => {
                            if claims.is_some() {
                                return Err(de::Error::duplicate_field("claims"))
                            }

                            claims = Some(map.next_value()?);
                        },
                        Field::LastBlock => {
                            if last_block.is_some() {
                                return Err(de::Error::duplicate_field("lastblock"))
                            }

                            last_block = Some(map.next_value()?);
                        },
                    }
                }
                let credits = credits.ok_or_else(|| de::Error::missing_field("credits"))?;
                let debits = debits.ok_or_else(|| de::Error::missing_field("debits"))?;
                let reward_state = reward_state.ok_or_else(|| de::Error::missing_field("rewardstate"))?;
                let claims = claims.ok_or_else(|| de::Error::missing_field("claims"))?;
                let last_block = last_block.ok_or_else(|| de::Error::missing_field("lastblock"))?;
                
                let mut state = PickleDb::new("temp.db", PickleDbDumpPolicy::NeverDump, SerializationMethod::Bin);
                
                let credits_result = state.set("credits", &credits);
                let debits_result = state.set("debits", &debits);
                let reward_state_result = state.set("reward_state", &reward_state);
                let claims_result = state.set("claims", &claims);
                let last_block_result = state.set("last_block", &last_block);

                if let Err(e) = credits_result { println!("Error setting to state in deserializer: {}", e) }
                if let Err(e) = debits_result { println!("Error setting to state in deserializer: {}", e) }
                if let Err(e) = reward_state_result { println!("Error setting to state in deserializer: {}", e) }
                if let Err(e) = claims_result { println!("Error setting to state in deserializer: {}", e) }
                if let Err(e) = last_block_result { println!("Error setting to state in deserializer: {}", e) }

                Ok(PendingNetworkState { state })
            }
        }
    
        const FIELDS: &[&str] = &["credits", "debits", "rewardstate", "claims", "lastblock"];
        deserializer.deserialize_struct("PendingNetworkState", FIELDS, NetworkStateVisitor)
    
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_new_network_state() {

    }

    #[test]
    fn test_restored_network_state() {

    }

    #[test]
    fn test_valid_network_state() {

    }

    #[test]
    fn test_invalid_network_state() {

    }

    #[test]
    fn test_network_state_updated_locally() {

    }

    #[test]
    fn test_network_state_updated_via_gossip() {
        
    }
}