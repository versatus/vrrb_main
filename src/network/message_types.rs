use crate::block::Block;
use crate::claim::Claim;
use crate::network::node::NodeAuth;
use crate::txn::Txn;
use ritelinked::LinkedHashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StateBlock(pub u128);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MessageType {
    AccountPubkeyMessage {
        addresses: LinkedHashMap<String, String>,
        sender_id: String,
    },
    NetworkStateDataBaseMessage {
        object: StateBlock,
        data: Vec<u8>,
        chunk_number: u32,
        total_chunks: u32,
        requestor: String,
        sender_id: String,
    },
    TxnMessage {
        txn: Txn,
        sender_id: String,
    },
    TxnValidatorMessage {
        txn_id: String,
        vote: bool,
        validator_pubkey: String,
        sender_id: String,
    },
    BlockMessage {
        block: Block,
        sender_id: String,
    },
    BlockChunkMessage {
        block_height: u128,
        chunk_number: u128,
        total_chunks: u128,
        data: Vec<u8>,
    },
    NeedBlockMessage {
        last_block: u128,
        sender_id: String,
    },
    MissingBlock {
        block: Block,
        requestor: String,
        sender_id: String,
    },
    BlockVoteMessage {
        block: Block,
        vote: bool,
        sender_id: String,
    },
    ClaimMessage {
        claim: Claim,
        sender_id: String,
    },
    ClaimStakeMessage {
        claim: Claim,
        sender_id: String,
    },
    ClaimUnstakeMessage {
        claim: Claim,
        sender_id: String,
    },
    ClaimForSaleMessage {
        updated_claim: Claim,
        sender_id: String,
    },
    ClaimSoldMessage {
        updated_claim: Claim,
        sender_id: String,
    },
    ClaimValidator {
        claim_number: u128,
        vote: bool,
        validator_pubkey: String,
        sender_id: String,
    },
    ExpiredClaimMessage {
        claim_number: u128,
        sender_id: String,
    },
    VIPMessage {
        proposal_id: String,
        sender_id: String,
        proposal_expiration: u128,
    },
    VIPVoteMessage {
        proposal_id: String,
        vote: bool,
        sender_id: String,
    },
    GetNetworkStateMessage {
        sender_id: String,
        requested_from: String,
        requestor_node_type: NodeAuth,
    },
    GetAccountStateMessage {
        sender_id: String,
        requested_from: String,
    },
    InvalidBlockMessage {
        block_height: u128,
        miner_id: String,
        sender_id: String,
    },
}

impl MessageType {
    pub fn as_bytes(self) -> Vec<u8> {
        serde_json::to_string(&self).unwrap().as_bytes().to_vec()
    }

    pub fn from_bytes(data: &[u8]) -> MessageType {
        serde_json::from_slice::<MessageType>(data).unwrap()
    }
}
