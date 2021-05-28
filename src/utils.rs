use crate::account::WalletAccount;
use std::collections::HashMap;

pub fn decay_calculator(initial: u32, epochs: u32) -> f64 {
    let b: f64 = 1.0f64 / initial as f64;
    let ln_b = b.log10();

    ln_b / epochs as f64
}

// pub fn insert_balance_into_state<T>(WalletAccount) -> HashMap<String> {

// }

// TODO: Write tests for this module