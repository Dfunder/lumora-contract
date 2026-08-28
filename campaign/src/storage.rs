//! Centralized campaign storage access.
//!
//! Persistent storage is used for campaign identity, totals, admin, and status
//! keys because those values must survive ledger TTL changes for the life of a
//! campaign and may be needed after the funding window closes. The higher rent
//! is worthwhile for durable state: `CampaignData`, `TotalRaised`,
//! `RaisedPerAsset`, `Admin`, and `ContractStatus`.
//!
//! Temporary storage is used for execution-scoped values that later flows can
//! recreate, refresh, or safely let expire. This keeps rent lower for high-cardinality
//! or short-lived state: `MilestoneData`, `DonorData`, `Locked`, and `Frozen`.

use soroban_sdk::{Address, Env};

use crate::{CampaignData, DataKey, DonorRecord, MilestoneData};
use common::{AssetInfo, CampaignStatus};

pub fn has_campaign_data(env: &Env) -> bool {
    env.storage().persistent().has(&DataKey::CampaignData)
}

pub fn set_campaign_data(env: &Env, campaign_data: &CampaignData) {
    env.storage()
        .persistent()
        .set(&DataKey::CampaignData, campaign_data);
}

pub fn get_campaign_data(env: &Env) -> Option<CampaignData> {
    env.storage().persistent().get(&DataKey::CampaignData)
}

pub fn set_total_raised(env: &Env, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::TotalRaised, &amount);
}

pub fn get_total_raised(env: &Env) -> Option<i128> {
    env.storage().persistent().get(&DataKey::TotalRaised)
}

pub fn set_raised_per_asset(env: &Env, asset: AssetInfo, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::RaisedPerAsset(asset), &amount);
}

pub fn get_raised_per_asset(env: &Env, asset: AssetInfo) -> Option<i128> {
    env.storage()
        .persistent()
        .get(&DataKey::RaisedPerAsset(asset))
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().persistent().set(&DataKey::Admin, admin);
}

pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&DataKey::Admin)
}

pub fn set_contract_status(env: &Env, status: CampaignStatus) {
    env.storage()
        .persistent()
        .set(&DataKey::ContractStatus, &status);
}

pub fn get_contract_status(env: &Env) -> Option<CampaignStatus> {
    env.storage().persistent().get(&DataKey::ContractStatus)
}

/// Sets the locked state to prevent concurrent modifications.
/// Used to guard against re-entrant calls and concurrent state modifications.
pub fn set_locked(env: &Env, locked: bool) {
    env.storage().temporary().set(&DataKey::Locked, &locked);
}

/// Checks if the contract is locked (unable to accept concurrent modifications).
/// Returns false if the lock state has not been set (default state).
pub fn is_locked(env: &Env) -> bool {
    env.storage()
        .temporary()
        .get(&DataKey::Locked)
        .unwrap_or(false)
}

/// Acquires the contract lock for exclusive operation execution.
/// This prevents concurrent modifications and re-entrant calls.
/// Must be paired with a corresponding call to `release_lock()`.
pub fn acquire_lock(env: &Env) -> Result<(), crate::Error> {
    if is_locked(env) {
        return Err(crate::Error::Reentrant);
    }
    set_locked(env, true);
    Ok(())
}

/// Releases the contract lock to allow subsequent operations.
pub fn release_lock(env: &Env) {
    set_locked(env, false);
}

/// Sets the frozen state to prevent state transitions.
/// A frozen contract cannot accept new operations or state modifications.
pub fn set_frozen(env: &Env, frozen: bool) {
    env.storage().temporary().set(&DataKey::Frozen, &frozen);
}

/// Checks if the contract is frozen (unable to accept modifications).
/// Returns false if the frozen state has not been set (default state).
pub fn is_frozen(env: &Env) -> bool {
    env.storage()
        .temporary()
        .get(&DataKey::Frozen)
        .unwrap_or(false)
}

pub fn milestone_key(index: u32) -> DataKey {
    DataKey::MilestoneData(index)
}

pub fn donor_key(donor: Address) -> DataKey {
    DataKey::DonorData(donor)
}

pub fn set_milestone_data(env: &Env, index: u32, milestone: &MilestoneData) {
    env.storage()
        .temporary()
        .set(&milestone_key(index), milestone);
}

pub fn get_milestone_data(env: &Env, index: u32) -> Option<MilestoneData> {
    env.storage().temporary().get(&milestone_key(index))
}

pub fn set_donor_data(env: &Env, donor: &Address, data: &DonorRecord) {
    env.storage()
        .temporary()
        .set(&donor_key(donor.clone()), data);
}

pub fn get_donor_data(env: &Env, donor: &Address) -> Option<DonorRecord> {
    env.storage().temporary().get(&donor_key(donor.clone()))
}

pub fn set_xlm_token(env: &Env, address: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::XlmTokenAddress, address);
}

pub fn get_xlm_token(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&DataKey::XlmTokenAddress)
}

pub fn set_min_donation_amount(env: &Env, amount: &i128) {
    env.storage()
        .persistent()
        .set(&DataKey::MinDonationAmount, amount);
}

pub fn get_min_donation_amount(env: &Env) -> Option<i128> {
    env.storage().persistent().get(&DataKey::MinDonationAmount)
}

pub fn set_campaign_end_time(env: &Env, timestamp: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::CampaignEndTime, &timestamp);
}

pub fn get_campaign_end_time(env: &Env) -> Option<u64> {
    env.storage().persistent().get(&DataKey::CampaignEndTime)
}

pub fn set_original_end_time(env: &Env, timestamp: &u64) {
    env.storage()
        .persistent()
        .set(&DataKey::OriginalEndTime, timestamp);
}

pub fn get_original_end_time(env: &Env) -> Option<u64> {
    env.storage().persistent().get(&DataKey::OriginalEndTime)
}

// ═══════════════════════════════════════════════════════════════════════════
// Storage Unit Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CampaignData, DonorRecord, MilestoneData};
    use common::{CampaignStatus, MilestoneStatus};
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{BytesN, Vec as SorobanVec};

    #[test]
    fn test_campaign_data_roundtrip() {
        let env = Env::default();
        let data = CampaignData {
            creator: Address::generate(&env),
            goal_amount: 50_000,
            raised_amount: 12_000,
            released_amount: 5_000,
            end_time: 999_999,
            status: CampaignStatus::Active,
            accepted_assets: SorobanVec::new(&env),
            milestone_count: 3,
            next_releasable_milestone: 1,
        };
        set_campaign_data(&env, &data);
        let loaded = get_campaign_data(&env).unwrap();
        assert_eq!(loaded.creator, data.creator);
        assert_eq!(loaded.goal_amount, data.goal_amount);
        assert_eq!(loaded.raised_amount, data.raised_amount);
        assert_eq!(loaded.released_amount, data.released_amount);
        assert_eq!(loaded.end_time, data.end_time);
        assert_eq!(loaded.status, data.status);
        assert_eq!(loaded.milestone_count, data.milestone_count);
        assert_eq!(
            loaded.next_releasable_milestone,
            data.next_releasable_milestone
        );
    }

    #[test]
    fn test_has_campaign_data_before_and_after_set() {
        let env = Env::default();
        assert!(!has_campaign_data(&env));

        let data = CampaignData {
            creator: Address::generate(&env),
            goal_amount: 100,
            raised_amount: 0,
            released_amount: 0,
            end_time: 1000,
            status: CampaignStatus::Active,
            accepted_assets: SorobanVec::new(&env),
            milestone_count: 0,
            next_releasable_milestone: 0,
        };
        set_campaign_data(&env, &data);
        assert!(has_campaign_data(&env));
    }

    #[test]
    fn test_milestone_data_roundtrip() {
        let env = Env::default();
        let milestone = MilestoneData {
            index: 2,
            target_amount: 7_500,
            description_hash: BytesN::from_array(&env, &[42u8; 32]),
            status: MilestoneStatus::Unlocked,
            released_at: None,
            release_tx: BytesN::from_array(&env, &[0u8; 32]),
        };
        set_milestone_data(&env, 2, &milestone);
        let loaded = get_milestone_data(&env, 2).unwrap();
        assert_eq!(loaded.index, 2);
        assert_eq!(loaded.target_amount, 7_500);
        assert_eq!(loaded.status, MilestoneStatus::Unlocked);
        assert_eq!(loaded.released_at, None);
    }

    #[test]
    fn test_milestone_data_not_found() {
        let env = Env::default();
        assert!(get_milestone_data(&env, 0).is_none());
        assert!(get_milestone_data(&env, 999).is_none());
    }

    #[test]
    fn test_milestone_data_multiple_indices() {
        let env = Env::default();
        for idx in 0..5u32 {
            let milestone = MilestoneData {
                index: idx,
                target_amount: (idx as i128 + 1) * 1_000,
                description_hash: BytesN::from_array(&env, &[idx as u8; 32]),
                status: MilestoneStatus::Locked,
                released_at: None,
                release_tx: BytesN::from_array(&env, &[0u8; 32]),
            };
            set_milestone_data(&env, idx, &milestone);
        }
        for idx in 0..5u32 {
            let loaded = get_milestone_data(&env, idx).unwrap();
            assert_eq!(loaded.index, idx);
            assert_eq!(loaded.target_amount, (idx as i128 + 1) * 1_000);
        }
    }

    #[test]
    fn test_donor_data_roundtrip() {
        let env = Env::default();
        let donor = Address::generate(&env);
        let record = DonorRecord {
            donor: donor.clone(),
            total_donated: 3_333,
            per_asset: SorobanVec::new(&env),
            last_donation_time: 12345,
        };
        set_donor_data(&env, &donor, &record);
        let loaded = get_donor_data(&env, &donor).unwrap();
        assert_eq!(loaded.total_donated, 3_333);
        assert_eq!(loaded.last_donation_time, 12345);
    }

    #[test]
    fn test_donor_data_not_found() {
        let env = Env::default();
        let donor = Address::generate(&env);
        assert!(get_donor_data(&env, &donor).is_none());
    }

    #[test]
    fn test_donor_data_independent_per_address() {
        let env = Env::default();
        let donor_a = Address::generate(&env);
        let donor_b = Address::generate(&env);

        let record_a = DonorRecord {
            donor: donor_a.clone(),
            total_donated: 1_000,
            per_asset: SorobanVec::new(&env),
            last_donation_time: 100,
        };
        let record_b = DonorRecord {
            donor: donor_b.clone(),
            total_donated: 2_000,
            per_asset: SorobanVec::new(&env),
            last_donation_time: 200,
        };
        set_donor_data(&env, &donor_a, &record_a);
        set_donor_data(&env, &donor_b, &record_b);

        assert_eq!(get_donor_data(&env, &donor_a).unwrap().total_donated, 1_000);
        assert_eq!(get_donor_data(&env, &donor_b).unwrap().total_donated, 2_000);
    }

    #[test]
    fn test_total_raised_roundtrip() {
        let env = Env::default();
        assert!(get_total_raised(&env).is_none());
        set_total_raised(&env, 42_000);
        assert_eq!(get_total_raised(&env), Some(42_000));
        set_total_raised(&env, 0);
        assert_eq!(get_total_raised(&env), Some(0));
    }

    #[test]
    fn test_raised_per_asset_roundtrip() {
        let env = Env::default();
        let asset_native = AssetInfo::Native;
        let asset_token = AssetInfo::Token(Address::generate(&env));

        assert!(get_raised_per_asset(&env, asset_native.clone()).is_none());

        set_raised_per_asset(&env, asset_native.clone(), 5_000);
        set_raised_per_asset(&env, asset_token.clone(), 3_000);

        assert_eq!(get_raised_per_asset(&env, asset_native), Some(5_000));
        assert_eq!(get_raised_per_asset(&env, asset_token), Some(3_000));
    }

    #[test]
    fn test_admin_roundtrip() {
        let env = Env::default();
        assert!(get_admin(&env).is_none());
        let admin = Address::generate(&env);
        set_admin(&env, &admin);
        assert_eq!(get_admin(&env), Some(admin));
    }

    #[test]
    fn test_contract_status_roundtrip() {
        let env = Env::default();
        assert!(get_contract_status(&env).is_none());
        set_contract_status(&env, CampaignStatus::Active);
        assert_eq!(get_contract_status(&env), Some(CampaignStatus::Active));
        set_contract_status(&env, CampaignStatus::Failed);
        assert_eq!(get_contract_status(&env), Some(CampaignStatus::Failed));
    }

    #[test]
    fn test_lock_acquire_and_release() {
        let env = Env::default();
        assert!(!is_locked(&env));

        let result = acquire_lock(&env);
        assert!(result.is_ok());
        assert!(is_locked(&env));

        // Double acquire must fail
        let result = acquire_lock(&env);
        assert_eq!(result, Err(crate::Error::Reentrant));

        release_lock(&env);
        assert!(!is_locked(&env));

        // Can acquire again after release
        let result = acquire_lock(&env);
        assert!(result.is_ok());
        release_lock(&env);
    }

    #[test]
    fn test_frozen_state() {
        let env = Env::default();
        assert!(!is_frozen(&env));

        set_frozen(&env, true);
        assert!(is_frozen(&env));

        set_frozen(&env, false);
        assert!(!is_frozen(&env));
    }

    #[test]
    fn test_xlm_token_roundtrip() {
        let env = Env::default();
        assert!(get_xlm_token(&env).is_none());
        let token = Address::generate(&env);
        set_xlm_token(&env, &token);
        assert_eq!(get_xlm_token(&env), Some(token));
    }

    #[test]
    fn test_min_donation_amount_roundtrip() {
        let env = Env::default();
        assert!(get_min_donation_amount(&env).is_none());
        set_min_donation_amount(&env, &500);
        assert_eq!(get_min_donation_amount(&env), Some(500));
        set_min_donation_amount(&env, &0);
        assert_eq!(get_min_donation_amount(&env), Some(0));
    }

    #[test]
    fn test_campaign_end_time_roundtrip() {
        let env = Env::default();
        assert!(get_campaign_end_time(&env).is_none());
        set_campaign_end_time(&env, 1_234_567);
        assert_eq!(get_campaign_end_time(&env), Some(1_234_567));
    }

    #[test]
    fn test_original_end_time_roundtrip() {
        let env = Env::default();
        assert!(get_original_end_time(&env).is_none());
        set_original_end_time(&env, &9_876_543);
        assert_eq!(get_original_end_time(&env), Some(9_876_543));
    }

    #[test]
    fn test_lock_and_frozen_are_independent() {
        let env = Env::default();
        // Setting frozen should not affect lock state
        set_frozen(&env, true);
        assert!(!is_locked(&env));
        acquire_lock(&env).unwrap();
        assert!(is_frozen(&env));
        assert!(is_locked(&env));

        release_lock(&env);
        assert!(is_frozen(&env));
        assert!(!is_locked(&env));

        set_frozen(&env, false);
        assert!(!is_frozen(&env));
    }
}
