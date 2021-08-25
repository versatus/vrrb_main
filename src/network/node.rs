#[allow(unused_imports)]
use crate::account::AccountState;
use crate::block::Block;
use crate::network::message;
use crate::network::protocol::{build_transport, VrrbNetworkBehavior};
use crate::network::voting::BallotBox;
use crate::reward::RewardState;
use crate::state::NetworkState;
use crate::network::command_utils;
use crate::network::command_utils::{Command};
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
pub enum NodeAuth {
    Full,
    Validate,
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
            let command_queue: Arc<Mutex<VecDeque<Command>>> = Arc::new(Mutex::new(VecDeque::new()));

            let behaviour = VrrbNetworkBehavior {
                gossipsub,
                identify,
                kademlia,
                ping,
                queue,
                command_queue,
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

        let atomic_command_queue =
            Arc::clone(&atomic_node.lock().unwrap().swarm.behaviour().command_queue);
        let task_node = Arc::clone(&atomic_node);
        thread::spawn(move || loop {
            while let Some(command) = atomic_command_queue.lock().unwrap().pop_front() {
                let cloned_node = Arc::clone(&task_node);
                thread::spawn(move || {
                    command_utils::handle_command(cloned_node, command);
                })
                .join()
                .unwrap()
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
                    Poll::Ready(Some(line)) => command_utils::handle_input_line(cloned_node, line),
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



