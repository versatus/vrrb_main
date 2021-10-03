use crate::block::Block;
use crate::header::BlockHeader;
use crate::network::command_utils::Command;
use crate::network::message_types::MessageType;
use crate::network::node::NodeAuth;
use crate::reward::RewardState;
use crate::state::NetworkState;
use crate::verifiable::Verifiable;
use crate::network::chunkable::Chunkable;
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use ritelinked::LinkedHashMap;
use serde::{Deserialize, Serialize};
use std::collections::LinkedList;
use std::error::Error;
use std::fmt;
use std::thread;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blockchain {
    pub genesis: Option<Block>,
    pub child: Option<Block>,
    pub parent: Option<Block>,
    pub chain: LinkedList<BlockHeader>,
    pub chain_db: String, // Path to the chain database.
    pub block_cache: LinkedHashMap<String, Block>,
    pub future_blocks: LinkedHashMap<String, Block>,
    pub invalid: LinkedHashMap<String, Block>,
    pub updating_state: bool,
    pub state_update_cache: LinkedHashMap<u128, LinkedHashMap<u128, Vec<u8>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvalidBlockErrorReason {
    BlockOutOfSequence,
    InvalidClaim,
    InvalidLastHash,
    InvalidStateHash,
    InvalidBlockHeight,
    InvalidBlockNonce,
    InvalidBlockReward,
    InvalidTxns,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidBlockError {
    pub details: InvalidBlockErrorReason,
}

impl Blockchain {
    pub fn new(path: &str) -> Blockchain {
        Blockchain {
            genesis: None,
            child: None,
            parent: None,
            chain: LinkedList::new(),
            chain_db: path.to_string(),
            block_cache: LinkedHashMap::new(),
            future_blocks: LinkedHashMap::new(),
            invalid: LinkedHashMap::new(),
            updating_state: false,
            state_update_cache: LinkedHashMap::new(),
        }
    }

    pub fn get_chain_db(&self) -> PickleDb {
        match PickleDb::load_bin(self.chain_db.clone(), PickleDbDumpPolicy::DumpUponRequest) {
            Ok(nst) => nst,
            Err(_) => PickleDb::new(
                self.chain_db.clone(),
                PickleDbDumpPolicy::DumpUponRequest,
                SerializationMethod::Bin,
            ),
        }
    }

    pub fn clone_chain_db(&self) -> PickleDb {
        let db = self.get_chain_db();
        let keys = db.get_all();

        let mut cloned_db = PickleDb::new(
            format!("temp_{}.db", self.chain_db.clone()),
            PickleDbDumpPolicy::NeverDump,
            SerializationMethod::Bin,
        );

        keys.iter().for_each(|k| {
            let block = db.get::<Block>(k);
            if let Some(block) = block {
                if let Err(e) = cloned_db.set(k, &block) {
                    println!("Error setting block with last_hash {} to cloned_db: {:?}", k, e);
                }
            }
        });

        drop(db);
        
        cloned_db
    }

    pub fn dump(&self, block: &Block) -> Result<(), Box<dyn Error>> {
        let mut db = self.get_chain_db();
        if let Err(e) = db.set(&block.header.last_hash, block) {
            return Err(Box::new(e));
        }

        if let Err(e) = db.dump() {
            return Err(Box::new(e));
        }

        Ok(())
    }

    pub fn get_block(&self, last_hash: &str) -> Option<Block> {
        let db = self.get_chain_db();
        db.get::<Block>(last_hash)
    }

    pub fn process_block(
        &mut self,
        network_state: &NetworkState,
        reward_state: &RewardState,
        block: &Block,
    ) -> Result<(), InvalidBlockError> {
        if let Some(genesis_block) = &self.genesis {
            if let Some(last_block) = &self.child {
                if let Err(e) = block.valid_block(&last_block, network_state, reward_state) {
                    return Err(e);
                } else {
                    self.parent = self.child.clone();
                    self.child = Some(block.clone());
                    self.chain.push_back(block.header.clone());
                    if self.block_cache.len() == 100 {
                        self.block_cache.pop_back();
                        self.block_cache.insert(block.hash.clone(), block.clone());
                    }

                    if let Err(e) = self.dump(&block) {
                        println!("Error dumping block to chain db: {:?}", e);
                    };

                    return Ok(());
                }
            } else {
                if let Err(e) = block.valid_block(&genesis_block, network_state, reward_state) {
                    return Err(e);
                } else {
                    self.child = Some(block.clone());
                    self.chain.push_back(block.header.clone());
                    if let Err(e) = self.dump(&block) {
                        println!("Error dumping block to chain db: {:?}", e);
                    };
                    Ok(())
                }
            }
        } else {
            // check that this is a valid genesis block.
            if block.header.block_height == 0 {
                if block.valid_genesis(network_state, reward_state) {
                    self.genesis = Some(block.clone());
                    self.child = Some(block.clone());
                    self.block_cache.insert(block.hash.clone(), block.clone());
                    self.chain.push_back(block.header.clone());
                    if let Err(e) = self.dump(&block) {
                        println!("Error dumping block to chain db: {:?}", e);
                    };
                    Ok(())
                } else {
                    self.invalid.insert(block.hash.clone(), block.clone());
                    Err(InvalidBlockError {
                        details: InvalidBlockErrorReason::General,
                    })
                }
            } else {
                // request genesis block.
                self.future_blocks
                    .insert(block.clone().header.last_hash, block.clone());
                Err(InvalidBlockError {
                    details: InvalidBlockErrorReason::BlockOutOfSequence,
                })
            }
        }
    }

    pub fn stash_future_blocks(&mut self, block: &Block) {
        self
            .future_blocks
            .insert(block.clone().header.last_hash, block.clone());
    }

    pub fn handle_invalid_block(
        &mut self,
        block: &Block,
        reason: InvalidBlockErrorReason,
        swarm_sender: tokio::sync::mpsc::UnboundedSender<Command>,
        node_id: String,
        node_type: NodeAuth,
        block_miner: String,
    ) {
        match reason {
            InvalidBlockErrorReason::BlockOutOfSequence => {
                // Stash block in blockchain.future_blocks
                // Request state update once. Set "updating_state" field
                // in blockchain to true, so that it doesn't request it on
                // receipt of new future blocks which will also be invalid.
                if !self.updating_state {
                    // send state request and set blockchain.updating state to true;
                    println!("Error: {:?}", reason);
                    if let Some((_, v)) = self.future_blocks.front() {
                        let message = MessageType::GetNetworkStateMessage {
                            sender_id: node_id,
                            requested_from: block_miner,
                            requestor_node_type: node_type.clone(),
                            lowest_block: v.header.block_height,
                        };

                        if let Err(e) = swarm_sender.send(Command::SendMessage(message.as_bytes()))
                        {
                            println!(
                                "Error sending state update request to swarm sender: {:?}",
                                e
                            );
                        };

                        self.updating_state = true;
                    }
                }
            }
            _ => {
                println!("{:?}", reason);
                if self.chain.len() - 1 > block.clone().header.block_height as usize {
                    //Inform miner that you have a longer chain than they do. Stash
                    // this block in the blockchain.invalid map
                    self.invalid
                        .insert(block.clone().header.last_hash, block.clone());
                    self.send_invalid_block_message(
                        block,
                        reason,
                        block_miner,
                        node_id,
                        swarm_sender.clone(),
                    );
                } else {
                    // Inform network of the blocks you are missing, i.e. the blocks
                    // between the current block height and blockchain.child block height.
                    self.future_blocks
                        .insert(block.clone().header.last_hash, block.clone());
                    self.send_missing_blocks_message(block, node_id, swarm_sender);
                }
            }
        }
    }

    pub fn send_invalid_block_message(
        &self,
        block: &Block,
        reason: InvalidBlockErrorReason,
        miner_id: String,
        sender_id: String,
        swarm_sender: tokio::sync::mpsc::UnboundedSender<Command>,
    ) {
        let message = MessageType::InvalidBlockMessage {
            block_height: block.clone().header.block_height,
            reason,
            miner_id,
            sender_id,
        };

        if let Err(e) = swarm_sender.send(Command::SendMessage(message.as_bytes())) {
            println!(
                "Error sending InvalidBlockMessage InvalidBlockHeight to swarm sender: {:?}",
                e
            );
        }
    }

    pub fn send_missing_blocks_message(
        &self,
        block: &Block,
        sender_id: String,
        swarm_sender: tokio::sync::mpsc::UnboundedSender<Command>,
    ) {
        let missing_blocks: Vec<u128> =
            (self.chain.len() as u128 - 1u128..block.clone().header.block_height).collect();

        let message = MessageType::NeedBlocksMessage {
            blocks_needed: missing_blocks,
            sender_id,
        };

        if let Err(e) = swarm_sender.send(Command::SendMessage(message.as_bytes())) {
            println!("Error sending NeedBlocksMessage to swarm sender: {:?}", e);
        }
    }

    pub fn send_state(
        &self,
        requested_from: String,
        lowest_block: u128,
        node_id: String,
        db: PickleDb,
        swarm_sender: tokio::sync::mpsc::UnboundedSender<Command>,
    ) {
        println!(
            "Received a state update request, send blocks {} -> {} to: {:?}",
            0,
            &lowest_block - 1,
            &requested_from
        );
        let current_blockchain = self.clone();
        let thread_swarm_sender = swarm_sender.clone();
        thread::spawn(move || {
            let mut iter = current_blockchain.chain.iter();
            let mut idx = 0;
            while idx < lowest_block {
                if let Some(header) = iter.next() {
                    if let Some(block) = db.get::<Block>(&header.last_hash) {
                        if let Some(chunks) = block.clone().chunk() {
                            for (idx, chunk) in chunks.iter().enumerate() {
                                let message = MessageType::BlockChunkMessage {
                                    sender_id: node_id.clone().to_string(),
                                    requestor: requested_from.clone(),
                                    block_height: block.clone().header.block_height,
                                    chunk_number: idx as u128 + 1u128,
                                    total_chunks: chunks.len() as u128,
                                    data: chunk.to_vec(),
                                };

                                if let Err(e) = thread_swarm_sender
                                    .send(Command::SendMessage(message.as_bytes()))
                                {
                                    println!("Error sending block chunk message to swarm: {:?}", e);
                                }
                            }
                        }
                    }

                    idx += 1;
                }
            }
        });
    }
}

impl InvalidBlockErrorReason {
    pub fn to_str(&self) -> &str {
        match self {
            Self::BlockOutOfSequence => "block out of sequence",
            Self::General => "general invalid block",
            Self::InvalidBlockHeight => "invalid block height",
            Self::InvalidClaim => "invalid claim",
            Self::InvalidLastHash => "invalid last hash",
            Self::InvalidStateHash => "invalid state hash",
            Self::InvalidBlockNonce => "invalid block nonce",
            Self::InvalidBlockReward => "invalid block reward",
            Self::InvalidTxns => "invalid txns in block",
        }
    }
}

impl fmt::Display for Blockchain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Blockchain")
    }
}

impl Error for Blockchain {}

impl fmt::Display for InvalidBlockError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for InvalidBlockError {
    fn description(&self) -> &str {
        &self.details.to_str()
    }
}

impl Error for InvalidBlockErrorReason {
    fn description(&self) -> &str {
        &self.to_str()
    }
}

impl fmt::Display for InvalidBlockErrorReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidBlockHeight => {
                write!(f, "invalid block height")
            }
            Self::InvalidClaim => {
                write!(f, "invalid claim")
            }
            Self::InvalidLastHash => {
                write!(f, "invalid last hash")
            }
            Self::InvalidStateHash => {
                write!(f, "invalid state hash")
            }
            Self::BlockOutOfSequence => {
                write!(f, "block out of sequence")
            }
            Self::InvalidBlockNonce => {
                write!(f, "invalid block nonce")
            }
            Self::InvalidBlockReward => {
                write!(f, "invalid block reward")
            }
            Self::InvalidTxns => {
                write!(f, "invalid txns in block")
            }
            Self::General => {
                write!(f, "general invalid block error")
            }
        }
    }
}
