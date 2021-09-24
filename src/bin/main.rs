use hex;
use libp2p::gossipsub::IdentTopic as Topic;
use libp2p::multiaddr::multiaddr;
use libp2p::Multiaddr;
use log::info;
use rand::Rng;
use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::File;
use tokio::io::AsyncBufReadExt;
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
    let (to_message_sender, to_message_receiver) = tokio::sync::broadcast::channel(100);
    let (from_message_sender, from_message_receiver) = tokio::sync::broadcast::channel(100);
    let (command_sender, command_receiver) = tokio::sync::broadcast::channel(100);
    //____________________________________________________________________________________________________

    let wallet = if let Some(secret_key) = std::env::args().nth(4) {
        WalletAccount::restore_from_private_key(secret_key)
    } else {
        WalletAccount::new()
    };

    let network_state = NetworkState::restore("test.db");
    let reward_state = RewardState::start();

    //____________________________________________________________________________________________________
    // Node initialization
    let to_message_handler = MessageHandler::new(to_message_sender.clone(), from_message_receiver);
    let from_message_handler =
        MessageHandler::new(from_message_sender.clone(), to_message_receiver);
    let command_handler = CommandHandler::new(command_sender.clone(), command_receiver);
    let (swarm_sender, mut swarm_receiver) = tokio::sync::broadcast::channel(100);
    let mut node = Node::new(
        node_type,
        swarm_sender.clone(),
        to_message_handler,
        from_message_handler,
        command_handler,
    );
    let node_id = node.id.clone();
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Swarm initialization
    let mut swarm = config_utils::configure_swarm(
        node.to_message_handler.sender.clone(),
        node.command_handler.sender.clone(),
        node_id.clone(),
        node.key.clone(),
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
            tokio::select! {
                event = swarm.next() => {
                    info!("Unhandled Swarm Event: {:?}", event);
                }
                message = swarm_receiver.recv() => {
                    if let Ok(message) = message {
                        match message {
                            Command::SendMessage(message) => {
                                let message = hex::encode(message);
                                if let Err(e) = swarm
                                                    .behaviour_mut()
                                                    .gossipsub
                                                    .publish(Topic::new("test-net"), message) {
                                    println!("Error publishing message: {:?}", e);
                                };
                            },
                            _ => {}
                        }
                    }
                }
            };
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
    let mut blockchain_command_receiver = command_sender.subscribe();
    let blockchain_command_sender = command_sender.clone();
    let mut blockchain_network_state = network_state.clone();
    let mut blockchain_reward_state = reward_state.clone();
    tokio::task::spawn(async move {
        let mut blockchain = Blockchain::new("test_chain_db.db");
        loop {
            if let Ok(command) = blockchain_command_receiver.try_recv() {
                match command {
                    Command::PendingBlock(block) => {
                        if let Err(_) = blockchain.process_block(
                            &blockchain_network_state,
                            &blockchain_reward_state,
                            &block,
                        ) {
                            if let Err(_) = blockchain_command_sender.send(Command::InvalidBlock(block.clone())) {
                                println!("Error sending command to receiver");
                            };
                        } else {
                            if let Err(_) = blockchain_command_sender.send(Command::ConfirmedBlock(block.clone())) {
                                println!("Error sending command to receiver");
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    });
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Mining thread
    let mut mining_command_receiver = command_sender.subscribe();
    let mining_command_sender = command_sender.clone();
    let mining_wallet = wallet.clone();
    let mut miner_network_state = network_state.clone();
    let mut miner_reward_state = reward_state.clone();
    tokio::task::spawn(async move {
        let mut miner = Miner::start(
            mining_wallet.clone().pubkey,
            mining_wallet.clone().get_address(1),
            miner_reward_state,
            miner_network_state,
        );
        loop {
            if let Ok(command) = mining_command_receiver.try_recv() {
                match command {
                    Command::MineBlock => {
                        miner.mining = true;
                    }
                    Command::ConfirmedBlock(block) => {
                        miner.last_block = Some(block.clone());
                        miner.mining = true;
                    }
                    Command::PendingBlock(_) => {
                        // Stop Mining until it's processed.
                        miner.mining = false;
                    }
                    Command::InvalidBlock(_) => {
                        miner.mining = true;
                    }
                    Command::StateUpdateCompleted => {
                        miner.mining = true;
                    }
                    Command::MineGenesis => {
                        if let Some(block) = miner.genesis() {
                            miner.last_block = Some(block.clone());
                            // Send pending block command.
                            // wait for Confirmed or Invalid block command.
                            if let Err(_) = mining_command_sender.send(Command::PendingBlock(block))
                            {
                                println!("Error sending to command receiver")
                            }
                        }
                    }
                    _ => {}
                }
            }

            if let Some(block) = miner.mine() {
                miner.last_block = Some(block.clone());
                if let Err(_) = mining_command_sender.send(Command::PendingBlock(block.clone())) {
                    println!("Error sending to command receiver")
                }
            } else {
                if let None = miner.last_block {
                    if let Err(_) = mining_command_sender.send(Command::MineGenesis) {
                        println!("Error sending command to receiver")
                    };
                }
            }
        }
    });
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // State thread
    let mut state_command_receiver = command_sender.subscribe();
    let _state_command_sender = command_sender.clone();
    let mut rng = rand::thread_rng();
    let file_suffix: u32 = rng.gen();
    let path = if let Some(path) = std::env::args().nth(2) {
        path
    } else {
        format!("test_{}.db", file_suffix)
    };
    tokio::task::spawn(async move {
        let _network_state = NetworkState::restore(&path);
        loop {
            tokio::select! {
                command = state_command_receiver.recv() => {
                    if let Ok(command) = command {
                        match command {
                            _ => {}
                        }
                    }
                }
            }
        }
    });
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Wallet thread
    let mut wallet_command_receiver = command_sender.subscribe();
    let wallet_command_sender = command_sender.clone();
    tokio::task::spawn(async move {
        loop {
            tokio::select! {
                command = wallet_command_receiver.recv() => {
                    if let Ok(Command::SendTxn(sender_address_number, receiver_address, amount)) = command {
                        let txn = wallet.clone().send_txn(sender_address_number, receiver_address, amount);
                        if let Ok(txn) = txn {
                            let message = MessageType::TxnMessage {
                                txn,
                                sender_id: node_id.clone().to_string(),
                            };
                            if let Err(e) = wallet_command_sender.send(Command::SendMessage(message.as_bytes())) {
                                println!("Error sending to command sender: {:?}", e);
                            };
                        }
                    }
                }
            }
        }
    });
    //____________________________________________________________________________________________________

    //____________________________________________________________________________________________________
    // Terminal Interface loop
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    loop {
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
                if let Err(_) = command_sender.send(command) {
                    println!("Error sending command to command receiver");
                };
            }
        }
    }
    //____________________________________________________________________________________________________

    Ok(())
}
