use crate::block::Block;
use crate::claim::Claim;
use crate::network::message_types::StateBlock;
use crate::state::NetworkState;
use crate::txn::Txn;
use crate::validator::TxnValidator;
use serde::{Deserialize, Serialize};
// use log::{info};

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
    ProcessTxn(Txn),
    ProcessTxnValidator(TxnValidator),
    ConfirmedBlock(Block),
    PendingBlock(Block),
    InvalidBlock(Block),
    ProcessClaim(Claim),
    CheckStateUpdateStatus((u128, Block, u128)),
    StateUpdateCompleted(NetworkState),
    StoreStateDbChunk(StateBlock, Vec<u8>, u32, u32),
    SendState(String, u128),
    SendMessage(Vec<u8>),
    MineBlock,
    MineGenesis,
    StopMine,
    GetState,
    ProcessBacklog,
    SendAddress,
    NonceUp,
    Quit,
}

impl Command {
    pub fn from_str(command_string: &str) -> Option<Command> {
        let args: Vec<&str> = command_string.split(' ').collect();
        if args.len() == 4 {
            match args[0] {
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
                _ => {
                    println!("Invalid command string");
                    None
                }
            }
        }
    }
}
