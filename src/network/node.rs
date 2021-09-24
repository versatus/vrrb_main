#[allow(unused_imports)]
use crate::account::AccountState;
use crate::handler::{CommandHandler, MessageHandler};
use crate::network::command_utils::Command;
use crate::network::message_types::MessageType;
use libp2p::gossipsub::GossipsubMessage;
use libp2p::{identity, PeerId};
// use log::info;
use crate::network::message;
use serde::{Deserialize, Serialize};
use std::error::Error;
use tokio::sync::broadcast;

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
    pub key: identity::Keypair,
    pub id: PeerId,
    pub node_type: NodeAuth,
    pub swarm_sender: broadcast::Sender<Command>,
    pub to_message_handler: MessageHandler<GossipsubMessage, MessageType>,
    pub from_message_handler: MessageHandler<MessageType, GossipsubMessage>,
    pub command_handler: CommandHandler<Command, Command>,
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
        swarm_sender: broadcast::Sender<Command>,
        to_message_handler: MessageHandler<GossipsubMessage, MessageType>,
        from_message_handler: MessageHandler<MessageType, GossipsubMessage>,
        command_handler: CommandHandler<Command, Command>,
    ) -> Node {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        Node {
            key: local_key,
            id: local_peer_id,
            node_type,
            swarm_sender,
            from_message_handler,
            to_message_handler,
            command_handler,
        }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let evt = {
                tokio::select! {
                    command = self.command_handler.receiver.recv() => {
                        if let Ok(command) = command {
                            Some(command)
                        } else {
                            None
                        }
                    }
                    to_message = self.to_message_handler.receiver.recv() => {
                        println!("to message handler received message: {:?}", to_message);
                        None
                    }
                    from_message = self.from_message_handler.receiver.recv() => {
                        println!("from message handler received message: {:?}", from_message);
                        if let Ok(message) = from_message {
                            message::process_message(message);
                        }
                        None
                    }
                }
            };
            if let Some(command) = evt {
                match command {
                    Command::SendMessage(message) => {
                        if let Some(message) = MessageType::from_bytes(&message) {
                            if let Err(e) = self
                                .swarm_sender
                                .send(Command::SendMessage(message.as_bytes()))
                            {
                                println!("Error publishing: {:?}", e);
                            }
                        }
                    }
                    Command::ForwardCommand(command_string) => {
                        if let Some(command) = Command::from_str(&command_string) {
                            if let Err(_) = self.command_handler.sender.send(command) {
                                println!("Error forwarding command");
                            };
                        }
                    }
                    Command::Test => {
                        if let Err(_) = self.swarm_sender.send(Command::SendMessage(
                            MessageType::Test {
                                test_string: "This is a test".to_string(),
                            }
                            .as_bytes(),
                        )) {
                            println!("Error sending message to message handler");
                        };
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
                    _ => {}
                }
            } else {
                continue;
            }
        }

        Ok(())
    }
}
