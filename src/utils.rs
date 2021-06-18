use crate::{account::{AccountState, WalletAccount}, block::Block, claim::Claim, reward::RewardState, state::NetworkState};

pub fn decay_calculator(initial: u128, epochs: u128) -> f64 {
    let b: f64 = 1.0f64 / initial as f64;
    let ln_b = b.log10();

    ln_b / epochs as f64
}

pub fn claim_test_setup() -> Option<(WalletAccount, AccountState, NetworkState, Claim)> {
    
        let mut account_state = AccountState::start();
        let mut network_state = NetworkState::restore("test_invalid_claim_staked.db");
        let reward_state = RewardState::start(&mut network_state);
        

        let (mut homesteader_wallet, updated_account_state) = WalletAccount::new(
            &mut account_state, &mut network_state);

        account_state = updated_account_state;

        let (_genesis_block, updated_account_state) = Block::genesis(
            reward_state, 
            &mut homesteader_wallet.clone(), 
            &mut account_state, 
            &mut network_state
        ).unwrap();

        account_state = updated_account_state;

        let claim_state = account_state.clone().claim_state;
        let (_ts, claim_to_homestead) = claim_state.claims
                                                        .iter()
                                                        .min_by_key(|x| x.0)
                                                        .unwrap();

        let (updated_wallet, updated_account_state) = claim_to_homestead.clone().homestead(
                                                        &mut homesteader_wallet, 
                                                        &mut account_state.clone().claim_state, 
                                                        &mut account_state.clone(), 
                                                        &mut network_state).unwrap();
        
        homesteader_wallet = updated_wallet;
        account_state = updated_account_state;

        Some((homesteader_wallet, account_state, network_state, claim_to_homestead.to_owned()))

}

pub fn txn_test_setup() {

}