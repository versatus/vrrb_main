pub trait Chunkable {
    fn chunk(&self) -> Option<Vec<Vec<u8>>>;
}