#[allow(unused_imports)]
use crate::account::AccountState;
use crate::block::Block;
use crate::network::command_utils;
use crate::network::command_utils::Command;
use crate::network::config_utils;
use crate::network::message;
use crate::network::message_utils;
use crate::network::protocol::VrrbNetworkBehavior;
use crate::network::voting::BallotBox;
use crate::reward::RewardState;
use crate::state::NetworkState;
use crate::wallet::WalletAccount;
use async_std::{io, task};
use futures::prelude::*;
use libp2p::multiaddr::multiaddr;
use libp2p::swarm::Swarm;
use libp2p::{identity, Multiaddr, PeerId};
use log::info;
use rand::Rng;
use ritelinked::LinkedHashMap;
use simplelog::{Config, LevelFilter, WriteLogger};
use std::collections::{HashMap};
use std::fs::File;
use std::{
    error::Error,
    sync::{mpsc::channel, mpsc::Sender, Arc, Mutex},
    task::{Context, Poll},
    thread,
};

pub const MAX_TRANSMIT_SIZE: usize = 2000000;

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
    pub command_sender: Sender<Command>,
    pub mining_sender: Sender<Command>,
    pub last_block: Option<Block>,
    pub block_sender: Sender<Block>,
    pub state_sender: Sender<Command>,
    pub ballot_box: Arc<Mutex<BallotBox>>,
    pub wallet: Arc<Mutex<WalletAccount>>,
    pub state_chunks: HashMap<u32, Vec<u8>>,
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
        let mut rng = rand::thread_rng();
        let log_file_suffix = rng.gen::<u8>();
        let log_file_path = format!("./data/vrrb_log_file_{}.log", log_file_suffix);
        let _ = WriteLogger::init(
            LevelFilter::Info,
            Config::default(),
            File::create(log_file_path).unwrap(),
        );

        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        let (message_sender, message_receiver) = channel();
        let (command_sender, command_receiver) = channel();
        let (block_sender, block_receiver) = channel();
        let (mining_sender, mining_receiver) = channel();
        let (state_sender, state_receiver) = channel();

        let swarm =
            config_utils::configure_swarm(message_sender.clone(), command_sender.clone()).await;

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
            command_sender: command_sender.clone(),
            mining_sender: mining_sender.clone(),
            block_sender: block_sender.clone(),
            state_sender: state_sender.clone(),
            account_state: Arc::clone(&account_state),
            network_state: Arc::clone(&network_state),
            reward_state: Arc::clone(&reward_state),
            wallet: Arc::clone(&wallet),
            state_chunks: HashMap::new(),
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

        let pubkey = atomic_node
            .lock()
            .unwrap()
            .wallet
            .lock()
            .unwrap()
            .pubkey
            .clone()
            .to_string();
        let addresses = atomic_node
            .lock()
            .unwrap()
            .wallet
            .lock()
            .unwrap()
            .addresses
            .clone();
        addresses.iter().for_each(|(_, addr)| {
            atomic_node
                .lock()
                .unwrap()
                .account_state
                .lock()
                .unwrap()
                .accounts_pk
                .insert(addr.to_string(), pubkey.clone());
        });

        let n_claims_owned = atomic_node
            .lock()
            .unwrap()
            .wallet
            .lock()
            .unwrap()
            .claims
            .len()
            .clone() as u128;
        atomic_node
            .lock()
            .unwrap()
            .account_state
            .lock()
            .unwrap()
            .claim_counter
            .insert(pubkey, n_claims_owned);

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

        let task_node = Arc::clone(&atomic_node);
        thread::spawn(move || loop {
            while let Some(message) = message_receiver.iter().next() {
                let cloned_node = Arc::clone(&task_node);
                thread::spawn(move || {
                    message::process_message(message, Arc::clone(&cloned_node));
                })
                .join()
                .unwrap();
            }
        });

        let task_node = Arc::clone(&atomic_node);
        thread::spawn(move || loop {
            while let Some(command) = command_receiver.iter().next() {
                let cloned_node = Arc::clone(&task_node);
                thread::spawn(move || {
                    command_utils::handle_command(Arc::clone(&cloned_node), command);
                })
                .join()
                .unwrap()
            }
        });

        let mut temp_blocks: LinkedHashMap<String, Block> = LinkedHashMap::new();
        let mut mining = false;
        let mut updating_state = false;
        let task_node = Arc::clone(&atomic_node);
        thread::spawn(move || loop {
            let cloned_node = Arc::clone(&task_node);

            if let Some(command) = mining_receiver.try_iter().next() {
                match command {
                    Command::MineBlock => {
                        mining = true;
                        info!(target: "starting_mining", "This node has started mining: {}", mining);
                    }
                    Command::StopMine => {
                        mining = false;
                        info!(target: "stopped_mining", "This node has stopped mining: {}", mining);
                    }
                    _ => {
                        info!(target: "mining", "Invalid command.")
                    }
                }
            }

            if let Some(command) = state_receiver.try_iter().next() {
                // send message to state updating thread to start updating state
                // when it is done updating the state send message back to this
                // thread to stop updating state.
                match command {
                    Command::GetState => {
                        info!(target: "get_state", "getting state");
                        updating_state = true;
                    }
                    Command::StateUpdateCompleted => {
                        info!(target: "get_state", "completed updating state");
                        updating_state = false;
                    }
                    Command::StoreStateChunk(chunk, chunk_number, total_chunks) => {
                        if updating_state {
                            info!(target: "get_state", "received state chunk");
                            info!(target: "get_state", "received chunk {} of {}", &chunk_number, &total_chunks);
                            let state_chunks_length =
                                cloned_node.lock().unwrap().state_chunks.clone().len();
                            if chunk_number == total_chunks
                                && state_chunks_length as u32 == total_chunks - 1
                            {
                                cloned_node
                                    .lock()
                                    .unwrap()
                                    .state_chunks
                                    .entry(chunk_number).or_insert(chunk);

                                let state_chunks = cloned_node.lock().unwrap().state_chunks.clone();
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
                                cloned_node.lock().unwrap().last_block =
                                    network_state.last_block.clone();
                                command_utils::handle_command(
                                    Arc::clone(&cloned_node),
                                    Command::ProcessBacklog,
                                );
                                info!(target: "last_block", "last block: {}", cloned_node.lock().unwrap().last_block.clone().unwrap().block_height);
                            } else {
                                info!(target: "get_state", "stashed chunk: {} of {}", &chunk_number, &total_chunks);
                                cloned_node
                                    .lock()
                                    .unwrap()
                                    .state_chunks
                                    .insert(chunk_number, chunk);
                            }
                        }
                    }
                    Command::ProcessBacklog => {
                        let task_node = Arc::clone(&cloned_node);
                        'backlog_processing: loop {
                            let inner_node = Arc::clone(&task_node);
                            let last_block = cloned_node.lock().unwrap().last_block.clone();
                            if let Some((last_block_hash, block)) = temp_blocks.pop_front() {
                                if last_block_hash != last_block.unwrap().block_hash {
                                    temp_blocks.to_back(&last_block_hash);
                                } else {                                
                                    info!(target: "state_update", "processing block {}", &block.block_height);
                                    message_utils::process_block(block, Arc::clone(&inner_node));
                                }
                            } else {
                                break 'backlog_processing;
                            }
                        }
                        info!(
                            "Finished processing backlog, last block: {}",
                            cloned_node
                                .lock()
                                .unwrap()
                                .last_block
                                .clone()
                                .unwrap()
                                .block_height
                        );
                        command_utils::handle_command(
                            Arc::clone(&cloned_node),
                            Command::StateUpdateCompleted,
                        );
                    }
                    _ => {
                        info!(target: "get_state", "invalid command sent to state receiver");
                    }
                }
            }

            if let Some(block) = block_receiver.try_iter().next() {
                temp_blocks.insert(block.clone().last_block_hash, block.clone());
                info!(target: "stashed_block", "Stashed block: {}", block.block_height);
            }

            if !updating_state {
                'block_processing: loop {
                    let cloned_node = Arc::clone(&task_node);
                    let last_block = cloned_node.lock().unwrap().last_block.clone();
                    if let Some((last_block_hash, block)) = temp_blocks.pop_front() {
                        if &block.block_height > &0 {
                            if let None = last_block {
                                temp_blocks.insert(block.clone().last_block_hash, block.clone());
                                message_utils::request_state(Arc::clone(&cloned_node), block.clone());
                                break 'block_processing;
                            } else {
                                info!(
                                    target: "Block", "Block: {}, block_claim_current_owner: {:?}",
                                    &block.block_height, &block.claim.current_owner
                                );
                                if last_block_hash != last_block.unwrap().block_hash {
                                    temp_blocks.to_back(&last_block_hash);
                                } else {
                                    message_utils::process_block(
                                        block.clone(),
                                        Arc::clone(&cloned_node),
                                    );
                                }
                            }
                        } else {
                            message_utils::process_block(block.clone(), Arc::clone(&cloned_node));
                        }
                    } else {
                        break 'block_processing;
                    }
                }
            }

            if mining && !updating_state {
                let inner_node = Arc::clone(&cloned_node);
                message_utils::mine_block(Arc::clone(&inner_node));
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
