use vrrb_main::{account::{
        AccountState, 
        WalletAccount
    }, block::Block, claim::{ClaimState}, reward::{RewardState}, state::NetworkState};
use std::{collections::HashMap};
fn main() {

    let mut account_state = AccountState::start();
    let mut network_state = NetworkState::restore("vrrb_network_state.db");
    let mut _reward_state = RewardState::start(&mut network_state);
    println!("{:?}", _reward_state);
    let db_iter = network_state.state.iter();
    for i in db_iter {
        match i.get_value::<AccountState>() {
            Some(ast) => account_state = ast,
            None => (),
        }
        match i.get_value::<RewardState>() {
            Some(rst) => _reward_state = rst,
            None => (),
        }
    }
    let _claim_state = account_state.claim_state.clone();
    let (_wallet, updated_account_state) = WalletAccount::new(&mut account_state, &mut network_state);
    account_state = updated_account_state;

    println!("{:?}\n\n", &account_state);
}


#[allow(dead_code)]
fn test_txn(sender: WalletAccount, account_state: &mut AccountState, network_state: &mut NetworkState) {
    let (mut receiver_wallet1, mut updated_account_state) = WalletAccount::new(account_state, network_state);
    let mut wallet = sender;
    let mut account_state = &mut updated_account_state;
    let (_receiver_wallet2, mut updated_account_state) = WalletAccount::new(account_state, network_state);
    account_state = &mut updated_account_state;
    let (
        updated_wallet, 
        mut updated_account_state
    ) = wallet.send_txn(
            &mut account_state, 
            (receiver_wallet1.address.clone(), 15), 
            network_state).unwrap();

    wallet = updated_wallet;
    account_state = &mut updated_account_state;
    wallet = wallet.get_balance(account_state.clone()).unwrap();
    receiver_wallet1 = receiver_wallet1.get_balance(account_state.clone()).unwrap();

    println!("{}\n", &wallet);
    println!("{}\n", &receiver_wallet1);
}

#[allow(dead_code)]
fn test_mine_genesis(
    wallet: &mut WalletAccount, 
    reward_state: &mut RewardState, 
    account_state: &mut AccountState, 
    network_state: &mut NetworkState
) {
    let (genesis_block, updated_account_state) = Block::genesis(
        reward_state.clone(), 
        wallet, 
        account_state, 
        network_state
    ).unwrap();

    println!("{:?}\n\n", &genesis_block);
    println!("{:?}\n\n", &updated_account_state);
}

#[allow(dead_code)]
fn test_wallet_restore(pk: &str, account_state: AccountState) {
    let restored_pk = account_state
        .accounts_sk
        .get(pk);
    println!("{:?}", &restored_pk);
}

#[allow(dead_code)]
fn test_homestead_claims(
    mut wallet: WalletAccount, 
    mut account_state: AccountState,
    claim_state: ClaimState,
    mut network_state: NetworkState,
) {
    for (_, claim) in &account_state.clone().claim_state.claims {
        let (
            updated_wallet, 
            updated_account_state
            ) = claim.homestead(
                    &mut wallet.clone(), 
                    &mut claim_state.clone(), 
                    &mut account_state.clone(), 
                    &mut network_state
                ).unwrap();

        wallet = updated_wallet;
        account_state = updated_account_state;
    }
    println!("{}\n\n", &wallet);
    println!("{:?}\n\n", &account_state);
}

#[allow(dead_code)]
fn test_mine_blocks(
    mut wallet: WalletAccount,
    mut account_state: AccountState,
    mut network_state: NetworkState,
    mut last_block: Block,
    reward_state: RewardState
) {
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
}