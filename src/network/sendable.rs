use crate::network::chunkable::Chunkable;

pub trait Sendable<T: Chunkable> {
    fn send(&self);
}