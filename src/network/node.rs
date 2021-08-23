#[allow(unused_imports)]
use crate::account::AccountState;
use crate::block::Block;
use crate::claim::{Claim, CustodianInfo};
use crate::network::message;
use crate::network::protocol::{build_transport, VrrbNetworkBehavior};
use crate::network::voting::BallotBox;
use crate::reward::RewardState;
use crate::state::NetworkState;
use crate::txn::Txn;
use crate::validator::Validator;
use crate::wallet::WalletAccount;
use async_std::{io, task};
use env_logger::{Builder, Env};
use futures::prelude::*;
use libp2p::gossipsub::MessageId;
use libp2p::gossipsub::{
    Gossipsub, GossipsubConfigBuilder, GossipsubMessage, IdentTopic as Topic, MessageAuthenticity,
    ValidationMode,
};
use libp2p::identify::{Identify, IdentifyConfig};
use libp2p::kad::{record::store::MemoryStore, Kademlia};
use libp2p::multiaddr::multiaddr;
use libp2p::ping::{Ping, PingConfig};
use libp2p::swarm::Swarm;
use libp2p::{identity, Multiaddr, PeerId};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::Duration;
use std::{
    error::Error,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    thread,
};

#[allow(dead_code)]
#[derive(Debug)]
pub enum Command {
    MineBlock,
    StopMine,
    AcquireClaim(u128, u128, u128), // Maximum Price, Maximum Maturity, Maximum Number of claims to acquire that fit the price/maturity requirements, address to purchase from.
    SellClaim(u128, u128),          // Claim Number, Price.
}

#[allow(dead_code)]
pub enum NodeAuth {
    Full,
    Validate,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum VrrbObject {
    Block(Block),
    Txn(Txn),
    Claim(Claim),
    Validator(Validator),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountPk {
    pub addresses: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkStateMessage {
    pub network_state: NetworkState,
    pub requestor: String,
}

#[allow(dead_code)]
pub struct Node {
    pub id: PeerId,
    pub node_type: NodeAuth,
    pub swarm: Swarm<VrrbNetworkBehavior>,
    pub network_state: Arc<Mutex<NetworkState>>,
    pub account_state: Arc<Mutex<AccountState>>,
    pub reward_state: Arc<Mutex<RewardState>>,
    pub command_queue: Arc<Mutex<VecDeque<Vec<String>>>>,
    pub last_block: Option<Block>,
    pub ballot_box: Arc<Mutex<BallotBox>>,
    pub temp_blocks: Option<HashMap<u128, Block>>,
    pub wallet: Arc<Mutex<WalletAccount>>,
}

impl NetworkStateMessage {
    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> NetworkStateMessage {
        serde_json::from_slice::<NetworkStateMessage>(data).unwrap()
    }
}

impl Node {
    pub async fn start(
        ballot_box: Arc<Mutex<BallotBox>>,
        node_type: NodeAuth,
        wallet: Arc<Mutex<WalletAccount>>,
        account_state: Arc<Mutex<AccountState>>,
        network_state: Arc<Mutex<NetworkState>>,
        reward_state: Arc<Mutex<RewardState>>,
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
                gossipsub_config,
            )
            .expect("Correct configuration");

            gossipsub.subscribe(&testnet_topic).unwrap();
            gossipsub.subscribe(&txn_topic).unwrap();
            gossipsub.subscribe(&claim_topic).unwrap();
            gossipsub.subscribe(&block_topic).unwrap();
            gossipsub.subscribe(&validator_topic).unwrap();
            let store = MemoryStore::new(local_peer_id);
            let kademlia = Kademlia::new(local_peer_id, store);
            let identify_config =
                IdentifyConfig::new("vrrb/test-net/1.0.0".to_string(), local_key.public());

            let identify = Identify::new(identify_config);
            let ping = Ping::new(PingConfig::new());
            let queue: Arc<Mutex<VecDeque<GossipsubMessage>>> =
                Arc::new(Mutex::new(VecDeque::new()));

            let behaviour = VrrbNetworkBehavior {
                gossipsub,
                identify,
                kademlia,
                ping,
                queue,
            };

            let transport = build_transport(local_key).await.unwrap();
            Swarm::new(transport, behaviour, local_peer_id)
        };

        let command_queue: Arc<Mutex<VecDeque<Vec<String>>>> =
            Arc::new(Mutex::new(VecDeque::new()));
        let account_state = Arc::clone(&account_state);
        let network_state = Arc::clone(&network_state);
        let reward_state = Arc::clone(&reward_state);
        let wallet = Arc::clone(&wallet);

        let node = Node {
            id: local_peer_id,
            node_type,
            swarm,
            last_block: None,
            ballot_box,
            temp_blocks: None,
            command_queue: Arc::clone(&command_queue),
            account_state: Arc::clone(&account_state),
            network_state: Arc::clone(&network_state),
            reward_state: Arc::clone(&reward_state),
            wallet: Arc::clone(&wallet),
        };

        let port = rand::thread_rng().gen_range(9292, 19292);
        // Listen on all interfaces and whatever port the OS assigns
        // TODO: Get the public IP of the node so external nodes can connect
        // and only listen on this address.
        let addr: Multiaddr = multiaddr!(Ip4([0, 0, 0, 0]), Tcp(port as u16));
        let atomic_node = Arc::new(Mutex::new(node));

        println!("{:?}", &addr);

        atomic_node
            .lock()
            .unwrap()
            .swarm
            .listen_on(addr.clone())
            .unwrap();

        atomic_node
            .lock()
            .unwrap()
            .swarm
            .behaviour_mut()
            .kademlia
            .add_address(&local_peer_id, addr.clone());

        if let Some(to_dial) = std::env::args().nth(1) {
            let dialing = to_dial.clone();
            match to_dial.parse() {
                Ok(to_dial) => match atomic_node.lock().unwrap().swarm.dial_addr(to_dial) {
                    Ok(_) => {
                        println!("Dialed {:?}", dialing);
                    }
                    Err(e) => println!("Dial {:?} failed: {:?}", dialing, e),
                },
                Err(err) => println!("Failed to parse address to dial {:?}", err),
            }
        }

        let atomic_message_queue = Arc::clone(&atomic_node.lock().unwrap().swarm.behaviour().queue);
        let task_node = Arc::clone(&atomic_node);
        thread::spawn(move || loop {
            while let Some(message) = atomic_message_queue.lock().unwrap().pop_front() {
                let cloned_node = Arc::clone(&task_node);
                thread::spawn(move || {
                    message::process_message(message, Arc::clone(&cloned_node));
                })
                .join()
                .unwrap();
            }
        });

        let mut stdin = io::BufReader::new(io::stdin()).lines();

        task::block_on(future::poll_fn(move |cx: &mut Context<'_>| {
            let task_node = Arc::clone(&atomic_node);

            // Finish bootstrapping by getting last block from network state, setting node last block
            // to network state last block. Request blocks you are missing.

            loop {
                let cloned_node = Arc::clone(&task_node);
                match stdin.try_poll_next_unpin(cx)? {
                    Poll::Ready(Some(line)) => handle_input_line(cloned_node, line),
                    Poll::Ready(None) => panic!("Stdin closed"),
                    Poll::Pending => break,
                }
            }

            let task_node = Arc::clone(&atomic_node);
            loop {
                match task_node.lock().unwrap().swarm.poll_next_unpin(cx) {
                    Poll::Ready(Some(event)) => match event {
                        _ => println!("Event --> {:?}", event),
                    },
                    Poll::Ready(None) | Poll::Pending => break,
                }
            }

            Poll::Pending
        }))
    }
}

impl AccountPk {
    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> AccountState {
        let mut buffer: Vec<u8> = vec![];
        data.iter().for_each(|x| buffer.push(*x));
        let to_string = String::from_utf8(buffer).unwrap();
        serde_json::from_str::<AccountState>(&to_string).unwrap()
    }
}

fn handle_input_line(node: Arc<Mutex<Node>>, line: String) {
    let args: Vec<&str> = line.split(' ').collect();
    let task_node = Arc::clone(&node);
    match args[0] {
        "SENDTXN" => {
            let txn = task_node.lock().unwrap().wallet.lock().unwrap().send_txn(
                args[1].parse::<u32>().unwrap(),
                args[2].to_string(),
                args[3].parse::<u128>().unwrap(),
            );

            if let Ok(txn) = txn {
                let header = "NEW_TXN".to_string().as_bytes().to_vec();
                let id = task_node
                    .lock()
                    .unwrap()
                    .id
                    .clone()
                    .to_string()
                    .as_bytes()
                    .to_vec();
                let message = message::structure_message(header, id, txn.as_bytes());
                if let Err(e) = node
                    .lock()
                    .unwrap()
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(Topic::new("test-net"), message)
                {
                    println!("Error publishing message: {:?}", e);
                };
            }
        }
        "MINEBLK" => {
            thread::spawn(move || loop {
                let cloned_node = Arc::clone(&task_node);
                thread::spawn(move || {
                    mine_block(cloned_node);
                })
                .join()
                .unwrap();
            });
        }
        "SENDADR" => {
            let cloned_node = Arc::clone(&task_node);
            thread::spawn(move || share_addresses(cloned_node))
                .join()
                .unwrap();
        }

        _ => {}
    }
}

pub fn share_addresses(node: Arc<Mutex<Node>>) {
    let header = "NEWADDR".to_string().as_bytes().to_vec();
    let id = node
        .lock()
        .unwrap()
        .id
        .clone()
        .to_string()
        .as_bytes()
        .to_vec();
    let mut addr_pubkey = HashMap::new();
    let wallet = node.lock().unwrap().wallet.lock().unwrap().clone();
    let pubkey = wallet.pubkey.clone();
    wallet.addresses.iter().for_each(|(_addr_num, addr)| {
        addr_pubkey.insert(addr.clone(), pubkey.clone());
    });
    let accounts = AccountPk {
        addresses: addr_pubkey,
    };

    let accounts_bytes = accounts.as_bytes();

    let message = message::structure_message(header, id, accounts_bytes);
    message::publish_message(Arc::clone(&node), message, "test-net".to_string());
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
            let header = "NEW_BLK".to_string().as_bytes().to_vec();
            let id = cloned_node
                .lock()
                .unwrap()
                .id
                .clone()
                .to_string()
                .as_bytes()
                .to_vec();
            let message = message::structure_message(header, id, block.as_bytes());

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
                let header = "NEW_BLK".to_string().as_bytes().to_vec();
                let id = cloned_node
                    .lock()
                    .unwrap()
                    .id
                    .clone()
                    .to_string()
                    .as_bytes()
                    .to_vec();
                let message = message::structure_message(header, id, block.as_bytes());
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
    if let Ok(_) = node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .state
        .set("lastblock", &block)
    {
        println!("Successfully set last block to network state");
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
        if let Ok(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("blockarchive", &map)
        {
            println!("Successfully set block archive to network state");
        }
    } else {
        let mut map: HashMap<u128, Block> = HashMap::new();
        map.insert(block.block_height.clone(), block.clone());
        if let Ok(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("blockarchive", &map)
        {
            println!("Successfully set block to blockarchive");
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

        if let Ok(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("claims", &map)
        {
            println!("Successfully set new claims to network state");
        }
    } else {
        let map = block.owned_claims.clone();
        if let Ok(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("claims", &map)
        {
            println!("Successfully set claims to network state");
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

        if let Ok(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("credits", &creditmap)
        {
            println!("Successfully set credits to network state");
        }

        if let Ok(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("debits", &debitmap)
        {
            println!("Successfully set debits to network state");
        }
    } else {
        let mut creditmap: HashMap<String, u128> = HashMap::new();
        let mut debitmap: HashMap<String, u128> = HashMap::new();

        block.data.iter().for_each(|(_txn_id, txn)| {
            creditmap.insert(txn.receiver_address.clone(), txn.txn_amount);
            debitmap.insert(txn.sender_address.clone(), txn.txn_amount);
        });

        creditmap.insert(block.miner.clone(), block.block_reward.amount);

        if let Ok(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("credits", &creditmap)
        {
            println!("Successfully set credits to network state");
        }

        if let Ok(_) = node
            .lock()
            .unwrap()
            .network_state
            .lock()
            .unwrap()
            .state
            .set("debits", &debitmap)
        {
            println!("Successfully set debits to network state");
        }
    }
}

pub fn update_reward_state(node: Arc<Mutex<Node>>, block: &Block) {
    let reward_state = Arc::clone(&node.lock().unwrap().reward_state);
    let mut reward_state = reward_state.lock().unwrap().clone();
    reward_state.update(block.clone().block_reward.category);

    if let Ok(_) = node
        .lock()
        .unwrap()
        .network_state
        .lock()
        .unwrap()
        .state
        .set("rewardstate", &reward_state)
    {
        println!("Successfully set reward_state to network state");
    }
}

pub fn send_state(node: Arc<Mutex<Node>>, requestor: String) {
    let network_state = node.lock().unwrap().network_state.lock().unwrap().clone();
    let network_state_message = NetworkStateMessage {
        network_state,
        requestor,
    };

    let network_state_bytes = network_state_message.as_bytes();
    let id = node
        .lock()
        .unwrap()
        .id
        .clone()
        .to_string()
        .as_bytes()
        .to_vec();
    let header = "LASTSTE".to_string().as_bytes().to_vec();
    let message = message::structure_message(header, id, network_state_bytes);
    message::publish_message(Arc::clone(&node), message, "test-net".to_string());
}
