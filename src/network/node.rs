#[allow(unused_imports)]
use crate::account::AccountState;
use crate::block::Block;
use crate::claim::Claim;
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
use serde::{Deserialize, Serialize};
use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::File;
use std::{
    error::Error,
    sync::{mpsc::channel, mpsc::Sender, Arc, Mutex},
    task::{Context, Poll},
    thread,
};

pub const MAX_TRANSMIT_SIZE: usize = 2000000;

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NodeAuth {
    // Builds a full block archive all blocks and all claims
    Archive,
    // Builds a Block Header archive and stores all claims
    Full,
    // Builds a Block Header and Claim Header archive. Maintains claims owned by this node. Can mine blocks and validate transactions
    // cannot validate claim exchanges.
    Light,
    // Stores last block header and all claim headers
    UltraLight,
    //TODO: Add a key field for the bootstrap node, sha256 hash of key in bootstrap node must == a bootstrap node key.
    Bootstrap,
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
    pub state_chunks: LinkedHashMap<u32, Vec<u8>>,
}

impl Node {
    pub fn get_id(&self) -> PeerId {
        self.id
    }

    pub fn get_node_type(&self) -> NodeAuth {
        self.node_type.clone()
    }

    pub fn get_network_state(&self) -> NetworkState {
        self.network_state.lock().unwrap().clone()
    }

    pub fn get_account_state(&self) -> AccountState {
        self.account_state.lock().unwrap().clone()
    }

    pub fn get_reward_state(&self) -> RewardState {
        self.reward_state.lock().unwrap().clone()
    }

    pub fn get_last_block(&self) -> Option<Block> {
        self.last_block.clone()
    }

    pub fn get_ballot_box(&self) -> BallotBox {
        self.ballot_box.lock().unwrap().clone()
    }

    pub fn get_wallet(&self) -> WalletAccount {
        self.wallet.lock().unwrap().clone()
    }

    pub fn get_wallet_pubkey(&self) -> String {
        self.get_wallet().pubkey.clone()
    }

    pub fn get_wallet_address(&self, address_number: u32) -> Option<String> {
        if let Some(entry) = self.get_wallet().addresses.get(&address_number) {
            Some(entry.to_owned())
        } else {
            None
        }
    }

    pub fn get_wallet_owned_claims(&self) -> LinkedHashMap<u128, Claim> {
        self.get_wallet().get_claims()
    }

    pub fn get_wallet_balances(&self) -> LinkedHashMap<String, LinkedHashMap<String, u128>> {
        self.get_wallet().update_balances(self.get_network_state());
        self.get_wallet().render_balances()
    }

    pub fn get_wallet_address_balance(&self, address_number: u32) -> Option<u128> {
        self.get_wallet()
            .get_address_balance(self.get_network_state(), address_number)
    }

    pub fn remove_mined_claim_from_wallet(&mut self, block: &Block) {
        let mut wallet = self.get_wallet();
        wallet.remove_mined_claims(block);
        self.wallet = Arc::new(Mutex::new(wallet));
    }

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

        let log_file_path = if let Some(path) = std::env::args().nth(3) {
            path
        } else {
            std::fs::create_dir_all("./data/vrrb")?;
            format!("./data/vrrb/vrrb_log_file_{}.log", log_file_suffix)
        };
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
            state_chunks: LinkedHashMap::new(),
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

        let pubkey = atomic_node.lock().unwrap().get_wallet().get_pubkey();

        let addresses = atomic_node
            .lock()
            .unwrap()
            .get_wallet()
            .get_wallet_addresses();

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

        let n_claims_owned = atomic_node.lock().unwrap().get_wallet().n_claims_owned();
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

        #[allow(unused_variables, unused_mut)]
        let mut temp_blocks: LinkedHashMap<String, Block> = LinkedHashMap::new();
        let mut block_archive_chunks: LinkedHashMap<u128, LinkedHashMap<u32, Vec<u8>>> =
            LinkedHashMap::new();
        let mut mining = false;
        let mut updating_state = false;
        let mut processed_backlog = false;
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
                    Command::CheckStateUpdateStatus((block_height, block, last_block)) => {
                        info!(target: "checking_state_update_status", "Checking state update status -> block height: {}", &block_height);
                        if !processed_backlog {
                            if let Some((_, last_stashed_block)) = temp_blocks.front() {

                                println!("last_block block height: {}", &last_stashed_block.block_height);
                                println!("last block sent in state update: {}", last_block);
                                if last_block < last_stashed_block.block_height - 1 {
                                    println!("You are not receiving enough blocks, request the difference");
                                }
                                if block_height == last_stashed_block.block_height - 1 {
                                    message::process_confirmed_block(
                                        block.clone(),
                                        Arc::clone(&cloned_node),
                                    );
                                    println!("State chunks completed, process backlog");
                                    command_utils::handle_command(
                                        Arc::clone(&cloned_node),
                                        Command::ProcessBacklog,
                                    );
                                    processed_backlog = true;
                                } else if block_height > last_stashed_block.block_height - 1 {
                                    println!("State chunks completed, process backlog");
                                    command_utils::handle_command(
                                        Arc::clone(&cloned_node),
                                        Command::ProcessBacklog,
                                    );
                                    processed_backlog = true;
                                } else {
                                    message::process_confirmed_block(
                                        block.clone(),
                                        Arc::clone(&cloned_node),
                                    );
                                    println!("processed block {}", &block_height);
                                }
                            } else {
                                message::process_confirmed_block(
                                    block.clone(),
                                    Arc::clone(&cloned_node)
                                );
                                println!("processed block {}", &block_height);
                            } 
                        }
                    }
                    Command::StateUpdateCompleted => {
                        info!(target: "get_state", "completed updating state");
                        updating_state = false;
                    }
                    Command::StoreStateDbChunk(object, data, chunk_number, total_chunks, _last_block) => {
                        if let Some(entry) = block_archive_chunks.get_mut(&object.0) {
                            entry.entry(chunk_number).or_insert(data);
                            if chunk_number == total_chunks || entry.len() == total_chunks as usize
                            {
                                // This is the final chunk of the block reassemble and process it.
                                let mut block_bytes = vec![];
                                entry.iter().for_each(|(_, v)| {
                                    block_bytes.extend(v);
                                });
                                let block = Block::from_bytes(&block_bytes);
                                message::process_confirmed_block(
                                    block.clone(),
                                    Arc::clone(&cloned_node),
                                );
                                println!("processed block {}", &object.0);
                                if let None = cloned_node.lock().unwrap().last_block.clone() {
                                    cloned_node.lock().unwrap().last_block = Some(block.clone());
                                } else {
                                    if object.0
                                        > cloned_node
                                            .lock()
                                            .unwrap()
                                            .last_block
                                            .clone()
                                            .unwrap()
                                            .block_height
                                    {
                                        cloned_node.lock().unwrap().last_block =
                                            Some(block.clone());
                                    }
                                }
                            }
                        } else {
                            let mut new_block_map = LinkedHashMap::new();
                            new_block_map.insert(chunk_number, data);
                            block_archive_chunks.insert(object.clone().0, new_block_map);
                        }
                    }
                    Command::ProcessBacklog => {
                        let task_node = Arc::clone(&cloned_node);
                        'backlog_processing: loop {
                            let inner_node = Arc::clone(&task_node);
                            let last_block = cloned_node.lock().unwrap().last_block.clone();
                            if let Some(block) = temp_blocks.get(&last_block.clone().unwrap().block_hash) {
                                info!(target: "state_update", "processing block {}", &block.block_height);
                                message_utils::process_block(block.clone(), Arc::clone(&inner_node));                                
                            } else {
                                info!(target: "state_update", "cannot find next block");
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

                        let network_state = task_node
                            .lock()
                            .unwrap()
                            .network_state
                            .lock()
                            .unwrap()
                            .clone();
                        let mut wallet = task_node.lock().unwrap().wallet.lock().unwrap().clone();
                        wallet.update_balances(network_state);
                        println!("Processed Backlog Balances: {:?}", wallet.render_balances());
                        }
                    _ => {
                        info!(target: "get_state", "invalid command sent to state receiver");
                    }
                }
            }

            if let Some(block) = block_receiver.try_iter().next() {
                let local_last_block = cloned_node.lock().unwrap().last_block.clone();
                if let Some(last_block) = local_last_block {
                    if temp_blocks.is_empty() && block.last_block_hash == last_block.block_hash {
                        updating_state = false;
                        processed_backlog = true;
                    }
                }
                if let Some(entry) = temp_blocks.get_mut(&block.last_block_hash) {
                    if entry.claim.claim_number > block.claim.claim_number {
                        *entry = block.clone();
                    }
                } else {
                    temp_blocks.insert(block.clone().last_block_hash, block.clone());
                }
                info!(target: "stashed_block", "Stashed block: {}", block.block_hash);
            }

            if !updating_state {
                'block_processing: loop {
                    let cloned_node = Arc::clone(&task_node);
                    let local_last_block = cloned_node.lock().unwrap().last_block.clone();
                    if let None = local_last_block {
                        if let Some((_, block)) = temp_blocks.pop_front() {
                            if &block.block_height > &0 {
                                message_utils::request_state(
                                    Arc::clone(&cloned_node),
                                    block.clone(),
                                );
                                mining = false;
                                break 'block_processing;
                            } else {
                                message_utils::process_block(
                                    block.clone(),
                                    Arc::clone(&cloned_node),
                                );
                            }
                        } else {
                            break 'block_processing;
                        }
                    } else {
                        if let Some(block) = temp_blocks.get(&local_last_block.clone().unwrap().block_hash) {
                            message_utils::process_block(
                                block.clone(),
                                Arc::clone(&cloned_node),
                            );
                            temp_blocks.remove(&local_last_block.unwrap().clone().block_hash);
                        } else {
                            break 'block_processing;
                        }
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
