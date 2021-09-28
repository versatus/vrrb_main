use hex;
use libp2p::gossipsub::IdentTopic as Topic;
use libp2p::multiaddr::multiaddr;
use libp2p::Multiaddr;
use log::info;
use rand::Rng;
use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::File;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;
use vrrb_lib::blockchain::Blockchain;
use vrrb_lib::handler::{CommandHandler, MessageHandler};
use vrrb_lib::miner::Miner;
use vrrb_lib::network::command_utils::Command;
use vrrb_lib::network::config_utils;
use vrrb_lib::network::message_types::MessageType;
use vrrb_lib::network::node::{Node, NodeAuth};
use vrrb_lib::reward::RewardState;
use vrrb_lib::state::NetworkState;
use vrrb_lib::wallet::WalletAccount;

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
    let (to_wallet_sender, mut to_wallet_receiver) = mpsc::unbounded_channel();
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
        format!("test_{}.db", file_suffix)
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
        to_wallet_sender.clone(),
        command_receiver,
    );

    let mut node = Node::new(node_type, command_handler, to_message_handler);
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
                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(Topic::new("test-net"), encoded) {
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
    let blockchain_reward_state = reward_state.clone();
    let blockchain_to_miner_sender = to_miner_sender.clone();
    tokio::task::spawn(async move {
        let mut blockchain = Blockchain::new("test_chain_db.db");
        loop {
            let miner_sender = blockchain_to_miner_sender.clone();
            if let Ok(command) = to_blockchain_receiver.try_recv() {
                match command {
                    Command::PendingBlock(block) => {
                        if let Err(_) = blockchain.process_block(
                            &blockchain_network_state,
                            &blockchain_reward_state,
                            &block,
                        ) {
                            info!(target: "invalid_block", "Block invalid, look for next block");
                            if let Err(_) =
                                miner_sender.send(Command::InvalidBlock(block.clone()))
                            {
                                println!("Error sending command to receiver");
                            };
                        } else {
                            blockchain_network_state.dump(&block);
                            if let Err(_) = miner_sender
                                .send(Command::ConfirmedBlock(block.clone()))
                            {
                                println!("Error sending command to receiver");
                            }

                            if let Err(_) = miner_sender.send(
                                Command::StateUpdateCompleted(blockchain_network_state.clone()),
                            ) {
                                println!(
                                    "Error sending state update completed command to receiver"
                                );
                            }
                        }
                    }
                    Command::StateUpdateCompleted(network_state) => {
                        blockchain_network_state = network_state.clone();
                    }
                    _ => {}
                }
            }
        }
    });
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Mining thread
    let mining_wallet = wallet.clone();
    let miner_network_state = network_state.clone();
    let miner_reward_state = reward_state.clone();
    let miner_to_miner_sender = to_miner_sender.clone();
    let miner_to_blockchain_sender = to_blockchain_sender.clone();
    let miner_to_swarm_sender = to_swarm_sender.clone();
    tokio::task::spawn(async move {
        let mut miner = Miner::start(
            mining_wallet.clone().pubkey,
            mining_wallet.clone().get_address(1),
            miner_reward_state,
            miner_network_state,
        );
        loop {
            let miner_sender = miner_to_miner_sender.clone();
            let blockchain_sender = miner_to_blockchain_sender.clone();
            let swarm_sender = miner_to_swarm_sender.clone();
            if let Ok(command) = to_miner_receiver.try_recv() {
                match command {
                    Command::SendMessage(message) => {
                        if let Err(e) = swarm_sender.send(Command::SendMessage(message))
                        {
                            println!("Error sending to swarm receiver: {:?}", e);
                        }
                    }
                    Command::MineBlock => {
                        miner.mining = true;
                        miner.init = true;
                    }
                    Command::ConfirmedBlock(block) => {
                        miner.last_block = Some(block.clone());
                    }
                    Command::ProcessTxn(txn) => {
                        miner.process_txn(txn);
                        println!("{:?}", &miner.txn_pool.pending);
                    }
                    Command::InvalidBlock(_) => {
                        miner.mining = true;
                    }
                    Command::StateUpdateCompleted(network_state) => {
                        miner.network_state = network_state.clone();
                        miner.mining = true;
                    }
                    Command::MineGenesis => {
                        if let Some(block) = miner.genesis() {
                            miner.last_block = Some(block.clone());
                            // Send pending block command.
                            // wait for Confirmed or Invalid block command.
                            miner.mining = false;
                            if let Err(_) = blockchain_sender.send(Command::PendingBlock(block.clone()))
                            {
                                println!("Error sending to command receiver")
                            }
                           let message = MessageType::BlockMessage {
                                block: block.clone(),
                                sender_id: node_id.to_string().clone(),
                            };

                            if let Err(e) = swarm_sender.send(Command::SendMessage(message.as_bytes()))
                            {
                                println!("Error sending to swarm receiver: {:?}", e);
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(block) = miner.mine() {
                if let Err(_) = blockchain_sender.send(Command::PendingBlock(block.clone())) {
                    println!("Error sending to command receiver");
                }

                let message = MessageType::BlockMessage {
                    block: block.clone(),
                    sender_id: node_id.to_string().clone(),
                };

                if let Err(e) = swarm_sender.send(Command::SendMessage(message.as_bytes()))
                {
                    println!("Error sending to swarm receiver: {:?}", e);
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                miner.mining = false;
            } else {
                if let None = miner.last_block {
                    if miner.mining {
                        if let Err(_) = miner_sender.send(Command::MineGenesis) {
                            println!("Error sending command to receiver")
                        };
                    }
                }
            }
        }
    });
    //____________________________________________________________________________________________________

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
                            let message = MessageType::TxnMessage { txn, sender_id: node_id.to_string().clone() };
                            if let Err(e) = swarm_sender.send(Command::SendMessage(message.as_bytes())) {
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
