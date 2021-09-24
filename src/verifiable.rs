use crate::account::AccountState;
use crate::block::Block;
use crate::reward::RewardState;
use crate::state::NetworkState;

pub trait Verifiable {
    fn verifiable(&self) -> bool;

    fn valid_block(
        &self,
        _last_block: &Block,
        _network_state: &NetworkState,
        _reward_state: &RewardState,
    ) -> bool {
        false
    }

    fn valid_genesis(&self, _network_state: &NetworkState, _reward_state: &RewardState) -> bool {
        false
    }

    fn valid_last_hash(&self, _last_block: &Block) -> bool {
        false
    }

    fn valid_state_hash(&self, _network_state: &NetworkState) -> bool {
        false
    }

    fn valid_block_reward(&self, _reward_state: &RewardState) -> bool {
        false
    }

    fn valid_next_block_reward(&self, _reward_state: &RewardState) -> bool {
        false
    }

    fn valid_txns(&self) -> bool {
        false
    }

    fn valid_block_nonce(&self, _last_block: &Block) -> bool {
        false
    }

    fn valid_txn(&self, _network_state: &NetworkState, _account_state: &AccountState) -> bool {
        false
    }

    fn valid_txn_signature(&self) -> bool {
        false
    }

    fn valid_amount(&self, _network_state: &NetworkState, _account_state: &AccountState) -> bool {
        false
    }

    fn check_double_spend(&self, _account_state: &AccountState) -> bool {
        false
    }
}
