use crate::validator::ValidatorOptions;

pub trait Verifiable {
    fn is_valid(
        &self, 
        _options: Option<ValidatorOptions>
    ) -> Option<bool> 
    { 
        Some(false) 
    }
}