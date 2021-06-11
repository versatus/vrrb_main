use crate::claim::ClaimOption;

pub trait Verifiable {
    fn is_valid(&self, _options: Option<ClaimOption>) -> Option<bool> { Some(false) }
}