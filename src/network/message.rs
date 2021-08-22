use crate::block::Block;
use crate::claim::{Claim, CustodianInfo};
use crate::txn::Txn;
use crate::state::{NetworkState};
use crate::network::node::{Node, AccountPk};
use crate::validator::{Validator, Message};
use crate::state::{PendingNetworkState};
use crate::network::node::{
    update_block_archive,
    update_claims,
    update_credits_and_debits,
    update_last_block,
    update_reward_state,
};

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

    if &data_string.chars().count() > &PEER_ID_END_INDEX {
        let header = &data_string[HEADER_START_INDEX..HEADER_END_INDEX];
        let sender_id = &data_string[PEER_ID_START_INDEX..PEER_ID_END_INDEX].to_string();
        let data = &data_string[PEER_ID_END_INDEX..].to_string();
        match header {
            "NEW_TXN" => {
                process_txn_message(data.as_bytes().to_vec(), Arc::clone(&node));
            },
            "UPD_TXN" => {
                process_txn_message(data.as_bytes().to_vec(), Arc::clone(&node));    
            },
            "CLM_SAL" => {
                // TODO: Need to add a ClaimSale Message in Validator for when a claim holder
                // places it for sale.
                process_claim_sale_message(data.as_bytes().to_vec(), Arc::clone(&node));
            },
            "CLM_ACQ" => {
                process_claim_acquired_message(data.as_bytes().to_vec(), Arc::clone(&node));
            },
            "NEW_BLK" => {
                let thread_data = data.clone();
                let thread_node = Arc::clone(&node);
                thread::spawn(move || {
                    process_new_block_message(thread_data.as_bytes().to_vec(), Arc::clone(&thread_node));
                }).join().unwrap();

            },
            "GET_BLK" => {
                process_get_block_message(sender_id.to_string(), Arc::clone(&node));              
            },
            "GETBLKS" => {
                // If node.node_auth == NodeAuth::Full, send a series of messages with blocks starting with
                // the first block ending with the most recent block.
                process_get_all_blocks_message(sender_id.to_string(), Arc::clone(&node));
            },
            "VRRB_IP" => {
                // Store in the ballot_box proposals hashmap with the proosal ID and expiration date.
                // Ask receiving node if they'd like to vote now, and provide ability to set reminder
                // at specified intervals to ask the node to cast the vote.
                let proposal_id = data[0..16].to_string();
                let proposal_expiration = u128::from_str(&data[16..]).unwrap();
                process_vrrb_ip_message(proposal_id, proposal_expiration, Arc::clone(&node));
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
                    process_last_block_message(data.as_bytes().to_vec(), Arc::clone(&node));
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
                    process_all_blocks_message(data.as_bytes().to_vec(), Arc::clone(&node));
                }
            },
            "TXN_VAL" => {
                // If valid add to validator vector for the txn.
                // If confirmed (2/3rds of validators with a minimum of 10 returned as valid)
                // set the txn as mineable.
                let thread_data = data.clone();
                thread::spawn(move || {
                    process_txn_validator_message(thread_data.clone().as_bytes().to_vec(), Arc::clone(&node));
                }).join().unwrap();
            },
            "CLM_VAL" => {
                // Same as above, but for claim validators
                let thread_data = data.clone();
                thread::spawn(move || {
                    process_claim_sale_validator_message(thread_data.clone().as_bytes().to_vec(), Arc::clone(&node));
                }).join().unwrap();
            },
            "INV_BLK" => {
                // If this node proposed the block, and the block is invalid, update local state with 
                // publish an invalid block message directed at the publisher
                // of the original block (using their PeerID in the message so that other nodes know to)
                // either ignore or forward to the original publisher.
                let proposer = data[0..52].to_string();
                let local_id = node.lock().unwrap().id.clone().to_string();
                if proposer == local_id {
                    process_invalid_block_message(Arc::clone(&node));
                }
            },
            "BLKVOTE" => {
                let vote: u32 = data.chars().nth(0).unwrap().to_digit(10).unwrap().clone();
                let data = data[1..].to_string().clone();
                thread::spawn(move || {
                    process_block_vote_message(data.as_bytes().to_vec(), vote, Arc::clone(&node));
                }).join().unwrap();
                    
            },
            "BLKARCV" => {
                let block = Block::from_bytes(data.as_bytes());
                thread::spawn(move || {
                    process_block_archive_message(block, Arc::clone(&node));
                }).join().unwrap();
            },
            "NEWADDR" => {
                let accounts = serde_json::from_str::<AccountPk>(&data);

                if let Ok(accounts_map) = accounts {
                    node.lock().unwrap().account_state.lock().unwrap().accounts_pk.extend(accounts_map.addresses.clone());
                    println!("Updated account_state accounts -> public key map with new address: {:?}", &accounts_map);
                }
            }
            _ => {}
        }
    } else {
        println!("{}", data_string);
    }
}

pub fn process_txn_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let txn = Txn::from_bytes(&data);
    println!("{:?}", txn);
    node.lock().unwrap().account_state.lock().unwrap().pending.insert(txn.txn_id.clone(), txn);

}

pub fn process_claim_sale_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let claim = Claim::from_bytes(&data);
    println!("{:?}", claim);
}

pub fn process_claim_acquired_message(bytes: Vec<u8>, node: Arc<Mutex<Node>>) {
    let claim = Claim::from_bytes(&bytes[..]);
    println!("{:?}", claim);
}

pub fn process_new_block_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let block = Block::from_bytes(&data);
    
    update_block_archive(Arc::clone(&node), &block);
    update_claims(Arc::clone(&node), &block);
    update_credits_and_debits(Arc::clone(&node), &block);
    update_last_block(Arc::clone(&node), &block);
    update_reward_state(Arc::clone(&node), &block);

    if let Err(e) = node.lock().unwrap().network_state.lock().unwrap().state.dump() {
        println!("Error dumping update to state: {:?}", e);
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

    let last_block = Block::from_bytes(&data);
    println!("{:?}", last_block);
}

pub fn process_get_all_blocks_message(sender_id: String, node: Arc<Mutex<Node>>) {
    
}

pub fn process_all_blocks_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let block = Block::from_bytes(&data);
    println!("{:?}", block);
}

pub fn process_txn_validator_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let validator = Validator::from_bytes(&data);
    println!("{:?}", validator);
}

pub fn process_claim_sale_validator_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {
    let validator = Validator::from_bytes(&data);
    println!("{:?}", validator);
}

pub fn process_claim_stake_message(_data: Vec<u8>, _node: Arc<Mutex<Node>>) {

}

pub fn process_claim_available_message(_data: Vec<u8>, _node: Arc<Mutex<Node>>) {

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

}

pub fn process_confirmed_block(block: Block, node: Arc<Mutex<Node>>) {

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
    if let Err(e) = node.lock().unwrap().swarm.behaviour_mut().gossipsub.publish(Topic::new(topic), message) {
        println!("Encountered error trying to publish message: {:?}", e);
    };
}