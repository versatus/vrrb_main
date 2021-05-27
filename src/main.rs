use vrrb_main::account::WalletAccount;
use vrrb_main::block::Block;
use vrrb_main::claim::Claim;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::thread;

fn main() {
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