// Track votes for a given vrrb improvement proposal
// Track votes for the network state hash at a given block height
// Track number of active nodes at a given time.
// When yes votes reaches 60% of nodes or no votes reaches 40% of nodes
// approve/deny proposal, or if the proposal expires calculate percentage
// of casted votes at expiration if 60% of votes are yes, approve 
// if not deny.
//
// For block proposals, when the next block is proposed calculate % of votes
// if it's 60% of casted votes, approve the block and confirm the state at the
// current block height. If not, require the proposing node to update their
// local state before reproposing the block.
use std::collections::HashMap;
use crate::txn::Txn;

pub struct BallotBox {
    // A hashmap containing the proposal ID as the key and a hashmap
    // with yes/no as the keys and the vote count as the value.
    // May need a timestamp for expiration.
    pub proposals: HashMap<String, HashMap<String, u128>>,
    // A hashmap containing the block height as the key, and a tuple containing
    // the state hash, a hashmap of votes, and a vector of transactions as the value
    pub state_hash: HashMap<u128, (String, HashMap<String, u128>, HashMap<String, Txn>)>,
    // May need a queue to communicate with other processes.
    pub node_count: u128,
    pub proposal_results: HashMap<String, bool>,
    pub state_hash_results: HashMap<u128, bool>,
}

impl BallotBox {
    pub fn new(
        proposals: HashMap<String, HashMap<String, u128>>,
        state_hash: HashMap<u128, (String, HashMap<String, u128>, HashMap<String, Txn>)>,
        node_count: u128,
        proposal_results: HashMap<String, bool>,
        state_hash_results: HashMap<u128, bool>,
    ) -> BallotBox 
    {
        BallotBox { proposals, state_hash, node_count, proposal_results, state_hash_results, }
    }

    pub fn tally_proposal_vote(&mut self, proposal_id: String, vote: bool) {

        match vote {
            true => { *self.proposals.get_mut(&proposal_id).unwrap().get_mut("yes").unwrap() += 1; },
            false => { *self.proposals.get_mut(&proposal_id).unwrap().get_mut("no").unwrap() += 1; }
        }
    }

    pub fn tally_state_hash_vote(&mut self, block_height: u128, vote: bool) {

        match vote {
            true => { *self.state_hash.get_mut(&block_height).unwrap().1.get_mut("yes").unwrap() +=1; },
            false => { *self.state_hash.get_mut(&block_height).unwrap().1.get_mut("no").unwrap() +=1; }
        }
    }

    pub fn proposal_vote_result(&mut self, proposal_id: String) {
        let proposal_votes = self.proposals.get_mut(&proposal_id).unwrap();
        let vote_total = *proposal_votes.get_mut("yes").unwrap() + *proposal_votes.get_mut("no").unwrap();

        let result = self.proposals[&proposal_id]["yes"] as f64 / vote_total as f64 >= 0.6;
        self.proposal_results.insert(proposal_id, result);
    }

    pub fn state_hash_vote_result(&mut self, block_height: u128) {
        let state_hash_votes = self.state_hash.get_mut(&block_height).unwrap();
        let vote_total = *state_hash_votes.1.get_mut("yes").unwrap() + *state_hash_votes.1.get_mut("no").unwrap();

        let result = self.state_hash[&block_height].1["yes"] as f64 / vote_total as f64 >= 0.6;
        self.state_hash_results.insert(block_height, result);
    }
}