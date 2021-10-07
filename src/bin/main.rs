use hex;
use libp2p::gossipsub::IdentTopic as Topic;
use libp2p::multiaddr::multiaddr;
use libp2p::Multiaddr;
use log::info;
use rand::Rng;
use ritelinked::LinkedHashMap;
use sha256::digest_bytes;
use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::File;
use std::thread;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;
use vrrb_lib::block::Block;
use vrrb_lib::blockchain::{Blockchain, InvalidBlockErrorReason, StateComponent};
use vrrb_lib::handler::{CommandHandler, MessageHandler};
use vrrb_lib::miner::Miner;
use vrrb_lib::network::chunkable::Chunkable;
use vrrb_lib::network::command_utils::Command;
use vrrb_lib::network::config_utils;
use vrrb_lib::network::message_types::MessageType;
use vrrb_lib::network::node::{Node, NodeAuth};
use vrrb_lib::reward::Category;
use vrrb_lib::reward::RewardState;
use vrrb_lib::state::Components;
use vrrb_lib::state::Ledger;
use vrrb_lib::state::NetworkState;
use vrrb_lib::wallet::WalletAccount;

const VALIDATOR_THRESHOLD: f64 = 0.60;
pub const NANO: u128 = 1;
pub const MICRO: u128 = NANO * 1000;
pub const MILLI: u128 = MICRO * 1000;
pub const SECOND: u128 = MILLI * 1000;

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //____________________________________________________________________________________________________
    // Setup log file and db files
    let mut rng = rand::thread_rng();
    let node_type = NodeAuth::Full;
    let log_file_suffix: u8 = rng.gen();
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
    //____________________________________________________________________________________________________

    // ___________________________________________________________________________________________________
    // setup message and command sender/receiver channels for communication betwen various threads
    let (to_blockchain_sender, mut to_blockchain_receiver) = mpsc::unbounded_channel();
    let (to_miner_sender, mut to_miner_receiver) = mpsc::unbounded_channel();
    let (to_message_sender, to_message_receiver) = mpsc::unbounded_channel();
    let (from_message_sender, from_message_receiver) = mpsc::unbounded_channel();
    let (command_sender, command_receiver) = mpsc::unbounded_channel();
    let (to_swarm_sender, mut to_swarm_receiver) = mpsc::unbounded_channel();
    let (to_state_sender, mut to_state_receiver) = mpsc::unbounded_channel();
    //____________________________________________________________________________________________________

    let wallet = if let Some(secret_key) = std::env::args().nth(4) {
        WalletAccount::restore_from_private_key(secret_key)
    } else {
        WalletAccount::new()
    };

    let mut rng = rand::thread_rng();
    let file_suffix: u32 = rng.gen();
    let path = if let Some(path) = std::env::args().nth(2) {
        path
    } else {
        format!("./data/vrrb/test_{}.db", file_suffix)
    };

    let network_state = NetworkState::restore(&path);
    let reward_state = RewardState::start();

    //____________________________________________________________________________________________________
    // Node initialization
    let to_message_handler = MessageHandler::new(from_message_sender.clone(), to_message_receiver);
    let from_message_handler =
        MessageHandler::new(to_message_sender.clone(), from_message_receiver);
    let command_handler = CommandHandler::new(
        to_miner_sender.clone(),
        to_blockchain_sender.clone(),
        to_swarm_sender.clone(),
        to_state_sender.clone(),
        command_receiver,
    );

    let mut node = Node::new(node_type.clone(), command_handler, to_message_handler);
    let node_id = node.id.clone();
    let node_key = node.key.clone();
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Swarm initialization
    let mut swarm = config_utils::configure_swarm(
        from_message_handler.sender.clone(),
        command_sender.clone(),
        node_id.clone(),
        node_key.clone(),
        wallet.pubkey.clone().to_string(),
        wallet.clone().get_address(1),
    )
    .await;

    let port = rand::thread_rng().gen_range(9292, 19292);
    let addr: Multiaddr = multiaddr!(Ip4([0, 0, 0, 0]), Tcp(port as u16));
    println!("{:?}", &addr);

    swarm.listen_on(addr.clone()).unwrap();
    swarm
        .behaviour_mut()
        .kademlia
        .add_address(&node.id.clone(), addr.clone());
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Dial peer if provided
    if let Some(to_dial) = std::env::args().nth(1) {
        let dialing = to_dial.clone();
        match to_dial.parse() {
            Ok(to_dial) => match swarm.dial_addr(to_dial) {
                Ok(_) => {
                    println!("Dialed {:?}", dialing);
                }
                Err(e) => println!("Dial {:?} failed: {:?}", dialing, e), //
            },
            Err(err) => println!("Failed to parse address to dial {:?}", err), //
        }
    }
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Swarm event thread
    tokio::task::spawn(async move {
        loop {
            let evt = {
                tokio::select! {
                    event = swarm.next() => {
                        info!("Unhandled Swarm Event: {:?}", event);
                        None
                    },
                    command = to_swarm_receiver.recv() => {
                        if let Some(command) = command {
                            match command {
                                Command::SendMessage(message) => {
                                    Some(message)
                                }
                                _ => {None}
                            }
                        } else {
                            None
                        }
                    }
                }
            };

            if let Some(message) = evt {
                let encoded = hex::encode(message);
                if let Err(e) = swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(Topic::new("test-net"), encoded)
                {
                    info!("Error sending to network: {:?}", e);
                };
            }
        }
    });
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Node thread
    tokio::task::spawn(async move {
        if let Err(_) = node.start().await {
            panic!("Unable to start node!")
        };
    });
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Blockchain thread
    let mut blockchain_network_state = network_state.clone();
    let mut blockchain_reward_state = reward_state.clone();
    let blockchain_to_miner_sender = to_miner_sender.clone();
    let blockchain_to_swarm_sender = to_swarm_sender.clone();
    let blockchain_to_blockchain_sender = to_blockchain_sender.clone();
    let blockchain_to_state_sender = to_state_sender.clone();
    thread::spawn(move || {
        let mut rng = rand::thread_rng();
        let file_suffix: u32 = rng.gen();
        let mut blockchain = Blockchain::new(&format!("./data/vrrb/test_{}.db", file_suffix));
        loop {
            let miner_sender = blockchain_to_miner_sender.clone();
            let swarm_sender = blockchain_to_swarm_sender.clone();
            let state_sender = blockchain_to_state_sender.clone();
            let blockchain_sender = blockchain_to_blockchain_sender.clone();
            // let blockchain_sender = blockchain_to_blockchain_sender.clone();
            if let Ok(command) = to_blockchain_receiver.try_recv() {
                match command {
                    Command::PendingBlock(block, sender_id) => {
                        if blockchain.updating_state {
                            blockchain
                                .future_blocks
                                .insert(block.clone().header.last_hash, block.clone());
                        } else {
                            if let Err(e) = blockchain.process_block(
                                &blockchain_network_state,
                                &blockchain_reward_state,
                                &block,
                            ) {
                                match e.details {
                                    InvalidBlockErrorReason::BlockOutOfSequence => {
                                        // Stash block in blockchain.future_blocks
                                        // Request state update once. Set "updating_state" field
                                        // in blockchain to true, so that it doesn't request it on
                                        // receipt of new future blocks which will also be invalid.
                                        if !blockchain.updating_state {
                                            // send state request and set blockchain.updating state to true;
                                            println!("Error: {:?}", e);
                                            if let Some((_, v)) = blockchain.future_blocks.front() {
                                                let message = MessageType::GetNetworkStateMessage {
                                                    sender_id: node_id.clone().to_string(),
                                                    requested_from: sender_id,
                                                    requestor_node_type: node_type.clone(),
                                                    lowest_block: v.header.block_height,
                                                    component: StateComponent::All,
                                                };

                                                if let Err(e) = swarm_sender
                                                    .send(Command::SendMessage(message.as_bytes()))
                                                {
                                                    println!("Error sending state update request to swarm sender: {:?}", e);
                                                };

                                                blockchain.updating_state = true;
                                            }
                                        }
                                    }
                                    InvalidBlockErrorReason::InvalidBlockHeight => {
                                        // request missing blocks if it's higher than yours
                                        // inform miner you have longer chain if it's lower than yours
                                        // so that they can request missing blocks.
                                    }
                                    InvalidBlockErrorReason::InvalidBlockNonce => {}
                                    InvalidBlockErrorReason::InvalidBlockReward => {}
                                    InvalidBlockErrorReason::InvalidLastHash => {}
                                    InvalidBlockErrorReason::InvalidStateHash => {}
                                    InvalidBlockErrorReason::InvalidClaim => {}
                                    InvalidBlockErrorReason::InvalidTxns => {}
                                    InvalidBlockErrorReason::General => {}
                                }

                                if let Err(_) =
                                    miner_sender.send(Command::InvalidBlock(block.clone()))
                                {
                                    println!("Error sending command to receiver");
                                };
                            } else {
                                blockchain_network_state.dump(&block);
                                if let Err(_) =
                                    miner_sender.send(Command::ConfirmedBlock(block.clone()))
                                {
                                    println!("Error sending command to receiver");
                                }

                                if let Err(_) = miner_sender.send(Command::StateUpdateCompleted(
                                    blockchain_network_state.clone(),
                                )) {
                                    println!(
                                        "Error sending state update completed command to receiver"
                                    );
                                }
                            }
                        }
                    }
                    Command::GetStateComponents(requestor, components) => match components {
                        StateComponent::All => {
                            let genesis_bytes = if let Some(genesis) = blockchain.clone().genesis {
                                Some(genesis.clone().as_bytes())
                            } else {
                                None
                            };
                            let child_bytes = if let Some(block) = blockchain.clone().child {
                                Some(block.clone().as_bytes())
                            } else {
                                None
                            };
                            let parent_bytes = if let Some(block) = blockchain.clone().parent {
                                Some(block.clone().as_bytes())
                            } else {
                                None
                            };
                            let current_ledger =
                                Some(blockchain_network_state.clone().db_to_ledger().as_bytes());
                            let current_network_state =
                                Some(blockchain_network_state.clone().as_bytes());
                            let components = Components {
                                genesis: genesis_bytes,
                                child: child_bytes,
                                parent: parent_bytes,
                                blockchain: None,
                                ledger: current_ledger,
                                network_state: current_network_state,
                                archive: None,
                            };

                            if let Err(e) = state_sender
                                .send(Command::RequestedComponents(requestor, components))
                            {
                                println!(
                                    "Error sending requested components to state receiver: {:?}",
                                    e
                                );
                            }
                        }
                        _ => {}
                    },
                    Command::StateUpdateComponents(components) => {
                        if let Some(bytes) = components.genesis {
                            blockchain.genesis = Some(Block::from_bytes(&bytes))
                        }

                        if let Some(bytes) = components.child {
                            blockchain.child = Some(Block::from_bytes(&bytes))
                        }
                        if let Some(bytes) = components.parent {
                            blockchain.parent = Some(Block::from_bytes(&bytes))
                        }
                        if let Some(bytes) = components.blockchain {
                            let mut new_blockchain = Blockchain::from_bytes(&bytes);
                            new_blockchain.future_blocks = blockchain.clone().future_blocks;
                            new_blockchain.chain_db = blockchain.clone().chain_db;
                            blockchain = new_blockchain;
                        }
                        if let Some(bytes) = components.network_state {
                            let mut new_network_state = NetworkState::from_bytes(&bytes);
                            new_network_state.path = blockchain_network_state.path;
                            blockchain_reward_state = new_network_state.reward_state;
                            blockchain_network_state = new_network_state;
                        }

                        if let Some(bytes) = components.ledger {
                            let new_ledger = Ledger::from_bytes(&bytes);
                            blockchain_network_state
                                .update_ledger(new_ledger, blockchain_reward_state);
                        }

                        if let Some(bytes) = components.archive {
                            let mut new_db = blockchain.chain_db_from_bytes(&bytes);
                            if let Err(e) = new_db.dump() {
                                println!("Error dumping db update: {:?}", e);
                            }
                        }

                        if let Err(e) = blockchain_sender.send(Command::ProcessBacklog) {
                            println!("Error sending process backlog command to blockchain receiver: {:?}", e);
                        }
                    }
                    Command::ProcessBacklog => {
                        let last_block = blockchain.clone().child.unwrap();
                        while let Some((_, block)) = blockchain.future_blocks.pop_front() {
                            if last_block.header.block_height == block.header.block_height {
                                println!("Block already processed, skipping")
                            } else {
                                if let Err(e) = blockchain.process_block(
                                    &blockchain_network_state,
                                    &blockchain_reward_state,
                                    &block,
                                ) {
                                    println!(
                                        "Error trying to process backlogged future blocks: {:?}",
                                        e
                                    );
                                } else {
                                    blockchain_network_state.dump(&block);
                                    if let Err(e) =
                                        miner_sender.send(Command::ConfirmedBlock(block.clone()))
                                    {
                                        println!(
                                            "Error sending confirmed backlog block to miner: {:?}",
                                            e
                                        );
                                    }
                                }
                            }
                        }
                        println!("Backlog processed");
                        if let Err(e) = miner_sender.send(Command::StateUpdateCompleted(
                            blockchain_network_state.clone(),
                        )) {
                            println!("Error sending updated network state to miner: {:?}", e);
                        }
                        blockchain.updating_state = false;
                    }
                    Command::StateUpdateCompleted(network_state) => {
                        blockchain_network_state = network_state.clone();
                    }
                    Command::ClaimAbandoned(_, claim) => {
                        blockchain_network_state.abandoned_claim(claim.hash);
                        if let Err(e) = miner_sender.send(Command::StateUpdateCompleted(
                            blockchain_network_state.clone(),
                        )) {
                            println!("Error sending updated network state to miner: {:?}", e);
                        }
                    }
                    Command::GetHeight => {
                        println!("Blockchain Height: {}", blockchain.chain.len());
                    }
                    _ => {}
                }
            }
        }
    });
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Mining thread
    let mut mining_wallet = wallet.clone();
    let miner_network_state = network_state.clone();
    let miner_reward_state = reward_state.clone();
    let miner_to_miner_sender = to_miner_sender.clone();
    let miner_to_blockchain_sender = to_blockchain_sender.clone();
    let miner_to_swarm_sender = to_swarm_sender.clone();
    thread::spawn(move || {
        let mut miner = Miner::start(
            mining_wallet.clone().pubkey,
            mining_wallet.clone().get_address(1),
            miner_reward_state,
            miner_network_state,
            0,
        );
        loop {
            let blockchain_sender = miner_to_blockchain_sender.clone();
            let swarm_sender = miner_to_swarm_sender.clone();
            let miner_sender = miner_to_miner_sender.clone();
            if let Ok(command) = to_miner_receiver.try_recv() {
                match command {
                    Command::SendMessage(message) => {
                        if let Err(e) = swarm_sender.send(Command::SendMessage(message)) {
                            println!("Error sending to swarm receiver: {:?}", e);
                        }
                    }
                    Command::MineBlock => {
                        miner.mining = true;
                        if let Some(last_block) = miner.last_block.clone() {
                            if let Some(claim) =
                                miner.clone().claim_map.get(&miner.clone().claim.pubkey)
                            {
                                let lowest_pointer = miner
                                    .get_lowest_pointer(last_block.header.next_block_nonce as u128);
                                if let Some((hash, _)) = lowest_pointer.clone() {
                                    if hash == claim.hash.clone() {
                                        let block = miner.mine();
                                        if let Some(block) = block {
                                            let message = MessageType::BlockMessage {
                                                block: block.clone(),
                                                sender_id: node_id.clone().to_string(),
                                            };

                                            if let Err(e) = swarm_sender
                                                .send(Command::SendMessage(message.as_bytes()))
                                            {
                                                println!("Error sending SendMessage command to swarm: {:?}", e);
                                            }

                                            if let Err(_) =
                                                blockchain_sender.send(Command::PendingBlock(
                                                    block.clone(),
                                                    node_id.clone().to_string(),
                                                ))
                                            {
                                                println!("Error sending PendingBlock command to blockchain");
                                            }
                                        } else {
                                            if let Err(e) = miner_sender.send(Command::MineBlock) {
                                                println!(
                                                    "Error sending miner sender MineBlock: {:?}",
                                                    e
                                                );
                                            }
                                        }
                                    } else {
                                        if miner.check_time_elapsed() > 30 {
                                            miner.current_nonce_timer = miner.get_timestamp();
                                            let mut abandoned_claim_map = miner.claim_map.clone();
                                            abandoned_claim_map.retain(|_, v| v.hash == hash);

                                            if let Some((_, v)) = abandoned_claim_map.front() {
                                                let message = MessageType::ClaimAbandonedMessage {
                                                    claim: v.clone(),
                                                    sender_id: miner.claim.pubkey.clone(),
                                                };

                                                miner
                                                    .abandoned_claim_counter
                                                    .insert(miner.claim.pubkey.clone(), v.clone());
                                                if let Err(e) = swarm_sender
                                                    .send(Command::SendMessage(message.as_bytes()))
                                                {
                                                    println!("Error sending ClaimAbandoned message to swarm: {:?}", e);
                                                }

                                                let mut abandoned_claim_map =
                                                    miner.abandoned_claim_counter.clone();
                                                abandoned_claim_map
                                                    .retain(|_, claim| v.hash == claim.hash);

                                                if abandoned_claim_map.len() as f64
                                                    / (miner.claim_map.len() as f64 - 1.0)
                                                    > VALIDATOR_THRESHOLD
                                                {
                                                    miner.claim_map.retain(|_, v| v.hash != hash);
                                                    if let Err(e) = blockchain_sender.send(
                                                        Command::ClaimAbandoned(
                                                            miner.claim.pubkey.clone(),
                                                            v.clone(),
                                                        ),
                                                    ) {
                                                        println!("Error forwarding confirmed abandoned claim to blockchain: {:?}", e);
                                                    }
                                                }
                                            }
                                        }
                                        if let Err(e) = miner_sender.send(Command::MineBlock) {
                                            println!(
                                                "Error sending miner sender MineBlock: {:?}",
                                                e
                                            );
                                        }
                                    }
                                } else {
                                    if let Err(e) = miner_sender.send(Command::NonceUp) {
                                        println!("Error sending NonceUp command to miner: {:?}", e);
                                    }
                                }
                            }
                        } else {
                            if let Err(e) = miner_sender.send(Command::MineGenesis) {
                                println!("Error sending mine genesis command to miner: {:?}", e);
                            };
                        }
                    }
                    Command::ConfirmedBlock(block) => {
                        miner.current_nonce_timer = block.header.timestamp;
                        let claim_map_hash = digest_bytes(
                            &serde_json::to_string(&miner.claim_map).unwrap().as_bytes(),
                        );
                        if let Some(hash) = &block.header.claim_map_hash {
                            if hash != &claim_map_hash {
                                println!("Different claim states");
                                println!("Claim Map: {:?}", miner.claim_map);
                            }
                        }
                        if let Category::Motherlode(_) = block.header.block_reward.category {
                            println!("*****{:?}*****\n", &block.header.block_reward.category);
                        }
                        miner.last_block = Some(block.clone());
                        block.txns.iter().for_each(|(k, _)| {
                            miner.txn_pool.confirmed.remove(&k.clone());
                        });
                        let mut new_claims = block.claims.clone();
                        new_claims = new_claims
                            .iter()
                            .map(|(k, v)| {
                                return (k.clone(), v.clone());
                            })
                            .collect();
                        new_claims.iter().for_each(|(k, v)| {
                            miner.claim_pool.confirmed.remove(k);
                            miner.claim_map.insert(k.clone(), v.clone());
                        });

                        // Check if the miner's claim nonce changed,
                        // if it did change, make sure that it HAD to change.
                        // If it did have to change (nonce up) and your local claim map is different
                        // nonce up the local claim map until it is in consensus.
                        miner.claim_map.replace(
                            block.header.claim.clone().pubkey,
                            block.header.claim.clone(),
                        );
                    }
                    Command::ProcessTxn(txn) => {
                        let txn_validator = miner.process_txn(txn.clone());
                        miner.check_confirmed(txn.txn_id.clone());
                        let message = MessageType::TxnValidatorMessage {
                            txn_validator,
                            sender_id: node_id.to_string().clone(),
                        };
                        if let Err(e) = miner_sender.send(Command::SendMessage(message.as_bytes()))
                        {
                            println!("Error sending SendMessage command to swarm: {:?}", e);
                        }
                    }
                    Command::ProcessClaim(claim) => {
                        miner
                            .claim_pool
                            .confirmed
                            .insert(claim.pubkey.clone(), claim.clone());
                    }
                    Command::ProcessTxnValidator(validator) => {
                        miner.process_txn_validator(validator.clone());
                        miner.check_confirmed(validator.txn.txn_id.clone());
                    }
                    Command::InvalidBlock(_) => {}
                    Command::StateUpdateCompleted(network_state) => {
                        miner.network_state = network_state.clone();
                        miner.claim_map = miner.network_state.get_claims();
                        if miner.mining {
                            if let Err(e) = miner_sender.send(Command::MineBlock) {
                                println!("Error sending MineBlock command to miner: {:?}", e);
                            }
                        }
                    }
                    Command::MineGenesis => {
                        if let Some(block) = miner.genesis() {
                            miner.last_block = Some(block.clone());
                            let message = MessageType::BlockMessage {
                                block: block.clone(),
                                sender_id: node_id.to_string().clone(),
                            };

                            if let Err(e) =
                                miner_sender.send(Command::SendMessage(message.as_bytes()))
                            {
                                println!("Error sending SendMessage command to swarm: {:?}", e);
                            }
                            if let Err(_) = blockchain_sender.send(Command::PendingBlock(
                                block.clone(),
                                node_id.clone().to_string(),
                            )) {
                                println!("Error sending to command receiver")
                            }
                        }
                    }
                    Command::SendAddress => {
                        let message = MessageType::ClaimMessage {
                            claim: miner.claim.clone(),
                            sender_id: node_id.clone().to_string(),
                        };

                        if let Err(e) = miner_sender.send(Command::SendMessage(message.as_bytes()))
                        {
                            println!("Error sending SendMessage command to swarm: {:?}", e);
                        }
                    }
                    Command::NonceUp => {
                        println!("*********NONCE UP*********");
                        miner.nonce_up();
                        println!("Claim Map: {:?}", miner.claim_map);
                        if let Err(e) = miner_sender.send(Command::MineBlock) {
                            println!("Error sending MineBlock command to miner: {:?}", e);
                        }
                    }
                    Command::ClaimAbandoned(pubkey, claim) => {
                        if let Some(_) = miner.claim_map.get(&pubkey) {
                            miner
                                .abandoned_claim_counter
                                .insert(pubkey.clone(), claim.clone());
                            let mut abandoned_claim_map = miner.abandoned_claim_counter.clone();
                            abandoned_claim_map.retain(|_, v| v.hash == claim.hash);

                            if abandoned_claim_map.len() as f64
                                / (miner.claim_map.len() as f64 - 1.0)
                                > VALIDATOR_THRESHOLD
                            {
                                miner.claim_map.retain(|_, v| v.hash != claim.hash);
                                if let Err(e) =
                                    blockchain_sender.send(Command::ClaimAbandoned(pubkey, claim))
                                {
                                    println!("Error forwarding confirmed abandoned claim to blockchain: {:?}", e);
                                }
                            }
                        }
                    }
                    Command::GetBalance(address_number) => {
                        println!(
                            "Balance: {}",
                            miner
                                .network_state
                                .get_balance(&mining_wallet.get_address(address_number))
                        )
                    }
                    _ => {}
                }
            }
        }
    });
    //____________________________________________________________________________________________________
    // State Sending Thread
    //____________________________________________________________________________________________________
    let state_to_swarm_sender = to_swarm_sender.clone();
    let state_to_blockchain_sender = to_blockchain_sender.clone();
    let mut state_chunk_cache = LinkedHashMap::new();
    thread::spawn(move || loop {
        let blockchain_sender = state_to_blockchain_sender.clone();
        let swarm_sender = state_to_swarm_sender.clone();
        if let Ok(command) = to_state_receiver.try_recv() {
            match command {
                Command::SendStateComponents(requestor, components) => {
                    if let Err(e) =
                        blockchain_sender.send(Command::GetStateComponents(requestor, components))
                    {
                        println!(
                            "Error sending GetStateComponents Command to blockchain: {:?}",
                            e
                        );
                    }
                }
                Command::RequestedComponents(requestor, components) => {
                    println!("Sending state components");
                    if let Some(chunks) = components.chunk() {
                        let mut idx = 0;
                        let total_chunks = chunks.clone().len() as u32;
                        for chunk in chunks {
                            let message = MessageType::StateComponentChunkMessage {
                                data: chunk,
                                chunk_number: idx + 1,
                                total_chunks,
                                requestor: requestor.clone(),
                                sender_id: node_id.clone().to_string(),
                            };

                            idx += 1;

                            if let Err(e) =
                                swarm_sender.send(Command::SendMessage(message.as_bytes()))
                            {
                                println!("Error sending to swarm sender: {:?}", e);
                            }
                        }
                    }
                }
                Command::StoreStateComponentChunk(data, chunk_number, total_chunks) => {
                    if chunk_number == total_chunks {
                        state_chunk_cache.insert(chunk_number, data);
                        let mut component_bytes = vec![];
                        state_chunk_cache.iter().for_each(|(_, v)| {
                            component_bytes.extend(v);
                        });

                        let components = Components::from_bytes(&component_bytes);
                        if let Err(e) =
                            blockchain_sender.send(Command::StateUpdateComponents(components))
                        {
                            println!(
                                "Error sending state update componetns to blockchain thread: {:?}",
                                e
                            );
                        }
                    } else {
                        state_chunk_cache.insert(chunk_number, data);
                    }
                }
                Command::ConfirmedBlock(_) => {
                    // Dump block to block archive.
                }
                _ => {}
            }
        }
    });

    //____________________________________________________________________________________________________
    // Terminal Interface loop
    let terminal_to_swarm_sender = to_swarm_sender.clone();
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    loop {
        let swarm_sender = terminal_to_swarm_sender.clone();
        let evt = {
            tokio::select! {
                // await an input from the user
                line = stdin.next_line() => Some(
                    line.expect("can get line").expect("can read line from stdin")
                )
            }
        };
        if let Some(line) = evt {
            if line == "QUIT" {
                // Clean up and inform the network that you are no longer mining so that
                // claim lowest pointers will be properly calculated.
                break;
            }
            // If there is some input from the user, attemmpt to convert the input to
            // a command and send to the command handler.
            if let Some(command) = Command::from_str(&line) {
                match command.clone() {
                    Command::SendTxn(addr_num, receiver, amount) => {
                        let txn = wallet.clone().send_txn(addr_num, receiver, amount);
                        if let Ok(txn) = txn {
                            let message = MessageType::TxnMessage {
                                txn,
                                sender_id: node_id.to_string().clone(),
                            };
                            if let Err(e) =
                                swarm_sender.send(Command::SendMessage(message.as_bytes()))
                            {
                                println!("Error sending to command receiver: {:?}", e);
                            };
                        }
                    }
                    _ => {
                        if let Err(_) = command_sender.send(command) {
                            println!("Error sending command to command receiver");
                        };
                    }
                }
            }
        }
    }
    //____________________________________________________________________________________________________

    Ok(())
}
