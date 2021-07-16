#[allow(unused_imports)]
use crate::account::{WalletAccount, AccountState};
use crate::state::NetworkState;
use crate::network::protocol::{VrrbNetworkBehavior, build_transport};
use crate::txn::Txn;
use crate::validator::{Validator, Message};
use crate::claim::{Claim, CustodianInfo};
use crate::block::Block;
use crate::reward::RewardState;
use async_std::{io, task};
use env_logger::{Builder, Env};
use futures::prelude::*;
use libp2p::kad::{Kademlia, record::store::MemoryStore};
use libp2p::gossipsub::MessageId;
use libp2p::gossipsub::{
    GossipsubConfigBuilder,
    GossipsubMessage, 
    IdentTopic as Topic,
    TopicHash,
    MessageAuthenticity, 
    ValidationMode,
    Gossipsub,
};
use libp2p::identify::{IdentifyConfig, Identify};
use libp2p::ping::{Ping, PingConfig};
use libp2p::swarm::{Swarm};
use libp2p::multiaddr::multiaddr;
use libp2p::{identity, PeerId, Multiaddr};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use std::{
    error::Error,
    task::{Context, Poll},
    thread,
    sync::{Arc, Mutex}
};
use std::collections::VecDeque;
use rand::{Rng};
use hex;

#[allow(dead_code)]
pub enum NodeAuth {
    Full(Vec<TopicHash>),
    Transact(Vec<TopicHash>),
    Validate(Vec<TopicHash>),
}

#[allow(dead_code)]
pub struct Node {
    pub id: PeerId,
    pub wallet: &'static mut WalletAccount,
    pub swarm: Swarm<VrrbNetworkBehavior>,
    pub account_state: &'static mut AccountState,
    pub network_state: &'static mut NetworkState,
    pub reward_state: &'static mut RewardState,
}

impl Node {

    pub async fn start(
        wallet: &'static mut WalletAccount, 
        account_state: &'static mut AccountState, 
        network_state: &'static mut NetworkState,
        reward_state: &'static mut RewardState,
    ) -> Result<(), Box<dyn Error>> {

        Builder::from_env(Env::default().default_filter_or("info")).init();

        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        // TODO pass topics in through the function header
        // such that not every node is required to subscribe/publish to every topic
        let testnet_topic = Topic::new("test-net");
        let txn_topic = Topic::new("txn");
        let claim_topic = Topic::new("claim");
        let block_topic = Topic::new("block");
        let validator_topic = Topic::new("validator");
        

        let swarm = {
            let message_id_fn = |message: &GossipsubMessage| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                MessageId::from(s.finish().to_string())
            };

        let gossipsub_config = GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .build()
            .expect("Valid config");
        
        let mut gossipsub: Gossipsub = Gossipsub::new(
            MessageAuthenticity::Signed(local_key.clone()), 
            gossipsub_config).expect("Correct configuration");
        
        gossipsub.subscribe(&testnet_topic).unwrap();
        gossipsub.subscribe(&txn_topic).unwrap();
        gossipsub.subscribe(&claim_topic).unwrap();
        gossipsub.subscribe(&block_topic).unwrap();
        gossipsub.subscribe(&validator_topic).unwrap();

        let store = MemoryStore::new(local_peer_id);
        let kademlia = Kademlia::new(local_peer_id, store);
        
        let identify_config = IdentifyConfig::new(
            "vrrb/test-net/1.0.0".to_string(),
            local_key.public(),
        );
        let identify = Identify::new(identify_config);

        let ping = Ping::new(PingConfig::new());
        let queue: Arc<Mutex<VecDeque<GossipsubMessage>>> = Arc::new(Mutex::new(VecDeque::new()));

        let behaviour = VrrbNetworkBehavior {
            gossipsub,
            identify,
            kademlia,
            ping, 
            queue
        };

        let transport = build_transport(local_key).await.unwrap();

        Swarm::new(transport, behaviour, local_peer_id)
        
        };

        let mut node = Node {
            id: local_peer_id,
            swarm,
            wallet,
            account_state,
            network_state,
            reward_state,
        };

        let port = rand::thread_rng().gen_range(9292, 19292);
        // Listen on all interfaces and whatever port the OS assigns
        // TODO: Get the public IP of the node so external nodes can connect
        // and only listen on this address.
        let addr: Multiaddr = multiaddr!(Ip4([0,0,0,0]), Tcp(port as u16));
        
        println!("{:?}", &addr);

        node.swarm.listen_on(addr.clone()).unwrap();

        node.swarm.behaviour_mut().kademlia.add_address(&local_peer_id, addr.clone());

        if let Some(to_dial) = std::env::args().nth(1) {
            let dialing = to_dial.clone();
            match to_dial.parse() {
                Ok(to_dial) => match node.swarm.dial_addr(to_dial) {
                    Ok(_) => {
                        println!("Dialed {:?}", dialing);
                        },
                    Err(e) => println!("Dial {:?} failed: {:?}", dialing, e)
                },
                Err(err) => println!("Failed to parse address to dial {:?}", err),
            }
        }

        let mut stdin = io::BufReader::new(io::stdin()).lines();

        task::block_on(future::poll_fn(move |cx: &mut Context<'_>| {
            let atomic_queue = Arc::clone(&node.swarm.behaviour().queue);

            thread::spawn(move || 
                loop {
                    match atomic_queue.lock().unwrap().pop_front() {
                        Some(message) => {
                            // TODO: process message in helper function.
                            // process_message(message, &mut node.clone());
                        },
                        None => {},
                }});

            loop {
                match stdin.try_poll_next_unpin(cx)? {
                    Poll::Ready(Some(line)) => {
                        handle_input_line(&mut node.swarm.behaviour_mut(), line)
                    },
                    Poll::Ready(None) => panic!("Stdin closed"),
                    Poll::Pending => break,
                }        
            }

            loop {
                match node.swarm.poll_next_unpin(cx) {
                    Poll::Ready(Some(event)) => {
                        match event {
                            _ => println!("Event --> {:?}", event)
                        }
                    }
                    Poll::Ready(None) | Poll::Pending => break
                }
            }

            Poll::Pending
        }))
    }
}

fn handle_input_line(behaviour: &mut VrrbNetworkBehavior, line: String) {
    // Message matching
    //
    // Insure topic is correct if so, then publish to topic, 
    // if not then return a message to the local peer indicating 
    // the message topic is incorrect.
    
    let message = hex::encode(line.as_bytes());
    let mut commands = line.split(' ');

    match commands.next() {
        Some("NEW_TXN") | Some("UPD_TXN") => {
            behaviour.gossipsub.publish(Topic::new("txn"), message).unwrap();
        }
        Some("CLM_HOM") | Some("CLM_SAL") | Some("CLM_ACQ") | Some("CLM_STK") => {
            behaviour.gossipsub.publish(Topic::new("claim"), message).unwrap();
        },
        Some("NEW_BLK") | Some("GET_BLK") => {
            behaviour.gossipsub.publish(Topic::new("block"), message).unwrap();
        },

        Some("TXN_VAL") | Some("CLM_VAL") | Some("BLK_VAL") => {
            behaviour.gossipsub.publish(Topic::new("validator"), message).unwrap();
        },
        Some("VRRB_IP") | Some("UPDST_P") => {
            behaviour.gossipsub.publish(Topic::new("test-net"), message).unwrap();
        },
        Some(_) => {},
        None => {},

    }
}

pub fn publish_validator(validator: Validator, node: &mut Node, header: &str) {
    let processed = validator.validate();
    let validator_bytes = processed.as_bytes();
    let header = hex::encode(String::from(header).as_bytes());
    let data = hex::encode(validator_bytes);
    let mut message = header.to_owned();
    message.push_str(" ");
    message.push_str(&data);
    node.swarm.behaviour_mut().gossipsub
        .publish(Topic::new("validator"), message.as_bytes()).unwrap();
}

pub fn process_message(message: GossipsubMessage, node: &mut Node) {
    let header = &String::from_utf8_lossy(&message.data)[0..7];
    let data = &String::from_utf8_lossy(&message.data)[9..];
    match header {
        "NEW_TXN" => {
            match hex::decode(data) {
                Ok(bytes) => {
                    let txn = Txn::from_bytes(&bytes[..]);
                    let txn_validator = Validator::new(
                        Message::Txn(txn, node.account_state.clone()),
                        node.wallet.clone(),
                        node.account_state.clone()
                    );
                    // UPDATE LOCAL Account State pending balances for sender(s)/
                    // receivers.
                    match txn_validator {
                        Some(validator) => {
                            publish_validator(validator, node, "TXN_VAL");
                        },
                        None => println!("You are not running a validator node or have no claims staked")
                    };

                },
                Err(e) => println!("Error encountered while decoding message data: {:?}", e)
            }
        },
        "UPD_TXN" => {},
        "CLM_HOM" => {
            match hex::decode(data) {
                Ok(bytes) => {
                    let claim = Claim::from_bytes(&bytes[..]);
                    let claim_validator = Validator::new(Message::ClaimHomesteaded(
                        claim.clone(), 
                        claim.current_owner.1.unwrap(), 
                        node.account_state.clone()), node.wallet.clone(), node.account_state.clone());
                    
                    match claim_validator {
                        Some(validator) => {
                            publish_validator(validator, node, "CLM_VAL");
                        },
                        None => println!("You are not running a validator node or have no claims staked")
                    }
                },
                Err(e) => {println!("Error encountered while decoding message data: {:?}", e)}
            }
        },
        "CLM_SAL" => {
            // TODO: Need to add a ClaimSale Message in Validator for when a claim holder
            // places it for sale.
            match hex::decode(data) {
                Ok(bytes) => {
                    let claim = Claim::from_bytes(&bytes[..]);
                    match claim.clone().chain_of_custody
                            .get(&claim.clone().current_owner.1.unwrap())
                            .unwrap()
                            .get("acquired_from")
                            .unwrap() 
                        {
                        Some(custodian_info) => {
                            match custodian_info {
                                CustodianInfo::AcquiredFrom((_, pubkey, _)) => {
                                    let claim_validator = Validator::new(Message::ClaimAcquired(
                                        claim.clone(), 
                                        pubkey.as_ref().unwrap().to_owned(), 
                                        node.account_state.clone(),
                                        claim.clone().current_owner.1.unwrap()
                                    ), node.wallet.clone(), node.account_state.clone());
                    
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

                    
                },
                Err(e) => {println!("Error encountered while decoding message data: {:?}", e)}
            }

        },
        "CLM_ACQ" => {
            match hex::decode(data) {
                Ok(bytes) => {
                    let claim = Claim::from_bytes(&bytes[..]);
                    match claim.clone().chain_of_custody
                            .get(&claim.clone().current_owner.1.unwrap())
                            .unwrap()
                            .get("acquired_from")
                            .unwrap() 
                        {
                        Some(custodian_info) => {
                            match custodian_info {
                                CustodianInfo::AcquiredFrom((_, pubkey, _)) => {
                                    let claim_validator = Validator::new(Message::ClaimAcquired(
                                        claim.clone(), 
                                        pubkey.as_ref().unwrap().to_owned(), 
                                        node.account_state.clone(),
                                        claim.clone().current_owner.1.unwrap()
                                    ), node.wallet.clone(), node.account_state.clone());
                    
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

                    
                },
                Err(e) => {println!("Error encountered while decoding message data: {:?}", e)}
            }
        },
        "NEW_BLK" => {
            match hex::decode(data) {
                Ok(bytes) => {
                    let block = Block::from_bytes(&bytes[..]);
                    let block_validator = Validator::new(
                        Message::NewBlock(Box::new(
                            (
                                last_block, 
                                block.clone(), 
                                block.miner.clone(), 
                                node.network_state.clone(),
                                node.account_state.clone(),
                                node.reward_state.clone()
                            ) 
                        )
                    ), node.wallet.clone(), node.account_state.clone());

                },
                Err(e) => {println!("Error encountered while decoding message data: {:?}", e)}
            }
        },
        "GET_BLK" => {},
        "VRRB_IP" => {},
        "UPDST_P" => {},
        _ => {}

    }
}