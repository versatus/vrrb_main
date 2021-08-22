use vrrb_lib::network::{node::{Node, NodeAuth}, voting::BallotBox};
use vrrb_lib::account::AccountState;
use vrrb_lib::wallet::WalletAccount;
use vrrb_lib::reward::RewardState;
use vrrb_lib::state::NetworkState;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use rand::prelude::*;

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {


    let account_state = Arc::new(Mutex::new(AccountState::start()));
    let path = "test.json".to_string();
    println!("{}", path);
    let network_state = Arc::new(Mutex::new(NetworkState::restore(&path)));
    let reward_state = Arc::new(Mutex::new(RewardState::start()));
    let node_type = NodeAuth::Full;
    let wallet = Arc::new(Mutex::new(WalletAccount::new(Arc::clone(&account_state))));
    let ballot_box = Arc::new(Mutex::new(BallotBox::new(HashMap::new(), HashMap::new(), 1, HashMap::new(), HashMap::new())));
    let mut rng = rand::thread_rng();
    let path_suffix = rng.gen::<u32>();
    let cache_path = format!("{}_{}.{}", "test", path_suffix, "json");

    let node = Node::start(ballot_box, node_type, wallet, account_state, network_state, reward_state, cache_path);
    
    node.await.unwrap();

    Ok(())
}
