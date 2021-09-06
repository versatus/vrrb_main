use crate::network::message;
use crate::network::message_types::MessageType;
use crate::network::message_types::StateBlock;
use crate::network::message_utils;
use crate::network::node::Node;
// use log::{info};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub const NEWTXN: &str = "NEW_TXN";
pub const SENDTXN: &str = "SENDTXN";
pub const GETSTATE: &str = "GET_STE";
pub const SENDSTATE: &str = "SENDSTE";
pub const MINEBLOCK: &str = "MINEBLK";
pub const STOPMINE: &str = "STPMINE";
pub const ACQUIRECLAIM: &str = "ACQRCLM";
pub const SELLCLAIM: &str = "SELLCLM";
pub const SENDADDRESS: &str = "SENDADR";
pub const TXNTOPIC: &str = "txn";

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
pub enum Command {
    SendTxn(u32, String, u128), // address number, receiver address, amount
    MineBlock,
    StopMine,
    GetState,
    CheckStateUpdateStatus(u128),
    StateUpdateCompleted,
    StoreStateDbChunk(StateBlock, Vec<u8>, u32, u32),
    ProcessBacklog,
    SendAddress,
    SendState(String),
    AcquireClaim(u128, u128, u128), // Maximum Price, Maximum Maturity, Maximum Number of claims to acquire that fit the price/maturity requirements, address to purchase from.
    SellClaim(u128, u128),          // Claim Number, Price.
}

impl Command {
    pub fn from_str(command_string: &str) -> Option<Command> {
        let args: Vec<&str> = command_string.split(' ').collect();
        if args.len() == 4 {
            match args[0] {
                ACQUIRECLAIM => {
                    return Some(Command::AcquireClaim(
                        args[1].parse::<u128>().unwrap(),
                        args[2].parse::<u128>().unwrap(),
                        args[3].parse::<u128>().unwrap(),
                    ))
                }
                SENDTXN => {
                    return Some(Command::SendTxn(
                        args[1].parse::<u32>().unwrap(),
                        args[2].to_string(),
                        args[3].parse::<u128>().unwrap(),
                    ))
                }
                _ => {
                    println!("Invalid command string!");
                    return None;
                }
            }
        } else if args.len() == 3 {
            match args[0] {
                SELLCLAIM => {
                    return Some(Command::SellClaim(
                        args[1].parse::<u128>().unwrap(),
                        args[2].parse::<u128>().unwrap(),
                    ))
                }
                _ => {
                    println!("Invalid command string!");
                    return None;
                }
            }
        } else if args.len() == 2 {
            match args[0] {
                SENDSTATE => return Some(Command::SendState(args[1].to_string())),
                _ => {
                    println!("Invalid command string!");
                    return None;
                }
            }
        } else {
            match command_string.clone() {
                GETSTATE => return Some(Command::GetState),
                MINEBLOCK => return Some(Command::MineBlock),
                STOPMINE => return Some(Command::StopMine),
                SENDADDRESS => return Some(Command::SendAddress),
                _ => {
                    println!("Invalid command string");
                    None
                }
            }
        }
    }
}

pub fn handle_command(node: Arc<Mutex<Node>>, command: Command) {
    let command_node = Arc::clone(&node);
    match command {
        Command::SendTxn(sender_address_number, receiver_address, amount) => {
            let wallet = command_node.lock().unwrap().wallet.lock().unwrap().clone();
            if let Ok(txn) = wallet.send_txn(sender_address_number, receiver_address, amount) {
                let txn_message = MessageType::TxnMessage {
                    txn: txn.clone(),
                    sender_id: command_node.lock().unwrap().id.clone().to_string(),
                };
                command_node
                    .lock()
                    .unwrap()
                    .account_state
                    .lock()
                    .unwrap()
                    .txn_pool
                    .pending
                    .insert(txn.txn_id.clone(), txn.clone());
                let message = message::structure_message(txn_message.as_bytes());
                message::publish_message(Arc::clone(&command_node), message, TXNTOPIC);
            };
        }
        Command::MineBlock => {
            if let Err(e) = node.lock().unwrap().mining_sender.send(Command::MineBlock) {
                println!("Error sending command to block mining: {:?}", e);
            };
        }
        Command::SendAddress => {
            let thread_node = Arc::clone(&command_node);
            std::thread::spawn(move || {
                let cloned_node = Arc::clone(&thread_node);
                message_utils::share_addresses(Arc::clone(&cloned_node))
            })
            .join()
            .unwrap();
        }
        Command::StopMine => {
            if let Err(e) = node.lock().unwrap().mining_sender.send(Command::StopMine) {
                println!("Error sending command to block mining: {:?}", e);
            };
        }
        Command::GetState => {
            if let Err(e) = node.lock().unwrap().state_sender.send(Command::GetState) {
                println!("Error sending command to state updator: {:?}", e);
            }
        }
        Command::StateUpdateCompleted => {
            if let Err(e) = node
                .lock()
                .unwrap()
                .state_sender
                .send(Command::StateUpdateCompleted)
            {
                println!("Error sending command to state updator: {:?}", e);
            }
        }
        Command::StoreStateDbChunk(object, chunk, chunk_number, total_chunks) => {
            if let Err(e) = node
                .lock()
                .unwrap()
                .state_sender
                .send(Command::StoreStateDbChunk(
                    object,
                    chunk,
                    chunk_number,
                    total_chunks,
                ))
            {
                println!("Error sending command to state updator: {:?}", e);
            }
        }
        Command::ProcessBacklog => {
            if let Err(e) = node
                .lock()
                .unwrap()
                .state_sender
                .send(Command::ProcessBacklog)
            {
                println!("Error sending command to state updator: {:?}", e);
            };
        }
        Command::CheckStateUpdateStatus(block_height) => {
            if let Err(e) = node
                .lock()
                .unwrap()
                .state_sender
                .send(Command::CheckStateUpdateStatus(block_height))
            {
                println!("Error sending command to state updator: {:?}", e);
            };
        }
        Command::AcquireClaim(_max_price, _max_maturity, _max_number) => {}
        Command::SellClaim(_claim_number, _price) => {}
        Command::SendState(_peer_id) => {}
    }
}

pub fn handle_input_line(node: Arc<Mutex<Node>>, line: String) {
    let _args: Vec<&str> = line.split(' ').collect();
    let _task_node = Arc::clone(&node);
    if let Some(command) = Command::from_str(&line) {
        if let Err(e) = node
            .lock()
            .unwrap()
            .swarm
            .behaviour()
            .command_sender
            .send(command)
        {
            println!(
                "Encountered Error sending message to command thread: {:?}",
                e
            );
        };
    }
}
