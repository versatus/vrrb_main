use vrrb_main::account::WalletAccount;
use vrrb_main::block::Block;
use vrrb_main::claim::Claim;
use vrrb_main::txn::Txn;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::thread;

fn main() {
    let wallet: WalletAccount = WalletAccount::new();
    let wallet2: WalletAccount = WalletAccount::new();
    let receiver: String = wallet2.address;

    // {
    //     let mut time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    //     let mut claim_vec: Vec<Claim> = vec![];
    //     for _ in 0..21 {
    //         time = time + Duration::from_secs(5);
    //         let claim = Claim::new(time.as_nanos());
    //         claim_vec.push(claim);
    //     }

    //     for claim in claim_vec.iter_mut() {
    //         *claim = claim.homestead(wallet.clone());
    //     }
    // }
    // {   
    //     let start_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    //     let txn = Txn::new(wallet, receiver, 30i128);
        
    //     let end_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    //     println!("{}", end_time.as_secs() - start_time.as_secs());
    //     println!("{:?}", txn);
    // }
    {
        let data = HashMap::new();
        let wallet = WalletAccount::new();
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let genesis_block: Block = Block::genesis();
        println!("{:?}", genesis_block);
        let claim_maturation_time = time + Duration::new(1, 0);
        let mut claim = Claim::new(claim_maturation_time.as_nanos());
        claim = claim.homestead(wallet.clone());
        println!("{:?}", claim.clone().current_owner.2.unwrap());
        thread::sleep(Duration::from_secs(2));
        let new_block: Block = Block::mine(claim, genesis_block, data, wallet.clone().public_key.to_string()).unwrap();
        println!("{:?}", new_block);
    }
}