use crate::network::message_types::MessageType;
use crate::state::NetworkState;
use crate::block::Block;
use libp2p::gossipsub::GossipsubMessage;

pub const PROPOSAL_EXPIRATION_KEY: &str = "expires";
pub const PROPOSAL_YES_VOTE_KEY: &str = "yes";
pub const PROPOSAL_NO_VOTE_KEY: &str = "no";

pub fn process_message(message: GossipsubMessage) {
    if let Some(message) = MessageType::from_bytes(
        &hex::decode(&String::from_utf8_lossy(&message.data).into_owned()).unwrap(),
    ) {
        match message.clone() {
            MessageType::TxnMessage { txn, .. } => {
                println!("Received txn: {:?}", txn);
            }
            MessageType::ClaimMessage { .. } => {}
            MessageType::BlockMessage { .. } => {}
            MessageType::VIPMessage { .. } => {}
            MessageType::GetNetworkStateMessage { .. } => {}
            MessageType::NetworkStateDataBaseMessage { .. } => {}
            MessageType::TxnValidatorMessage { .. } => {}
            MessageType::ClaimValidator { .. } => {}
            MessageType::InvalidBlockMessage { .. } => {}
            MessageType::BlockVoteMessage { .. } => {}
            MessageType::AccountPubkeyMessage { .. } => {}
            MessageType::NeedBlocksMessage { .. } => {}
            MessageType::MissingBlock { .. } => {}
            _ => {}
        }
    }
}

pub fn process_txn_message() {}

pub fn process_confirmed_block(_network_state: &mut NetworkState, _block: &Block) {}

pub fn process_vrrb_ip_message() {}

pub fn process_get_block_message() {}

pub fn process_txn_validator_message() {}

pub fn process_claim_for_sale_message() {}

pub fn process_claim_staked_message() {}

pub fn process_claim_sold_message() {}

pub fn process_claim_sold_validator_message() {}

pub fn process_claim_stake_validator_message() {}

pub fn process_claim_for_sale_validator_message() {}

pub fn process_invalid_block_message() {}

pub fn process_state_db_message() {}

pub fn process_network_state_complete_message() {}

pub fn structure_message() {}

pub fn publish_message() {}
