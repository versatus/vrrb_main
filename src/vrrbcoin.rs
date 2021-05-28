use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Token {
    Ticker(String),
    Name(String),
    Units(i32),
}

// TODO: Rename this file, this is not 
// the vrrbcoin file, this is for smart contract tokens
// TODO: Write tests for this module