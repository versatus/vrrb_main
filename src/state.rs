use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use serde::{Serialize};
pub struct NetworkState {
    state: PickleDb
}

impl NetworkState {
    pub fn new() -> NetworkState {
        let db = PickleDb::new(
            "vrrb_network_state.db", 
            PickleDbDumpPolicy::AutoDump, 
            SerializationMethod::Bin,
        );

        NetworkState {
            state: db
        }
    }

    pub fn update<T: Serialize>(&mut self, state_obj: T, state_obj_type: &str) {
        self.state.set(state_obj_type, &state_obj).unwrap()
    }

    pub fn restore(path: &str) -> Option<NetworkState> {
        let db = PickleDb::load_bin(path, PickleDbDumpPolicy::AutoDump).unwrap();

        //TODO: Add error propagation so that if there's no network state db that exists
        // it creates a new one.
        Some(NetworkState {
            state: db
        })
    }
}