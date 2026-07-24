#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    Symbol, Vec,
};

pub mod storage;

const MAX_MILESTONES: u32 = 5;

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
    CampaignCancelled = 11,
    DonationFailed = 12,
    MilestoneNotFound = 13,
    DonationTooSmall = 14,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    CampaignData,
    MilestoneData(u32),
    DonorData(Address),
    TotalRaised,
    ContractStatus,
    RaisedPerAsset(AssetInfo),
    Locked,
    Admin,
    Frozen,
    XlmTokenAddress,
    MinDonationAmount,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CampaignStatus {
    Active,
    Successful,
    Failed,
    GoalReached,
    Ended,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractStatus {
    Active,
    Paused,
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
pub enum AssetInfo {
    Native,
    Token(Address),
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

/// Creator-supplied milestone parameters accepted by `initialize`.
///
/// This intentionally excludes the runtime fields (`status`, `released_at`,
/// `release_tx`) that only exist once a milestone has been recorded on-chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneInput {
    pub target_amount: i128,
    pub description_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneData {
    pub index: u32,
    pub target_amount: i128,
    pub description_hash: BytesN<32>,
    pub status: MilestoneStatus,
    pub released_at: Option<u64>,
    /// Hash of the release transaction, or all-zero bytes if the milestone
    /// has not been released yet. `soroban-sdk` 20.x cannot derive an XDR
    /// `ScVal` conversion for `Option<BytesN<N>>`, so a sentinel is used
    /// instead of `Option`.
    pub release_tx: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerAssetBreakdown {
    pub asset: AssetInfo,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DonorRecord {
    pub donor: Address,
    pub total_donated: i128,
    pub per_asset: Vec<PerAssetBreakdown>,
    pub last_donation_time: u64,
}

fn expect_campaign_data(env: &Env) -> CampaignData {
    storage::get_campaign_data(env).expect("campaign not initialized")
}

fn get_token_address(env: &Env, asset: &AssetInfo) -> Address {
    match asset {
        AssetInfo::Native => storage::get_xlm_token(env).expect("XLM token address not set"),
        AssetInfo::Token(address) => address.clone(),
    }
}

fn is_accepted_asset(accepted: &Vec<AssetInfo>, target: &AssetInfo) -> bool {
    for i in 0..accepted.len() {
        if accepted.get_unchecked(i) == *target {
            return true;
        }
    }
    false
}

#[contract]
pub struct CampaignContract;

#[contractimpl]
impl CampaignContract {
    /// Deploys and initializes a campaign. Callable exactly once.
    pub fn initialize(
        env: Env,
        creator: Address,
        goal_amount: i128,
        end_time: u64,
        accepted_assets: Vec<AssetInfo>,
        milestones: Vec<MilestoneInput>,
    ) -> Result<(), Error> {
        creator.require_auth();

        if storage::has_campaign_data(&env) {
            return Err(Error::AlreadyInitialized);
        }

        if goal_amount <= 0 {
            return Err(Error::InvalidGoalAmount);
        }

        if end_time <= env.ledger().timestamp() {
            return Err(Error::InvalidEndTime);
        }

        if accepted_assets.is_empty() {
            return Err(Error::NoAcceptedAssets);
        }

        let milestone_count = milestones.len();
        if milestone_count == 0 || milestone_count > MAX_MILESTONES {
            return Err(Error::InvalidMilestones);
        }

        let mut previous_amount: i128 = 0;
        for i in 0..milestone_count {
            let milestone = milestones.get_unchecked(i);
            if milestone.target_amount <= previous_amount {
                return Err(Error::InvalidMilestones);
            }
            previous_amount = milestone.target_amount;
        }

        if milestones.get_unchecked(milestone_count - 1).target_amount != goal_amount {
            return Err(Error::InvalidMilestones);
        }

        let campaign_data = CampaignData {
            creator: creator.clone(),
            goal_amount,
            raised_amount: 0,
            end_time,
            status: CampaignStatus::Active,
            accepted_assets: accepted_assets.clone(),
            milestone_count,
        };
        storage::set_campaign_data(&env, &campaign_data);

        for i in 0..milestone_count {
            let input = milestones.get_unchecked(i);
            let milestone_data = MilestoneData {
                index: i,
                target_amount: input.target_amount,
                description_hash: input.description_hash.clone(),
                status: MilestoneStatus::Locked,
                released_at: None,
                release_tx: BytesN::from_array(&env, &[0u8; 32]),
            };
            storage::set_milestone_data(&env, i, &milestone_data);
        }

        env.events().publish(
            (Symbol::new(&env, "campaign_initialized"), creator),
            (goal_amount, end_time, accepted_assets, milestones),
        );

        Ok(())
    }

    pub fn get_campaign_info(env: Env) -> CampaignData {
        expect_campaign_data(&env)
    }

    pub fn require_creator(env: Env) {
        let data = expect_campaign_data(&env);
        data.creator.require_auth();
    }

    pub fn get_milestone(env: Env, index: u32) -> MilestoneData {
        let data = expect_campaign_data(&env);
        if index >= data.milestone_count {
            panic!("MilestoneNotFound");
        }
        storage::get_milestone_data(&env, index)
            .expect("MilestoneNotFound")
    }

    pub fn get_all_milestones(env: Env) -> Vec<MilestoneData> {
        let data = expect_campaign_data(&env);
        let mut milestones: Vec<MilestoneData> = Vec::new(&env);
        for i in 0..data.milestone_count {
            let milestone = storage::get_milestone_data(&env, i)
                .expect("MilestoneNotFound");
            milestones.push_back(milestone);
        }
        milestones
    }

    pub fn donate(env: Env, donor: Address, amount: i128, asset: AssetInfo) -> Result<(), Error> {
        donor.require_auth();

        if amount <= 0 {
            panic!("amount must be positive");
        }

        let mut data = expect_campaign_data(&env);

        if env.ledger().timestamp() > data.end_time {
            return Err(Error::CampaignEnded);
        }

        if !matches!(
            data.status,
            CampaignStatus::Active | CampaignStatus::GoalReached
        ) {
            return Err(Error::CampaignNotActive);
        }

        if !is_accepted_asset(&data.accepted_assets, &asset) {
            panic!("asset not accepted");
        }

        let token_address = get_token_address(&env, &asset);
        let token_client = soroban_sdk::token::TokenClient::new(&env, &token_address);
        token_client.transfer(&donor, &env.current_contract_address(), &amount);

        data.raised_amount += amount;

        let mut goal_just_reached = false;
        if data.raised_amount >= data.goal_amount && data.status != CampaignStatus::GoalReached {
            data.status = CampaignStatus::GoalReached;
            goal_just_reached = true;
        }

        for i in 0..data.milestone_count {
            if let Some(mut milestone) = storage::get_milestone_data(&env, i) {
                if milestone.status == MilestoneStatus::Locked
                    && data.raised_amount >= milestone.target_amount
                {
                    milestone.status = MilestoneStatus::Unlocked;
                    storage::set_milestone_data(&env, i, &milestone);
                    env.events().publish(
                        (symbol_short!("milestone"),),
                        (i, milestone.target_amount),
                    );
                }
            }
        }

        storage::set_campaign_data(&env, &data);

        if goal_just_reached {
            env.events().publish(
                (symbol_short!("goal_reach"),),
                (data.raised_amount, data.goal_amount),
            );
        }

        let mut donor_record = storage::get_donor_data(&env, &donor).unwrap_or(DonorRecord {
            donor: donor.clone(),
            total_donated: 0,
            per_asset: Vec::new(&env),
            last_donation_time: 0,
        });

        donor_record.total_donated += amount;
        donor_record.last_donation_time = env.ledger().timestamp();

        let mut found = false;
        let mut i = 0;
        while i < donor_record.per_asset.len() {
            let mut item = donor_record.per_asset.get_unchecked(i);
            if item.asset == asset {
                item.amount += amount;
                donor_record.per_asset.set(i, item);
                found = true;
                break;
            }
            i += 1;
        }
        if !found {
            donor_record.per_asset.push_back(PerAssetBreakdown {
                asset: asset.clone(),
                amount,
            });
        }

        storage::set_donor_data(&env, &donor, &donor_record);

        env.events().publish(
            (symbol_short!("donation"),),
            (donor, amount, asset, data.raised_amount),
        );

        Ok(())
    }
}

#[cfg(test)]
mod test;