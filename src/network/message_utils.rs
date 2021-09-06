#![allow(unused_imports)]
use crate::block::Block;
use crate::claim::CustodianInfo;
use crate::claim::{Claim, CustodianOption};
use crate::network::chunkable::Chunkable;
use crate::network::command_utils;
use crate::network::command_utils::Command;
use crate::network::message;
use crate::network::message_types::{MessageType, StateBlock};
use crate::network::node::MAX_TRANSMIT_SIZE;
use crate::network::node::{Node, NodeAuth};
use crate::state::{BlockArchive, NetworkState};
use crate::utils::restore_db;
use crate::validator::ValidatorOptions;
use crate::verifiable::Verifiable;
use libp2p::gossipsub::error::PublishError;
use libp2p::gossipsub::IdentTopic as Topic;
use log::info;
use ritelinked::LinkedHashMap;
use std::sync::{mpsc::channel, mpsc::Sender, Arc, Mutex};
use std::thread;
use std::time::Duration;

pub fn share_addresses(node: Arc<Mutex<Node>>) {
    let mut addr_pubkey = LinkedHashMap::new();
    let wallet = node.lock().unwrap().wallet.lock().unwrap().clone();
    let pubkey = wallet.pubkey.clone();
    wallet.addresses.iter().for_each(|(_addr_num, addr)| {
        addr_pubkey.insert(addr.clone(), pubkey.clone());
    });
    let accounts = MessageType::AccountPubkeyMessage {
        addresses: addr_pubkey,
        sender_id: node.lock().unwrap().id.clone().to_string(),
    };

    let accounts_bytes = accounts.as_bytes();

    let message = message::structure_message(accounts_bytes);
    message::publish_message(Arc::clone(&node), message, "test-net");
}

pub fn mine_block(node: Arc<Mutex<Node>>) {
    let cloned_node = Arc::clone(&node);
    let last_block = cloned_node.lock().unwrap().last_block.clone();
    let miner = Arc::clone(&cloned_node.lock().unwrap().wallet);
    let network_state = Arc::clone(&cloned_node.lock().unwrap().network_state);
    let reward_state = Arc::clone(&cloned_node.lock().unwrap().reward_state);
    let account_state = Arc::clone(&cloned_node.lock().unwrap().account_state);

    if let None = last_block {
        let block = Block::genesis(
            Arc::clone(&miner),
            Arc::clone(&reward_state),
            Arc::clone(&account_state),
        );
        info!(target: "genesis_block", "attempting to mine genesis block");

        if let Ok(block) = block {
            info!(target: "genesis_block", "mined genesis block");
            let block_message = MessageType::BlockMessage {
                block: block.clone(),
                sender_id: cloned_node.lock().unwrap().id.clone().to_string(),
            };
            let message = message::structure_message(block_message.as_bytes());
            if let Err(e) = cloned_node
                .lock()
                .unwrap()
                .swarm
                .behaviour_mut()
                .gossipsub
                .publish(Topic::new("test-net"), message.clone())
            {
                {
                    info!(target: "protocol_error", "Error publishing message: {:?}", e)
                };
            }
            info!(target: "genesis_block", "published genesis block to the network");
            if let Err(e) = cloned_node.lock().unwrap().block_sender.send(block.clone()) {
                println!("Error sending block to block processing thread: {:?}", e);
            }
            info!(target: "genesis_block", "sent block to block thread");
        }
    } else {
        let claims = cloned_node.lock().unwrap().get_wallet_owned_claims();
        let address = cloned_node.lock().unwrap().get_wallet_pubkey();
        let next_claim_number = last_block.clone().unwrap().claim.claim_number + 1;

        if let Some((_claim_number, claim)) = claims.front() {
            let mut claim = claim.clone();
            let signature = node
                .lock()
                .unwrap()
                .wallet
                .lock()
                .unwrap()
                .sign(&claim.claim_payload.clone().unwrap());

            if let Some(map) = claim.chain_of_custody.get_mut(&address) {
                if let Some(entry) = map.get_mut(&CustodianOption::BuyerSignature) {
                    *entry = Some(CustodianInfo::BuyerSignature(Some(
                        signature.unwrap().to_string(),
                    )));
                };
            }

            if let Some(Ok(block)) = Block::mine(
                claim.clone(),
                last_block.unwrap(),
                Arc::clone(&account_state),
                Arc::clone(&reward_state),
                Arc::clone(&network_state),
                Arc::clone(&miner),
            ) {
                let block_message = MessageType::BlockMessage {
                    block: block.clone(),
                    sender_id: node.lock().unwrap().id.clone().to_string(),
                };
                let message = message::structure_message(block_message.as_bytes());
                if let Err(e) = cloned_node
                    .lock()
                    .unwrap()
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(Topic::new("test-net"), message)
                {
                    info!(target: "prtocol_error", "Error sending message to network: {:?}", e);
                }
                if let Err(e) = cloned_node.lock().unwrap().block_sender.send(block.clone()) {
                    println!("Error sending block to block processing thread: {:?}", e);
                }
            }
        } else {
            let mut claims = cloned_node
                .lock()
                .unwrap()
                .account_state
                .lock()
                .unwrap()
                .claim_pool
                .confirmed
                .clone();
            loop {
                let loop_node = Arc::clone(&cloned_node);
                let mut adjusted_claim_map: LinkedHashMap<u128, Claim> = LinkedHashMap::new();
                if let Some(claim) = claims.clone().get(&next_claim_number) {
                    if claim.is_expired() {
                        claims.iter_mut().for_each(|(_, mut v)| {
                            v.claim_number -= 1;
                            adjusted_claim_map.insert(v.clone().claim_number, v.clone());
                        });
                        // send expired claim message
                        let sender_id = cloned_node.lock().unwrap().id.clone().to_string();
                        let expired_claim_message = MessageType::ExpiredClaimMessage {
                            claim_number: next_claim_number,
                            sender_id,
                        };
                        let message = message::structure_message(expired_claim_message.as_bytes());
                        info!(target: "expired_claim", "claim {} has expired, adjusted all claims in the claim map", &next_claim_number);
                        message::publish_message(Arc::clone(&loop_node), message, "claim");
                        loop_node
                            .lock()
                            .unwrap()
                            .account_state
                            .lock()
                            .unwrap()
                            .claim_pool
                            .confirmed = adjusted_claim_map.clone();
                        let mut nodes_claims = adjusted_claim_map.clone();
                        nodes_claims
                            .retain(|_, v| v.current_owner.clone().unwrap() == address.clone());
                        loop_node.lock().unwrap().wallet.lock().unwrap().claims = nodes_claims;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    }
}

pub fn update_last_confirmed_block(node: Arc<Mutex<Node>>, block: &Block) {
    node.lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .last_block = Some(block.clone());
    node.lock().unwrap().last_block = Some(block.clone());
}

pub fn update_block_archive(node: Arc<Mutex<Node>>, block: &Block) {
    node.lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .block_archive
        .update(block);
}

pub fn update_claims(node: Arc<Mutex<Node>>, block: &Block) {
    // If there are fewer than 50 claims in the confirmed claim pool
    // pull claims from the network state database and extend the vector until there
    // are 100 claims, otherwise, don't extend it.
    node.lock()
        .unwrap()
        .account_state
        .lock()
        .unwrap()
        .claim_pool
        .confirmed
        .extend(block.owned_claims.clone());
    // update the network state claims map
    node.lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .get_claims()
        .extend(block.owned_claims.clone());
    // get the pubkey of the node
    let pubkey = node.lock().unwrap().wallet.lock().unwrap().pubkey.clone();
    // get the claims from the block
    let mut nodes_claims = block.owned_claims.clone();
    // filter it for those that are owned by this node.
    nodes_claims.retain(|_, v| v.current_owner == Some(pubkey.clone()));
    // extend the node's wallets' claims map with it's new claims
    node.lock()
        .unwrap()
        .wallet
        .lock()
        .unwrap()
        .claims
        .extend(nodes_claims);
    // remove the claim used to mine this block from the claim pool
    node.lock()
        .unwrap()
        .account_state
        .lock()
        .unwrap()
        .claim_pool
        .confirmed
        .remove(&block.claim.claim_number);
    // and from the network state.
    // increment the claim_counter for each claim allocated to an owner
    block.owned_claims.clone().iter().for_each(|(_, v)| {
        let mut claim_counter = node
            .lock()
            .unwrap()
            .account_state
            .lock()
            .unwrap()
            .claim_counter
            .clone();
        if let Some(entry) = claim_counter.get_mut(&v.current_owner.clone().unwrap()) {
            *entry += 1;
        } else {
            claim_counter.insert(v.current_owner.clone().unwrap(), 1);
        }
        node.lock()
            .unwrap()
            .account_state
            .lock()
            .unwrap()
            .claim_counter = claim_counter;
    })
}

pub fn update_credits_and_debits(node: Arc<Mutex<Node>>, block: &Block) {
    node.lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .update_credits_and_debits(block);
    block.data.iter().for_each(|(txn_id, _txn)| {
        node.lock()
            .unwrap()
            .account_state
            .lock()
            .unwrap()
            .txn_pool
            .confirmed
            .remove(&txn_id.clone());
        node.lock()
            .unwrap()
            .account_state
            .lock()
            .unwrap()
            .txn_pool
            .pending
            .remove(&txn_id.clone());
    });
    let network_state = node.lock().unwrap().network_state.lock().unwrap().clone();
    node.lock()
        .unwrap()
        .wallet
        .lock()
        .unwrap()
        .txns_in_block(&block, network_state.clone());
}

pub fn update_reward_state(node: Arc<Mutex<Node>>, block: &Block) {
    let reward_state = Arc::clone(&node.lock().unwrap().reward_state);
    reward_state
        .lock()
        .unwrap()
        .update(block.clone().block_reward.category);

    node.lock().unwrap().reward_state = Arc::clone(&reward_state);
    node.lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .reward_state = reward_state.lock().unwrap().clone();
}

pub fn request_state(node: Arc<Mutex<Node>>, block: Block) {
    let sender_id = node.lock().unwrap().id.clone().to_string();
    let requested_from = block.claim.current_owner.clone().unwrap();
    let requestor_node_type = node.lock().unwrap().node_type.clone();
    let message = MessageType::GetNetworkStateMessage {
        sender_id,
        requested_from,
        requestor_node_type,
    };
    command_utils::handle_command(Arc::clone(&node), Command::GetState);

    let message = message::structure_message(message.as_bytes());
    if let Err(PublishError::InsufficientPeers) = node
        .lock()
        .unwrap()
        .swarm
        .behaviour_mut()
        .gossipsub
        .publish(Topic::new("test-net"), message)
    {
        // You are either not connected to any peers or you are the first peer
        // operate as if you are the first peer.
        if let Err(_) = node
            .lock()
            .unwrap()
            .command_sender
            .send(Command::StateUpdateCompleted)
        {
            println!("Error sending the command to the command receiver")
        };
    };
}

pub fn link_blocks(blocks: LinkedHashMap<String, Block>) -> LinkedHashMap<String, Block> {
    let mut sorted_blocks = LinkedHashMap::new();

    while sorted_blocks.len() < blocks.clone().len()
    {
        for (last_block_hash, block) in blocks.clone() {
            if !blocks.clone().contains_key(&last_block_hash) && sorted_blocks.is_empty() {
                sorted_blocks.insert(last_block_hash, block);
            } else if !sorted_blocks.is_empty() && last_block_hash == sorted_blocks.back().unwrap().1.block_hash {
                sorted_blocks.insert(last_block_hash, block);
            }
        } 
    }

    sorted_blocks
}

pub fn send_state(node: Arc<Mutex<Node>>, requestor: String, _requestor_node_type: NodeAuth) {
    info!(target: "sending_state", "Sending state to {}", requestor);
    let cloned_node = Arc::clone(&node);
    let network_state = cloned_node.lock().unwrap().get_network_state();
    let sender_id = cloned_node.lock().unwrap().get_id().to_string();
    if let Some(db) = network_state.get_block_archive_db() {
        let keys = db.get_all();
        let mut keys_as_ints = vec![];
        keys.iter().for_each(|k| keys_as_ints.push(k.parse::<u128>().unwrap()));
        keys_as_ints.sort_unstable();
        keys_as_ints.iter().for_each(|k| {
            let block_height = k.clone();
            let block = db.get::<Block>(&k.to_string()).unwrap();
            if let Some(chunks) = block.chunk() {
                let mut iter_counter = 1;
                chunks.iter().for_each(|data| {
                    let message = MessageType::NetworkStateDataBaseMessage {
                        object: StateBlock(block_height.clone()),
                        data: data.to_vec(),
                        chunk_number: iter_counter as u32,
                        total_chunks: chunks.clone().len() as u32,
                        requestor: requestor.clone(),
                        sender_id: sender_id.clone(),
                    };
                    let message = message::structure_message(message.as_bytes());
                    message::publish_message(Arc::clone(&cloned_node), message, "test-net");
                    iter_counter += 1;
                });
            } else {
                let message = MessageType::NetworkStateDataBaseMessage {
                    object: StateBlock(block_height.clone()),
                    data: block.as_bytes(),
                    chunk_number: 1u32,
                    total_chunks: 1u32,
                    requestor: requestor.clone(),
                    sender_id: sender_id.clone(),
                };
                let message = message::structure_message(message.as_bytes());
                message::publish_message(Arc::clone(&cloned_node), message, "test-net");
            }
            thread::sleep(Duration::from_millis(250));
        });
    } else {
        // Send CannotProvideStateMessage
    }
}

pub fn process_block(block: Block, node: Arc<Mutex<Node>>) {
    let cloned_node = Arc::clone(&node);
    let last_block = cloned_node.lock().unwrap().last_block.clone();
    if let None = last_block {
        if block.block_height == 0 {
            message::process_confirmed_block(block.clone(), Arc::clone(&cloned_node));
            info!(target: "genesis_block", "Set genesis block to network state");
        }
    } else {
        cloned_node
            .lock()
            .unwrap()
            .wallet
            .lock()
            .unwrap()
            .claims
            .retain(|claim_number, _| claim_number != &block.claim.claim_number);
        cloned_node
            .lock()
            .unwrap()
            .account_state
            .lock()
            .unwrap()
            .claim_pool
            .confirmed
            .retain(|claim_number, _| claim_number != &block.claim.claim_number);
        let reward_state = cloned_node
            .lock()
            .unwrap()
            .reward_state
            .lock()
            .unwrap()
            .clone();
        let network_state = cloned_node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .clone();
        let validator_options =
            ValidatorOptions::NewBlock(last_block.clone().unwrap(), reward_state, network_state);
        let pubkey = cloned_node
            .lock()
            .unwrap()
            .wallet
            .lock()
            .unwrap()
            .pubkey
            .clone();

        if block.miner == pubkey {
            // This node mined this block, cannot confirm it, await for confirmation message.
            message::process_confirmed_block(block.clone(), Arc::clone(&cloned_node));
        } else {
            if let Some(true) = block.is_valid(Some(validator_options)) {
                message::process_confirmed_block(block.clone(), Arc::clone(&cloned_node));
                cloned_node.lock().unwrap().last_block = Some(block.clone());
                info!(
                    target: "confirmed_block",
                    "Set block with block_height {} and block hash {} to network state -> claim maturation time: {}, claim_number: {}",
                    &block.block_height, &block.block_hash, &block.claim.expiration_time, &block.claim.claim_number
                );
            }
        }
    }
}

pub fn set_network_state(
    node: Arc<Mutex<Node>>,
    state_chunks: LinkedHashMap<u32, Vec<u8>>,
    total_chunks: u32,
) {
    let cloned_node = Arc::clone(&node);
    let mut chunk_vec = vec![];
    (1..=total_chunks).map(|x| x).for_each(|x| {
        if let Some(chunk) = state_chunks.get(&x) {
            chunk_vec.extend(chunk);
        }
        info!(target: "get_state", "extended chunk vec with chunk {}", &x);
    });

    info!(target: "get_state", "chunk_vec length: {}", &chunk_vec.len());
    let network_state = NetworkState::from_bytes(&chunk_vec);
    cloned_node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .credits = network_state.credits.clone();
    cloned_node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .debits = network_state.debits.clone();
    cloned_node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .reward_state = network_state.reward_state.clone();
    cloned_node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .claims = network_state.claims.clone();
    cloned_node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .block_archive = network_state.block_archive.clone();
    cloned_node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .last_block = network_state.last_block.clone();
    cloned_node.lock().unwrap().reward_state =
        Arc::new(Mutex::new(network_state.reward_state.clone()));
    cloned_node.lock().unwrap().last_block = network_state.last_block.clone();
}
