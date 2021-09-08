use rand::Rng;
use ritelinked::LinkedHashMap;
use std::sync::{Arc, Mutex};
use vrrb_lib::account::AccountState;
use vrrb_lib::network::{
    node::{Node, NodeAuth},
    voting::BallotBox,
};
use vrrb_lib::pool::{Pool, PoolKind};
use vrrb_lib::reward::RewardState;
use vrrb_lib::state::NetworkState;
use vrrb_lib::wallet::WalletAccount;

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    let mut rng = rand::thread_rng();
    let file_suffix: u32 = rng.gen();
    let txn_pool = Pool::new(PoolKind::Txn);
    let claim_pool = Pool::new(PoolKind::Claim);
    let account_state = Arc::new(Mutex::new(AccountState::start(txn_pool, claim_pool)));
    let path = if let Some(path) = std::env::args().nth(2) {
        path
    } else {
        format!("test_{}.db", file_suffix)
    };
    let network_state = Arc::new(Mutex::new(NetworkState::restore(&path)));
    let reward_state = Arc::new(Mutex::new(RewardState::start()));
    let node_type = NodeAuth::Full;
    let wallet = if let Some(secret_key) = std::env::args().nth(4) {
        WalletAccount::restore_from_private_key(secret_key)
    } else {
        WalletAccount::new()
    };
    let wallet = Arc::new(Mutex::new(wallet));
    let ballot_box = Arc::new(Mutex::new(BallotBox::new(
        LinkedHashMap::new(),
        LinkedHashMap::new(),
        1,
        LinkedHashMap::new(),
        LinkedHashMap::new(),
    )));
    let node = Node::start(
        ballot_box,
        node_type,
        wallet,
        account_state,
        network_state,
        reward_state,
    );

    node.await.unwrap();

    Ok(())
}
