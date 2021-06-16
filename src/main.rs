use vrrb_main::{account::{
        AccountState, 
        WalletAccount
    }, block::Block, claim::{ClaimState}, reward::{RewardState}, state::NetworkState};
use std::{collections::HashMap, sync::mpsc, thread};
fn main() {
    println!("Welcome to VRRB");
    let mut account_state = AccountState::start();
    let mut network_state = NetworkState::restore("vrrb_network_state.db");
    let mut reward_state = RewardState::start(&mut network_state);

    let (mut wallet, updated_account_state) = WalletAccount::new(
        &mut account_state, 
        &mut network_state
    );
    account_state = updated_account_state;

    let (genesis_block, updated_account_state) = Block::genesis(
        reward_state, 
        &mut wallet, 
        &mut account_state, 
        &mut network_state    
    ).unwrap();

    account_state = updated_account_state;
    wallet = wallet.get_balance(account_state.clone()).unwrap();

    let mut last_block = genesis_block;
    
    loop {
        

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut thread_account_state = account_state;
            let mut thread_wallet = wallet;
            let mut thread_network_state = network_state;

            for (_ts, claim) in thread_account_state.clone().claim_state.claims {
                let result = claim.to_owned().homestead(
                    &mut thread_wallet, 
                    &mut thread_account_state.clone().claim_state, 
                    &mut thread_account_state, 
                    &mut thread_network_state
                );

                let (updated_wallet, updated_account_state) = result.unwrap();
                thread_wallet = updated_wallet;
                thread_account_state = updated_account_state;
            }

            tx.send((thread_wallet, thread_account_state, thread_network_state)).unwrap();

        }).join().unwrap();

        let (mut loop_wallet, loop_account_state, loop_network_state) = rx.recv().unwrap();
        loop_wallet = loop_wallet.get_balance(loop_account_state.clone()).unwrap();

        println!("{}\n\n", &loop_wallet);

        wallet = loop_wallet;
        account_state = loop_account_state;
        network_state = loop_network_state;

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut thread_account_state = account_state.clone();
            let mut thread_wallet = wallet;
            let mut thread_network_state = network_state;
            let thread_reward_state = reward_state;
            let mut last_block = last_block;

            for claim in &thread_wallet.clone().claims {
                let new_block = Block::mine(
                    &thread_reward_state, 
                    claim.clone().unwrap(), 
                    last_block, 
                    HashMap::new(), 
                    &mut thread_wallet,
                    &mut thread_account_state,
                    &mut thread_network_state
                );
                let (block, updated_account_state) = new_block.unwrap().unwrap();
                
                println!("{}\n\n", &block);
                last_block = block;
                thread_wallet = thread_wallet.remove_mined_claims(&last_block);
                thread_account_state = updated_account_state;
            }

            tx.send((thread_wallet, thread_account_state, thread_network_state, thread_reward_state, last_block)).unwrap();

        }).join().unwrap();

        let (
            mut loop_wallet, 
            loop_account_state, 
            loop_network_state,
            loop_reward_state,
            loop_last_block
        ) = rx.recv().unwrap();
    
        loop_wallet = loop_wallet.get_balance(loop_account_state.clone()).unwrap();
        
        println!("{}\n\n", &loop_wallet);

        wallet = loop_wallet;
        account_state = loop_account_state;
        network_state = loop_network_state;
        reward_state = loop_reward_state;
        last_block = loop_last_block;

    }
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
            ) = claim.to_owned().homestead(
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
            &mut network_state)
                .unwrap()
                .unwrap();
        last_block = new_block;
        account_state = updated_account_state;
        wallet = wallet.get_balance(account_state.clone()).unwrap();
        wallet = wallet.remove_mined_claims(&last_block);
    }
}