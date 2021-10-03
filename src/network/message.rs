use crate::network::command_utils::Command;
use crate::network::message_types::{MessageType, StateBlock};
use libp2p::gossipsub::GossipsubMessage;

pub const PROPOSAL_EXPIRATION_KEY: &str = "expires";
pub const PROPOSAL_YES_VOTE_KEY: &str = "yes";
pub const PROPOSAL_NO_VOTE_KEY: &str = "no";

pub fn process_message(message: GossipsubMessage, node_id: String) -> Option<Command> {
    if let Some(message) = MessageType::from_bytes(
        &hex::decode(&String::from_utf8_lossy(&message.data).into_owned()).unwrap(),
    ) {
        match message.clone() {
            MessageType::TxnMessage { txn, .. } => Some(Command::ProcessTxn(txn)),
            MessageType::BlockMessage { block, sender_id, .. } => Some(Command::PendingBlock(block, sender_id)),
            MessageType::TxnValidatorMessage { txn_validator, .. } => {
                Some(Command::ProcessTxnValidator(txn_validator))
            }
            MessageType::ClaimMessage { claim, .. } => Some(Command::ProcessClaim(claim)),
            MessageType::GetNetworkStateMessage {
                sender_id,
                requested_from,
                lowest_block,
                ..
            } => {
                if requested_from == node_id {
                    Some(Command::SendState(sender_id, lowest_block))
                } else {
                    None
                }
            },
            MessageType::BlockChunkMessage {
                requestor,
                block_height,
                chunk_number,
                total_chunks,
                data,
                ..
            } => {
                if requestor == node_id {
                    return Some(Command::StoreStateDbChunk(
                        StateBlock(block_height),
                        data,
                        chunk_number as u32,
                        total_chunks as u32,
                    ));
                }
                return None;
            },
            MessageType::NeedGenesisBlock { sender_id, requested_from } => {
                if requested_from == node_id {
                    return Some(Command::SendGenesis(sender_id));
                }
                return None
            }
            _ => None,
        }
    } else {
        None
    }
}
