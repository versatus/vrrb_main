use crate::block::Block;
use crate::claim::Claim;
use crate::network::command_utils;
use crate::network::command_utils::Command;
use crate::network::message_types::{MessageType, StateBlock};
use crate::network::message_utils;
use crate::network::message_utils::{
    update_block_archive, update_claims, update_credits_and_debits, update_last_confirmed_block,
    update_last_state, update_reward_state, update_state_hash,
};
use crate::network::node::Node;
use crate::txn::Txn;
use crate::validator::{Message as ValidatorMessage, Validator};
use libp2p::gossipsub::{GossipsubMessage, IdentTopic as Topic};
use log::info;
use ritelinked::LinkedHashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;

pub const PROPOSAL_EXPIRATION_KEY: &str = "expires";
pub const PROPOSAL_YES_VOTE_KEY: &str = "yes";
pub const PROPOSAL_NO_VOTE_KEY: &str = "no";

pub fn process_message(message: GossipsubMessage, node: Arc<Mutex<Node>>) {
    let message = MessageType::from_bytes(
        &hex::decode(&String::from_utf8_lossy(&message.data).into_owned()).unwrap(),
    );

    match message.clone() {
        MessageType::TxnMessage { txn, .. } => {
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_txn_message(txn, Arc::clone(&thread_node));
            })
            .join()
            .unwrap();
        }
        MessageType::ClaimMessage { .. } => {
            // Don't know if we still need this enum type.
        }
        MessageType::BlockMessage { block, .. } => {
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                if let Err(e) = thread_node.lock().unwrap().block_sender.clone().send(block) {
                    println!("Error sending message to block processing thread: {:?}", e);
                };
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
        MessageType::GetNetworkStateMessage {
            sender_id,
            requested_from,
            requestor_node_type,
        } => {
            let thread_node = Arc::clone(&node);
            let local_pubkey = thread_node
                .lock()
                .unwrap()
                .wallet
                .lock()
                .unwrap()
                .pubkey
                .clone();
            if local_pubkey == requested_from {
                thread::spawn(move || {
                    message_utils::send_state(
                        Arc::clone(&thread_node),
                        sender_id,
                        requestor_node_type,
                    );
                })
                .join()
                .unwrap();
            }
        }
        MessageType::NetworkStateDataBaseMessage {
            object,
            data,
            chunk_number,
            total_chunks,
            last_block,
            requestor,
            ..
        } => {
            let thread_node = Arc::clone(&node);
            let node_id = thread_node.lock().unwrap().id.to_string().clone();
            thread::spawn(move || {
                if requestor == node_id {
                    println!("Received state_db_message");
                    process_state_db_message(
                        object,
                        data,
                        chunk_number,
                        total_chunks,
                        last_block,
                        Arc::clone(&thread_node),
                    );
                }
            })
            .join()
            .unwrap();
        }
        MessageType::TxnValidatorMessage {
            txn_id,
            vote,
            validator_pubkey,
            ..
        } => {
            // If valid add to validator vector for the txn.
            // If confirmed (2/3rds of validators with a minimum of 10 returned as valid)
            // set the txn as mineable.
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_txn_validator_message(
                    txn_id,
                    vote,
                    validator_pubkey,
                    Arc::clone(&thread_node),
                );
            })
            .join()
            .unwrap();
        }
        MessageType::ClaimValidator {
            claim_number,
            vote,
            validator_pubkey,
            ..
        } => {
            // Same as above, but for claim validators
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_claim_sold_validator_message(
                    claim_number,
                    vote,
                    validator_pubkey,
                    Arc::clone(&thread_node),
                );
            })
            .join()
            .unwrap();
        }
        MessageType::ExpiredClaimMessage { claim_number, .. } => {
            let thread_node = Arc::clone(&node);
            thread::spawn(move || {
                process_expired_claim_message(claim_number, Arc::clone(&thread_node));
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
                    .extend(addresses.clone());
                let mut pubkey = HashSet::new();

                addresses.iter().for_each(|(_, v)| {
                    pubkey.insert(v);
                });

                let pubkey = pubkey.iter().next().unwrap().clone();

                let mut claim_counter = thread_node
                    .lock()
                    .unwrap()
                    .account_state
                    .lock()
                    .unwrap()
                    .claim_counter
                    .clone();
                if let None = claim_counter.get_mut(&pubkey.clone()) {
                    claim_counter.insert(pubkey.clone(), 0);
                }

                let claims_owned: u128 = thread_node
                    .lock()
                    .unwrap()
                    .account_state
                    .lock()
                    .unwrap()
                    .claim_pool
                    .confirmed
                    .iter()
                    .filter(|(_, v)| v.current_owner == Some(pubkey.clone()))
                    .count() as u128;

                if let Some(entry) = claim_counter.get_mut(&pubkey.clone()) {
                    *entry += claims_owned;
                }

                println!("{:?}", claim_counter);
                thread_node
                    .lock()
                    .unwrap()
                    .account_state
                    .lock()
                    .unwrap()
                    .claim_counter = claim_counter;
            })
            .join()
            .unwrap();
            println!("Updated account_state accounts -> public key map with new address");
        }
        MessageType::NeedBlockMessage { .. } => {}
        MessageType::MissingBlock { .. } => {}

        _ => {}
    }
}

pub fn process_txn_message(txn: Txn, node: Arc<Mutex<Node>>) {
    let cloned_node = Arc::clone(&node);
    let last_block = cloned_node.lock().unwrap().last_block.clone();
    if let None = last_block {
        // message_utils::request_state(Arc::clone(&node));
        // // stash blocks.
    } else {
        info!(target: "txn_message", "New transaction received: {:?}", &txn);
        cloned_node
            .lock()
            .unwrap()
            .account_state
            .lock()
            .unwrap()
            .txn_pool
            .pending
            .insert(txn.txn_id.clone(), txn.clone());

        let pubkey = node.lock().unwrap().wallet.lock().unwrap().pubkey.clone();
        let account_state_string = cloned_node
            .lock()
            .unwrap()
            .account_state
            .lock()
            .unwrap()
            .clone()
            .to_string();
        let network_state_string = cloned_node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .clone()
            .to_string();
        let txn_string = txn.clone().to_string();
        let sender_id = node.lock().unwrap().id.clone().to_string();
        let message = ValidatorMessage::Txn(txn_string, account_state_string, network_state_string);
        // validate message, update validator, and then update the txn_pool.pending or move it to confirmed if it is the confirming validator
        let account_state = cloned_node
            .lock()
            .unwrap()
            .account_state
            .lock()
            .unwrap()
            .clone();
        let validator_option = Validator::new(message, pubkey.clone(), account_state);
        if let Some(mut validator) = validator_option {
            validator.validate();
            let validator_message = MessageType::TxnValidatorMessage {
                txn_id: txn.txn_id.clone(),
                vote: validator.valid.clone(),
                validator_pubkey: pubkey.clone(),
                sender_id,
            };
            let validator_bytes = validator_message.as_bytes();
            let message = structure_message(validator_bytes);
            publish_message(Arc::clone(&cloned_node), message, "validator");
            info!(target: "txn_validator", "Validator for txn {} generated and disseminated", &txn.txn_id);
        }

        info!(target: "txn_message", "Placed txn into txn_pool.pending: {:?}", &txn);
    }
}

pub fn process_confirmed_block(block: Block, node: Arc<Mutex<Node>>) {
    update_last_state(Arc::clone(&node));
    node.lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .dump_last_state();
    update_block_archive(Arc::clone(&node), &block);
    update_claims(Arc::clone(&node), &block);
    update_credits_and_debits(Arc::clone(&node), &block);
    update_last_confirmed_block(Arc::clone(&node), &block);
    update_reward_state(Arc::clone(&node), &block);
    update_state_hash(Arc::clone(&node), &block);
    thread::spawn(move || {
        node.lock().unwrap().network_state.lock().unwrap().dump(block);
        info!(target: "state_dump", "Dumped network state to {}", &node.lock().unwrap().network_state.lock().unwrap().path.clone());
    }).join().unwrap();
}

pub fn process_vrrb_ip_message(
    proposal_id: String,
    proposal_expiration: u128,
    node: Arc<Mutex<Node>>,
) {
    let mut proposal_map = LinkedHashMap::new();
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
    let _message = structure_message(message.as_bytes());
}

pub fn process_txn_validator_message(
    txn_id: String,
    vote: bool,
    validator_pubkey: String,
    node: Arc<Mutex<Node>>,
) {
    // Store validator in the txn validators vector. If this validator is the confirming
    // validator, move the txn from pending to confirmed. If the txn is no longer in pending
    // check if it is in confirmed. If it is not in confirmed either, dispose of the validator.
    let cloned_node = Arc::clone(&node);
    let mut txn_pool = cloned_node
        .lock()
        .unwrap()
        .account_state
        .lock()
        .unwrap()
        .txn_pool
        .clone();
    let n_peers = cloned_node
        .lock()
        .unwrap()
        .swarm
        .behaviour()
        .gossipsub
        .all_peers()
        .count();
    if let Some(entry) = txn_pool.pending.get_mut(&txn_id) {
        entry
            .validators
            .insert(validator_pubkey.clone(), vote.clone());
        let mut confirmations = entry.validators.clone();
        confirmations.retain(|_, v| v.clone());
        if n_peers < 10 {
            if entry.validators.clone().len() > n_peers / 10 {
                if confirmations.len() as f64 / entry.validators.clone().len() as f64 >= 2.0 / 3.0 {
                    cloned_node
                        .lock()
                        .unwrap()
                        .account_state
                        .lock()
                        .unwrap()
                        .txn_pool
                        .confirmed
                        .insert(entry.txn_id.clone(), entry.clone());
                    cloned_node
                        .lock()
                        .unwrap()
                        .account_state
                        .lock()
                        .unwrap()
                        .txn_pool
                        .pending
                        .remove(&txn_id.clone());
                }
            }
        }
        info!(target: "validator_set", "Set validator in pending transaction");
    } else if let Some(entry) = node
        .lock()
        .unwrap()
        .account_state
        .lock()
        .unwrap()
        .txn_pool
        .clone()
        .confirmed
        .get_mut(&txn_id)
    {
        entry
            .validators
            .insert(validator_pubkey.clone(), vote.clone());
        info!(target: "validator_set", "Set validator in confirmed transaction");
    }
}

pub fn process_claim_for_sale_message(_claim: Claim, _node: Arc<Mutex<Node>>) {
    // validate the claim
    // set validator in claim struct
    // send validator message
}

pub fn process_claim_staked_message(_claim: Claim, _node: Arc<Mutex<Node>>) {
    // validate the claim
    // set validator in claim struct
    // send validator message
}

pub fn process_claim_sold_message(_claim: Claim, _node: Arc<Mutex<Node>>) {
    // validate the claim
    // set validator in claim struct
    // send validator message
}

pub fn process_claim_sold_validator_message(
    _claim_number: u128,
    _vote: bool,
    _validator_pubkey: String,
    _node: Arc<Mutex<Node>>,
) {
    // set validator in claim struct
    // if validator pushes the valid or invalid validators over
    // the threshold for validation/invalidation then confirm
    // the updated claim and replace the claim in the confirmed claim
    // pool, and remove the updated claim from the pending claim pool.
}

pub fn process_claim_stake_validator_message(
    _claim_number: u128,
    _vote: bool,
    _validator_pubkey: String,
    _node: Arc<Mutex<Node>>,
) {
    // set the validator in the claim struct
    // if the validator pushes the valid or invalid validators over
    // the threshold for validation/invalidation, confirm the updated
    // claim and replace the claim in the confirmed claim pool, and remove
    // the updated claim from the pending claim pool.
}

pub fn process_claim_for_sale_validator_message(
    _claim_number: u128,
    _vote: bool,
    _validator_pubkey: String,
    _node: Arc<Mutex<Node>>,
) {
    // set the validator in the claim struct
    // if the validator pushes the valid or invalid validators over
    // the threshold for validation/invalidation, confirm the updated
    // claim and replace the claim in the confirmed claim pool, and remove
    // the updated claim from the pending claim pool.
}

pub fn process_invalid_block_message(_node: Arc<Mutex<Node>>) {}

pub fn process_block_vote_message(block: Block, vote: bool, node: Arc<Mutex<Node>>) {
    let cloned_node = Arc::clone(&node);
    let ballot_box = Arc::clone(&cloned_node.lock().unwrap().ballot_box);
    let mut block_vote_tally = ballot_box.lock().unwrap().state_hash.clone();
    info!(
        target: "block_vote_message", "Received a block vote message for block {} with block hash {} vote: {}",
        &block.block_height, block.block_hash, vote
    );
    match vote {
        false => {
            if let Some((_hash, map, _txn_map)) = block_vote_tally.get_mut(&block.block_height) {
                *map.get_mut("no").unwrap() += 1;
            } else {
                let hash = block.state_hash.clone();
                let mut vote_map = LinkedHashMap::new();
                let txn_map = block.data.clone();
                vote_map.insert("no".to_string(), 1u128);
                block_vote_tally.insert(block.block_height.clone(), (hash, vote_map, txn_map));
            }
        }
        true => {
            // TODO: check number of peers, if less than 3, and a single confirmed vote comes in
            // confirm the block.
            if let Some((_hash, map, _txn_map)) = block_vote_tally.get_mut(&block.block_height) {
                *map.get_mut("yes").unwrap() += 1;
            } else {
                let hash = block.state_hash.clone();
                let mut vote_map = LinkedHashMap::new();
                let txn_map = block.data.clone();
                vote_map.insert("yes".to_string(), 1u128);
                block_vote_tally.insert(block.block_height.clone(), (hash, vote_map, txn_map));
            }
        }
    }
    ballot_box.lock().unwrap().state_hash = block_vote_tally.clone();
    info!(
        target: "block_vote_message", "Added {} vote to vote tally for block {} with block_hash {}",
        &vote, &block.block_height, &block.block_hash
    );
    // let n_peers = node.lock().unwrap().swarm.behaviour_mut().gossipsub.all_peers().count().clone();
}

pub fn process_expired_claim_message(claim_number: u128, node: Arc<Mutex<Node>>) {
    let cloned_node = Arc::clone(&node);
    let mut adjusted_claim_map: LinkedHashMap<u128, Claim> = LinkedHashMap::new();
    let mut claims = cloned_node
        .lock()
        .unwrap()
        .account_state
        .lock()
        .unwrap()
        .claim_pool
        .confirmed
        .clone();
    let pubkey = cloned_node
        .lock()
        .unwrap()
        .wallet
        .lock()
        .unwrap()
        .pubkey
        .clone()
        .to_string();
    if let Some(claim) = claims.clone().get(&claim_number) {
        if claim.is_expired() {
            claims.iter_mut().for_each(|(_, mut v)| {
                v.claim_number -= 1;
                adjusted_claim_map.insert(v.clone().claim_number, v.clone());
            });
            cloned_node
                .lock()
                .unwrap()
                .account_state
                .lock()
                .unwrap()
                .claim_pool
                .confirmed = adjusted_claim_map.clone();
            let mut nodes_claims = adjusted_claim_map.clone();
            nodes_claims.retain(|_, v| v.current_owner.clone().unwrap() == pubkey.clone());
            cloned_node.lock().unwrap().wallet.lock().unwrap().claims = nodes_claims;
        }
    }
}

pub fn process_state_db_message(
    object: StateBlock,
    data: Vec<u8>,
    chunk_number: u32,
    total_chunks: u32,
    last_block: u128,
    node: Arc<Mutex<Node>>,
) {
    let cloned_node = Arc::clone(&node);
    if total_chunks == 1 {
        let block = Block::from_bytes(&data);
        let db_option = cloned_node
            .lock()
            .unwrap()
            .get_network_state()
            .get_block_archive_db();
        if let Some(_db) = db_option {
            command_utils::handle_command(
                Arc::clone(&cloned_node),
                Command::CheckStateUpdateStatus((object.0, block, last_block)),
            );
        }
    } else {
        command_utils::handle_command(
            Arc::clone(&node),
            Command::StoreStateDbChunk(object, data, chunk_number, total_chunks, last_block),
        );
    }
}

pub fn process_network_state_complete_message(node: Arc<Mutex<Node>>) {
    command_utils::handle_command(Arc::clone(&node), Command::ProcessBacklog);
}

pub fn structure_message(message: Vec<u8>) -> String {
    hex::encode(message)
}

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
