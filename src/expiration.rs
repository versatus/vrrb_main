pub trait Expiration {
    fn is_expired(&self) -> bool;
}