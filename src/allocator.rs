use crate::account::AccountState;
use crate::claim::{Claim, CustodianInfo, CustodianOption};
use crate::wallet::WalletAccount;
use log::info;
use rand::{
    distributions::{Distribution, WeightedIndex},
    thread_rng,
};
use ritelinked::LinkedHashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn allocate_claims(
    claim_vec: Vec<Claim>,
    miner: Arc<Mutex<WalletAccount>>,
    block_number: u128,
    account_state: Arc<Mutex<AccountState>>,
) -> LinkedHashMap<u128, Claim> {
    if block_number > 1 {
        // get all the claims owned
        let owner_map = account_state.lock().unwrap().claim_counter.clone();
        let total_claims: u128 = account_state.lock().unwrap().claim_pool.confirmed.clone().len() as u128;
        
        //ISSUE: O(n) operation
        let claims_owned: Vec<_> = owner_map.iter().collect();

        //ISSUE: O(n) operation
        let mut claim_allocation_choices: Vec<(String, f64)> = claims_owned
            .iter()
            .map(|(account, claims_owned)| {
                let claims_owned = if *claims_owned.clone() == 0u128 {
                    1u128
                } else {
                    *claims_owned.clone()
                };
                let weight = 1.0 / (claims_owned as f64 + 1.0 / total_claims as f64);
                return (account.to_string(), weight);
            })
            .collect();

        let dist = WeightedIndex::new(claim_allocation_choices.iter().map(|item| item.1)).unwrap();
        let mut rng = thread_rng();

        let mut claims_awarded: LinkedHashMap<u128, Claim> = LinkedHashMap::new();

        for claim in claim_vec {
            let mut updated_claim = claim.clone();
            let miner_pubkey = miner.clone().lock().unwrap().pubkey.clone();
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();

            let winner = claim_allocation_choices[dist.sample(&mut rng)].0.clone();
            
            if let Some(entry) = account_state.lock().unwrap().claim_counter.get_mut(&winner) {
                *entry += 1
            } else {
                account_state.lock().unwrap().claim_counter.insert(winner.clone(), 1);
            }

            info!(target: "claim_winner", "{:?}", winner);
            let mut custodian_info: LinkedHashMap<CustodianOption, Option<CustodianInfo>> =
                LinkedHashMap::new();

            custodian_info.insert(
                CustodianOption::Homesteader,
                Some(CustodianInfo::Homesteader(true)),
            );
            custodian_info.insert(
                CustodianOption::AcquisitionTime,
                Some(CustodianInfo::AcquisitionTimestamp(time)),
            );
            custodian_info.insert(
                CustodianOption::Price,
                Some(CustodianInfo::AcquisitionPrice(0)),
            );
            custodian_info.insert(
                CustodianOption::Seller,
                Some(CustodianInfo::AcquiredFrom(Some(miner_pubkey))),
            );
            custodian_info.insert(
                CustodianOption::OwnerNumber,
                Some(CustodianInfo::OwnerNumber(1)),
            );
            custodian_info.insert(
                CustodianOption::BuyerSignature,
                Some(CustodianInfo::BuyerSignature(None)),
            );
            custodian_info.insert(
                CustodianOption::SellerSignature,
                Some(CustodianInfo::SellerSignature(None)),
            );

            updated_claim
                .chain_of_custody
                .insert(winner.clone(), custodian_info);

            let serialized_chain_of_custody =
                serde_json::to_string(&updated_claim.chain_of_custody).unwrap();
            let payload = format!(
                "{},{},{},{}",
                &claim.expiration_time.to_string(),
                &claim.price.to_string(),
                &claim.available.to_string(),
                &serialized_chain_of_custody
            );

            updated_claim.claim_payload = Some(payload.clone());
            let miner_signature = miner.lock().unwrap().sign(&payload).unwrap().to_string();
            updated_claim
                .chain_of_custody
                .get_mut(&winner.clone())
                .unwrap()
                .insert(
                    CustodianOption::SellerSignature,
                    Some(CustodianInfo::SellerSignature(Some(miner_signature))),
                );
            updated_claim.current_owner = Some(winner.clone());

            claims_awarded.insert(updated_claim.claim_number, updated_claim.to_owned());

            if claim_allocation_choices.len() > 20 {
                claim_allocation_choices.retain(|(account, _weight)| *account != winner);
            }
        }

        claims_awarded
    } else {
        let mut claims_awarded: LinkedHashMap<u128, Claim> = LinkedHashMap::new();
        let miner_pubkey = miner.clone().lock().unwrap().pubkey.clone();
        let mut claim_counter = account_state.lock().unwrap().claim_counter.clone();

        for claim in claim_vec {
            let mut updated_claim = claim.clone();        
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let winner = miner_pubkey.clone();
            if let Some(entry) = claim_counter.get_mut(&miner_pubkey.clone()) {
                *entry += 1
            };
            let mut custodian_info: LinkedHashMap<CustodianOption, Option<CustodianInfo>> =
                LinkedHashMap::new();

            custodian_info.insert(
                CustodianOption::Homesteader,
                Some(CustodianInfo::Homesteader(true)),
            );
            custodian_info.insert(
                CustodianOption::AcquisitionTime,
                Some(CustodianInfo::AcquisitionTimestamp(time)),
            );
            custodian_info.insert(
                CustodianOption::Price,
                Some(CustodianInfo::AcquisitionPrice(0)),
            );
            custodian_info.insert(
                CustodianOption::Seller,
                Some(CustodianInfo::AcquiredFrom(Some(miner_pubkey.clone()))),
            );
            custodian_info.insert(
                CustodianOption::OwnerNumber,
                Some(CustodianInfo::OwnerNumber(1)),
            );
            custodian_info.insert(
                CustodianOption::BuyerSignature,
                Some(CustodianInfo::BuyerSignature(None)),
            );
            custodian_info.insert(
                CustodianOption::SellerSignature,
                Some(CustodianInfo::SellerSignature(None)),
            );

            updated_claim
                .chain_of_custody
                .insert(winner.clone(), custodian_info);

            let serialized_chain_of_custody =
                serde_json::to_string(&updated_claim.chain_of_custody).unwrap();
            let payload = format!(
                "{},{},{},{}",
                &claim.expiration_time.to_string(),
                &claim.price.to_string(),
                &claim.available.to_string(),
                &serialized_chain_of_custody
            );

            updated_claim.claim_payload = Some(payload.clone());

            let miner_signature = miner
                .lock()
                .unwrap()
                .sign(&payload.clone())
                .unwrap()
                .to_string();
            updated_claim
                .chain_of_custody
                .get_mut(&winner.clone())
                .unwrap()
                .insert(
                    CustodianOption::SellerSignature,
                    Some(CustodianInfo::SellerSignature(Some(miner_signature))),
                );
            updated_claim.current_owner = Some(winner.clone());
            claims_awarded.insert(updated_claim.claim_number, updated_claim.to_owned());
        }
        account_state.lock().unwrap().claim_counter = claim_counter;
        claims_awarded
    }
}
