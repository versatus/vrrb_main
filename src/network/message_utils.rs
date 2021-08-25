use crate::block::Block;
use crate::claim::{Claim, CustodianInfo};
use crate::network::message;
use crate::network::node::Node;
use crate::network::message_types::MessageType;
use libp2p::gossipsub::IdentTopic as Topic;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};


pub fn share_addresses(node: Arc<Mutex<Node>>) {
    let mut addr_pubkey = HashMap::new();
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
            Arc::clone(&network_state),
            Arc::clone(&reward_state),
            Arc::clone(&account_state),
        );

        if let Ok(block) = block {

            let block_message = MessageType::BlockMessage {
                block: block.clone(),
                sender_id: node.lock().unwrap().id.clone().to_string(),
            };

            let message = message::structure_message(block_message.as_bytes());

            update_last_block(Arc::clone(&cloned_node), &block);
            update_block_archive(Arc::clone(&cloned_node), &block);
            update_claims(Arc::clone(&cloned_node), &block);
            update_credits_and_debits(Arc::clone(&cloned_node), &block);
            update_reward_state(Arc::clone(&cloned_node), &block);

            if let Err(e) = cloned_node
                .lock()
                .unwrap()
                .network_state
                .lock()
                .unwrap()
                .state
                .dump()
            {
                println!("Error dumping update to network state: {:?}", e);
            }

            if let Err(e) = cloned_node
                .lock()
                .unwrap()
                .swarm
                .behaviour_mut()
                .gossipsub
                .publish(Topic::new("test-net"), message)
            {
                println!("Error sending message to network: {:?}", e);
            }
        }
    } else {
        let claims = cloned_node
            .lock()
            .unwrap()
            .account_state
            .lock()
            .unwrap()
            .claims
            .clone();
        let address = cloned_node
            .lock()
            .unwrap()
            .wallet
            .lock()
            .unwrap()
            .pubkey
            .clone();
        if let Some(claim) = claims
            .iter()
            .filter(|(_claim_number, claim)| claim.current_owner == Some(address.clone()))
            .min_by_key(|x| x.0)
        {
            let mut claim_to_mine = claim.1.clone();
            let signature = node
                .lock()
                .unwrap()
                .wallet
                .lock()
                .unwrap()
                .sign(&claim_to_mine.claim_payload.clone().unwrap());
            if let Some(map) = claim_to_mine.chain_of_custody.get_mut(&address) {
                if let Some(entry) = map.get_mut("buyer_signature") {
                    *entry = Some(CustodianInfo::BuyerSignature(Some(
                        signature.unwrap().to_string(),
                    )));
                };
            }
            let block = Block::mine(
                claim_to_mine,
                last_block.unwrap(),
                Arc::clone(&account_state),
                Arc::clone(&reward_state),
                Arc::clone(&network_state),
                Arc::clone(&miner),
            );
            if let Some(Ok(block)) = block {
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
                    println!("Error sending message to network: {:?}", e);
                }

                update_last_block(Arc::clone(&cloned_node), &block);
                update_block_archive(Arc::clone(&cloned_node), &block);
                update_claims(Arc::clone(&cloned_node), &block);
                update_credits_and_debits(Arc::clone(&cloned_node), &block);
                update_reward_state(Arc::clone(&cloned_node), &block);
            }
        } else {
            println!("No claims to mine");
        };
    }
}

pub fn update_last_block(node: Arc<Mutex<Node>>, block: &Block) {
    node.lock().unwrap().last_block = Some(block.clone());
    if let Err(_) = node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .state
        .set("lastblock", &block)
    {
        println!("Error setting last block to network state");
    }
}

pub fn update_block_archive(node: Arc<Mutex<Node>>, block: &Block) {
    let block_archive = node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .state
        .get::<HashMap<u128, Block>>("blockarchive")
        .clone();

    if let Some(mut map) = block_archive {
        map.insert(block.block_height.clone(), block.clone());
        if let Err(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("blockarchive", &map)
        {
            println!("Error setting block archive to network state");
        }
    } else {
        let mut map: HashMap<u128, Block> = HashMap::new();
        map.insert(block.block_height.clone(), block.clone());
        if let Err(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("blockarchive", &map)
        {
            println!("Error setting block archive to network state");
        }
    }
}

pub fn update_claims(node: Arc<Mutex<Node>>, block: &Block) {
    node.lock()
        .unwrap()
        .account_state
        .lock()
        .unwrap()
        .claims
        .extend(block.owned_claims.clone());

    node.lock()
        .unwrap()
        .account_state
        .lock()
        .unwrap()
        .claims
        .retain(|claim_number, _| claim_number != &block.claim.claim_number);

    let claims = node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .state
        .get::<HashMap<u128, Claim>>("claims")
        .clone();
    if let Some(mut map) = claims {
        map.extend(block.owned_claims.clone());

        if let Err(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("claims", &map)
        {
            println!("Error setting claims to network state");
        }
    } else {
        let map = block.owned_claims.clone();
        if let Err(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("claims", &map)
        {
            println!("Error setting claims to network state");
        }
    }
}

pub fn update_credits_and_debits(node: Arc<Mutex<Node>>, block: &Block) {
    let credits = node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .state
        .get::<HashMap<String, u128>>("credits")
        .clone();

    let debits = node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .state
        .get::<HashMap<String, u128>>("debits")
        .clone();

    if let (Some(mut creditmap), Some(mut debitmap)) = (credits, debits) {
        block.data.iter().for_each(|(_txn_id, txn)| {
            if let Some(entry) = creditmap.get_mut(&txn.receiver_address) {
                *entry += txn.txn_amount;
            } else {
                creditmap.insert(txn.receiver_address.clone(), txn.txn_amount.clone());
            }
            if let Some(entry) = debitmap.get_mut(&txn.sender_address) {
                *entry += txn.txn_amount;
            } else {
                debitmap.insert(txn.sender_address.clone(), txn.txn_amount.clone());
            }
        });

        if let Some(entry) = creditmap.get_mut(&block.miner) {
            *entry += block.block_reward.amount;
        } else {
            creditmap.insert(block.miner.clone(), block.block_reward.amount.clone());
        }

        if let Err(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("credits", &creditmap)
        {
            println!("Error setting credits to network state");
        }

        if let Err(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("debits", &debitmap)
        {
            println!("Error setting debits to network state");
        }
    } else {
        let mut creditmap: HashMap<String, u128> = HashMap::new();
        let mut debitmap: HashMap<String, u128> = HashMap::new();

        block.data.iter().for_each(|(_txn_id, txn)| {
            creditmap.insert(txn.receiver_address.clone(), txn.txn_amount);
            debitmap.insert(txn.sender_address.clone(), txn.txn_amount);
        });

        creditmap.insert(block.miner.clone(), block.block_reward.amount);

        if let Err(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("credits", &creditmap)
        {
            println!("Error set credits to network state");
        }

        if let Err(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("debits", &debitmap)
        {
            println!("Error set debits to network state");
        }
    }
}

pub fn update_reward_state(node: Arc<Mutex<Node>>, block: &Block) {
    let reward_state = Arc::clone(&node.lock().unwrap().reward_state);
    let mut reward_state = reward_state.lock().unwrap().clone();
    reward_state.update(block.clone().block_reward.category);

    if let Err(_) = node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .state
        .set("rewardstate", &reward_state)
    {
        println!("Error setting reward_state to network state");
    }
}

pub fn send_state(node: Arc<Mutex<Node>>, requestor: String) {
    let network_state = node.lock().unwrap().network_state.lock().unwrap().clone();
    let network_state_message = MessageType::NetworkStateMessage {
        network_state,
        requestor,
        sender_id: node.lock().unwrap().id.clone().to_string(),
    };

    let network_state_bytes = network_state_message.as_bytes();

    let message = message::structure_message(network_state_bytes);
    message::publish_message(Arc::clone(&node), message, "test-net");
}
