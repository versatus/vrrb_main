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
use ritelinked::LinkedHashMap;
#[derive(Debug, Clone)]
pub struct BallotBox {
    pub proposals: LinkedHashMap<String, LinkedHashMap<String, u128>>,
    pub proposal_results: LinkedHashMap<String, bool>,
}

impl BallotBox {
    pub fn new(
        proposals: LinkedHashMap<String, LinkedHashMap<String, u128>>,
        proposal_results: LinkedHashMap<String, bool>,
    ) -> BallotBox {
        BallotBox {
            proposals,
            proposal_results,
        }
    }

    pub fn tally_proposal_vote(&mut self, proposal_id: String, vote: bool) {
        match vote {
            true => {
                *self
                    .proposals
                    .get_mut(&proposal_id)
                    .unwrap()
                    .get_mut("yes")
                    .unwrap() += 1;
            }
            false => {
                *self
                    .proposals
                    .get_mut(&proposal_id)
                    .unwrap()
                    .get_mut("no")
                    .unwrap() += 1;
            }
        }
    }

    pub fn proposal_vote_result(&mut self, proposal_id: String) {
        let proposal_votes = self.proposals.get_mut(&proposal_id).unwrap();
        let vote_total =
            *proposal_votes.get_mut("yes").unwrap() + *proposal_votes.get_mut("no").unwrap();

        let result = self.proposals[&proposal_id]["yes"] as f64 / vote_total as f64 >= 0.6;
        self.proposal_results.insert(proposal_id, result);
    }
}
