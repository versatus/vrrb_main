// use crate::{
//     account::{
//         AccountState, 
//         WalletAccount
//     }, 
//     block::Block, 
//     claim::Claim, 
//     reward::RewardState, 
//     state::NetworkState, 
//     validator::Validator,
//     txn::Txn,
// };

pub fn decay_calculator(initial: u128, epochs: u128) -> f64 {
    let b: f64 = 1.0f64 / initial as f64;
    let ln_b = b.log10();

    ln_b / epochs as f64
}

// pub fn claim_homesteading_test_setup(state_path: &str) -> Option<(WalletAccount, AccountState, NetworkState, Claim)> {
    
//     let mut account_state = AccountState::start();
//     let mut network_state = NetworkState::restore(state_path);
//     let reward_state = RewardState::start(&mut network_state);
    

//     let mut homesteader_wallet = WalletAccount::new(
//         &mut account_state, &mut network_state);

//     let (_genesis_block, updated_account_state) = Block::genesis(
//         reward_state, 
//         homesteader_wallet.address.clone(), 
//         &mut account_state, 
//         &mut network_state,
//     ).unwrap();

//     account_state = updated_account_state;

//     let claim_state = account_state.clone().claim_state;
//     let (_ts, claim_to_homestead) = claim_state.claims
//                                                     .iter()
//                                                     .min_by_key(|x| x.0)
//                                                     .unwrap();

//     let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
//                                                     &mut homesteader_wallet, 
//                                                     &mut account_state.clone().claim_state, 
//                                                     &mut account_state.clone(), 
//                                                     &mut network_state).unwrap();
    
//     homesteader_wallet = updated_wallet;
//     account_state = updated_account_state;

//     Some((homesteader_wallet, account_state, network_state, claim_to_homestead.to_owned()))

// }

// pub fn txn_test_setup(state_path: &str) -> Option<(
//     WalletAccount, 
//     WalletAccount, 
//     AccountState, 
//     NetworkState, 
//     RewardState,
//     Txn,
//     Vec<Validator>
// )> 
// {
//     let mut account_state = AccountState::start();
//     let mut network_state = NetworkState::restore(state_path);
//     let reward_state = RewardState::start(&mut network_state);
    
//     let mut wallet_1 = WalletAccount::new(
//         &mut account_state, 
//         &mut network_state
//     );

//     let mut wallet_2 = WalletAccount::new(
//         &mut account_state,
//         &mut network_state,
//     );

//     wallet_1 = wallet_1.get_balance(account_state.clone()).unwrap();
//     wallet_2 = wallet_2.get_balance(account_state.clone()).unwrap();

//     let result = wallet_1.send_txn(
//         &mut account_state, 
//         (wallet_2.address.clone(), 15_u128), 
//         &mut network_state);

//     match result {
//         Ok(txn) => {
//             println!("{:?}", txn);
//         }
//         Err(e) => println!("Error attempting to send txn to receiver: {} -> {}", 
//             wallet_2.address, 
//             e
//         )
//     }

//     let txn_id = account_state.pending.keys().cloned().collect::<Vec<String>>()[0].clone();
//     let txn = account_state.pending.get(&txn_id).unwrap().0.clone();
//     let validators_vec = account_state.pending.get(&txn_id).unwrap().1.clone();

//     let (_block, updated_account_state) = Block::genesis(
//         reward_state, 
//         wallet_1.address.clone(), 
//         &mut account_state, 
//         &mut network_state,
//     ).unwrap();

//     account_state = updated_account_state;
    
//     Some((wallet_1, wallet_2, account_state, network_state, reward_state, txn, validators_vec))
// }

// pub fn block_test_setup() {

// }

// pub fn claim_acquisition_test_setup() {

// }
