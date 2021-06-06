use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use serde::{Serialize};
pub struct NetworkState {
    pub state: PickleDb
}

impl NetworkState {
    
    pub fn update<T: Serialize>(&mut self, state_obj: T, state_obj_type: &str) {
        self.state.set(state_obj_type, &state_obj).unwrap()
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
            state: db
        }    
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