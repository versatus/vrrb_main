use crate::block::Block;
use crate::claim::Claim;
use crate::network::message_types::MessageType;
use crate::network::message_utils::{
    update_block_archive, update_claims, update_credits_and_debits, update_last_block,
    update_reward_state,
};
use crate::network::node::Node;
use crate::state::NetworkState;
use crate::txn::Txn;
use crate::validator::{Validator, ValidatorOptions};
use crate::verifiable::Verifiable;
use libp2p::gossipsub::{GossipsubMessage, IdentTopic as Topic, TopicHash};
use libp2p::PeerId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

pub const PROPOSAL_EXPIRATION_KEY: &str = "expires";
pub const PROPOSAL_YES_VOTE_KEY: &str = "yes";
pub const PROPOSAL_NO_VOTE_KEY: &str = "no";

pub fn process_message(message: GossipsubMessage, node: Arc<Mutex<Node>>) {
    let message = MessageType::from_bytes(
        &hex::decode(&String::from_utf8_lossy(&message.data).into_owned()).unwrap(),
    );

    match message {
        MessageType::TxnMessage { txn, .. } => {
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_txn_message(txn, Arc::clone(&thread_node));
            })
            .join()
            .unwrap();
        }
        MessageType::ClaimMessage { claim, .. } => {
            // TODO: Need to add a ClaimSale Message in Validator for when a claim holder
            // places it for sale.
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_claim_message(claim, Arc::clone(&thread_node));
            })
            .join()
            .unwrap();
        }
        MessageType::BlockMessage { block, .. } => {
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_new_block_message(block, Arc::clone(&thread_node));
            })
            .join()
            .unwrap();
        }
        MessageType::VIPMessage {
            proposal_id,
            proposal_expiration,
            ..
        } => {
            // Store in the ballot_box proposals hashmap with the proosal ID and expiration date.
            // Ask receiving node if they'd like to vote now, and provide ability to set reminder
            // at specified intervals to ask the node to cast the vote.
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_vrrb_ip_message(proposal_id, proposal_expiration, Arc::clone(&thread_node));
            })
            .join()
            .unwrap();
        }
        MessageType::GetNetworkStateMessage { sender_id } => {
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_get_all_blocks_message(sender_id, Arc::clone(&thread_node))
            })
            .join()
            .unwrap();
        }
        MessageType::NetworkStateMessage {
            network_state,
            requestor,
            ..
        } => {
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                if requestor == node.lock().unwrap().id.to_string().clone() {
                    process_state_message(network_state, Arc::clone(&thread_node));
                }
            })
            .join()
            .unwrap();
        }
        MessageType::TxnValidatorMessage { validator, .. } => {
            // If valid add to validator vector for the txn.
            // If confirmed (2/3rds of validators with a minimum of 10 returned as valid)
            // set the txn as mineable.
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_txn_validator_message(
                    validator.clone().as_bytes().to_vec(),
                    Arc::clone(&thread_node),
                );
            })
            .join()
            .unwrap();
        }
        MessageType::ClaimValidator { validator, .. } => {
            // Same as above, but for claim validators
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_claim_sale_validator_message(
                    validator.clone().as_bytes().to_vec(),
                    Arc::clone(&thread_node),
                );
            })
            .join()
            .unwrap();
        }
        MessageType::InvalidBlockMessage { miner_id, .. } => {
            // If this node proposed the block, and the block is invalid, update local state with
            // publish an invalid block message directed at the publisher
            // of the original block (using their PeerID in the message so that other nodes know to)
            // either ignore or forward to the original publisher.
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                let local_id = node.lock().unwrap().id.clone().to_string();
                if miner_id == local_id {
                    process_invalid_block_message(Arc::clone(&thread_node));
                }
            })
            .join()
            .unwrap();
        }
        MessageType::BlockVoteMessage { block, vote, .. } => {
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_block_vote_message(block, vote, Arc::clone(&thread_node));
            })
            .join()
            .unwrap();
        }
        MessageType::AccountPubkeyMessage { addresses, .. } => {
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                thread_node
                    .lock()
                    .unwrap()
                    .account_state
                    .lock()
                    .unwrap()
                    .accounts_pk
                    .extend(addresses);
            })
            .join()
            .unwrap();
            println!("Updated account_state accounts -> public key map with new address");
        }
        MessageType::NeedBlockMessage { .. } => {}
        MessageType::MissingBlock{ .. } => {}

        _ => {}
    }
}

pub fn process_txn_message(txn: Txn, node: Arc<Mutex<Node>>) {
    println!("{:?}", txn);
    node.lock()
        .unwrap()
        .account_state
        .lock()
        .unwrap()
        .pending
        .insert(txn.txn_id.clone(), txn);
}

pub fn process_claim_message(claim: Claim, _node: Arc<Mutex<Node>>) {}

pub fn process_new_block_message(block: Block, node: Arc<Mutex<Node>>) {
    if &block.block_height > &1 {
        let n_peers = node.lock().unwrap().swarm.behaviour_mut().gossipsub.all_peers().count().clone();
        let last_block = node.lock().unwrap().last_block.clone().unwrap();
        let pubkey = block.clone().claim.current_owner.unwrap();
        let network_state = node.lock().unwrap().network_state.lock().unwrap().clone();
        let account_state = node.lock().unwrap().account_state.lock().unwrap().clone();
        let reward_state = node.lock().unwrap().reward_state.lock().unwrap().clone();
        let id = node.lock().unwrap().id.clone().to_string();
        let validator_options = ValidatorOptions::NewBlock(
            last_block, block.clone(), pubkey, account_state, reward_state, network_state
        );

        let vote = block.is_valid(Some(validator_options));

        match vote {
            Some(true) => {
                if n_peers < 3 {
                    let cloned_node = Arc::clone(&node);
                    process_confirmed_block_message(block.clone(), cloned_node);
                }
                let message = MessageType::BlockVoteMessage {
                    block,
                    vote: true,
                    sender_id: id,
                };
                let message = structure_message(message.as_bytes());
                publish_message(Arc::clone(&node), message, "test-net");
            },
            Some(false) => {
                let message = MessageType::BlockVoteMessage {
                    block,
                    vote: false,
                    sender_id: id,
                };
                let message = structure_message(message.as_bytes());
                publish_message(Arc::clone(&node), message, "test-net");
        
            },
            None => {

            }
        }
    } else {
        process_confirmed_block_message(block.clone(), Arc::clone(&node));
    }
}

pub fn process_confirmed_block_message(block: Block, node: Arc<Mutex<Node>>) {
    update_block_archive(Arc::clone(&node), &block);
    update_claims(Arc::clone(&node), &block);
    update_credits_and_debits(Arc::clone(&node), &block);
    update_last_block(Arc::clone(&node), &block);
    update_reward_state(Arc::clone(&node), &block);

    if let Err(e) = node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .state
        .dump()
    {
        println!("Error dumping update to state: {:?}", e);
    }
}

pub fn process_vrrb_ip_message(
    proposal_id: String,
    proposal_expiration: u128,
    node: Arc<Mutex<Node>>,
) {
    let mut proposal_map = HashMap::new();
    proposal_map.insert(PROPOSAL_EXPIRATION_KEY.to_owned(), proposal_expiration);
    proposal_map.insert(PROPOSAL_YES_VOTE_KEY.to_owned(), 0u128);
    proposal_map.insert(PROPOSAL_NO_VOTE_KEY.to_owned(), 0u128);
    node.lock()
        .unwrap()
        .ballot_box
        .lock()
        .unwrap()
        .proposals
        .entry(proposal_id)
        .or_insert(proposal_map);
}

pub fn process_get_block_message(peer_id: String, node: Arc<Mutex<Node>>) {
    let block = node.lock().unwrap().last_block.clone().unwrap();
    let sender_id = node.lock().unwrap().id.clone();
    let message = MessageType::MissingBlock {
        block,
        requestor: peer_id,
        sender_id: sender_id.to_string(),
    };
    let message = structure_message(message.as_bytes());
    publish_last_block(message, node);
}

pub fn process_missing_block_message(block: Block, node: Arc<Mutex<Node>>) {
    process_confirmed_block_message(block, Arc::clone(&node));
}

pub fn process_get_all_blocks_message(sender_id: String, node: Arc<Mutex<Node>>) {
    let cloned_node = Arc::clone(&node);
    let network_state = cloned_node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .clone();
    let id = cloned_node.lock().unwrap().id.clone().to_string();
    let message = MessageType::NetworkStateMessage {
        network_state,
        requestor: sender_id,
        sender_id: id,
    };
    let message = structure_message(message.as_bytes());
    publish_message(Arc::clone(&node), message, "test-net");
}

pub fn process_all_blocks_message(data: Vec<u8>, node: Arc<Mutex<Node>>) {}

pub fn process_txn_validator_message(data: Vec<u8>, _node: Arc<Mutex<Node>>) {}

pub fn process_claim_sale_validator_message(data: Vec<u8>, _node: Arc<Mutex<Node>>) {}

pub fn process_claim_stake_message(_data: Vec<u8>, _node: Arc<Mutex<Node>>) {}

pub fn process_claim_available_message(_data: Vec<u8>, _node: Arc<Mutex<Node>>) {}

pub fn process_invalid_block_message(_node: Arc<Mutex<Node>>) {}

pub fn process_block_vote_message(block: Block, vote: bool, node: Arc<Mutex<Node>>) {
    let cloned_node = Arc::clone(&node);
    let ballot_box = Arc::clone(&cloned_node.lock().unwrap().ballot_box);
    let mut block_vote_tally = ballot_box.lock().unwrap().state_hash.clone();
    match vote {
        false => {
            if let Some((_hash, map, _txn_map)) = block_vote_tally
                .get_mut(&block.block_height)
            {
                *map.get_mut("no").unwrap() += 1;
            } else {
                let hash = block.state_hash.clone();
                let mut vote_map = HashMap::new();
                let txn_map = block.data.clone();
                vote_map.insert("no".to_string(), 1u128);
                block_vote_tally
                    .insert(block.block_height.clone(), (hash, vote_map, txn_map));
            }
        }
        true => {
            if let Some((_hash, map, _txn_map)) = block_vote_tally
                .get_mut(&block.block_height)
            {
                *map.get_mut("yes").unwrap() += 1;
            } else {
                let hash = block.state_hash.clone();
                let mut vote_map = HashMap::new();
                let txn_map = block.data.clone();
                vote_map.insert("yes".to_string(), 1u128);
                block_vote_tally
                    .insert(block.block_height.clone(), (hash, vote_map, txn_map));
            }
        }
    }
    ballot_box.lock().unwrap().state_hash = block_vote_tally.clone();
    // let n_peers = node.lock().unwrap().swarm.behaviour_mut().gossipsub.all_peers().count().clone();

}

pub fn process_state_message(state: NetworkState, node: Arc<Mutex<Node>>) {
    node.lock().unwrap().network_state = Arc::new(Mutex::new(state));
}

pub fn process_block_archive_message(_block: Block, _node: Arc<Mutex<Node>>) {}

pub fn process_confirmed_block(_block: Block, _node: Arc<Mutex<Node>>) {}

pub fn structure_message(message: Vec<u8>) -> String {
    hex::encode(message)
}

pub fn publish_validator(_validator: Validator, _node: Arc<Mutex<Node>>, _header: &str) {}

pub fn publish_last_block(_message: String, _node: Arc<Mutex<Node>>) {}

pub fn publish_message(node: Arc<Mutex<Node>>, message: String, topic: &str) {
    if let Err(e) = node
        .lock()
        .unwrap()
        .swarm
        .behaviour_mut()
        .gossipsub
        .publish(Topic::new(topic), message)
    {
        println!("Encountered error trying to publish message: {:?}", e);
    };
}
