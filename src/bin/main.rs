use vrrb_lib::network::{node::{Node, NodeAuth}, voting::BallotBox};
use vrrb_lib::account::{
    WalletAccount, AccountState
};
use vrrb_lib::reward::RewardState;
use vrrb_lib::state::NetworkState;
use std::{
    error::Error,
    sync::{Arc, Mutex},
    collections::HashMap,
};

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let account_state = Arc::new(Mutex::new(AccountState::start()));
    let network_state = Arc::new(Mutex::new(NetworkState::restore("test.db")));
    let reward_state = Arc::new(Mutex::new(RewardState::start(Arc::clone(&network_state))));
    let node_type = NodeAuth::Full;
    let wallet = Arc::new(
        Mutex::new(
            WalletAccount::new(
                Arc::clone(&account_state), 
                Arc::clone(&network_state)
            )));
    let ballot_box = Arc::new(
        Mutex::new(
            BallotBox::new(
                HashMap::new(), 
                HashMap::new(), 
                1, 
                HashMap::new(), 
                HashMap::new()
            )));

    let node = Node::start(wallet, account_state, network_state, reward_state, ballot_box, node_type);
    
    node.await.unwrap();

    Ok(())
}