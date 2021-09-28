use crate::network::command_utils::Command;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub trait Handler<T, V> {
    fn send(&self, message: T) -> Option<T>;
    fn recv(&mut self) -> Option<V>;
}

// T is MessageType, V is GossipSubMessage in to_message_handler
// T is GossipSubMessage, V is MessageType in from_message_handler
pub struct MessageHandler<T, V> {
    pub sender: UnboundedSender<T>,
    pub receiver: UnboundedReceiver<V>,
}

pub struct CommandHandler {
    pub to_mining_sender: UnboundedSender<Command>,
    pub to_blockchain_sender: UnboundedSender<Command>,
    pub to_swarm_sender: UnboundedSender<Command>,
    pub to_wallet_sender: UnboundedSender<Command>,
    pub receiver: UnboundedReceiver<Command>,
}

impl<T: Clone, V: Clone> MessageHandler<T, V> {
    pub fn new(sender: UnboundedSender<T>, receiver: UnboundedReceiver<V>) -> MessageHandler<T, V> {
        MessageHandler { sender, receiver }
    }
}

impl CommandHandler {
    pub fn new(
        to_mining_sender: UnboundedSender<Command>,
        to_blockchain_sender: UnboundedSender<Command>,
        to_swarm_sender: UnboundedSender<Command>,
        to_wallet_sender: UnboundedSender<Command>,
        receiver: UnboundedReceiver<Command>,
    ) -> CommandHandler {
        CommandHandler {
            to_mining_sender,
            to_blockchain_sender,
            to_swarm_sender,
            to_wallet_sender,
            receiver,
        }
    }

    pub fn handle_command(&mut self, command: Command) {
        match command {
            Command::SendTxn(sender_address_number, receiver_address, amount) => {
                println!("SendTxn command received, forwarding to wallet");
                if let Err(e) = self.to_wallet_sender.send(Command::SendTxn(
                    sender_address_number,
                    receiver_address,
                    amount,
                )) {
                    println!("Error sending to wallet: {:?}", e);
                }
            }
            Command::MineBlock => {}
            Command::SendAddress => {
                //TODO: Change this to send claim
                if let Err(e) = self.to_mining_sender.send(Command::SendAddress) {
                    println!("Error sending to mining sender: {:?}", e);
                }
            }
            Command::StopMine => {
                if let Err(e) = self.to_mining_sender.send(Command::StopMine) {
                    println!("Error sending to mining sender: {:?}", e);
                }
            }
            Command::GetState => {
                //TODO: request the state from the most recent confirmed block miner's node.
            }
            Command::ProcessTxn(txn) => {
                if let Err(e) = self.to_mining_sender.send(Command::ProcessTxn(txn)) {
                    println!("Error sending transaction to mining sender for processing: {:?}", e);
                }
            }
            Command::StateUpdateCompleted(network_state) => {
                if let Err(e) = self.to_mining_sender.send(Command::StateUpdateCompleted(network_state)) {
                    println!("Error sending updated network state to mining receiver: {:?}", e);
                }
            }
            Command::StoreStateDbChunk(
                _object,
                _chunk,
                _chunk_number,
                _total_chunks,
                _last_block,
            ) => {}
            Command::ProcessBacklog => {}
            Command::CheckStateUpdateStatus((_block_height, _block, _last_block)) => {}
            Command::NewPeer(_peer_id, _pubkey) => {}
            Command::RemovePeer(_peer_id) => {}
            Command::PruneMiners(_connected_peers) => {}
            Command::Quit => {
                // TODO: Inform all the threads that you're shutting down.
            }
            Command::SendMessage(message) => {
                if let Err(e) = self.to_swarm_sender.send(Command::SendMessage(message)) {
                    println!("Error sending message command to swarm: {:?}", e);
                }
            }
            Command::SendState(_peer_id) => {}
            Command::ConfirmedBlock(_block) => {}
            Command::PendingBlock(block) => {
                if let Err(e) = self.to_mining_sender.send(Command::PendingBlock(block.clone())) {
                    println!("Error sending pending block to miner: {:?}", e);
                }
                if let Err(e) = self.to_blockchain_sender.send(Command::PendingBlock(block.clone())) {
                    println!("Error sending pending block to blockchain: {:?}", e);
                }
            }
            Command::InvalidBlock(_block) => {}
            Command::MineGenesis => {}
        }
    }
}

impl<T: Clone, V: Clone> Handler<T, V> for MessageHandler<T, V> {
    fn send(&self, command: T) -> Option<T> {
        if let Err(_) = self.sender.send(command.clone()) {
            return None;
        } else {
            return Some(command);
        }
    }

    fn recv(&mut self) -> Option<V> {
        if let Ok(message) = self.receiver.try_recv() {
            return Some(message);
        } else {
            return None;
        }
    }
}
