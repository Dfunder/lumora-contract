#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Unauthorized = 1,
    AlreadyInitialized = 2,
    InvalidGoalAmount = 3,
    InvalidEndTime = 4,
    NoAcceptedAssets = 5,
    InvalidMilestones = 6,
    InvalidAmount = 7,
    NotAcceptedAsset = 8,
    CampaignNotActive = 9,
    CampaignEnded = 10,
}

#[contracttype]
pub enum DataKey {
    CampaignData,
    MilestoneData(u32),
    DonorData(Address),
    TotalRaised,
    ContractStatus,
    RaisedPerAsset,
    Locked,
    Admin,
    Frozen,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CampaignStatus {
    Active,
    GoalReached,
    Ended,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MilestoneStatus {
    Locked,
    Unlocked,
    Released,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetInfo {
    pub asset_code: String,
    pub issuer: Option<Address>,
}

impl AssetInfo {
    pub fn is_xlm(&self) -> bool {
        self.asset_code == String::from_str("XLM")
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignData {
    pub creator: Address,
    pub goal_amount: i128,
    pub raised_amount: i128,
    pub end_time: u64,
    pub status: CampaignStatus,
    pub accepted_assets: Vec<AssetInfo>,
    pub milestone_count: u32,
}

pub mod storage {
    use super::*;

    pub fn has_campaign_data(env: &Env) -> bool {
        env.storage().instance().has(&DataKey::CampaignData)
    }

    pub fn get_campaign_data(env: &Env) -> CampaignData {
        env.storage()
            .instance()
            .get(&DataKey::CampaignData)
            .expect("campaign not initialized")
    }

    pub fn set_campaign_data(env: &Env, data: &CampaignData) {
        env.storage().instance().set(&DataKey::CampaignData, data);
    }
}

#[contract]
pub struct CampaignContract;

#[contractimpl]
impl CampaignContract {
    pub fn get_campaign_info(env: Env) -> CampaignData {
        storage::get_campaign_data(&env)
    }

    pub fn require_creator(env: Env) {
        let data = storage::get_campaign_data(&env);
        data.creator.require_auth();
    }
}
