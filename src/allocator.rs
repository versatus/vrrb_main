use crate::account::AccountState;
use crate::claim::{Claim, CustodianInfo};
use crate::state::NetworkState;
use crate::wallet::WalletAccount;
use rand::{
    distributions::{Distribution, WeightedIndex},
    thread_rng,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn allocate_claims(
    claim_vec: Vec<Claim>,
    miner: Arc<Mutex<WalletAccount>>,
    network_state: Arc<Mutex<NetworkState>>,
    block_number: u128,
    account_state: Arc<Mutex<AccountState>>,
) -> HashMap<u128, Claim> {
    if block_number > 1 {
        // get all the claims owned
        let claims_owned: HashMap<u128, Claim> =
            network_state.lock().unwrap().state.get("claims").unwrap();
        let accounts_pk = account_state.lock().unwrap().accounts_pk.clone();
        let accounts: Vec<String> = accounts_pk
            .values()
            .map(|value| value.clone())
            .collect::<Vec<String>>();

        // current owner is the pubkey
        let claim_owners: Vec<String> = claims_owned
            .clone()
            .iter()
            .map(|(_, value)| value.current_owner.clone().unwrap())
            .collect();

        let mut owner_map: HashMap<String, u128> = HashMap::new();

        claim_owners.iter().for_each(|owner| {
            let counter = owner_map.entry(owner.to_owned()).or_insert(0);
            *counter += 1;
        });

        let total_claims: u128 = claims_owned.len() as u128;
        let no_claims_owned: HashMap<String, u128> = accounts
            .iter()
            .filter(|pubkey| !owner_map.contains_key(pubkey.clone()))
            .map(|pubkey| return (pubkey.clone(), 0u128))
            .collect::<HashMap<String, u128>>();

        let mut claims_owned: Vec<_> = owner_map.iter().collect();
        let no_claims_owned: Vec<_> = no_claims_owned.iter().collect();
        claims_owned.extend(no_claims_owned);

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

        let mut claims_awarded: HashMap<u128, Claim> = HashMap::new();

        for claim in claim_vec {
            let mut updated_claim = claim.clone();
            let miner_pubkey = miner.clone().lock().unwrap().pubkey.clone();
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let winner = claim_allocation_choices[dist.sample(&mut rng)].0.clone();
            let mut custodian_info: HashMap<String, Option<CustodianInfo>> = HashMap::new();

            custodian_info.insert(
                "homesteader".to_string(),
                Some(CustodianInfo::Homesteader(true)),
            );
            custodian_info.insert(
                "acquisition_timestamp".to_string(),
                Some(CustodianInfo::AcquisitionTimestamp(time)),
            );
            custodian_info.insert(
                "acquisition_price".to_string(),
                Some(CustodianInfo::AcquisitionPrice(0)),
            );
            custodian_info.insert(
                "acquired_from".to_string(),
                Some(CustodianInfo::AcquiredFrom(Some(miner_pubkey))),
            );
            custodian_info.insert(
                "owner_number".to_string(),
                Some(CustodianInfo::OwnerNumber(1)),
            );
            custodian_info.insert(
                "buyer_signature".to_string(),
                Some(CustodianInfo::BuyerSignature(None)),
            );
            custodian_info.insert(
                "seller_signature".to_string(),
                Some(CustodianInfo::SellerSignature(None)),
            );

            updated_claim
                .chain_of_custody
                .insert(winner.clone(), custodian_info);

            let serialized_chain_of_custody =
                serde_json::to_string(&updated_claim.chain_of_custody).unwrap();
            let payload = format!(
                "{},{},{},{}",
                &claim.maturation_time.to_string(),
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
                    "seller_signature".to_string(),
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
        let mut claims_awarded: HashMap<u128, Claim> = HashMap::new();

        for claim in claim_vec {
            let mut updated_claim = claim.clone();
            let miner_pubkey = miner.clone().lock().unwrap().pubkey.clone();
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let winner = miner_pubkey.clone();
            let mut custodian_info: HashMap<String, Option<CustodianInfo>> = HashMap::new();

            custodian_info.insert(
                "homesteader".to_string(),
                Some(CustodianInfo::Homesteader(true)),
            );
            custodian_info.insert(
                "acquisition_timestamp".to_string(),
                Some(CustodianInfo::AcquisitionTimestamp(time)),
            );
            custodian_info.insert(
                "acquisition_price".to_string(),
                Some(CustodianInfo::AcquisitionPrice(0)),
            );
            custodian_info.insert(
                "acquired_from".to_string(),
                Some(CustodianInfo::AcquiredFrom(Some(miner_pubkey.clone()))),
            );
            custodian_info.insert(
                "owner_number".to_string(),
                Some(CustodianInfo::OwnerNumber(1)),
            );
            custodian_info.insert(
                "buyer_signature".to_string(),
                Some(CustodianInfo::BuyerSignature(None)),
            );
            custodian_info.insert(
                "seller_signature".to_string(),
                Some(CustodianInfo::SellerSignature(None)),
            );

            updated_claim
                .chain_of_custody
                .insert(winner.clone(), custodian_info);

            let serialized_chain_of_custody =
                serde_json::to_string(&updated_claim.chain_of_custody).unwrap();
            let payload = format!(
                "{},{},{},{}",
                &claim.maturation_time.to_string(),
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
                    "seller_signature".to_string(),
                    Some(CustodianInfo::SellerSignature(Some(miner_signature))),
                );
            updated_claim.current_owner = Some(winner.clone());
            claims_awarded.insert(updated_claim.claim_number, updated_claim.to_owned());
        }
        claims_awarded
    }
}
