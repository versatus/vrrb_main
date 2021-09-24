use crate::block::Block;
use crate::network::message_types::StateBlock;
use crate::network::node::Node;
// use log::{info};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
pub const QUIT: &str = "QUIT";
pub const TEST: &str = "TEST";

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Command {
    SendTxn(u32, String, u128), // address number, receiver address, amount
    MineBlock,
    MineGenesis,
    StopMine,
    GetState,
    ConfirmedBlock(Block),
    PendingBlock(Block),
    InvalidBlock(Block),
    CheckStateUpdateStatus((u128, Block, u128)),
    StateUpdateCompleted,
    StoreStateDbChunk(StateBlock, Vec<u8>, u32, u32, u128),
    PruneMiners(HashSet<String>),
    ProcessBacklog,
    SendAddress,
    SendState(String),
    AcquireClaim(u128, u128, u128), // Maximum Price, Maximum Maturity, Maximum Number of claims to acquire that fit the price/maturity requirements, address to purchase from.
    SellClaim(u128, u128),          // Claim Number, Price.
    RemovePeer(String),
    NewPeer(String, String),
    SendMessage(Vec<u8>),
    ForwardCommand(String),
    Quit,
    Test,
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
                QUIT => return Some(Command::Quit),
                TEST => return Some(Command::Test),
                _ => {
                    println!("Invalid command string");
                    None
                }
            }
        }
    }
}

pub fn handle_command(_node: Node, command: Command) {
    match command {
        Command::SendTxn(_sender_address_number, _receiver_address, _amount) => {}
        Command::MineBlock => {}
        Command::SendAddress => {}
        Command::StopMine => {}
        Command::GetState => {}
        Command::StateUpdateCompleted => {}
        Command::StoreStateDbChunk(_object, _chunk, _chunk_number, _total_chunks, _last_block) => {}
        Command::ProcessBacklog => {}
        Command::CheckStateUpdateStatus((_block_height, _block, _last_block)) => {}
        Command::NewPeer(_peer_id, _pubkey) => {}
        Command::RemovePeer(_peer_id) => {}
        Command::PruneMiners(_connected_peers) => {}
        Command::Quit => {}
        Command::SendMessage(_message) => {}
        Command::AcquireClaim(_max_price, _max_maturity, _max_number) => {}
        Command::SellClaim(_claim_number, _price) => {}
        Command::SendState(_peer_id) => {}
        Command::ForwardCommand(_command_string) => {}
        Command::Test => {}
        Command::ConfirmedBlock(_block) => {}
        Command::PendingBlock(_block) => {}
        Command::InvalidBlock(_block) => {}
        Command::MineGenesis => {}
    }
}

pub fn handle_input_line(node: Arc<Mutex<Node>>, line: String) {
    let _args: Vec<&str> = line.split(' ').collect();
    let _task_node = Arc::clone(&node);
    if let Err(e) = node
        .lock()
        .unwrap()
        .swarm_sender
        .send(Command::ForwardCommand(line))
    {
        println!(
            "Encountered Error sending message to command thread: {:?}",
            e
        );
    };
}
