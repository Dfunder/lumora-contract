#![no_std]

use common::{
    check_not_already_initialized, is_asset_accepted, validate_add, validate_div, validate_mul,
    validate_sub, AssetInfo, CampaignStatus, MilestoneStatus,
};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    Symbol, Vec,
};

pub mod storage;

const MAX_MILESTONES: u32 = 5;
const REFUND_WINDOW: u64 = 30 * 24 * 60 * 60; // 30 days in seconds
const MAX_DEADLINE_EXTENSION: u64 = 90 * 24 * 60 * 60; // 90 days in seconds
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// Thrown when an unauthorized caller attempts to perform an operation that requires
    /// elevated permissions (e.g., admin-only actions, creator-only actions).
    /// Recoverable: This is a bad input issue - only the authorized caller can retry.
    Unauthorized = 1,

    /// Thrown when initialize() is called more than once on a contract.
    /// Terminal: This indicates a re-initialization attack or incorrect usage.
    AlreadyInitialized = 2,

    /// Thrown when a campaign is interacted with before it has been initialized.
    /// Recoverable: The caller must initialize the contract first before performing other operations.
    NotInitialized = 21,

    /// Thrown when the goal amount provided during initialization is <= 0.
    /// Recoverable: Fix the goal amount to a positive value and retry initialization.
    InvalidGoalAmount = 3,

    /// Thrown when the end time provided during initialization is in the past.
    /// Recoverable: Fix the end time to a future timestamp and retry initialization.
    InvalidEndTime = 4,

    /// Thrown when no accepted assets are provided during initialization.
    /// Recoverable: Provide at least one accepted asset and retry initialization.
    NoAcceptedAssets = 5,

    /// Thrown when milestones provided during initialization are invalid (wrong count, non-increasing amounts, last milestone != goal).
    /// Recoverable: Fix the milestones to meet the validation criteria and retry initialization.
    InvalidMilestones = 6,

    /// Thrown when an invalid amount (<=0) is provided for an operation.
    /// Recoverable: Provide a valid positive amount and retry.
    InvalidAmount = 7,

    /// Thrown when an asset that is not in the campaign's accepted assets list is used in a donation.
    /// Recoverable: Use an accepted asset or add the asset to the campaign's accepted assets list.
    AssetNotAccepted = 8,

    /// Thrown when an operation is attempted on a campaign that is not in an active state.
    /// Recoverable: Verify the campaign's current status before attempting the operation.
    CampaignNotActive = 9,

    /// Thrown when an operation is attempted on a campaign that has passed its end time.
    /// Recoverable: Cannot be retried - campaign has concluded.
    CampaignEnded = 10,

    /// Thrown when attempting to cancel a campaign that has remaining funds in the contract.
    /// Recoverable: Withdraw or distribute all funds before attempting to cancel.
    CannotCancelWithFunds = 22,

    /// Thrown when a refund is attempted after the refund window (30 days after campaign end) has closed.
    /// Recoverable: Cannot be retried - refund window has expired.
    RefundWindowClosed = 19,

    /// Thrown when a milestone with the specified index does not exist on the campaign.
    /// Recoverable: Verify the milestone index exists before attempting the operation.
    MilestoneNotFound = 13,

    /// Thrown when attempting to release a milestone that is still in Locked status.
    /// Recoverable: Wait for the milestone to be unlocked (when enough funds are raised) before attempting to release.
    MilestoneNotUnlocked = 17,

    /// Thrown when attempting to release a milestone out of order - a previous milestone has not been released.
    /// Recoverable: Release milestones in sequential order.
    PreviousMilestoneNotReleased = 15,

    /// Thrown when a donation is less than the campaign's minimum donation amount.
    /// Recoverable: Increase the donation amount to meet the minimum and retry.
    DonationTooSmall = 14,

    /// Thrown when an arithmetic operation would cause an overflow or underflow.
    /// Terminal: This indicates a critical bug in the contract's accounting logic.
    Overflow = 18,

    /// Thrown when a reentrant call is detected on the contract.
    /// Terminal: This indicates a reentrancy attack or incorrect usage of reentrant calls.
    Reentrant = 23,

    /// Thrown when an operation is attempted on a frozen contract.
    /// Recoverable: Cannot be retried - contract is frozen and cannot accept modifications.
    ContractFrozen = 24,

    /// Thrown when insufficient balance in the contract to perform an operation (e.g., withdrawal, transfer).
    /// Recoverable: Ensure the contract has enough funds before attempting the operation.
    InsufficientContractBalance = 25,

    /// Thrown when a requested deadline extension would push the campaign end time
    /// more than 90 days past the original end time.
    /// Recoverable: Provide a new_end_time within the 90-day extension limit.
    DeadlineExceedsLimit = 26,

    // Legacy errors maintained for backward compatibility
    CampaignCancelled = 11,
    DonationFailed = 12,
    MilestoneAlreadyReleased = 16,
    NoRefundAvailable = 20,
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
    CampaignEndTime,
    OriginalEndTime,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignData {
    pub creator: Address,
    pub goal_amount: i128,
    pub raised_amount: i128,
    pub released_amount: i128,
    pub end_time: u64,
    pub status: CampaignStatus,
    pub accepted_assets: Vec<AssetInfo>,
    pub milestone_count: u32,
    pub next_releasable_milestone: u32,
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

fn get_campaign_data(env: &Env) -> Result<CampaignData, Error> {
    storage::get_campaign_data(env).ok_or(Error::NotInitialized)
}

fn get_token_address(env: &Env, asset: &AssetInfo) -> Result<Address, Error> {
    match asset {
        AssetInfo::Native => storage::get_xlm_token(env).ok_or(Error::NotInitialized),
        AssetInfo::Token(address) => Ok(address.clone()),
    }
}

#[contract]
pub struct CampaignContract;

#[contractimpl]
impl CampaignContract {
    /// Deploys and initializes a campaign. Callable exactly once.
    /// Enforces explicit authorization and prevents re-initialization attacks.
    pub fn initialize(
        env: Env,
        creator: Address,
        goal_amount: i128,
        end_time: u64,
        accepted_assets: Vec<AssetInfo>,
        milestones: Vec<MilestoneInput>,
        min_donation_amount: i128,
    ) -> Result<(), Error> {
        creator.require_auth();

        // Check that contract is not frozen
        if storage::is_frozen(&env) {
            return Err(Error::Unauthorized);
        }

        // Check that contract is not already initialized
        check_not_already_initialized(storage::has_campaign_data(&env))
            .map_err(|_| Error::AlreadyInitialized)?;

        if goal_amount <= 0 {
            return Err(Error::InvalidGoalAmount);
        }

        if end_time <= env.ledger().timestamp() {
            return Err(Error::InvalidEndTime);
        }

        if accepted_assets.is_empty() {
            return Err(Error::NoAcceptedAssets);
        }

        if min_donation_amount < 0 {
            return Err(Error::InvalidAmount);
        }

        let milestone_count = milestones.len();
        if milestone_count == 0 || milestone_count > MAX_MILESTONES {
            return Err(Error::InvalidMilestones);
        }

        let mut previous_amount: i128 = 0;
        for i in 0..milestone_count {
            let milestone = milestones.get(i).unwrap();
            if milestone.target_amount <= previous_amount {
                return Err(Error::InvalidMilestones);
            }
            previous_amount = milestone.target_amount;
        }

        if milestones.last().unwrap().target_amount != goal_amount {
            return Err(Error::InvalidMilestones);
        }

        // Store min donation amount
        storage::set_min_donation_amount(&env, &min_donation_amount);

        // Record the original end time so deadline extensions are always capped
        // relative to the initial deadline, even across repeated extensions.
        storage::set_original_end_time(&env, &end_time);

        let campaign_data = CampaignData {
            creator: creator.clone(),
            goal_amount,
            raised_amount: 0,
            released_amount: 0,
            end_time,
            status: CampaignStatus::Active,
            accepted_assets: accepted_assets.clone(),
            milestone_count,
            next_releasable_milestone: 0,
        };
        storage::set_campaign_data(&env, &campaign_data);

        for i in 0..milestone_count {
            let input = milestones.get(i).unwrap();
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
            (Symbol::new(&env, "campaign_initialized"), env.current_contract_address(), creator),
            (goal_amount, end_time, accepted_assets, milestones),
        );

        Ok(())
    }

    pub fn release_milestone(
        env: Env,
        milestone_index: u32,
        recipient: Address,
    ) -> Result<(), Error> {
        let mut campaign_data = get_campaign_data(&env)?;
        campaign_data.creator.require_auth();

        // Check that contract is not frozen
        if storage::is_frozen(&env) {
            return Err(Error::ContractFrozen);
        }

        // Check that contract is not locked
        if storage::is_locked(&env) {
            return Err(Error::Reentrant);
        }

        // Acquire lock to prevent concurrent modifications
        storage::acquire_lock(&env)?;

        // Verify creator is the only one who can release milestones
        if campaign_data.creator != env.current_contract_address() {
            // Creator has already authorized via require_auth above
        }

        if milestone_index != campaign_data.next_releasable_milestone {
            storage::release_lock(&env);
            return Err(Error::PreviousMilestoneNotReleased);
        }

        let mut milestone = match Self::get_milestone(env.clone(), milestone_index) {
            Ok(m) => m,
            Err(e) => {
                storage::release_lock(&env);
                return Err(e);
            }
        };

        if milestone.status == MilestoneStatus::Released {
            storage::release_lock(&env);
            return Err(Error::MilestoneAlreadyReleased);
        }
        if milestone.status != MilestoneStatus::Unlocked {
            storage::release_lock(&env);
            return Err(Error::MilestoneNotUnlocked);
        }

        let total_raised = campaign_data.raised_amount;
        let release_amount = if milestone_index == 0 {
            // First milestone: release the full target amount of the first milestone
            milestone.target_amount
        } else {
            // Subsequent milestones: release the difference between current and previous milestone's target
            let previous_milestone = match storage::get_milestone_data(&env, milestone_index - 1) {
                Some(m) => m,
                None => {
                    storage::release_lock(&env);
                    return Err(Error::MilestoneNotFound);
                }
            };
            match validate_sub(milestone.target_amount, previous_milestone.target_amount) {
                Ok(amt) => amt,
                Err(_) => {
                    storage::release_lock(&env);
                    return Err(Error::Overflow);
                }
            }
        };

        let mut total_released_this_milestone: i128 = 0;

        for (i, asset_info) in campaign_data.accepted_assets.iter().enumerate() {
            let asset_raised = storage::get_raised_per_asset(&env, asset_info.clone()).unwrap_or(0);
            if asset_raised > 0 {
                let per_asset_release = if i == (campaign_data.accepted_assets.len() - 1) as usize {
                    // Last asset, release the remainder
                    match validate_sub(release_amount, total_released_this_milestone) {
                        Ok(amt) => amt,
                        Err(_) => {
                            storage::release_lock(&env);
                            return Err(Error::Overflow);
                        }
                    }
                } else {
                    match validate_mul(asset_raised, release_amount) {
                        Ok(product) => match validate_div(product, total_raised) {
                            Ok(quot) => quot,
                            Err(_) => {
                                storage::release_lock(&env);
                                return Err(Error::Overflow);
                            }
                        },
                        Err(_) => {
                            storage::release_lock(&env);
                            return Err(Error::Overflow);
                        }
                    }
                };

                if per_asset_release > 0 {
                    total_released_this_milestone =
                        match validate_add(total_released_this_milestone, per_asset_release) {
                            Ok(amt) => amt,
                            Err(_) => {
                                storage::release_lock(&env);
                                return Err(Error::Overflow);
                            }
                        };

                    let token_address = get_token_address(&env, &asset_info)?;
                    let token_client = soroban_sdk::token::TokenClient::new(&env, &token_address);
                    token_client.transfer(
                        &env.current_contract_address(),
                        &recipient,
                        &per_asset_release,
                    );
                    env.events().publish(
                        (
                            Symbol::new(&env, "milestone_released"),
                            env.current_contract_address(),
                        ),
                        (
                            milestone_index,
                            per_asset_release,
                            asset_info.clone(),
                            recipient.clone(),
                            env.ledger().timestamp(),
                        ),
                        (per_asset_release, env.ledger().timestamp()),
                    );
                }
            }
        }

        milestone.status = MilestoneStatus::Released;
        milestone.released_at = Some(env.ledger().timestamp());
        storage::set_milestone_data(&env, milestone_index, &milestone);

        campaign_data.released_amount =
            match validate_add(campaign_data.released_amount, release_amount) {
                Ok(amt) => amt,
                Err(_) => {
                    storage::release_lock(&env);
                    return Err(Error::Overflow);
                }
            };
        campaign_data.next_releasable_milestone += 1;
        storage::set_campaign_data(&env, &campaign_data);

        storage::release_lock(&env);
        Ok(())
    }

    pub fn get_campaign_info(env: Env) -> Result<CampaignData, Error> {
        get_campaign_data(&env)
    }

    pub fn get_min_donation_amount(env: Env) -> i128 {
        storage::get_min_donation_amount(&env).unwrap_or(0)
    }

    /// Sets the XLM token address for Native asset handling.
    /// This must be called before any donations with Native assets.
    pub fn set_xlm_token(env: Env, token_address: Address) {
        storage::set_xlm_token(&env, &token_address);
    }

    /// Returns the donor record for a given address.
    /// Returns None for an address that has never donated
    /// (instead of panicking).
    pub fn get_donor_record(env: Env, donor: Address) -> Option<DonorRecord> {
        storage::get_donor_data(&env, &donor)
    }

    /// Returns the total amount raised so far.
    /// Returns 0 if the campaign hasn't been initialized or
    /// no donations have been made yet.
    pub fn get_total_raised(env: Env) -> i128 {
        storage::get_campaign_data(&env)
            .map(|data| data.raised_amount)
            .unwrap_or(0)
    }

    pub fn require_creator(env: Env) -> Result<(), Error> {
        let data = get_campaign_data(&env)?;
        data.creator.require_auth();
        Ok(())
    }

    pub fn get_milestone(env: Env, index: u32) -> Result<MilestoneData, Error> {
        let data = get_campaign_data(&env)?;
        if index >= data.milestone_count {
            return Err(Error::MilestoneNotFound);
        }
        storage::get_milestone_data(&env, index).ok_or(Error::MilestoneNotFound)
    }

    pub fn get_all_milestones(env: Env) -> Result<Vec<MilestoneData>, Error> {
        let data = get_campaign_data(&env)?;
        let mut milestones: Vec<MilestoneData> = Vec::new(&env);
        for i in 0..data.milestone_count {
            let milestone = storage::get_milestone_data(&env, i).ok_or(Error::MilestoneNotFound)?;
            milestones.push_back(milestone);
        }
        Ok(milestones)
    }

    /// Checks if a donor is eligible for a refund.
    /// Returns true if:
    /// - The campaign is in Cancelled or Failed status
    /// - The current time is within the refund window (30 days from campaign end)
    /// - The donor has a non-zero donation record
    pub fn is_refund_eligible(env: Env, donor: Address) -> bool {
        let campaign_data = match storage::get_campaign_data(&env) {
            Some(data) => data,
            None => return false,
        };

        // Check campaign status - only Cancelled or Failed campaigns allow refunds
        if !matches!(
            campaign_data.status,
            CampaignStatus::Cancelled | CampaignStatus::Failed
        ) {
            return false;
        }

        // Check refund window
        let campaign_end_time = match storage::get_campaign_end_time(&env) {
            Some(time) => time,
            None => return false,
        };

        let current_time = env.ledger().timestamp();
        if current_time > campaign_end_time + REFUND_WINDOW {
            return false;
        }

        // Check if donor has a record with non-zero donations
        match storage::get_donor_data(&env, &donor) {
            Some(donor_record) => donor_record.total_donated > 0,
            None => false,
        }
    }

    /// Returns the remaining time in the refund window for a campaign.
    /// Returns None if the campaign is not in a refundable state or the window has closed.
    pub fn get_refund_window_remaining(env: Env) -> Option<u64> {
        let campaign_data = storage::get_campaign_data(&env)?;

        if !matches!(
            campaign_data.status,
            CampaignStatus::Cancelled | CampaignStatus::Failed
        ) {
            return None;
        }

        let campaign_end_time = storage::get_campaign_end_time(&env)?;
        let current_time = env.ledger().timestamp();

        if current_time > campaign_end_time + REFUND_WINDOW {
            return None;
        }

        Some((campaign_end_time + REFUND_WINDOW) - current_time)
    }

    /// Refunds the donor's exact contributions per asset.
    /// This function:
    /// - Checks refund eligibility and window
    /// - Refunds each asset exactly as contributed (no rounding losses)
    /// - Clears the donor's record after successful refund
    /// - Panics with RefundWindowClosed if called after the window
    pub fn refund(env: Env, donor: Address) -> Result<(), Error> {
        donor.require_auth();

        // Check that contract is not frozen
        if storage::is_frozen(&env) {
            return Err(Error::ContractFrozen);
        }

        let campaign_data = get_campaign_data(&env)?;

        // Check campaign status
        if !matches!(
            campaign_data.status,
            CampaignStatus::Cancelled | CampaignStatus::Failed
        ) {
            return Err(Error::CampaignNotActive);
        }

        // Check and enforce refund window
        let campaign_end_time = match storage::get_campaign_end_time(&env) {
            Some(time) => time,
            None => return Err(Error::NoRefundAvailable),
        };

        let current_time = env.ledger().timestamp();
        if current_time > campaign_end_time + REFUND_WINDOW {
            return Err(Error::RefundWindowClosed);
        }

        // Get donor record
        let donor_record = match storage::get_donor_data(&env, &donor) {
            Some(record) => record,
            None => return Err(Error::NoRefundAvailable),
        };

        if donor_record.total_donated == 0 {
            return Err(Error::NoRefundAvailable);
        }

        // Refund each asset exactly as contributed (no rounding)
        let mut total_refunded: i128 = 0;
        for per_asset in donor_record.per_asset.iter() {
            if per_asset.amount > 0 {
                let token_address = get_token_address(&env, &per_asset.asset)?;
                let token_client = soroban_sdk::token::TokenClient::new(&env, &token_address);

                token_client.transfer(&env.current_contract_address(), &donor, &per_asset.amount);

                total_refunded = match validate_add(total_refunded, per_asset.amount) {
                    Ok(amt) => amt,
                    Err(_) => return Err(Error::Overflow),
                };

                env.events().publish(
                    (symbol_short!("refund"),),
                    (donor.clone(), per_asset.amount, per_asset.asset.clone()),
                );
            }
        }

        // Clear donor record after successful refund
        storage::set_donor_data(
            &env,
            &donor,
            &DonorRecord {
                donor: donor.clone(),
                total_donated: 0,
                per_asset: Vec::new(&env),
                last_donation_time: 0,
            },
        );

        Ok(())
    }

    /// Cancels the campaign and starts the refund window.
    /// Only callable by the campaign creator.
    /// Only permitted while no funds have been raised (`raised_amount == 0`);
    /// otherwise fails with `CannotCancelWithFunds`.
    /// Sets the campaign status to Cancelled and records the end time for refund window calculation.
    pub fn cancel_campaign(env: Env) -> Result<(), Error> {
        let mut campaign_data = get_campaign_data(&env)?;
        campaign_data.creator.require_auth();

        // Check that contract is not frozen
        if storage::is_frozen(&env) {
            return Err(Error::ContractFrozen);
        }

        // Only allow cancellation from Active or GoalReached status
        if !matches!(
            campaign_data.status,
            CampaignStatus::Active | CampaignStatus::GoalReached
        ) {
            return Err(Error::CampaignNotActive);
        }

        // Cancellation is only permitted while nothing has been raised
        if campaign_data.raised_amount != 0 {
            return Err(Error::CannotCancelWithFunds);
        }

        // Update campaign status
        campaign_data.status = CampaignStatus::Cancelled;
        storage::set_campaign_data(&env, &campaign_data);

        // Set campaign end time to start refund window
        let current_time = env.ledger().timestamp();
        storage::set_campaign_end_time(&env, current_time);

        env.events().publish(
            (Symbol::new(&env, "campaign_cancelled"),),
            (campaign_data.creator.clone(), current_time),
        );

        Ok(())
    }

    /// Extends the campaign deadline to `new_end_time`.
    /// Only callable by the campaign creator while the campaign is Active or GoalReached.
    /// The new end time must be strictly later than the current end time and may
    /// not push the deadline more than 90 days past the ORIGINAL end time,
    /// even across repeated extensions.
    pub fn extend_deadline(env: Env, new_end_time: u64) -> Result<(), Error> {
        let mut campaign_data = get_campaign_data(&env)?;
        campaign_data.creator.require_auth();

        // Check that contract is not frozen
        if storage::is_frozen(&env) {
            return Err(Error::ContractFrozen);
        }

        // Only allow extensions while the campaign is Active or GoalReached
        if !matches!(
            campaign_data.status,
            CampaignStatus::Active | CampaignStatus::GoalReached
        ) {
            return Err(Error::CampaignNotActive);
        }

        // The new deadline must be later than the current one
        if new_end_time <= campaign_data.end_time {
            return Err(Error::InvalidEndTime);
        }

        // Cap at 90 days past the original end time, even across repeated extensions
        let original_end_time =
            storage::get_original_end_time(&env).unwrap_or(campaign_data.end_time);
        let max_allowed = match original_end_time.checked_add(MAX_DEADLINE_EXTENSION) {
            Some(time) => time,
            None => return Err(Error::Overflow),
        };
        if new_end_time > max_allowed {
            return Err(Error::DeadlineExceedsLimit);
        }

        campaign_data.end_time = new_end_time;
        storage::set_campaign_data(&env, &campaign_data);

        env.events().publish(
            (Symbol::new(&env, "deadline_extended"),),
            (campaign_data.creator.clone(), new_end_time),
        );

        Ok(())
    }

    /// Returns the current campaign status together with the number of days
    /// remaining until the campaign's end time.
    /// `days_remaining` is computed from the current ledger timestamp and is
    /// negative once the deadline has passed.
    pub fn get_campaign_status(env: Env) -> Result<(CampaignStatus, i64), Error> {
        let campaign_data = get_campaign_data(&env)?;
        let seconds_remaining = campaign_data.end_time as i64 - env.ledger().timestamp() as i64;
        let mut days_remaining = seconds_remaining / SECONDS_PER_DAY;
        if seconds_remaining % SECONDS_PER_DAY != 0 {
            // Round away from zero so any time left counts as a full day and
            // any time past the deadline reports as negative.
            if seconds_remaining > 0 {
                days_remaining += 1;
            } else {
                days_remaining -= 1;
            }
        }
        Ok((campaign_data.status, days_remaining))
    }

    /// Returns the total refundable amount for a donor.
    /// This is the sum of all per-asset contributions that haven't been refunded yet.
    pub fn get_refundable_amount(env: Env, donor: Address) -> i128 {
        match storage::get_donor_data(&env, &donor) {
            Some(donor_record) => donor_record.total_donated,
            None => 0,
        }
    }

    /// Marks the campaign as failed and starts the refund window.
    /// Only callable by the campaign creator.
    /// Sets the campaign status to Failed and records the end time for refund window calculation.
    pub fn fail_campaign(env: Env) -> Result<(), Error> {
        let mut campaign_data = get_campaign_data(&env)?;
        campaign_data.creator.require_auth();

        // Check that contract is not frozen
        if storage::is_frozen(&env) {
            return Err(Error::ContractFrozen);
        }

        // Only allow failure from Active or GoalReached status
        if !matches!(
            campaign_data.status,
            CampaignStatus::Active | CampaignStatus::GoalReached
        ) {
            return Err(Error::CampaignNotActive);
        }

        // Update campaign status
        campaign_data.status = CampaignStatus::Failed;
        storage::set_campaign_data(&env, &campaign_data);

        // Set campaign end time to start refund window
        let current_time = env.ledger().timestamp();
        storage::set_campaign_end_time(&env, current_time);

        env.events().publish(
            (symbol_short!("failed"),),
            (campaign_data.creator.clone(), current_time),
        );

        Ok(())
    }

    /// Ends the campaign early at the creator's discretion.
    /// Only callable by the campaign creator while the campaign is Active or
    /// GoalReached; fails if the campaign is already Ended or Cancelled.
    /// Ending does not prevent the final milestone from being released:
    /// `release_milestone` validates milestone state, not campaign status.
    /// Does not start the refund window (ending is not a failure mode), so the
    /// stored end time is left untouched.
    pub fn end_campaign(env: Env) -> Result<(), Error> {
        let mut campaign_data = get_campaign_data(&env)?;
        campaign_data.creator.require_auth();

        // Check that contract is not frozen
        if storage::is_frozen(&env) {
            return Err(Error::ContractFrozen);
        }

        // Only allow ending from Active or GoalReached status
        if !matches!(
            campaign_data.status,
            CampaignStatus::Active | CampaignStatus::GoalReached
        ) {
            return Err(Error::CampaignNotActive);
        }

        // Update campaign status
        campaign_data.status = CampaignStatus::Ended;
        storage::set_campaign_data(&env, &campaign_data);

        env.events().publish(
            (Symbol::new(&env, "campaign_ended"),),
            (campaign_data.creator.clone(), env.ledger().timestamp()),
        );

        Ok(())
    }

    /// Permissionlessly transitions an expired-but-still-Active campaign to
    /// Ended and emits the `campaign_ended` event.
    ///
    /// Anyone may call this: deadline enforcement must not depend on the
    /// creator's cooperation. Without a permissionless transition, a creator
    /// could simply refuse to act and keep a dead campaign nominally Active,
    /// blocking downstream flows that key off the status. This call is
    /// idempotent - calling it on a non-expired or already-ended campaign is a
    /// no-op returning Ok.
    pub fn update_status(env: Env) -> Result<(), Error> {
        let mut campaign_data = get_campaign_data(&env)?;

        if env.ledger().timestamp() > campaign_data.end_time
            && campaign_data.status == CampaignStatus::Active
        {
            campaign_data.status = CampaignStatus::Ended;
            storage::set_campaign_data(&env, &campaign_data);

            env.events().publish(
                (Symbol::new(&env, "campaign_ended"),),
                (campaign_data.creator.clone(), env.ledger().timestamp()),
            );
        }

        Ok(())
    }

    pub fn donate(env: Env, donor: Address, amount: i128, asset: AssetInfo) -> Result<(), Error> {
        donor.require_auth();

        // Check that contract is not frozen
        if storage::is_frozen(&env) {
            return Err(Error::ContractFrozen);
        }

        // Check that contract is not locked
        if storage::is_locked(&env) {
            return Err(Error::Reentrant);
        }

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let min_donation = Self::get_min_donation_amount(env.clone());
        if min_donation > 0 && amount < min_donation {
            return Err(Error::DonationTooSmall);
        }

        let mut data = get_campaign_data(&env)?;

        if env.ledger().timestamp() > data.end_time {
            // A donation attempt against an expired, still-Active campaign
            // triggers the same transition as `update_status` so the status
            // never stays Active past the deadline.
            if data.status == CampaignStatus::Active {
                data.status = CampaignStatus::Ended;
                storage::set_campaign_data(&env, &data);
                env.events().publish(
                    (Symbol::new(&env, "campaign_ended"),),
                    (data.creator.clone(), env.ledger().timestamp()),
                );
            }
            return Err(Error::CampaignEnded);
        }

        if !matches!(
            data.status,
            CampaignStatus::Active | CampaignStatus::GoalReached
        ) {
            return Err(Error::CampaignNotActive);
        }

        if !is_asset_accepted(&data.accepted_assets, &asset) {
            return Err(Error::AssetNotAccepted);
        }

        let token_address = get_token_address(&env, &asset)?;
        let token_client = soroban_sdk::token::TokenClient::new(&env, &token_address);
        token_client.transfer(&donor, &env.current_contract_address(), &amount);

        data.raised_amount =
            validate_add(data.raised_amount, amount).map_err(|_| Error::Overflow)?;

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
                        (
                            Symbol::new(&env, "milestone_unlocked"),
                            env.current_contract_address(),
                        ),
                        (i, milestone.target_amount, data.raised_amount),
                    );
                }
            }
        }

        storage::set_campaign_data(&env, &data);

        if goal_just_reached {
            env.events().publish(
                (symbol_short!("goal_rch"),),
                (data.raised_amount, data.goal_amount),
            );
        }

        let mut donor_record = storage::get_donor_data(&env, &donor).unwrap_or(DonorRecord {
            donor: donor.clone(),
            total_donated: 0,
            per_asset: Vec::new(&env),
            last_donation_time: 0,
        });

        donor_record.total_donated =
            validate_add(donor_record.total_donated, amount).map_err(|_| Error::Overflow)?;
        donor_record.last_donation_time = env.ledger().timestamp();

        // soroban_sdk::Vec has no in-place mutation, so rebuild the per-asset
        // breakdown, accumulating into the matching entry when it exists.
        let mut found = false;
        let mut updated_per_asset: Vec<PerAssetBreakdown> = Vec::new(&env);
        for item in donor_record.per_asset.iter() {
            if item.asset == asset {
                updated_per_asset.push_back(PerAssetBreakdown {
                    asset: item.asset.clone(),
                    amount: validate_add(item.amount, amount).map_err(|_| Error::Overflow)?,
                });
                found = true;
            } else {
                updated_per_asset.push_back(item.clone());
            }
        }
        donor_record.per_asset = updated_per_asset;

        if !found {
            donor_record.per_asset.push_back(PerAssetBreakdown {
                asset: asset.clone(),
                amount,
            });
        }

        storage::set_donor_data(&env, &donor, &donor_record);

        let total_asset_raised = storage::get_raised_per_asset(&env, asset.clone()).unwrap_or(0);
        storage::set_raised_per_asset(
            &env,
            asset.clone(),
            validate_add(total_asset_raised, amount).map_err(|_| Error::Overflow)?,
        );

        // Convert AssetInfo to asset_code string for event data
        let asset_code = match &asset {
            AssetInfo::Native => soroban_sdk::String::from_str(&env, "XLM"),
            AssetInfo::Token(addr) => addr.to_string(),
        };

        env.events().publish(
            (Symbol::new(&env, "donation_received"), env.current_contract_address()),
            (donor, amount, asset_code, data.raised_amount, env.ledger().timestamp()),
        );

        Ok(())
    }
}

#[cfg(test)]
mod event_test;

mod test;
