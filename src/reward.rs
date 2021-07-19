use rand::{Rng, distributions::{Distribution, WeightedIndex}, thread_rng};
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;
use std::sync::{Arc, Mutex};
use crate::{state::NetworkState, utils::decay_calculator};

// Generate a random variable reward to include in new blocks
pub const TOTAL_NUGGETS: u128 = 80000000;
pub const TOTAL_VEINS: u128 = 1400000;
pub const TOTAL_MOTHERLODES: u128 = 20000;
pub const N_BLOCKS_PER_EPOCH: u128 = 16000000;
pub const NUGGET_FINAL_EPOCH: u128 = 300;
pub const VEIN_FINAL_EPOCH: u128 = 200;
pub const MOTHERLODE_FINAL_EPOCH: u128 = 100;
pub const FLAKE_REWARD_RANGE: (u128, u128) = (1, 8);
pub const GRAIN_REWARD_RANGE: (u128, u128) = (8, 64);
pub const NUGGET_REWARD_RANGE: (u128, u128) = (64, 512);
pub const VEIN_REWARD_RANGE: (u128, u128) = (512, 4096);
pub const MOTHERLODE_REWARD_RANGE: (u128, u128) = (4096, 32769);
pub const GENESIS_REWARD: u128 = 200_000_000;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter)]
pub enum Category {
    Flake(Option<u128>),
    Grain(Option<u128>),
    Nugget(Option<u128>),
    Vein(Option<u128>),
    Motherlode(Option<u128>),
    Genesis(Option<u128>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardState {
    pub epoch: u128,
    pub next_epoch_block: u128,
    pub current_block: u128,
    pub n_nuggets_remaining: u128,
    pub n_veins_remaining: u128,
    pub n_motherlodes_remaining: u128,
    pub n_nuggets_current_epoch: u128,
    pub n_veins_current_epoch: u128,
    pub n_motherlodes_current_epoch: u128,
    pub n_flakes_current_epoch: u128,
    pub n_grains_current_epoch: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reward {
    pub miner: Option<String>,
    pub category: Category,
    pub amount: u128,
}

impl RewardState {
    pub fn start(network_state: Arc<Mutex<NetworkState>>) -> RewardState {
        let n_nuggets_ce: u128 = (decay_calculator(
            TOTAL_NUGGETS, NUGGET_FINAL_EPOCH) * 
            TOTAL_NUGGETS as f64) as u128;
        let n_veins_ce: u128 = (decay_calculator(
            TOTAL_NUGGETS, NUGGET_FINAL_EPOCH) * 
            TOTAL_NUGGETS as f64) as u128;
        let n_motherlodes_ce: u128 = (decay_calculator(
            TOTAL_NUGGETS, NUGGET_FINAL_EPOCH) * 
            TOTAL_NUGGETS as f64) as u128;
        let remaining_blocks = N_BLOCKS_PER_EPOCH - (n_nuggets_ce + n_veins_ce + n_motherlodes_ce);
        let n_flakes_ce: u128 = (remaining_blocks as f64 * 0.6f64) as u128;
        let n_grains_ce: u128 = (remaining_blocks as f64 * 0.4f64) as u128;

        let reward_state = RewardState {
            current_block: 0,
            epoch: 1,
            next_epoch_block: 16000000,
            n_nuggets_remaining: TOTAL_NUGGETS,
            n_veins_remaining: TOTAL_VEINS,
            n_motherlodes_remaining: TOTAL_MOTHERLODES,
            n_nuggets_current_epoch: n_nuggets_ce,
            n_veins_current_epoch: n_veins_ce,
            n_motherlodes_current_epoch: n_motherlodes_ce,
            n_flakes_current_epoch: n_flakes_ce,
            n_grains_current_epoch: n_grains_ce,
            
        };
        let state_result = network_state.lock().unwrap().update(reward_state, "reward_state");
        
        match state_result {
            Ok(()) => {},
            Err(e) => { println!("Error in updating network state: {:?}", e) }
        }
        
        reward_state
    }
    pub fn update(&self, last_reward: Category, network_state: &mut NetworkState) -> Self {
        let mut n_nuggets_ce: u128 = self.n_nuggets_current_epoch;
        let mut n_veins_ce: u128 = self.n_veins_current_epoch;
        let mut n_motherlodes_ce: u128 = self.n_motherlodes_current_epoch;
        let mut n_flakes_ce: u128 = self.n_flakes_current_epoch;
        let mut n_grains_ce: u128 = self.n_grains_current_epoch;
        let remaining_blocks_in_ce: u128 = self.next_epoch_block - (self.current_block + 1);
        if remaining_blocks_in_ce != 0 {
            n_nuggets_ce = match last_reward {
                Category::Nugget(Some(_)) => n_nuggets_ce - 1,
                _ => n_nuggets_ce,
                };
            n_veins_ce = match last_reward {
                Category::Vein(Some(_)) => n_veins_ce - 1,
                _ => n_veins_ce,
                };
            n_motherlodes_ce = match last_reward {
                Category::Motherlode(Some(_)) => n_motherlodes_ce - 1,
                _ => n_motherlodes_ce,
                };
            n_flakes_ce = match last_reward {
                Category::Flake(Some(_)) => n_flakes_ce - 1,
                _ => n_flakes_ce,
                };
            n_grains_ce = match last_reward {
                Category::Grain(Some(_)) => n_grains_ce -1,
                _ => n_grains_ce,
            };
        } else {
            n_nuggets_ce = (decay_calculator(
            TOTAL_NUGGETS, NUGGET_FINAL_EPOCH) * 
            self.n_nuggets_remaining as f64) as u128;
            n_veins_ce = (decay_calculator(
                TOTAL_NUGGETS, NUGGET_FINAL_EPOCH) * 
                self.n_veins_remaining as f64) as u128;
            n_motherlodes_ce = (decay_calculator(
                TOTAL_NUGGETS, NUGGET_FINAL_EPOCH) * 
                self.n_motherlodes_remaining as f64) as u128;
            let remaining_blocks = N_BLOCKS_PER_EPOCH - (n_nuggets_ce + n_veins_ce + n_motherlodes_ce);
            n_flakes_ce = (remaining_blocks as f64 * 0.6f64) as u128;
            n_grains_ce = (remaining_blocks as f64 * 0.4f64) as u128;
        }

        let updated_reward_state = Self {
            current_block: self.current_block + 1,
            epoch: if self.current_block + 1 != self.next_epoch_block {
                self.epoch
            } else {
                self.epoch + 1
            },
            next_epoch_block: if self.current_block + 1 != self.next_epoch_block {
                self.next_epoch_block
            } else {
                self.next_epoch_block + N_BLOCKS_PER_EPOCH
            },
            n_nuggets_remaining: match last_reward {
                Category::Nugget(Some(_)) => self.n_nuggets_remaining - 1,
                _ => self.n_nuggets_remaining,
            },
            n_veins_remaining: match last_reward {
                Category::Vein(Some(_)) => self.n_veins_remaining - 1,
                _ => self.n_veins_remaining,
            },
            n_motherlodes_remaining: match last_reward {
                Category::Motherlode(Some(_)) => self.n_motherlodes_remaining - 1,
                _ => self.n_motherlodes_remaining,
            },
            n_nuggets_current_epoch: n_nuggets_ce,
            n_veins_current_epoch: n_veins_ce,
            n_motherlodes_current_epoch: n_motherlodes_ce,
            n_flakes_current_epoch: n_flakes_ce,
            n_grains_current_epoch: n_grains_ce,
            };
        let state_result = network_state.update(updated_reward_state, "reward_state");
        
        match state_result {
            Ok(()) => {},
            Err(e) => { println!("Error in updating network state: {:?}", e)}
        }
        
        updated_reward_state
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();
        
        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> RewardState {
        let mut buffer: Vec<u8> = vec![];

        data.iter().for_each(|x| buffer.push(*x));

        let to_string = String::from_utf8(buffer).unwrap();

        serde_json::from_str::<RewardState>(&to_string).unwrap()
    }
}

impl Reward {
    pub fn new(miner: Option<String>, reward_state: Arc<Mutex<RewardState>>) -> Reward {
        let category: Category = Category::new(Arc::clone(&reward_state));
        Reward {
            miner,
            category,
            amount: match category {
                Category::Flake(Some(amount)) => amount,
                Category::Grain(Some(amount)) => amount,
                Category::Nugget(Some(amount)) => amount,
                Category::Vein(Some(amount)) => amount,
                Category::Motherlode(Some(amount)) => amount,
                _ => 0,             // Add error handling, as this should NEVER happen.
            },
        }
    }
    pub fn genesis(miner: Option<String>) -> Reward {
        let category = Category::Genesis(Some(GENESIS_REWARD as u128));
        Reward {
            miner,
            category,
            amount: match category {
                Category::Genesis(Some(amount)) => amount,
                _ => 0,
            } 
        }
    }
    
    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();

        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> Reward {
        let mut buffer: Vec<u8> = vec![];

        data.iter().for_each(|x| buffer.push(*x));

        let to_string = String::from_utf8(buffer).unwrap();

        serde_json::from_str::<Reward>(&to_string).unwrap()
    }
}

impl Category {
    pub fn new(reward_state: Arc<Mutex<RewardState>>) -> Category {
        Category::generate_category(reward_state).amount()
    }

    pub fn generate_category(reward_state: Arc<Mutex<RewardState>>) -> Category {
        let items = vec![
            (Category::Flake(None), reward_state.lock().unwrap().n_flakes_current_epoch.clone()),
            (Category::Grain(None), reward_state.lock().unwrap().n_grains_current_epoch.clone()),
            (Category::Nugget(None), reward_state.lock().unwrap().n_nuggets_current_epoch.clone()),
            (Category::Vein(None), reward_state.lock().unwrap().n_veins_current_epoch.clone()),
            (Category::Motherlode(None), reward_state.lock().unwrap().n_veins_current_epoch.clone()),
            ];
        let dist = WeightedIndex::new(items.iter().map(|item| item.1)).unwrap();
        let mut rng = rand::thread_rng();
        items[dist.sample(&mut rng)].0
    }

    pub fn amount(&self) -> Category {
        let mut rng = thread_rng();
        match self {
            Self::Genesis(None) => Category::Genesis(None),
            Self::Flake(None) => Category::Flake(Some(
                rng.gen_range(FLAKE_REWARD_RANGE.0, FLAKE_REWARD_RANGE.1)
            )),
            Self::Grain(None) => Category::Grain(Some(
                rng.gen_range(GRAIN_REWARD_RANGE.0, GRAIN_REWARD_RANGE.1)
            )),
            Self::Nugget(None) => Category::Nugget(Some(
                rng.gen_range(NUGGET_REWARD_RANGE.0, NUGGET_REWARD_RANGE.1)
            )),
            Self::Vein(None) => Category::Vein(Some(
                rng.gen_range(VEIN_REWARD_RANGE.0, VEIN_REWARD_RANGE.1)
            )),
            Self::Motherlode(None) => Category::Motherlode(Some(
                rng.gen_range(MOTHERLODE_REWARD_RANGE.0, MOTHERLODE_REWARD_RANGE.1)
            )),
            Self::Genesis(Some(amount)) => Self::Genesis(Some(*amount)),
            Self::Flake(Some(amount)) => Self::Flake(Some(*amount)),
            Self::Grain(Some(amount)) => Self::Grain(Some(*amount)),
            Self::Nugget(Some(amount)) => Self::Nugget(Some(*amount)),
            Self::Vein(Some(amount)) => Self::Vein(Some(*amount)),
            Self::Motherlode(Some(amount)) => Self::Motherlode(Some(*amount)),
        }
    }
    
    pub fn as_bytes(&self) -> Vec<u8> {
        let as_string = serde_json::to_string(self).unwrap();

        as_string.as_bytes().iter().copied().collect()
    }

    pub fn from_bytes(data: &[u8]) -> Category {
        let mut buffer: Vec<u8> = vec![];

        data.iter().for_each(|x| buffer.push(*x));

        let to_string = String::from_utf8(buffer).unwrap();

        serde_json::from_str::<Category>(&to_string).unwrap()
    }
    
}

pub fn valid_reward(category: Category, reward_state: RewardState) -> Option<bool> {
    match category {
        Category::Flake(amount) => {
            match amount {
                Some(amt) => {
                    if amt < FLAKE_REWARD_RANGE.0 || amt > FLAKE_REWARD_RANGE.1 {
                        return Some(false)
                    }
                    if reward_state.n_flakes_current_epoch == 0 {
                        return Some(false)
                    }
                },
                None => { return Some(false) }
            }
        },
        Category::Grain(amount) => {
            match amount {
                Some(amt) => {
                    if amt < GRAIN_REWARD_RANGE.0 || amt > GRAIN_REWARD_RANGE.1 {
                        return Some(false)
                    }

                    if reward_state.n_grains_current_epoch == 0 {
                        return Some(false)
                    }                                        
                },
                None => { return Some(false) }
            }
        },
        Category::Nugget(amount) => {
            match amount {
                Some(amt) => {
                    if amt < NUGGET_REWARD_RANGE.0 || amt > NUGGET_REWARD_RANGE.1 {
                        return Some(false)
                    }

                    if reward_state.n_nuggets_current_epoch == 0 {
                        return Some(false)
                    }

                    if reward_state.n_nuggets_remaining == 0 {
                        return Some(false)
                    }

                    if reward_state.epoch > NUGGET_FINAL_EPOCH {
                        return Some(false)
                    } 
                    
                    if reward_state.epoch == NUGGET_FINAL_EPOCH
                    && reward_state.n_nuggets_remaining > 1 {
                        return Some(false)
                    }
                    
                },
                None => { return Some(false) }
            }
        },
        Category::Vein(amount) => {
            match amount {
                Some(amt) => {
                    if amt < VEIN_REWARD_RANGE.0 || amt > VEIN_REWARD_RANGE.1 {
                        return Some(false)
                    }
                    if reward_state.n_veins_current_epoch == 0 {
                        return Some(false)
                    }

                    if reward_state.n_veins_remaining == 0 {
                        return Some(false)
                    }

                    if reward_state.epoch > VEIN_FINAL_EPOCH {
                        return Some(false)
                    } 
                    
                    if reward_state.epoch == VEIN_FINAL_EPOCH
                    && reward_state.n_veins_remaining > 1 {
                        return Some(false)
                    }                                        
                },
                None => { return Some(false) }
            }                                
        },
        Category::Motherlode(amount) => {
            match amount {
                Some(amt) => {
                    if amt < MOTHERLODE_REWARD_RANGE.0 || amt > MOTHERLODE_REWARD_RANGE.1 {
                        return Some(false)
                    }
                    if reward_state.n_motherlodes_current_epoch == 0 {
                        return Some(false)
                    }

                    if reward_state.n_motherlodes_remaining == 0 {
                        return Some(false)
                    }

                    if reward_state.epoch > MOTHERLODE_FINAL_EPOCH {
                        return Some(false)
                    } 
                    
                    if reward_state.epoch == MOTHERLODE_FINAL_EPOCH
                    && reward_state.n_motherlodes_remaining > 1 {
                        return Some(false)
                    }                                        
                },
                None => { return Some(false) }
            }
        },
        Category::Genesis(amount) => {
            match amount {
                Some(amt) => {
                    if amt != GENESIS_REWARD {
                        return Some(false)
                    }
                },
                None => { return Some(false) }
            }
        },
    }
    Some(true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_reward_state_starting_point() {

    }

    #[test]
    fn test_reward_state_updates_after_mined_block() {

    }

    #[test]
    fn test_restored_reward_state() {

    }

    #[test]
    fn test_reward_category_valid_amount() {

    }
    
    #[test]
    fn test_reward_category_invalid_amount() {

    }
}
