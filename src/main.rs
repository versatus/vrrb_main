use vrrb_main::account::WalletAccount;
use vrrb_main::claim::Claim;
use vrrb_main::txn::Txn;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    let wallet: WalletAccount = WalletAccount::new();
    let wallet2: WalletAccount = WalletAccount::new();
    let receiver: String = wallet2.address;

    {
        let mut time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let mut claim_vec: Vec<Claim> = vec![];
        for _ in 0..21 {
            time = time + Duration::from_secs(5);
            let claim = Claim::new(time.as_nanos());
            claim_vec.push(claim);
        }

        println!("Claim_Vector: {:?}", claim_vec);

        for claim in claim_vec.iter_mut() {
            *claim = claim.homestead(wallet.clone());
        }
        println!("Claim_Vector: {:?}", claim_vec);
    }
    {
        let txn = Txn::new(wallet, receiver, 30i128);
        println!("Txn: {:?}", txn);
    }
}