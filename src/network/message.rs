use crate::block::Block;
use crate::claim::{Claim, CustodianInfo};
use crate::txn::Txn;
use crate::network::node::{Node, NodeAuth};
use crate::validator::{Validator, Message};
use crate::state::{PendingNetworkState};
use libp2p::gossipsub::{
    GossipsubMessage, 
    IdentTopic as Topic,
};
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::str::FromStr;

pub const HEADER_START_INDEX: usize = 0;
pub const HEADER_END_INDEX: usize = 7;
pub const PEER_ID_START_INDEX: usize =7;
pub const PEER_ID_END_INDEX: usize = 59;
pub const PROPOSAL_ID_START_INDEX: usize = 8;
pub const PROPOSAL_ID_END_INDEX: usize = 24;
pub const PROPOSAL_EXPIRATION_KEY: &str = "expires";
pub const PROPOSAL_YES_VOTE_KEY: &str = "yes";
pub const PROPOSAL_NO_VOTE_KEY: &str = "no";

pub fn process_message(message: GossipsubMessage, node: Arc<Mutex<Node>>) {

    let data_string = &String::from_utf8_lossy(
        &hex::decode(
            &String::from_utf8_lossy(&message.data).into_owned()
        ).unwrap()
    ).into_owned();

    let header = &data_string[HEADER_START_INDEX..HEADER_END_INDEX];
    let sender_id = &data_string[PEER_ID_START_INDEX..PEER_ID_END_INDEX];
    let data = &data_string[PEER_ID_END_INDEX..];

    println!("{:?}", &data_string[0..7]);
    println!("{:?}", &data_string[7..59]);
    println!("{:?}", &data_string[59..]);
    
    match header {
        "NEW_TXN" => {
            process_txn_message(data.as_bytes().to_vec(), node)
        },
        "UPD_TXN" => {
            process_txn_message(data.as_bytes().to_vec(), node)    
        },
        "CLM_HOM" => {
            process_claim_acquired_message(data.as_bytes().to_vec(), node)
        },
        "CLM_SAL" => {
            // TODO: Need to add a ClaimSale Message in Validator for when a claim holder
            // places it for sale.
            process_claim_sale_message(data.as_bytes().to_vec(), node)
        },
        "CLM_ACQ" => {
            process_claim_acquired_message(data.as_bytes().to_vec(), node)
        },
        "NEW_BLK" => {
            process_new_block_message(data.as_bytes().to_vec(), node)
        },
        "GET_BLK" => {
            process_get_block_message(sender_id.to_string(), node);              
        },
        "GETBLKS" => {
            // If node.node_auth == NodeAuth::Full, send a series of messages with blocks starting with
            // the first block ending with the most recent block.
            process_get_all_blocks_message(sender_id.to_string(), node);
        },
        "VRRB_IP" => {
            // Store in the ballot_box proposals hashmap with the proosal ID and expiration date.
            // Ask receiving node if they'd like to vote now, and provide ability to set reminder
            // at specified intervals to ask the node to cast the vote.
            let proposal_id = data[0..16].to_string();
            let proposal_expiration = u128::from_str(&data[16..]).unwrap();
            process_vrrb_ip_message(proposal_id, proposal_expiration, node);
        },
        "UPD_STE" => {

            let requestor = data[0..52].to_string();

            if requestor == node.lock().unwrap().id.to_string() {
                // Request last state + mineable transactions.
            }
        },
        "LST_BLK" => {
            let requestor = data[0..52].to_string();
            let data = data[52..].to_string();
            if requestor == node.lock().unwrap().id.to_string() {          
                process_last_block_message(data.as_bytes().to_vec(), node);
            }
        },
        "ALLBLKS" => {
            // Publish all the blocks from the beginning of the blockchain, 
            // this is for new archive node
            let requestor = data[0..52].to_string();
            let data = data[52..].to_string();
            // the highest block will be included in this message as well, so get the highest block to ensure that
            // when the highest block is completed, check and ensure that the network state is in consensus.
            // sequence blocks and store them into a temporary hashmap to then update the state.
            if requestor == node.lock().unwrap().id.to_string() {
                process_all_blocks_message(data.as_bytes().to_vec(), node);
            }
        },
        "TXN_VAL" => {
            // If valid add to validator vector for the txn.
            // If confirmed (2/3rds of validators with a minimum of 10 returned as valid)
            // set the txn as mineable.
            process_txn_validator_message(data.as_bytes().to_vec(), node);
        },
        "INV_BLK" => {
            // If this node proposed the block, and the block is invalid, update local state with 
            // publish an invalid block message directed at the publisher
            // of the original block (using their PeerID in the message so that other nodes know to)
            // either ignore or forward to the original publisher.
            let proposer = data[0..52].to_string();

            if proposer == node.lock().unwrap().id.to_string() {
                process_invalid_block_message(node);
            }
        },
        "BLKVOTE" => {
            let vote: u32 = data.chars().nth(0).unwrap().to_digit(10).unwrap();
            let data = data[1..].to_string();
            process_block_vote_message(data.as_bytes().to_vec(), vote, node);    
        },
        "BLKARCV" => {
            let block = Block::from_bytes(data.as_bytes());
            process_block_archive_message(block, node);
        }
        _ => {}

    }
}

pub fn process_txn_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let message_node = node.lock().unwrap();
    let mut txn = Txn::from_bytes(&data);
    println!("{:?}", &txn);
    let txn_validator = Validator::new(
        Message::Txn(
            serde_json::to_string(&txn).unwrap(), 
            serde_json::to_string(&message_node.account_state.clone().lock().unwrap().clone()).unwrap()
        ),
        message_node.wallet.clone().lock().unwrap().pubkey.clone(),
        message_node.account_state.clone().lock().unwrap().clone()
    );
    // UPDATE LOCAL Account State pending balances for sender(s)
    // receivers.

    drop(message_node);

    match txn_validator {
        Some(validator) => { txn.validators.push(validator.clone()); publish_validator(validator, node, "TXN_VAL"); },
        None => println!("You are not running a validator node or have no claims staked")
    };

}

pub fn process_claim_homesteading_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let message_node = Arc::clone(&node);
    let claim = Claim::from_bytes(&data);
    if claim.claim_number > 1000 {
        let claim_validator = Validator::new(
            Message::ClaimHomesteaded(
                serde_json::to_string(&claim).unwrap(), 
                claim.current_owner.0.unwrap(), 
                serde_json::to_string(&message_node.lock().unwrap().account_state.clone().lock().unwrap().clone()).unwrap()
            ), 
            message_node.lock().unwrap().wallet.clone().lock().unwrap().pubkey.clone(),
            message_node.lock().unwrap().account_state.clone().lock().unwrap().clone()
        );

        drop(message_node);

        match claim_validator {
            Some(validator) => {
                publish_validator(validator, node, "CLM_VAL");
            },
            None => println!("You are not running a validator node or have no claims staked")
        }
    } else {
        node.lock().unwrap().account_state.lock().unwrap().claims.entry(claim.claim_number.clone()).or_insert(claim.clone());
        node.lock().unwrap().account_state.lock().unwrap().owned_claims.entry(claim.claim_number).or_insert(claim.current_owner.0.unwrap().clone());
    }
}

pub fn process_claim_sale_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let claim = Claim::from_bytes(&data);
    match claim.clone().chain_of_custody.get(&claim.clone().current_owner.1.unwrap()).unwrap().get("acquired_from").unwrap() {
        Some(custodian_info) => {
            match custodian_info {
                CustodianInfo::AcquiredFrom((pubkey, _)) => {
                    let message_node = node.lock().unwrap();
                    let claim_validator = Validator::new(
                        Message::ClaimAcquired(
                            serde_json::to_string(&claim).unwrap(), 
                            pubkey.as_ref().unwrap().to_owned(), 
                            serde_json::to_string(&message_node.account_state.clone().lock().unwrap().clone()).unwrap(),
                            claim.clone().current_owner.0.unwrap()
                        ), 
                        message_node.wallet.clone().lock().unwrap().pubkey.clone(), 
                        message_node.account_state.clone().lock().unwrap().clone()
                    );
                                
                    drop(message_node);

                    match claim_validator {
                        Some(validator) => {
                            publish_validator(validator, node, "CLM_VAL");
                        },
                        None => println!("You are not running a validator node or have no claims staked")
                    }
                },
                _ => {println!("Invalid CustodianInfo option for this process")}
            }
        },
        None => println!("There is no previous owner, this claim cannot be sold or acquired until it has been homesteaded first")
    }
}

pub fn process_claim_acquired_message(bytes: Vec<u8>, node: Arc<Mutex<Node>>) {
    let claim = Claim::from_bytes(&bytes[..]);
    match claim.clone().chain_of_custody.get(&claim.clone().current_owner.1.unwrap()).unwrap().get("acquired_from").unwrap() {
        Some(custodian_info) => {
            match custodian_info {
                CustodianInfo::AcquiredFrom((pubkey, _)) => {
                    let message_node = node.lock().unwrap();
                    let claim_validator = Validator::new(
                        Message::ClaimAcquired(
                            serde_json::to_string(&claim).unwrap(), 
                            pubkey.as_ref().unwrap().to_owned(), 
                            serde_json::to_string(&message_node.account_state.clone().lock().unwrap().clone()).unwrap(),
                            claim.clone().current_owner.1.unwrap()
                        ), 
                        message_node.wallet.clone().lock().unwrap().pubkey.clone(), 
                        message_node.account_state.clone().lock().unwrap().clone()
                    );
                                
                    drop(message_node);

                    match claim_validator {
                        Some(validator) => {
                            publish_validator(validator, node, "CLM_VAL");
                        },
                        None => println!("You are not running a validator node or have no claims staked")
                    }
                },
                _ => {println!("Invalid CustodianInfo option for this process")}

            }
        },
        None => println!("There is no previous owner, this claim cannot be sold or acquired until it has been homesteaded first")
    }
}

pub fn process_new_block_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let message_node = Arc::clone(&node);
    let block = Block::from_bytes(&data);

    if block.claim.claim_number < 1000 {

        let block_validator = Validator::new(
            Message::NewBlock(
                serde_json::to_string(&message_node.lock().unwrap().last_block.as_ref().unwrap()).unwrap(), 
                serde_json::to_string(&block).unwrap(), 
                block.miner.clone(), 
                serde_json::to_string(&message_node.lock().unwrap().network_state.clone().lock().unwrap().clone()).unwrap(),
                serde_json::to_string(&message_node.lock().unwrap().account_state.clone().lock().unwrap().clone()).unwrap(),
                serde_json::to_string(&message_node.lock().unwrap().reward_state.clone().lock().unwrap().clone()).unwrap()
            ), 
            message_node.lock().unwrap().wallet.clone().lock().unwrap().pubkey.clone(),
            message_node.lock().unwrap().account_state.clone().lock().unwrap().clone()
        );

        drop(message_node);

        match block_validator {
            Some(validator) => {
                match &validator.valid {
                    true => {
                        let header = "BLKVOTE".to_string().as_bytes().to_vec();
                        let id = node.lock().unwrap().id.to_string().as_bytes().to_vec();
                        let block_height = block.block_height;
                        let vote = vec![1u8];
                        let mut message_bytes = vec![];
                        message_bytes.extend(vote);
                        message_bytes.extend(block_height.to_ne_bytes().to_vec());
                        let message_bytes = structure_message(header, id, message_bytes);

                        publish_message(node.clone(), message_bytes, "block".to_string());
                    },
                    false => {
                        // Send false message back to network with reason for rejection
                        let header = "BLKVOTE".to_string().as_bytes().to_vec();
                        let id = node.lock().unwrap().id.to_string().as_bytes().to_vec();
                        let vote = vec![0u8];

                        let message_bytes = structure_message(header, id, vote);
                        publish_message(node.clone(), message_bytes, "block".to_string());
                    }
                }   
            },
            None => { println!("You are not running a validator node or you do not have any claims staked") }
        }
    }
}

pub fn process_vrrb_ip_message(proposal_id: String, proposal_expiration: u128, node: Arc<Mutex<Node>>) {
    
    let mut proposal_map = HashMap::new();
    proposal_map.insert(PROPOSAL_EXPIRATION_KEY.to_owned(), proposal_expiration);
    proposal_map.insert(PROPOSAL_YES_VOTE_KEY.to_owned(), 0u128);
    proposal_map.insert(PROPOSAL_NO_VOTE_KEY.to_owned(), 0u128);
    node.lock().unwrap().ballot_box.lock().unwrap().proposals.entry(proposal_id).or_insert(proposal_map);
}

pub fn process_get_block_message(peer_id: String, node: Arc<Mutex<Node>>) {
    let header = "LST_BLK".to_string().as_bytes().to_vec();
    let id = node.lock().unwrap().id.clone().to_string().as_bytes().to_vec();
    let mut to_peer = peer_id.as_bytes().to_vec();
    let block_bytes = node.lock().unwrap().last_block.clone().unwrap().as_bytes();
    to_peer.extend(block_bytes);

    let message = structure_message(header, id, to_peer);
    publish_last_block(message, node);

}

pub fn process_last_block_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {

    let local_last_block_height = node.lock().unwrap().network_state.clone().lock().unwrap().state.get::<Block>("last_block").unwrap().block_height;
    let last_block = Block::from_bytes(&data);

    if local_last_block_height == last_block.block_height - 1 {
        // update state with last block information.
        let pending_network_state = PendingNetworkState::temp(node.lock().unwrap().network_state.clone(), last_block.clone());
                            
        if pending_network_state.hash(&last_block.timestamp.to_ne_bytes()) == last_block.state_hash {
            // You are now in consensus as of the last block, otherwise you are missing something
        } else {

            let message = structure_message(
                "ALLBLKS".as_bytes().to_vec(), 
                node.lock().unwrap().id.clone().to_string().as_bytes().to_vec(), 
                node.lock().unwrap().last_block.clone().unwrap().block_height.to_string().as_bytes().to_vec()
            );
            publish_message(node, message, "block".to_string());
        }
    }
}

pub fn process_get_all_blocks_message(sender_id: String, node: Arc<Mutex<Node>>) {
    match node.clone().lock().unwrap().node_type {
        NodeAuth::Full => {
            // Get the largest block height (the largest key), and then iterate through
            // the archived blocks and send a message for each blocks.
            let block_archive: Option<HashMap<u128, Block>> = node.lock().unwrap().network_state.lock().unwrap().state.get("block_archive");
            match block_archive {
                Some(map) => {
                    // Sort the map by key and send each block in a message.
                    map.iter().for_each(|(_height, block)| {
                        let mut message_bytes = sender_id.as_bytes().to_vec();
                        message_bytes.extend(serde_json::to_string::<Block>(&block).unwrap().as_bytes().to_vec());
                        let message = structure_message(
                            "BLKARCV".to_string().as_bytes().to_vec(), 
                            node.clone().lock().unwrap().id.to_string().as_bytes().to_vec(), 
                            message_bytes
                        );
                        publish_message(node.clone(), message, "block".to_string());
                    })

                },
                None => {}
            }
        },
        _ => {
            // Nothing to do if you're not a running a full node with the archived blocks
        }
    }
}

pub fn process_all_blocks_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let block = Block::from_bytes(&data);
    match node.lock().unwrap().temp_blocks.clone() {
        Some(mut map) => {
            map.entry(block.clone().block_height).or_insert(block.clone());
        },
        None => {
            let mut message_node = node.lock().unwrap();
            let mut map = HashMap::new();
            map.entry(block.block_height).or_insert(block);
            message_node.temp_blocks = Some(map);
        }
    }
}

pub fn process_txn_validator_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let txn_validator_thread = thread::spawn(move || {
        let validator = Validator::from_bytes(&data);
        match validator.message.clone() {
            Message::Txn(txn, _) => {
                let txn = serde_json::from_str::<Txn>(&txn).unwrap();
                node.lock().unwrap().account_state.lock().unwrap().pending.get_mut(&txn.txn_id).unwrap().validators.push(validator);

                // If the transaction has been validated by more than the number of required validators, move from pending to mineable
                if ((node.lock().unwrap().account_state.lock().unwrap()
                                .pending.get(&txn.txn_id).unwrap()
                                .validators.iter().filter(|v| v.valid).count() as f64 / 
                    node.lock().unwrap().account_state.lock().unwrap()
                                .pending.get(&txn.txn_id)
                                .unwrap().validators.len() as f64 
                    ) >= 2.0/3.0) && node.lock().unwrap().account_state.lock().unwrap().pending.get(&txn.txn_id).unwrap().validators.len() >= 10 {
                        node.lock().unwrap().account_state.lock().unwrap().mineable.insert(txn.txn_id.clone(), txn.clone());
                        node.lock().unwrap().account_state.lock().unwrap().pending.remove(&txn.txn_id);
                    }
            },
            _ => { 
                // This is an invalid message type for this particular function
            },
        }
    });

    txn_validator_thread.join().unwrap();  
}

pub fn process_invalid_block_message(node: Arc<Mutex<Node>>) {

    let message = structure_message(
        "UPD_STE".as_bytes().to_vec(), 
        node.lock().unwrap().id.clone().to_string().as_bytes().to_vec(), 
        node.lock().unwrap().last_block.clone().unwrap().block_height.to_string().as_bytes().to_vec()
    );
    publish_message(node, message, "test-net".to_string());
}

pub fn process_block_vote_message(data: Vec<u8>, vote: u32, node: Arc<Mutex<Node>>) {

    let block = Block::from_bytes(&data);

    match vote {
        0 => {
            if let Some((_hash, map, _txn_map)) = node.lock().unwrap().ballot_box
                                                    .lock().unwrap().state_hash
                                                    .get_mut(&block.block_height) {
                *map.get_mut("no").unwrap() += 1;
            } else {
                let hash = block.state_hash.clone();
                let mut vote_map = HashMap::new();
                let txn_map = block.data.clone();
                vote_map.insert("no".to_string(), 1u128);
                node.lock().unwrap().ballot_box.lock().unwrap()
                    .state_hash.insert(block.block_height.clone(), (hash, vote_map, txn_map));
            }
        },
        1 => {
            if let Some((_hash, map, _txn_map)) = node.lock().unwrap().ballot_box
                                                    .lock().unwrap().state_hash
                                                    .get_mut(&block.block_height) {
                *map.get_mut("yes").unwrap() += 1;

            } else {
                let hash = block.state_hash.clone();
                let mut vote_map = HashMap::new();
                let txn_map = block.data.clone();
                vote_map.insert("yes".to_string(), 1u128);
                node.lock().unwrap().ballot_box.lock().unwrap()
                    .state_hash.insert(block.block_height.clone(), (hash, vote_map, txn_map));
            }
        },
        _ => { // Anything other than a 0 or a 1 is invalid.
        }
    }
}

pub fn process_block_archive_message(block: Block, node: Arc<Mutex<Node>>) {
    let mut block_archive = node.lock().unwrap().network_state.lock().unwrap().state.get::<HashMap<u128, Block>>("block_archive").unwrap();
    block_archive.entry(block.block_height).or_insert(block);
    node.lock().unwrap().network_state.lock().unwrap().state.set("block_archive", &block_archive).unwrap();
}

pub fn structure_message(header: Vec<u8>, peer_id: Vec<u8>, message: Vec<u8>) -> String {
    
    let mut message_bytes: Vec<u8> = vec![];
    message_bytes.extend(&header);
    message_bytes.extend(&peer_id);
    message_bytes.extend(message);

    hex::encode(message_bytes)
}

pub fn publish_validator(validator: Validator, node: Arc<Mutex<Node>>, header: &str) {
    // Publish a validator as bytes to the validator channel
    let processed = validator.validate();
    let validator_bytes = processed.as_bytes();
    let message = structure_message(
        header.as_bytes().to_vec(), node.lock().unwrap().id.clone().to_string().as_bytes().to_vec(), validator_bytes.to_vec()
    );

    publish_message(node, message, "validator".to_string())
}

pub fn publish_last_block(message: String, node: Arc<Mutex<Node>>) {
    // Publish the last confiremd block (directed at the requestor using PeerID)
    publish_message(node, message, "block".to_string());    
}

pub fn publish_message(node: Arc<Mutex<Node>>, message: String, topic: String) {
    node.lock().unwrap().swarm.behaviour_mut().gossipsub.publish(Topic::new(topic), message).unwrap();
}