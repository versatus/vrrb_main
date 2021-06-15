use rand::{distributions::{Distribution, WeightedIndex}, thread_rng};

pub struct Arbiter {
    pub addresses: Vec<String>,
    pub coin_flips: Vec<(String, u8)>,
    pub winner: Option<(String, u8)>,
}

impl Arbiter {
    pub fn new(addresses: Vec<String>) -> Arbiter {

        Arbiter {
            addresses,
            coin_flips: vec![],
            winner: None,
        }
    }

    pub fn tie_handler(&mut self) {
        self.coin_flips = Arbiter::flip_coin(self.addresses.clone());

        for (address, flip_value) in self.clone().coin_flips.iter() {
            if self.winner.is_some() && self.winner.clone().unwrap().1 < *flip_value {
                self.winner = Some((address.to_owned(), flip_value.to_owned()));
            } else if self.winner.is_some() && self.winner.clone().unwrap().1 == *flip_value {
                self.tie_handler();
            } else if self.winner.is_none() {
                self.winner = Some((address.to_owned(), flip_value.to_owned()))
            } else {
                continue
            }
        }
    }

    pub fn flip_coin(addresses: Vec<String>) -> Vec<(String, u8)> {
        let choices = [0u8, 1u8];
        let weights = [1, 1];

        let dist = WeightedIndex::new(&weights).unwrap();
        let mut rng = thread_rng();
        let mut flips = vec![];

        for address in addresses.iter() {
            flips.push((address.to_owned(), choices[dist.sample(&mut rng)]))
        }

        flips
    }
}

impl Clone for Arbiter {
    fn clone(&self) -> Self {

        Self {
            addresses: self.addresses.clone(),
            coin_flips: self.coin_flips.clone(),
            winner: self.winner.clone(),
        }

    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_winner() {
        let addresses = vec!["homesteader1".to_string(), "homesteader2".to_string()];
        let mut arbiter = Arbiter::new(addresses);
        arbiter.tie_handler();
        println!("{:?}", arbiter.winner);

        assert_eq!(1u8, arbiter.winner.unwrap().1);
    }

    #[test]
    fn test_loser() {
        let mut addresses = vec!["homesteader1".to_string(), "homesteader2".to_string()];
        let mut arbiter = Arbiter::new(addresses.clone());
        arbiter.tie_handler();
        println!("{:?}", arbiter.winner);
        addresses.remove(addresses.clone().iter().position(|x| *x == arbiter.clone().winner.unwrap().0).unwrap());
        assert_ne!(addresses[0], arbiter.winner.unwrap().0);
    }
}