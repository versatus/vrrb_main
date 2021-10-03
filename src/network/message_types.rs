use crate::block::Block;
use crate::claim::Claim;
use crate::network::node::NodeAuth;
use crate::txn::Txn;
use crate::validator::TxnValidator;
use crate::blockchain::InvalidBlockErrorReason;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StateBlock(pub u128);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MessageType {
    NetworkStateDataBaseMessage {
        object: StateBlock,
        data: Vec<u8>,
        chunk_number: u32,
        total_chunks: u32,
        last_block: u128,
        requestor: String,
        sender_id: String,
    },
    TxnMessage {
        txn: Txn,
        sender_id: String,
    },
    TxnValidatorMessage {
        txn_validator: TxnValidator,
        sender_id: String,
    },
    BlockMessage {
        block: Block,
        sender_id: String,
    },
    BlockChunkMessage {
        sender_id: String,
        requestor: String,
        block_height: u128,
        chunk_number: u128,
        total_chunks: u128,
        data: Vec<u8>,
    },
    ClaimMessage {
        claim: Claim,
        sender_id: String,
    },
    NeedBlocksMessage {
        blocks_needed: Vec<u128>,
        sender_id: String,
    },
    NeedBlockMessage {
        block_last_hash: String,
        sender_id: String,
        requested_from: String,
    },
    MissingBlock {
        block: Block,
        requestor: String,
        sender_id: String,
    },
    GetNetworkStateMessage {
        sender_id: String,
        requested_from: String,
        requestor_node_type: NodeAuth,
        lowest_block: u128,
    },
    InvalidBlockMessage {
        block_height: u128,
        reason: InvalidBlockErrorReason,
        miner_id: String,
        sender_id: String,
    },
    DisconnectMessage {
        sender_id: String,
        pubkey: String,
    },
    NeedGenesisBlock {
        sender_id: String,
        requested_from: String,
    },
    MissingGenesis {
        block: Block,
        requestor: String,
        sender_id: String,
    }
}

impl MessageType {
    pub fn as_bytes(self) -> Vec<u8> {
        serde_json::to_string(&self).unwrap().as_bytes().to_vec()
    }

    pub fn from_bytes(data: &[u8]) -> Option<MessageType> {
        if let Ok(message) = serde_json::from_slice::<MessageType>(data) {
            Some(message)
        } else {
            None
        }
    }
}
