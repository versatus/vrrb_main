#[allow(unused_imports)]
use async_std::{io, task};
use futures::prelude::*;
use libp2p::{
    NetworkBehaviour,
    PeerId,
    Swarm,
    development_transport,
    identity,
    gossipsub,
    mdns::{Mdns, MdnsConfig, MdnsEvent},
    swarm::{NetworkBehaviourEventProcess, SwarmEvent}
};
use crate::account::WalletAccount;
use std::{collections::HashMap, error::Error, task::{Context, Poll}};
#[allow(dead_code)]
pub enum Channel {
    Transactions,
    Blocks,
    Claims,
}
#[allow(dead_code)]
pub enum NodeAuth {
    Full,
    Transact,
    Validate,
}
#[allow(dead_code)]
pub struct Node {
    pubkey: String,
    id: PeerId,
    wallet: WalletAccount,
    subscriptions: Vec<Channel>,
    peers: HashMap<String, String>,
    auth: Option<NodeAuth>
}

impl Node {
    pub fn new() {
    }
}