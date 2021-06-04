use vrrb_main::{account::{
        AccountState, 
        WalletAccount, 
        StateOption
    }, block::Block, claim::{ClaimState}, reward::{RewardState}, state::NetworkState};
use std::{collections::HashMap, thread, time};
fn main() {
    
    let mut account_state = AccountState::start();
    let claim_state = ClaimState::start();
    let reward_state = RewardState::start();
    let mut wallet = WalletAccount::new();
    let mut network_state = NetworkState::new();
    account_state = account_state.update(StateOption::NewAccount(wallet.clone()), &mut network_state).unwrap();
    let (genesis_block, updated_account_state)  = Block::genesis(
        reward_state.clone(), 
        &mut wallet.clone(), 
        &mut account_state.clone(), &mut network_state).unwrap();
    account_state = updated_account_state;
    wallet = wallet.get_balance(account_state.clone()).unwrap();

    for (_, claim) in &account_state.clone().claim_state.claims {
        let (
            updated_wallet, 
            updated_account_state
        ) = claim.homestead(&mut wallet.clone(), &mut claim_state.clone(), &mut account_state.clone(), &mut network_state).unwrap();
        wallet = updated_wallet;
        account_state = updated_account_state;
    }

    let mut last_block = genesis_block;
    thread::sleep(time::Duration::from_secs(1));
    for claim in &wallet.clone().claims {
        let data = HashMap::new();
        let (new_block, updated_account_state) = Block::mine(
            &reward_state, 
            claim.clone().unwrap(), 
            last_block, 
            data, 
            &mut wallet.clone(), 
            &mut account_state.clone(), 
            &mut account_state.clone().claim_state.clone(), &mut network_state)
                .unwrap()
                .unwrap();
        last_block = new_block;
        account_state = updated_account_state;
        wallet = wallet.get_balance(account_state.clone()).unwrap();
        wallet = wallet.remove_mined_claims(&last_block);
    }

    let mut receiver_wallet1 = WalletAccount::new();
    let receiver_wallet2 = WalletAccount::new();
    account_state = account_state.update(StateOption::NewAccount(receiver_wallet1.clone()), &mut network_state).unwrap();
    account_state = account_state.update(StateOption::NewAccount(receiver_wallet2.clone()), &mut network_state).unwrap();

    let (
        updated_wallet, 
        updated_account_state) = wallet.send_txn(
            &mut account_state, (receiver_wallet1.address.clone(), 15), &mut network_state).unwrap();
    wallet = updated_wallet;
    account_state = updated_account_state;
    wallet = wallet.get_balance(account_state.clone()).unwrap();
    receiver_wallet1 = receiver_wallet1.get_balance(account_state.clone()).unwrap();

    println!("{}\n", &wallet);
    println!("{}\n", &receiver_wallet1);

}
