use crate::block::Block;
use crate::header::BlockHeader;
use crate::reward::RewardState;
use crate::state::NetworkState;
use crate::verifiable::Verifiable;
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use ritelinked::LinkedHashMap;
use std::collections::LinkedList;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
            Err(_) => {
                PickleDb::new(
                    self.chain_db.clone(),
                    PickleDbDumpPolicy::DumpUponRequest,
                    SerializationMethod::Bin,
                )
            }
        }
    }

    pub fn dump(&self, block: &Block) -> Result<(), Box<dyn Error>> {
        let mut db = self.get_chain_db();
        
        if let Err(e) = db.set(&block.header.last_hash, block) {
            return Err(Box::new(e))
        }

        if let Err(e) = db.dump() {
            return Err(Box::new(e))
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
                // request a state update.
                self.future_blocks
                    .insert(block.clone().header.last_hash, block.clone());
                Err(InvalidBlockError {
                    details: InvalidBlockErrorReason::BlockOutOfSequence,
                })
            }
        }
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
