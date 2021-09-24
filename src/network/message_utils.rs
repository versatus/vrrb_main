#![allow(unused_imports)]
use crate::block::Block;
use crate::claim::Claim;
use crate::network::chunkable::Chunkable;
use crate::network::command_utils;
use crate::network::command_utils::Command;
use crate::network::message;
use crate::network::message_types::{MessageType, StateBlock};
use crate::network::node::MAX_TRANSMIT_SIZE;
use crate::network::node::{Node, NodeAuth};
use crate::state::{BlockArchive, NetworkState};
use crate::utils::restore_db;
use crate::validator::ValidatorOptions;
use crate::verifiable::Verifiable;
use libp2p::gossipsub::error::PublishError;
use libp2p::gossipsub::IdentTopic as Topic;
use log::info;
use ritelinked::LinkedHashMap;
use std::sync::{mpsc::channel, mpsc::Sender, Arc, Mutex};
use std::thread;
use std::time::Duration;

pub fn share_addresses() {}

pub fn mine_block() {}

pub fn update_last_confirmed_block() {}

pub fn update_block_archive() {}

pub fn update_credits_and_debits() {}

pub fn update_reward_state() {}

pub fn update_state_hash() {}

pub fn update_last_state() {}

pub fn request_state() {}

pub fn send_missing_blocks() {}

pub fn send_state() {}

pub fn process_block() {}

pub fn set_network_state() {}
