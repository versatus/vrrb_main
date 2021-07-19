use std::fmt::{self, Debug, Error, Formatter};
use crate::{account::AccountState, reward::RewardState};
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

pub struct NetworkState<'a> {
    pub state: PickleDb,
    _marker: &'a u8
}

impl<'a> NetworkState<'a> {

    pub fn update<T: Serialize>(
        &mut self, state_obj: T, state_obj_type: &str) -> Result<(), error::ErrorType> {
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
            _marker: &1
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

    pub fn from_bytes(data: &'a [u8]) -> NetworkState<'a> {
        serde_json::from_slice::<NetworkState>(data).unwrap()
    }
}

impl<'a> Clone for NetworkState<'a> {
    fn clone(&self) -> NetworkState<'a> {
        let mut cloned_db = PickleDb::new("temp.db", PickleDbDumpPolicy::NeverDump, SerializationMethod::Bin);
        let account_state: Option<AccountState> = self.state.get("account_state");
        let reward_state: Option<RewardState> = self.state.get("reward_state");

        if let Some(account_state) = account_state {
            let cloned_result = cloned_db.set("account_state", &account_state);
            if let Err(e) = cloned_result {println!("Error setting to cloned_state: {}", e)}
        }

        if let Some(reward_state) = reward_state {
            let cloned_result = cloned_db.set("reward_state", &reward_state);

            if let Err(e) = cloned_result {println!("Error setting to cloned_state: {}", e)}
        }

        NetworkState {
            state: cloned_db,
            _marker: &1
        }
    }
}

impl<'a> Debug for NetworkState<'a> {
    fn fmt(&self, _f: &mut Formatter) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a> Serialize for NetworkState<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map: HashMap<&'a str, String> = HashMap::new();

        let account_state = serde_json::to_string::<AccountState>(
            &self.state.get("account_state").unwrap()
        ).unwrap();

        let reward_state = serde_json::to_string::<AccountState>(
            &self.state.get("account_state").unwrap()
        ).unwrap();

        map.insert("account_state", account_state);
        map.insert("reward_state", reward_state);

        let mut serialized_map = serializer.serialize_map(Some(map.len()))?;

        for (k, v) in map {
            serialized_map.serialize_entry(&k, &v)?;
        }

        serialized_map.end()
    }
}

impl<'de> Deserialize<'de> for NetworkState<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de> 
    {

        enum Field { AccountState, RewardState }

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
                            "accountstate" => Ok(Field::AccountState),
                            "rewardstate" => Ok(Field::RewardState),
                            _ => Err(de::Error::unknown_field(value, FIELDS))
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct NetworkStateVisitor;

        impl<'de> Visitor<'de> for NetworkStateVisitor {
            type Value = NetworkState<'de>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct NetworkState")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<NetworkState<'de>, V::Error>
            where
                V: SeqAccess<'de>,
            
            {
                let account_state = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let reward_state = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                
                let mut state = PickleDb::new("temp.db", PickleDbDumpPolicy::NeverDump, SerializationMethod::Bin);
                let account_state_result = state.set("account_state", &account_state);
                let reward_state_result = state.set("reward_state", &reward_state);

                if let Err(e) = account_state_result {println!("Error setting to state: {}", e)}

                if let Err(e) = reward_state_result { println!("Error setting to state: {}", e)}

                Ok(NetworkState { state, _marker: &1 })
            }

            fn visit_map<V>(self, mut map: V) -> Result<NetworkState<'de>, V::Error>
            where
                V: MapAccess<'de>,

            {
                let mut account_state = None;
                let mut reward_state = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::AccountState => {
                            if account_state.is_some() {
                                return Err(de::Error::duplicate_field("accountstate"))
                            }

                            account_state = Some(map.next_value()?);
                        },
                        Field::RewardState => {
                            if reward_state.is_some() {
                                return Err(de::Error::duplicate_field("rewardstate"))
                            }

                            reward_state = Some(map.next_value()?);
                        }
                    }
                }

                let account_state = account_state.ok_or_else(|| de::Error::missing_field("accountstate"))?;
                let reward_state = reward_state.ok_or_else(|| de::Error::missing_field("rewardstate"))?;


                let mut state = PickleDb::new("temp.db", PickleDbDumpPolicy::NeverDump, SerializationMethod::Bin);
                let account_state_result = state.set("account_state", &account_state);
                let reward_state_result = state.set("reward_state", &reward_state);

                if let Err(e) = account_state_result { println!("Error setting to state in deserializer: {}", e) }

                if let Err(e) = reward_state_result {println!("Error setting to state in deserializer: {}", e)}
                
                Ok(NetworkState { state, _marker: &1 })
            }
        }
    
        const FIELDS: &[&str] = &["accountstate", "rewardstate"];
        deserializer.deserialize_struct("NetworkState", FIELDS, NetworkStateVisitor)
    
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