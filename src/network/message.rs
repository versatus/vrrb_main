use crate::block::Block;
use crate::network::command_utils::Command;
use crate::network::message_types::MessageType;
use crate::state::NetworkState;
use crate::txn::Txn;
use libp2p::gossipsub::GossipsubMessage;
use std::thread;

pub const PROPOSAL_EXPIRATION_KEY: &str = "expires";
pub const PROPOSAL_YES_VOTE_KEY: &str = "yes";
pub const PROPOSAL_NO_VOTE_KEY: &str = "no";

pub fn process_message(message: GossipsubMessage) -> Option<Command> {
    if let Some(message) = MessageType::from_bytes(
        &hex::decode(&String::from_utf8_lossy(&message.data).into_owned()).unwrap(),
    ) {
        match message.clone() {
            MessageType::TxnMessage { txn, .. } => {
                println!("received txn");
                Some(Command::ProcessTxn(txn))
            }
            MessageType::BlockMessage { block, .. } => {
                println!("Received Block");
                Some(Command::PendingBlock(block))
            }
            MessageType::GetNetworkStateMessage { .. } => {None}
            MessageType::NetworkStateDataBaseMessage { .. } => {None}
            MessageType::TxnValidatorMessage { .. } => {None}
            MessageType::InvalidBlockMessage { .. } => {None}
            MessageType::NeedBlocksMessage { .. } => {None}
            MessageType::MissingBlock { .. } => {None}
            _ => {None}
        }
    } else {
        None
    }
}

pub fn process_txn_message(txn: Txn) {
    println!("Txn: {:?}", txn);
}

pub fn process_block_message(block: Block) {
    println!("Block: {:?}", block);
}
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
