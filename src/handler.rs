use tokio::sync::broadcast::{Receiver, Sender};

pub trait Handler<T, V> {
    fn send(&self, message: T) -> Option<T>;
    fn recv(&mut self) -> Option<V>;
}

pub struct MessageHandler<T, V> {
    pub sender: Sender<T>,
    pub receiver: Receiver<V>,
}

pub struct CommandHandler<T, V> {
    pub sender: Sender<T>,
    pub receiver: Receiver<V>
}

impl<T: Clone, V: Clone> MessageHandler<T, V> {
    pub fn new(sender: Sender<T>, receiver: Receiver<V>) -> MessageHandler<T, V> {
        MessageHandler {
            sender,
            receiver
        }
    }
}

impl<T: Clone, V: Clone> CommandHandler<T, V> {
    pub fn new(sender: Sender<T>, receiver: Receiver<V>) -> CommandHandler<T, V> {
        CommandHandler {
            sender,
            receiver,
        }
    }
}

impl<T: Clone, V: Clone> Handler<T, V> for MessageHandler<T, V> {
    fn send(&self, command: T) -> Option<T> {
        if let Err(_) = self.sender.send(command.clone()) {
            return None
        } else {
            return Some(command)
        }
    }

    fn recv(&mut self) -> Option<V> {
        if let Ok(message) = self.receiver.try_recv() {
            return Some(message)
        } else {
            return None
        }
    }
}