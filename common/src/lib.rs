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
    ArithmeticOverflow = 15,
}

// ─── Utility Functions ───────────────────────────────────────────────────────

/// Soroban's zero address, see https://github.com/stellar/rs-soroban-env/blob/main/soroban-env-host/src/host/primitives.rs#L1232
const ZERO_ADDRESS_STR: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWhf";

/// Validates that an address is not the zero address.
pub fn validate_address(env: &Env, address: &Address) -> Result<(), ErrorCode> {
    let zero_address = Address::from_string(&soroban_sdk::String::from_str(env, ZERO_ADDRESS_STR));
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

/// Validates that an addition will not overflow.
pub fn validate_add(a: i128, b: i128) -> Result<i128, ErrorCode> {
    a.checked_add(b).ok_or(ErrorCode::ArithmeticOverflow)
}

/// Validates that a subtraction will not overflow.
pub fn validate_sub(a: i128, b: i128) -> Result<i128, ErrorCode> {
    a.checked_sub(b).ok_or(ErrorCode::ArithmeticOverflow)
}

/// Validates that a multiplication will not overflow.
pub fn validate_mul(a: i128, b: i128) -> Result<i128, ErrorCode> {
    a.checked_mul(b).ok_or(ErrorCode::ArithmeticOverflow)
}

/// Validates that a division will not result in a division by zero.
pub fn validate_div(a: i128, b: i128) -> Result<i128, ErrorCode> {
    a.checked_div(b).ok_or(ErrorCode::ArithmeticOverflow)
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
pub fn is_asset_accepted(accepted: &soroban_sdk::Vec<AssetInfo>, target: &AssetInfo) -> bool {
    for asset in accepted.iter() {
        if assets_equal(&asset, target) {
            return true;
        }
    }
    false
}

/// Returns the description hash as a fixed-size byte array.
pub fn description_hash_bytes(hash: &BytesN<32>) -> [u8; 32] {
    hash.to_array()
}

// ─── Authorization and State Validation ──────────────────────────────────────

/// Validates that the creator is authorized to perform an operation.
/// This is called after `creator.require_auth()` has been invoked to ensure
/// explicit authorization checks are in place at operation boundaries.
pub fn check_creator_auth(env: &Env, creator: &Address, caller: &Address) -> Result<(), ErrorCode> {
    if *creator != *caller {
        return Err(ErrorCode::Unauthorized);
    }
    Ok(())
}

/// Validates that the contract is not frozen (unable to accept modifications).
/// Frozen contracts prevent state transitions and new operations.
pub fn check_contract_not_frozen(env: &Env, is_frozen: bool) -> Result<(), ErrorCode> {
    if is_frozen {
        return Err(ErrorCode::Unauthorized);
    }
    Ok(())
}

/// Validates that the contract is not locked (no concurrent modifications).
/// Locked contracts prevent re-entrant calls and concurrent state modifications.
pub fn check_contract_not_locked(env: &Env, is_locked: bool) -> Result<(), ErrorCode> {
    if is_locked {
        return Err(ErrorCode::Unauthorized);
    }
    Ok(())
}

/// Validates that the contract has not been already initialized.
/// Prevents re-initialization attacks.
pub fn check_not_already_initialized(is_initialized: bool) -> Result<(), ErrorCode> {
    if is_initialized {
        return Err(ErrorCode::AlreadyInitialized);
    }
    Ok(())
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
        assert!(!assets_equal(
            &AssetInfo::Native,
            &AssetInfo::Token(address)
        ));
    }

    #[test]
    fn test_is_asset_accepted() {
        let env = Env::default();
        let address = Address::generate(&env);

        let mut accepted = soroban_sdk::Vec::new(&env);
        accepted.push_back(AssetInfo::Native);
        accepted.push_back(AssetInfo::Token(address.clone()));

        assert!(is_asset_accepted(&accepted, &AssetInfo::Native));
        assert!(is_asset_accepted(
            &accepted,
            &AssetInfo::Token(address.clone())
        ));

        let other_address = Address::generate(&env);
        assert!(!is_asset_accepted(
            &accepted,
            &AssetInfo::Token(other_address)
        ));
    }

    #[test]
    fn test_check_creator_auth() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let other = Address::generate(&env);

        assert!(check_creator_auth(&env, &creator, &creator).is_ok());
        assert_eq!(
            check_creator_auth(&env, &creator, &other),
            Err(ErrorCode::Unauthorized)
        );
    }

    #[test]
    fn test_check_contract_not_frozen() {
        assert!(check_contract_not_frozen(&Env::default(), false).is_ok());
        assert_eq!(
            check_contract_not_frozen(&Env::default(), true),
            Err(ErrorCode::Unauthorized)
        );
    }

    #[test]
    fn test_check_contract_not_locked() {
        assert!(check_contract_not_locked(&Env::default(), false).is_ok());
        assert_eq!(
            check_contract_not_locked(&Env::default(), true),
            Err(ErrorCode::Unauthorized)
        );
    }

    #[test]
    fn test_check_not_already_initialized() {
        assert!(check_not_already_initialized(false).is_ok());
        assert_eq!(
            check_not_already_initialized(true),
            Err(ErrorCode::AlreadyInitialized)
        );
    }

    // ─── Arithmetic Validation Tests ─────────────────────────────────────────

    #[test]
    fn test_validate_add_success() {
        assert_eq!(validate_add(100, 200), Ok(300));
        assert_eq!(validate_add(0, 0), Ok(0));
        assert_eq!(validate_add(-10, 5), Ok(-5));
        assert_eq!(validate_add(i128::MAX, 0), Ok(i128::MAX));
    }

    #[test]
    fn test_validate_add_overflow() {
        assert_eq!(validate_add(i128::MAX, 1), Err(ErrorCode::ArithmeticOverflow));
        assert_eq!(validate_add(i128::MAX, i128::MAX), Err(ErrorCode::ArithmeticOverflow));
    }

    #[test]
    fn test_validate_sub_success() {
        assert_eq!(validate_sub(300, 100), Ok(200));
        assert_eq!(validate_sub(0, 0), Ok(0));
        assert_eq!(validate_sub(5, 10), Ok(-5));
        assert_eq!(validate_sub(i128::MIN, 0), Ok(i128::MIN));
    }

    #[test]
    fn test_validate_sub_overflow() {
        assert_eq!(validate_sub(i128::MIN, 1), Err(ErrorCode::ArithmeticOverflow));
        assert_eq!(validate_sub(0, i128::MIN), Err(ErrorCode::ArithmeticOverflow));
    }

    #[test]
    fn test_validate_mul_success() {
        assert_eq!(validate_mul(100, 200), Ok(20_000));
        assert_eq!(validate_mul(0, i128::MAX), Ok(0));
        assert_eq!(validate_mul(-5, 10), Ok(-50));
    }

    #[test]
    fn test_validate_mul_overflow() {
        assert_eq!(validate_mul(i128::MAX, 2), Err(ErrorCode::ArithmeticOverflow));
        assert_eq!(validate_mul(i128::MAX, i128::MAX), Err(ErrorCode::ArithmeticOverflow));
        assert_eq!(validate_mul(i128::MIN, 2), Err(ErrorCode::ArithmeticOverflow));
    }

    #[test]
    fn test_validate_div_success() {
        assert_eq!(validate_div(200, 100), Ok(2));
        assert_eq!(validate_div(0, 1), Ok(0));
        assert_eq!(validate_div(-50, 10), Ok(-5));
        assert_eq!(validate_div(7, 2), Ok(3)); // integer truncation
    }

    #[test]
    fn test_validate_div_by_zero() {
        assert_eq!(validate_div(100, 0), Err(ErrorCode::ArithmeticOverflow));
        assert_eq!(validate_div(0, 0), Err(ErrorCode::ArithmeticOverflow));
    }

    // ─── Address Validation Tests ─────────────────────────────────────────────

    #[test]
    fn test_validate_address_valid() {
        let env = Env::default();
        let address = Address::generate(&env);
        assert!(validate_address(&env, &address).is_ok());
    }

    #[test]
    fn test_validate_address_zero_address() {
        let env = Env::default();
        let zero_address = Address::from_string(&soroban_sdk::String::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWhf",
        ));
        assert_eq!(
            validate_address(&env, &zero_address),
            Err(ErrorCode::InvalidAddress)
        );
    }

    // ─── Timestamp Validation Tests ───────────────────────────────────────────

    #[test]
    fn test_validate_future_timestamp() {
        let env = Env::default();
        env.ledger().with_mut(|l| l.timestamp = 1000);

        assert!(validate_future_timestamp(&env, 1001).is_ok());
        assert!(validate_future_timestamp(&env, 2000).is_ok());
        // Same as current timestamp should fail (must be strictly future)
        assert_eq!(
            validate_future_timestamp(&env, 1000),
            Err(ErrorCode::InvalidTimestamp)
        );
        assert_eq!(
            validate_future_timestamp(&env, 999),
            Err(ErrorCode::InvalidTimestamp)
        );
    }

    // ─── Description Hash Tests ───────────────────────────────────────────────

    #[test]
    fn test_description_hash_bytes() {
        let env = Env::default();
        let hash = BytesN::from_array(&env, &[42u8; 32]);
        let arr = description_hash_bytes(&hash);
        assert_eq!(arr, [42u8; 32]);
    }

    #[test]
    fn test_description_hash_bytes_zero() {
        let env = Env::default();
        let hash = BytesN::from_array(&env, &[0u8; 32]);
        let arr = description_hash_bytes(&hash);
        assert_eq!(arr, [0u8; 32]);
    }

    // ─── Current Timestamp Test ───────────────────────────────────────────────

    #[test]
    fn test_current_timestamp() {
        let env = Env::default();
        env.ledger().with_mut(|l| l.timestamp = 42_000);
        assert_eq!(current_timestamp(&env), 42_000);
    }

    // ─── Asset Equality Edge Cases ────────────────────────────────────────────

    #[test]
    fn test_assets_equal_different_tokens() {
        let env = Env::default();
        let addr_a = Address::generate(&env);
        let addr_b = Address::generate(&env);
        assert!(!assets_equal(
            &AssetInfo::Token(addr_a),
            &AssetInfo::Token(addr_b)
        ));
    }

    #[test]
    fn test_is_asset_accepted_empty_list() {
        let env = Env::default();
        let accepted = soroban_sdk::Vec::<AssetInfo>::new(&env);
        assert!(!is_asset_accepted(&accepted, &AssetInfo::Native));
    }

    // ─── i128 Boundary Tests ──────────────────────────────────────────────────

    #[test]
    fn test_validate_add_i128_min() {
        assert_eq!(validate_add(i128::MIN, 0), Ok(i128::MIN));
        assert_eq!(validate_add(i128::MIN, -1), Err(ErrorCode::ArithmeticOverflow));
    }

    #[test]
    fn test_validate_mul_by_zero() {
        assert_eq!(validate_mul(i128::MAX, 0), Ok(0));
        assert_eq!(validate_mul(0, i128::MAX), Ok(0));
    }

    #[test]
    fn test_validate_div_i128_min_by_neg1() {
        // i128::MIN / -1 would overflow in two's complement
        assert_eq!(validate_div(i128::MIN, -1), Err(ErrorCode::ArithmeticOverflow));
    }
}
