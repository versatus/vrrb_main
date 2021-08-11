#[allow(unused_imports)]
use crate::account::{WalletAccount, AccountState};
use crate::state::NetworkState;
use crate::network::protocol::{VrrbNetworkBehavior, build_transport};
use crate::txn::Txn;
use crate::block::Block;
use crate::reward::RewardState;
use crate::network::voting::BallotBox;
use crate::network::message;
use async_std::{io, task};
use env_logger::{Builder, Env};
use futures::prelude::*;
use libp2p::kad::{Kademlia, record::store::MemoryStore};
use libp2p::gossipsub::MessageId;
use libp2p::gossipsub::{
    GossipsubConfigBuilder,
    GossipsubMessage, 
    IdentTopic as Topic,
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
use std::collections::{VecDeque, HashMap};
use rand::{Rng};
use hex;

#[allow(dead_code)]
pub enum NodeAuth {
    Full,
    Validate,
}

#[allow(dead_code)]
pub struct Node {
    pub id: PeerId,
    pub node_type: NodeAuth,
    pub wallet: Arc<Mutex<WalletAccount>>,
    pub swarm: Swarm<VrrbNetworkBehavior>,
    pub account_state: Arc<Mutex<AccountState>>,
    pub network_state: Arc<Mutex<NetworkState>>,
    pub reward_state: Arc<Mutex<RewardState>>,
    pub last_block: Option<Block>,
    pub ballot_box: Arc<Mutex<BallotBox>>,
    pub temp_blocks: Option<HashMap<u128, Block>>,
}

impl Node {

    pub async fn start(
        wallet: Arc<Mutex<WalletAccount>>, 
        account_state: Arc<Mutex<AccountState>>, 
        network_state: Arc<Mutex<NetworkState>>,
        reward_state: Arc<Mutex<RewardState>>,
        ballot_box: Arc<Mutex<BallotBox>>,
        node_type: NodeAuth,
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

        let node = Node {
            id: local_peer_id,
            node_type,
            swarm,
            wallet,
            account_state,
            network_state,
            reward_state,
            last_block: None,
            ballot_box,
            temp_blocks: None,
        };

        let port = rand::thread_rng().gen_range(9292, 19292);
        // Listen on all interfaces and whatever port the OS assigns
        // TODO: Get the public IP of the node so external nodes can connect
        // and only listen on this address.
        let addr: Multiaddr = multiaddr!(Ip4([0,0,0,0]), Tcp(port as u16));
        
        let atomic_node = Arc::new(Mutex::new(node));

        println!("{:?}", &addr);

        atomic_node.lock().unwrap().swarm
            .listen_on(addr.clone())
            .unwrap();

        atomic_node.lock().unwrap().swarm
            .behaviour_mut().kademlia
            .add_address(&local_peer_id, addr.clone());

        if let Some(to_dial) = std::env::args().nth(1) {
            let dialing = to_dial.clone();
            match to_dial.parse() {
                Ok(to_dial) => match atomic_node.lock().unwrap().swarm.dial_addr(to_dial) {
                    Ok(_) => {
                        println!("Dialed {:?}", dialing);
                        },
                    Err(e) => println!("Dial {:?} failed: {:?}", dialing, e)
                },
                Err(err) => println!("Failed to parse address to dial {:?}", err),
            }
        }

        let mut stdin = io::BufReader::new(io::stdin()).lines();
        let atomic_queue = Arc::clone(&atomic_node.lock().unwrap().swarm.behaviour().queue);
        let task_node = Arc::clone(&atomic_node);

        thread::spawn(move || 
            loop {
                let cloned_node = Arc::clone(&task_node);
                match atomic_queue.lock().unwrap().pop_front() {
                    Some(message) => {
                        message::process_message(message, cloned_node);
                    },
                    None => {},
            }});

        task::block_on(future::poll_fn(move |cx: &mut Context<'_>| {
            
            let task_node = Arc::clone(&atomic_node);
            loop {
                let cloned_node = Arc::clone(&task_node);
                match stdin.try_poll_next_unpin(cx)? {
                    Poll::Ready(Some(line)) => {
                        handle_input_line(cloned_node, line)
                    },
                    Poll::Ready(None) => panic!("Stdin closed"),
                    Poll::Pending => break,
                }        
            }

            let task_node = Arc::clone(&atomic_node);
            loop {
                match task_node.lock().unwrap().swarm.poll_next_unpin(cx) {
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

fn handle_input_line(node: Arc<Mutex<Node>>, line: String) {
    // Message matching
    //
    // Insure topic is correct if so, then publish to topic, 
    // if not then return a message to the local peer indicating 
    // the message topic is incorrect.
    let args = line.split(' ').collect::<Vec<&str>>();
    match args[0] {
        "SENDTXN" | "UPD_TXN" => {
            let sender = node.lock().unwrap().wallet.clone();
            let sender_address = sender.lock().unwrap().addresses[&args[1].parse::<u32>().unwrap()].clone();
            let receiver = args[2].to_string();
            let amount = args[3].parse::<u128>().unwrap();
            let mut header: String = "".to_string();

            if args[0] == "SENDTXN" {
                header.push_str("NEW_TXN");
            } else {
                header.push_str("UPD_TXN");
            }

            let txn = Txn::new(sender, sender_address, receiver, amount).as_bytes();

            let mut message_bytes = vec![];
            let id = node.lock().unwrap().id.to_string().as_bytes().to_vec();
            let header = header.as_bytes().to_vec();

            println!("{}", header.len());
            println!("{}", id.len());
            
            message_bytes.extend(header);
            message_bytes.extend(id);
            message_bytes.extend(txn);

            let message = hex::encode(message_bytes);
            node.lock().unwrap()
                .swarm.behaviour_mut()
                .gossipsub.publish(Topic::new("txn"), message).unwrap();
        },
        
        "GET_BLK" => {
            let message = hex::encode(line.as_bytes());
            node.lock().unwrap()
                .swarm.behaviour_mut()
                .gossipsub.publish(Topic::new("block"), message).unwrap();
        },
        "GETPEER" => {},
        "GET_STE" => {},
        "GET_TXN" => {},
        "VRRB_IP" | "UPDST_P" => {
            let message = hex::encode(line.as_bytes());
            node.lock().unwrap()
                .swarm.behaviour_mut()
                .gossipsub.publish(Topic::new("test-net"), message).unwrap();
        },
        "MINEBLK" => {},

        _ => {},
    }
}


