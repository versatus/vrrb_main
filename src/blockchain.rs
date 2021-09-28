use crate::block::Block;
use crate::header::BlockHeader;
use crate::reward::RewardState;
use crate::state::NetworkState;
use crate::verifiable::Verifiable;
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
}

#[derive(Debug)]
pub struct InvalidBlockError {
    details: String,
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
        }
    }

    pub fn process_block(
        &mut self,
        network_state: &NetworkState,
        reward_state: &RewardState,
        block: &Block,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(genesis_block) = &self.genesis {
            if let Some(last_block) = &self.child {
                if block.valid_block(&last_block, network_state, reward_state) {
                    self.parent = self.child.clone();
                    self.child = Some(block.clone());
                    self.chain.push_back(block.header.clone());
                    if self.block_cache.len() == 100 {
                        self.block_cache.pop_back();
                        self.block_cache.insert(block.hash.clone(), block.clone());
                    }
                    Ok(())
                } else {
                    self.invalid.insert(block.hash.clone(), block.clone());
                    Err(Box::new(InvalidBlockError {
                        details: "Invalid block".to_string(),
                    }))
                }
            } else {
                if block.valid_block(&genesis_block, network_state, reward_state) {
                    self.child = Some(block.clone());
                    self.chain.push_back(block.header.clone());
                    Ok(())
                } else {
                    self.invalid.insert(block.hash.clone(), block.clone());
                    Err(Box::new(InvalidBlockError {
                        details: "Invalid block".to_string(),
                    }))
                }
            }
        } else {
            // check that this is a valid genesis block.
            if block.valid_genesis(network_state, reward_state) {
                self.genesis = Some(block.clone());
                self.child = Some(block.clone());
                self.block_cache.insert(block.hash.clone(), block.clone());
                self.chain.push_back(block.header.clone());
                Ok(())
            } else {
                self.invalid.insert(block.hash.clone(), block.clone());
                Err(Box::new(InvalidBlockError {
                    details: "Invalid genesis".to_string(),
                }))
            }
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
        &self.details
    }
}
