#![no_std]

use soroban_sdk::{contracterror, contracttype, Address, BytesN, Env};

/// Current version of the common crate.
/// Bump this on any breaking change.
pub const VERSION: u32 = 1;

/// Returns the current version number.
pub fn version() -> u32 {
    VERSION
}

// ─── Shared Types ────────────────────────────────────────────────────────────

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

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ErrorCode {
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
    InvalidAddress = 13,
    InvalidTimestamp = 14,
}

// ─── Utility Functions ───────────────────────────────────────────────────────

/// Validates that an address is not the zero address.
pub fn validate_address(env: &Env, address: &Address) -> Result<(), ErrorCode> {
    let zero_address = Address::from_string(&soroban_sdk::String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWhf",
    ));
    if *address == zero_address {
        return Err(ErrorCode::InvalidAddress);
    }
    Ok(())
}

/// Validates that a timestamp is in the future.
pub fn validate_future_timestamp(env: &Env, timestamp: u64) -> Result<(), ErrorCode> {
    if timestamp <= env.ledger().timestamp() {
        return Err(ErrorCode::InvalidTimestamp);
    }
    Ok(())
}

/// Validates that an amount is positive.
pub fn validate_positive_amount(amount: i128) -> Result<(), ErrorCode> {
    if amount <= 0 {
        return Err(ErrorCode::InvalidAmount);
    }
    Ok(())
}

/// Returns the current ledger timestamp.
pub fn current_timestamp(env: &Env) -> u64 {
    env.ledger().timestamp()
}

/// Checks if a campaign has ended based on its end time.
pub fn is_campaign_ended(env: &Env, end_time: u64) -> bool {
    env.ledger().timestamp() >= end_time
}

/// Compares two asset infos for equality.
pub fn assets_equal(a: &AssetInfo, b: &AssetInfo) -> bool {
    a == b
}

/// Checks if an asset is in a list of accepted assets.
pub fn is_asset_accepted(
    env: &Env,
    accepted: &soroban_sdk::Vec<AssetInfo>,
    target: &AssetInfo,
) -> bool {
    for i in 0..accepted.len() {
        if accepted.get_unchecked(i) == *target {
            return true;
        }
    }
    false
}

/// Returns the description hash as a fixed-size byte array.
pub fn description_hash_bytes(hash: &BytesN<32>) -> [u8; 32] {
    hash.to_array()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_version() {
        assert_eq!(version(), 1);
    }

    #[test]
    fn test_validate_positive_amount() {
        assert!(validate_positive_amount(100).is_ok());
        assert!(validate_positive_amount(0).is_err());
        assert!(validate_positive_amount(-1).is_err());
    }

    #[test]
    fn test_is_campaign_ended() {
        let env = Env::default();
        env.ledger().with_mut(|l| l.timestamp = 1000);

        assert!(is_campaign_ended(&env, 999));
        assert!(is_campaign_ended(&env, 1000));
        assert!(!is_campaign_ended(&env, 1001));
    }

    #[test]
    fn test_assets_equal() {
        let env = Env::default();
        let address = Address::generate(&env);

        assert!(assets_equal(&AssetInfo::Native, &AssetInfo::Native));
        assert!(assets_equal(
            &AssetInfo::Token(address.clone()),
            &AssetInfo::Token(address.clone())
        ));
        assert!(!assets_equal(&AssetInfo::Native, &AssetInfo::Token(address)));
    }

    #[test]
    fn test_is_asset_accepted() {
        let env = Env::default();
        let address = Address::generate(&env);

        let mut accepted = soroban_sdk::Vec::new(&env);
        accepted.push_back(AssetInfo::Native);
        accepted.push_back(AssetInfo::Token(address.clone()));

        assert!(is_asset_accepted(&env, &accepted, &AssetInfo::Native));
        assert!(is_asset_accepted(
            &env,
            &accepted,
            &AssetInfo::Token(address.clone())
        ));

        let other_address = Address::generate(&env);
        assert!(!is_asset_accepted(
            &env,
            &accepted,
            &AssetInfo::Token(other_address)
        ));
    }
}
