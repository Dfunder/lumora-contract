#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, String,
    Vec,
};

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
    XlmTokenAddress,
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneData {
    pub index: u32,
    pub target_amount: i128,
    pub description_hash: BytesN<32>,
    pub status: MilestoneStatus,
    pub released_at: Option<u64>,
    pub release_tx: Option<BytesN<32>>,
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

    pub fn has_donor_data(env: &Env, donor: &Address) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::DonorData(donor.clone()))
    }

    pub fn get_donor_data(env: &Env, donor: &Address) -> Option<DonorRecord> {
        env.storage()
            .instance()
            .get(&DataKey::DonorData(donor.clone()))
    }

    pub fn set_donor_data(env: &Env, donor: &Address, data: &DonorRecord) {
        env.storage()
            .instance()
            .set(&DataKey::DonorData(donor.clone()), data);
    }

    pub fn get_xlm_token(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::XlmTokenAddress)
            .expect("XLM token address not set")
    }

    pub fn set_xlm_token(env: &Env, address: &Address) {
        env.storage()
            .instance()
            .set(&DataKey::XlmTokenAddress, address);
    }
}

fn get_token_address(env: &Env, asset: &AssetInfo) -> Address {
    if asset.is_xlm() {
        storage::get_xlm_token(env)
    } else {
        asset
            .issuer
            .clone()
            .expect("non-XLM asset must have an issuer")
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
    pub fn get_campaign_info(env: Env) -> CampaignData {
        storage::get_campaign_data(&env)
    }

    pub fn require_creator(env: Env) {
        let data = storage::get_campaign_data(&env);
        data.creator.require_auth();
    }

    pub fn donate(env: Env, donor: Address, amount: i128, asset: AssetInfo) {
        donor.require_auth();

        if amount <= 0 {
            panic!("amount must be positive");
        }

        let mut data = storage::get_campaign_data(&env);

        if data.status != CampaignStatus::Active {
            panic!("campaign is not active");
        }

        let deadline_reached = env.ledger().timestamp() >= data.end_time;
        if deadline_reached {
            data.status = CampaignStatus::Ended;
            storage::set_campaign_data(&env, &data);
            panic!("campaign has ended");
        }

        if !is_accepted_asset(&data.accepted_assets, &asset) {
            panic!("asset not accepted");
        }

        let token_address = get_token_address(&env, &asset);
        let token_client = soroban_sdk::token::TokenClient::new(&env, &token_address);
        token_client.transfer(&donor, &env.current_contract_address(), &amount);

        data.raised_amount += amount;

        if data.raised_amount >= data.goal_amount {
            data.status = CampaignStatus::GoalReached;
        }

        storage::set_campaign_data(&env, &data);

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
            donor_record
                .per_asset
                .push_back(PerAssetBreakdown {
                    asset: asset.clone(),
                    amount,
                });
        }

        storage::set_donor_data(&env, &donor, &donor_record);

        env.events().publish(
            (symbol_short!("donation"),),
            (donor, amount, asset, data.raised_amount),
        );
    }
}
