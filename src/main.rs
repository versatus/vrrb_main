use vrrb_main::{
    account::{AccountState, WalletAccount, StateOption}, 
    claim::{ClaimState},
    block::Block,
    reward::{RewardState},
    // txn::Txn
};
use std::collections::HashMap;


fn main() {
    let mut acct_state = AccountState::start();
    let mut wallet = WalletAccount::new();
    let mut claim_state = ClaimState::start();
    let reward_state = RewardState::start();
    acct_state = acct_state.update(StateOption::NewAccount(wallet.clone())).unwrap();
    let (genesis_block, mut acct_state) = Block::genesis(
        reward_state.clone(), 
        &mut wallet.clone(), 
        &mut acct_state.clone()).unwrap();
    wallet = wallet.get_balance(acct_state.clone()).unwrap();

    for (_ts, claim) in acct_state.clone().claim_state.claims {
        if claim.available {
            let (
                _claim, 
                new_wallet, 
                account_state, 
                _claim_state
            ) = claim.homestead(
                    &mut wallet.clone(), 
                    &mut claim_state, 
                    &mut acct_state)
                        .unwrap();
            wallet = new_wallet;
            acct_state = account_state;
        }
    }
    let mut last_block = genesis_block; 
    for (_i, claim) in wallet.claims.iter().enumerate() {
        let claim = claim.clone().unwrap();
        let (block, new_acct_state) = Block::mine(
            &reward_state.clone(), 
            claim, 
            last_block, 
            HashMap::new(), 
            &mut wallet.clone(), 
            &mut acct_state.clone(), 
            &claim_state.clone()).unwrap().unwrap();
        
        acct_state = new_acct_state;
        last_block = block.clone();
        println!("{:?}\n", block.clone())
    }

    wallet = wallet.get_balance(acct_state).unwrap();
    println!("{}", wallet);

}