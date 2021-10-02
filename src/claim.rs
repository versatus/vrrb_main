use crate::verifiable::Verifiable;
use serde::{Deserialize, Serialize};
use sha256::digest_bytes;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub pubkey: String,
    pub address: String,
    pub hash: String,
    pub start: Option<u8>,
    pub nonce: u128,
    pub eligible: bool,
}

impl Claim {
    pub fn new(pubkey: String, address: String, claim_nonce: u128) -> Claim {
        let iters = if let Some(n) = claim_nonce.checked_mul(10) {
            n
        } else {
            claim_nonce
        };

        let mut hash = pubkey.clone();
        (0..iters).for_each(|_| {
            hash = digest_bytes(hash.as_bytes());
        });

        Claim {
            pubkey,
            address,
            hash: hash,
            start: None,
            nonce: claim_nonce,
            eligible: false,
        }
    }

    pub fn nonce_up(&mut self) {
        self.nonce = self.nonce + 1;
        let iters = if let Some(n) = self.nonce.clone().checked_mul(10) {
            n
        } else {
            self.nonce.clone()
        };

        let mut hash = self.hash.clone();
        (0..iters).for_each(|_| {
            hash = digest_bytes(hash.as_bytes());
        });

        self.hash = hash;
    }

    pub fn get_pointer(&mut self, nonce: u128) -> Option<u128> {
        let nonce_hex = format!("{:x}", nonce);
        let nonce_string_len = nonce_hex.chars().count();
        let mut pointers = vec![];
        nonce_hex.chars().enumerate().for_each(|(idx, c)| {
            let res = self.hash.find(c);
            if let Some(n) = res {
                let n = n as u128;
                let n = n.checked_pow(idx as u32);
                if let Some(n) = n {
                    pointers.push(n as u128);
                }
            }
        });

        if pointers.len() == nonce_string_len {
            let pointer: u128 = pointers.iter().sum();
            Some(pointer)
        } else {
            None
        }
    }

    pub fn from_string(claim_string: String) -> Claim {
        serde_json::from_str::<Claim>(&claim_string).unwrap()
    }
}

impl Verifiable for Claim {
    fn verifiable(&self) -> bool {
        true
    }
}
