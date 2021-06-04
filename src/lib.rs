pub mod account;
pub mod claim;
pub mod block;
pub mod txn;
pub mod reward;
pub mod vrrbcoin;
pub mod utils;
pub mod state;

#[cfg(test)]
mod tests {
    use crate::{account::{AccountState, StateOption, WalletAccount}, block::Block, claim::ClaimState, reward::RewardState, state::NetworkState};
    #[test]
    fn homestead_claims() {
        let mut acct_state = AccountState::start();
        let mut wallet = WalletAccount::new();
        let mut claim_state = ClaimState::start();
        let reward_state = RewardState::start();
        let mut network_state = NetworkState::new();
        acct_state = acct_state.update(StateOption::NewAccount(wallet.clone()), &mut network_state).unwrap();
        let (_genesis_block, mut acct_state) = Block::genesis(
            reward_state.clone(), 
            &mut wallet.clone(), 
            &mut acct_state.clone(), &mut network_state).unwrap();
        wallet = wallet.get_balance(acct_state.clone()).unwrap();

        for (_ts, claim) in acct_state.clone().claim_state.claims {
            if claim.available {
                let (
                    new_wallet, 
                    account_state, 
                ) = claim.homestead(
                        &mut wallet.clone(), 
                        &mut claim_state, 
                        &mut acct_state, &mut network_state)
                            .unwrap();
                wallet = new_wallet;
                acct_state = account_state;
            }
        }
        assert_eq!(wallet.claims.len(), 20);
    }
}