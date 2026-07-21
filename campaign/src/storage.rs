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

use crate::{AssetInfo, CampaignData, ContractStatus, DataKey, DonorRecord, MilestoneData};

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

pub fn set_contract_status(env: &Env, status: ContractStatus) {
    env.storage()
        .persistent()
        .set(&DataKey::ContractStatus, &status);
}

pub fn get_contract_status(env: &Env) -> Option<ContractStatus> {
    env.storage().persistent().get(&DataKey::ContractStatus)
}

pub fn set_locked(env: &Env, locked: bool) {
    env.storage().temporary().set(&DataKey::Locked, &locked);
}

pub fn is_locked(env: &Env) -> bool {
    env.storage()
        .temporary()
        .get(&DataKey::Locked)
        .unwrap_or(false)
}

pub fn set_frozen(env: &Env, frozen: bool) {
    env.storage().temporary().set(&DataKey::Frozen, &frozen);
}

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
