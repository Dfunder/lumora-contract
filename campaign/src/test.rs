use crate::{
    CampaignContract, CampaignContractClient, CampaignData, CampaignStatus, Error, MilestoneData,
    MilestoneInput, MilestoneStatus,
};
use common::AssetInfo;
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::{Address, BytesN, Env, IntoVal as _, Symbol};

fn desc_hash(env: &Env, bytes: [u8; 32]) -> BytesN<32> {
    BytesN::from_array(env, &bytes)
}

fn assert_campaign_data_equal(a: &CampaignData, b: &CampaignData) {
    assert_eq!(a.creator, b.creator, "creator mismatch");
    assert_eq!(a.goal_amount, b.goal_amount, "goal_amount mismatch");
    assert_eq!(a.raised_amount, b.raised_amount, "raised_amount mismatch");
    assert_eq!(
        a.released_amount, b.released_amount,
        "released_amount mismatch"
    );
    assert_eq!(a.end_time, b.end_time, "end_time mismatch");
    assert_eq!(a.status, b.status, "status mismatch");
    assert_eq!(
        a.accepted_assets, b.accepted_assets,
        "accepted_assets mismatch"
    );
    assert_eq!(
        a.milestone_count, b.milestone_count,
        "milestone_count mismatch"
    );
    assert_eq!(
        a.next_releasable_milestone, b.next_releasable_milestone,
        "next_releasable_milestone mismatch"
    );
}

fn assert_milestone_data_equal(a: &MilestoneData, b: &MilestoneData) {
    assert_eq!(a.index, b.index, "index mismatch");
    assert_eq!(a.target_amount, b.target_amount, "target_amount mismatch");
    assert_eq!(
        a.description_hash, b.description_hash,
        "description_hash mismatch"
    );
    assert_eq!(a.status, b.status, "status mismatch");
    assert_eq!(a.released_at, b.released_at, "released_at mismatch");
    assert_eq!(a.release_tx, b.release_tx, "release_tx mismatch");
}

#[test]
fn test_initialize_success() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 5_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [1; 32]),
        },
    ];
    let min_donation = 100;

    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );
    assert!(result.is_ok(), "initialization failed: {:?}", result);

    let campaign_data = client.get_campaign_info();
    assert_eq!(campaign_data.creator, creator);
    assert_eq!(campaign_data.goal_amount, goal_amount);
    assert_eq!(campaign_data.end_time, end_time);
    assert_eq!(campaign_data.status, CampaignStatus::Active);
    assert_eq!(campaign_data.milestone_count, 2);

    let events = env.events().all();
    assert_eq!(events.len(), 1);
}

#[test]
fn test_donate_and_check_milestones() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 5_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [1; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let donor = Address::generate(&env);
    let donation_amount = 6_000;
    let result = client.try_donate(&donor, &donation_amount, &AssetInfo::Native);
    assert!(result.is_ok(), "donation failed: {:?}", result);

    let campaign_data = client.get_campaign_info();
    assert_eq!(campaign_data.raised_amount, donation_amount);

    let milestone1 = client.get_milestone(&0);
    assert_eq!(milestone1.status, MilestoneStatus::Unlocked);

    let milestone2 = client.get_milestone(&1);
    assert_eq!(milestone2.status, MilestoneStatus::Locked);

    let events = env.events().all();
    let donation_event = events.last().unwrap();
    assert_eq!(donation_event.0, contract_id);
    assert_eq!(
        donation_event.1,
        soroban_sdk::vec![&env, Symbol::new(&env, "donation").into_val(&env)]
    );
    let payload: (Address, i128, AssetInfo, i128) = donation_event.2.into_val(&env);
    assert_eq!(
        payload,
        (
            donor.clone(),
            donation_amount,
            AssetInfo::Native,
            donation_amount
        )
    );
}

#[test]
fn test_invalid_milestone_order() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 8_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
        MilestoneInput {
            target_amount: 5_000, // Invalid order
            description_hash: desc_hash(&env, [1; 32]),
        },
    ];
    let min_donation = 100;

    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMilestones)));
}

#[test]
fn test_donation_below_minimum() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let donor = Address::generate(&env);
    let donation_amount = 50; // Below minimum
    let result = client.try_donate(&donor, &donation_amount, &AssetInfo::Native);
    assert_eq!(result, Err(Ok(Error::DonationTooSmall)));
}

#[test]
fn test_get_nonexistent_milestone() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let result = client.try_get_milestone(&1);
    assert_eq!(result, Err(Ok(Error::MilestoneNotFound)));
}

#[test]
fn test_initialize_by_unauthorized_caller() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let unauthorized_caller = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    let result = CampaignContractClient::new(&env, &contract_id).try_initialize(
        &unauthorized_caller,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );
    assert!(result.is_err());
}

#[test]
fn test_re_initialization_prevented() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );
    assert!(result.is_ok());

    let new_goal_amount = 20_000;
    let result = client.try_initialize(
        &creator,
        &new_goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn test_release_milestone_by_non_creator() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let non_creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 5_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [1; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let donor = Address::generate(&env);
    client.donate(&donor, &6_000, &AssetInfo::Native);

    let result =
        CampaignContractClient::new(&env, &contract_id).try_release_milestone(&0, &non_creator);
    assert!(result.is_err());
}

#[test]
fn test_release_milestones_in_order_enforced() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 15_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 5_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [1; 32]),
        },
        MilestoneInput {
            target_amount: 15_000,
            description_hash: desc_hash(&env, [2; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let donor = Address::generate(&env);
    client.donate(&donor, &16_000, &AssetInfo::Native);

    let result = client.try_release_milestone(&2, &creator);
    assert_eq!(result, Err(Ok(Error::PreviousMilestoneNotReleased)));

    let result = client.try_release_milestone(&1, &creator);
    assert_eq!(result, Err(Ok(Error::PreviousMilestoneNotReleased)));

    let result = client.try_release_milestone(&0, &creator);
    assert!(result.is_ok());

    let result = client.try_release_milestone(&1, &creator);
    assert!(result.is_ok());

    let result = client.try_release_milestone(&2, &creator);
    assert!(result.is_ok());
}

#[test]
fn test_cannot_release_milestone_twice() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let donor = Address::generate(&env);
    client.donate(&donor, &10_000, &AssetInfo::Native);

    let result = client.try_release_milestone(&0, &creator);
    assert!(result.is_ok());

    let result = client.try_release_milestone(&0, &creator);
    assert_eq!(result, Err(Ok(Error::MilestoneAlreadyReleased)));
}

#[test]
fn test_cannot_release_locked_milestone() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let result = client.try_release_milestone(&0, &creator);
    assert_eq!(result, Err(Ok(Error::MilestoneNotUnlocked)));
}

#[test]
fn test_donate_freezes_state_validation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let donor = Address::generate(&env);

    let result = client.try_donate(&donor, &5_000, &AssetInfo::Native);
    assert!(result.is_ok());

    let campaign_data = client.get_campaign_info();
    assert_eq!(campaign_data.raised_amount, 5_000);
}

#[test]
fn test_unauthorized_donor_cannot_donate() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let unauthorized_donor = Address::generate(&env);

    let result = CampaignContractClient::new(&env, &contract_id).try_donate(
        &unauthorized_donor,
        &5_000,
        &AssetInfo::Native,
    );
    assert!(result.is_err());
}

#[test]
fn test_donation_validates_campaign_status() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 100;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    env.ledger().with_mut(|l| l.timestamp = end_time + 1);

    let donor = Address::generate(&env);

    let result = client.try_donate(&donor, &5_000, &AssetInfo::Native);
    assert_eq!(result, Err(Ok(Error::CampaignEnded)));
}

// ─── Release Amount Calculation Tests ─────────────────────────────────────

/// Single-milestone campaign: release amount must equal the milestone's
/// target_amount (since `released_amount` starts at 0).
#[test]
fn test_release_amount_single_milestone() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let donor = Address::generate(&env);
    client.donate(&donor, &10_000, &AssetInfo::Native);

    let result = client.try_release_milestone(&0, &creator);
    assert!(result.is_ok());

    let campaign_data = client.get_campaign_info();
    assert_eq!(campaign_data.released_amount, 10_000);
}

// ─── Refund Tests ───────────────────────────────────────────────────────────────

#[test]
fn test_multi_asset_refund_exact_calculation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;

    // Setup multi-asset campaign
    let token_address = Address::generate(&env);
    let accepted_assets = soroban_sdk::vec![
        &env,
        AssetInfo::Native,
        AssetInfo::Token(token_address.clone()),
    ];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    // Set XLM token address for Native asset
    client.set_xlm_token(&token_address);

    let donor = Address::generate(&env);

    // Donate with multiple assets
    client.donate(&donor, &3_000, &AssetInfo::Native);
    client.donate(&donor, &2_500, &AssetInfo::Token(token_address.clone()));

    // Verify donor record
    let donor_record = client.get_donor_record(&donor).unwrap();
    assert_eq!(donor_record.total_donated, 5_500);
    assert_eq!(donor_record.per_asset.len(), 2);

    // Cancel campaign to enable refunds
    client.cancel_campaign();

    // Verify refund eligibility
    assert!(client.is_refund_eligible(&donor));

    // Process refund
    let result = client.try_refund(&donor);
    assert!(result.is_ok(), "refund failed: {:?}", result);

    // Verify donor record was cleared
    let donor_record_after = client.get_donor_record(&donor);
    assert!(donor_record_after.is_none() || donor_record_after.unwrap().total_donated == 0);

    // Verify cannot refund twice
    let result = client.try_refund(&donor);
    assert_eq!(result, Err(Ok(Error::NoRefundAvailable)));
}

#[test]
fn test_refund_window_boundary_at_expiration() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let token_address = Address::generate(&env);
    client.set_xlm_token(&token_address);

    let donor = Address::generate(&env);
    client.donate(&donor, &5_000, &AssetInfo::Native);

    // Cancel campaign
    client.cancel_campaign();

    // Get cancellation time
    let campaign_data = client.get_campaign_info();
    let cancel_time = env.ledger().timestamp();

    // Test refund exactly at window boundary (should succeed)
    env.ledger()
        .with_mut(|l| l.timestamp = cancel_time + 30 * 24 * 60 * 60);
    assert!(client.is_refund_eligible(&donor));
    let result = client.try_refund(&donor);
    assert!(
        result.is_ok(),
        "refund at boundary should succeed: {:?}",
        result
    );
}

#[test]
fn test_refund_window_closed_after_boundary() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let token_address = Address::generate(&env);
    client.set_xlm_token(&token_address);

    let donor = Address::generate(&env);
    client.donate(&donor, &5_000, &AssetInfo::Native);

    // Cancel campaign
    client.cancel_campaign();
    let cancel_time = env.ledger().timestamp();

    // Advance time past refund window
    env.ledger()
        .with_mut(|l| l.timestamp = cancel_time + 30 * 24 * 60 * 60 + 1);

    // Verify not eligible
    assert!(!client.is_refund_eligible(&donor));

    // Try to refund - should fail with RefundWindowClosed
    let result = client.try_refund(&donor);
    assert_eq!(result, Err(Ok(Error::RefundWindowClosed)));
}

#[test]
fn test_refund_only_in_cancelled_or_failed_status() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let token_address = Address::generate(&env);
    client.set_xlm_token(&token_address);

    let donor = Address::generate(&env);
    client.donate(&donor, &5_000, &AssetInfo::Native);

    // Try to refund while campaign is still active - should fail
    assert!(!client.is_refund_eligible(&donor));
    let result = client.try_refund(&donor);
    assert_eq!(result, Err(Ok(Error::CampaignNotActive)));
}

#[test]
fn test_fail_campaign_starts_refund_window() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let token_address = Address::generate(&env);
    client.set_xlm_token(&token_address);

    let donor = Address::generate(&env);
    client.donate(&donor, &5_000, &AssetInfo::Native);

    // Fail campaign
    client.fail_campaign();

    // Verify status is Failed
    let campaign_data = client.get_campaign_info();
    assert_eq!(campaign_data.status, CampaignStatus::Failed);

    // Verify refund eligibility
    assert!(client.is_refund_eligible(&donor));

    // Process refund
    let result = client.try_refund(&donor);
    assert!(result.is_ok(), "refund after failure failed: {:?}", result);
}

#[test]
fn test_refund_requires_donor_authorization() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let asset_a = AssetInfo::Native;
    let asset_b_addr = Address::generate(&env);
    let asset_b = AssetInfo::Token(asset_b_addr);
    let accepted_assets = soroban_sdk::vec![&env, asset_a, asset_b.clone()];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let token_address = Address::generate(&env);
    client.set_xlm_token(&token_address);

    let donor = Address::generate(&env);
    client.donate(&donor, &5_000, &AssetInfo::Native);

    // Cancel campaign
    client.cancel_campaign();

    // Try to refund without donor authorization
    let unauthorized_caller = Address::generate(&env);
    let result = CampaignContractClient::new(&env, &contract_id).try_refund(&donor);
    // Authorization should fail at require_auth()
    assert!(result.is_err());
}

#[test]
fn test_get_refund_window_remaining() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    // Before cancellation, should return None
    assert_eq!(client.get_refund_window_remaining(), None);

    // Cancel campaign
    client.cancel_campaign();
    let cancel_time = env.ledger().timestamp();

    // Should return remaining time
    let remaining = client.get_refund_window_remaining();
    assert!(remaining.is_some());
    assert_eq!(remaining.unwrap(), 30 * 24 * 60 * 60);

    // Advance time by 10 days
    env.ledger()
        .with_mut(|l| l.timestamp = cancel_time + 10 * 24 * 60 * 60);
    let remaining = client.get_refund_window_remaining();
    assert!(remaining.is_some());
    assert_eq!(remaining.unwrap(), 20 * 24 * 60 * 60);

    // After window closes, should return None
    env.ledger()
        .with_mut(|l| l.timestamp = cancel_time + 30 * 24 * 60 * 60 + 1);
    assert_eq!(client.get_refund_window_remaining(), None);
}

#[test]
fn test_get_refundable_amount() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let token_address = Address::generate(&env);
    client.set_xlm_token(&token_address);

    let donor = Address::generate(&env);

    // Initially no refundable amount
    assert_eq!(client.get_refundable_amount(&donor), 0);

    // After donation
    client.donate(&donor, &5_000, &AssetInfo::Native);
    assert_eq!(client.get_refundable_amount(&donor), 5_000);

    // After refund
    client.cancel_campaign();
    client.refund(&donor);
    assert_eq!(client.get_refundable_amount(&donor), 0);
}

#[test]
fn test_cancel_campaign_with_one_stroop_raised_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    // Minimum donation of 1 stroop so a single-stroop donation is accepted
    let min_donation = 1;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    let token_address = Address::generate(&env);
    client.set_xlm_token(&token_address);

    let donor = Address::generate(&env);
    client.donate(&donor, &1, &AssetInfo::Native);

    // Cancel must fail once any funds have been raised (even a single stroop)
    let result = client.try_cancel_campaign();
    assert_eq!(result, Err(Ok(Error::CannotCancelWithFunds)));

    // Status remains Active
    let campaign_data = client.get_campaign_info();
    assert_eq!(campaign_data.status, CampaignStatus::Active);
}

#[test]
fn test_extend_deadline_beyond_90_day_cap_fails() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let day: u64 = 24 * 60 * 60;
    let original_end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &original_end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    // Extension exactly to the cap (original + 90 days) is allowed
    let capped_end_time = original_end_time + 90 * day;
    let result = client.try_extend_deadline(&capped_end_time);
    assert!(result.is_ok(), "extension to the 90-day cap failed");
    assert_eq!(client.get_campaign_info().end_time, capped_end_time);

    // Extension one second past the cap (original + 90 days) must fail
    let result = client.try_extend_deadline(&(capped_end_time + 1));
    assert_eq!(result, Err(Ok(Error::DeadlineExceedsLimit)));

    // End time is unchanged after the rejected extension
    assert_eq!(client.get_campaign_info().end_time, capped_end_time);
}

#[test]
fn test_extend_deadline_chained_twice() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let day: u64 = 24 * 60 * 60;
    let original_end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &original_end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    // First extension: original + 30 days
    let first_extension = original_end_time + 30 * day;
    client.extend_deadline(&first_extension);
    assert_eq!(client.get_campaign_info().end_time, first_extension);

    // Second chained extension: still measured against the ORIGINAL deadline.
    // original + 90 days is allowed; anything past it is rejected even though
    // it is later than the current end time.
    let second_extension = original_end_time + 89 * day;
    client.extend_deadline(&second_extension);
    assert_eq!(client.get_campaign_info().end_time, second_extension);

    // The cap applies relative to the original end time, not the latest one
    let result = client.try_extend_deadline(&(original_end_time + 91 * day));
    assert_eq!(result, Err(Ok(Error::DeadlineExceedsLimit)));
}

#[test]
fn test_get_campaign_status_days_remaining() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let day: u64 = 24 * 60 * 60;
    let start = env.ledger().timestamp();
    let end_time = start + 5 * day;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    // 5 full days remain
    let (status, days_remaining) = client.get_campaign_status();
    assert_eq!(status, CampaignStatus::Active);
    assert_eq!(days_remaining, 5);

    // Advance to 12 hours before the deadline: partial day rounds up to 1
    env.ledger().with_mut(|l| l.timestamp = end_time - day / 2);
    let (_, days_remaining) = client.get_campaign_status();
    assert_eq!(days_remaining, 1);

    // After the deadline days_remaining becomes negative
    env.ledger().with_mut(|l| l.timestamp = end_time + 3 * day);
    let (_, days_remaining) = client.get_campaign_status();
    assert_eq!(days_remaining, -3);
}

#[test]
fn test_end_campaign_by_creator() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    // Creator ends the campaign before the deadline
    client.end_campaign();
    assert_eq!(client.get_campaign_info().status, CampaignStatus::Ended);

    // Ending again fails because the campaign is already Ended
    let result = client.try_end_campaign();
    assert_eq!(result, Err(Ok(Error::CampaignNotActive)));
}

#[test]
fn test_update_status_transitions_expired_active_campaign() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    // Advance past the deadline: status stays Active until someone acts
    env.ledger().with_mut(|l| l.timestamp = end_time + 1);
    assert_eq!(client.get_campaign_info().status, CampaignStatus::Active);

    // A random non-creator account can trigger the transition
    let anyone = Address::generate(&env);
    let client_anyone = CampaignContractClient::new(&env, &contract_id);
    let _ = anyone;
    client_anyone.update_status();
    assert_eq!(client.get_campaign_info().status, CampaignStatus::Ended);

    // Idempotent: calling again on an already-Ended campaign is a no-op
    client.update_status();
    assert_eq!(client.get_campaign_info().status, CampaignStatus::Ended);
}

#[test]
fn test_update_status_before_deadline_is_noop() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    // Before the deadline update_status must not change anything
    client.update_status();
    assert_eq!(client.get_campaign_info().status, CampaignStatus::Active);
}

#[test]
fn test_donation_after_deadline_marks_campaign_ended() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);

    let creator = Address::generate(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: desc_hash(&env, [0; 32]),
        },
    ];
    let min_donation = 100;

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );

    // Advance past the deadline
    env.ledger().with_mut(|l| l.timestamp = end_time + 1);
    assert_eq!(client.get_campaign_info().status, CampaignStatus::Active);

    // Donation attempt rejects and triggers the transition to Ended
    let donor = Address::generate(&env);
    let result = client.try_donate(&donor, &500, &AssetInfo::Native);
    assert_eq!(result, Err(Ok(Error::CampaignEnded)));
    assert_eq!(client.get_campaign_info().status, CampaignStatus::Ended);
}
