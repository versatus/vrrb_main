#[allow(unused_imports)]
use crate::account::AccountState;
use crate::handler::{CommandHandler, MessageHandler};
use crate::network::command_utils::Command;
use crate::network::message;
use crate::network::message_types::MessageType;
use libp2p::gossipsub::GossipsubMessage;
use libp2p::{identity, PeerId};
use serde::{Deserialize, Serialize};
use std::error::Error;

pub const MAX_TRANSMIT_SIZE: usize = 65000;

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
    pub key: identity::Keypair,
    pub id: PeerId,
    pub node_type: NodeAuth,
    pub command_handler: CommandHandler,
    pub message_handler: MessageHandler<MessageType, GossipsubMessage>,
}

impl Node {
    pub fn get_id(&self) -> PeerId {
        self.id
    }

    pub fn get_node_type(&self) -> NodeAuth {
        self.node_type.clone()
    }

    pub fn new(
        node_type: NodeAuth,
        command_handler: CommandHandler,
        message_handler: MessageHandler<MessageType, GossipsubMessage>,
    ) -> Node {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        Node {
            key: local_key,
            id: local_peer_id,
            node_type,
            command_handler,
            message_handler,
        }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let evt = {
                tokio::select! {
                    command = self.command_handler.receiver.recv() => {
                        if let Some(command) = command {
                            Some(command)
                        } else {
                            None
                        }
                    }
                    from_message = self.message_handler.receiver.recv() => {
                        if let Some(message) = from_message {
                           message::process_message(message, self.id.clone().to_string())
                        } else {
                            None
                        }
                    }
                }
            };
            if let Some(command) = evt {
                match command {
                    Command::SendMessage(message) => {
                        if let Some(message) = MessageType::from_bytes(&message) {
                            if let Err(e) = self
                                .command_handler
                                .to_swarm_sender
                                .send(Command::SendMessage(message.as_bytes()))
                            {
                                println!("Error publishing: {:?}", e);
                            }
                        }
                    }
                    Command::Quit => {
                        //TODO:
                        // 1. Inform peers. DONE
                        // 2. Before Ok(()) at the end of this method
                        //    be sure to join all the threads in this method by setting them to variables
                        //    and winding them down at the end after exiting this event loop.
                        // 3. Print out the node's wallet secret key, the state db filepath and the
                        //    block archive filepath so users can restore their wallet and state
                        //    when rejoining.

                        break;
                    }
                    Command::SendAddress => {
                        if let Err(e) = self
                            .command_handler
                            .to_mining_sender
                            .send(Command::SendAddress)
                        {
                            println!("Error sending SendAddress command to miner: {:?}", e);
                        }
                    }
                    Command::MineBlock => {
                        if let Err(e) = self
                            .command_handler
                            .to_mining_sender
                            .send(Command::StartMiner)
                        {
                            println!("Error sending mine block command to mining thread: {:?}", e);
                        }
                    }
                    Command::SendState(requested_from, lowest_block) => {
                        if let Err(e) = self
                            .command_handler
                            .to_blockchain_sender
                            .send(Command::SendState(requested_from, lowest_block))
                        {
                            println!("Error sending state request to blockchain thread: {:?}", e);
                        }
                    }
                    Command::StoreStateDbChunk(object, data, chunk_number, total_chunks) => {
                        if let Err(e) = self.command_handler.to_blockchain_sender.send(
                            Command::StoreStateDbChunk(object, data, chunk_number, total_chunks),
                        ) {
                            println!(
                                "Error sending StoreStateDbChunk to blockchain thread: {:?}",
                                e
                            );
                        }
                    }
                    _ => {
                        self.command_handler.handle_command(command);
                    }
                }
            } else {
                continue;
            }
        }

        Ok(())
    }
}
