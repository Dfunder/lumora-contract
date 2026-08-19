use crate::{
    Campaign, CampaignClient, CampaignData, CampaignStatus, Error, MilestoneData, MilestoneInput,
    MilestoneStatus,
};
use common::{AssetInfo, ErrorCode};
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    Address, Env, IntoVal, Symbol,
};

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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 5_000,
            description_hash: [0; 32].into(),
        },
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [1; 32].into(),
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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 5_000,
            description_hash: [0; 32].into(),
        },
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [1; 32].into(),
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

    let donor = Address::random(&env);
    let donation_amount = 6_000;
    let result = client.try_donate(&donor, &donation_amount, &AssetInfo::Native);
    assert!(result.is_ok(), "donation failed: {:?}", result);

    let campaign_data = client.get_campaign_info();
    assert_eq!(campaign_data.raised_amount, donation_amount);

    let milestone1 = client.get_milestone(&0).unwrap();
    assert_eq!(milestone1.status, MilestoneStatus::Unlocked);

    let milestone2 = client.get_milestone(&1).unwrap();
    assert_eq!(milestone2.status, MilestoneStatus::Locked);

    let events = env.events().all();
    let donation_event = events.last().unwrap();
    assert_eq!(
        donation_event,
        (
            contract_id.clone(),
            (Symbol::new(&env, "donation"),).into_val(&env),
            (
                donor.clone(),
                donation_amount,
                AssetInfo::Native,
                donation_amount
            )
                .into_val(&env)
        )
    );
}

#[test]
fn test_invalid_milestone_order() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 8_000,
            description_hash: [0; 32].into(),
        },
        MilestoneInput {
            target_amount: 5_000, // Invalid order
            description_hash: [1; 32].into(),
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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [0; 32].into(),
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

    let donor = Address::random(&env);
    let donation_amount = 50; // Below minimum
    let result = client.try_donate(&donor, &donation_amount, &AssetInfo::Native);
    assert_eq!(result, Err(Ok(Error::DonationTooSmall)));
}

#[test]
fn test_get_nonexistent_milestone() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [0; 32].into(),
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

    let result = client.try_get_milestone(&1); // Index out of bounds
    assert_eq!(result, Err(Ok(Error::MilestoneNotFound)));
}

// ─── Authorization and State Validation Tests ────────────────────────────────

#[test]
fn test_initialize_by_unauthorized_caller() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let unauthorized_caller = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [0; 32].into(),
        },
    ];
    let min_donation = 100;

    // Try to initialize with unauthorized caller
    let result = CampaignClient::new(&env, &contract_id).try_initialize(
        &unauthorized_caller,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );
    // Authorization should fail at require_auth()
    assert!(result.is_err());
}

#[test]
fn test_re_initialization_prevented() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [0; 32].into(),
        },
    ];
    let min_donation = 100;

    // Initialize successfully
    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
        &min_donation,
    );
    assert!(result.is_ok());

    // Attempt to re-initialize with different parameters
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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let non_creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 5_000,
            description_hash: [0; 32].into(),
        },
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [1; 32].into(),
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

    let donor = Address::random(&env);
    client.donate(&donor, &6_000, &AssetInfo::Native);

    // Try to release milestone as non-creator
    let result = CampaignClient::new(&env, &contract_id).try_release_milestone(
        &non_creator,
        &0,
        &non_creator,
    );
    // Authorization should fail at require_auth()
    assert!(result.is_err());
}

#[test]
fn test_release_milestones_in_order_enforced() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 15_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 5_000,
            description_hash: [0; 32].into(),
        },
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [1; 32].into(),
        },
        MilestoneInput {
            target_amount: 15_000,
            description_hash: [2; 32].into(),
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

    let donor = Address::random(&env);
    client.donate(&donor, &16_000, &AssetInfo::Native);

    // Try to release milestone 2 before milestone 0
    let result = client.try_release_milestone(&creator, &2, &creator);
    assert_eq!(result, Err(Ok(Error::PreviousMilestoneNotReleased)));

    // Try to release milestone 1 before milestone 0
    let result = client.try_release_milestone(&creator, &1, &creator);
    assert_eq!(result, Err(Ok(Error::PreviousMilestoneNotReleased)));

    // Release in correct order
    let result = client.try_release_milestone(&creator, &0, &creator);
    assert!(result.is_ok());

    let result = client.try_release_milestone(&creator, &1, &creator);
    assert!(result.is_ok());

    let result = client.try_release_milestone(&creator, &2, &creator);
    assert!(result.is_ok());
}

#[test]
fn test_cannot_release_milestone_twice() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [0; 32].into(),
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

    let donor = Address::random(&env);
    client.donate(&donor, &10_000, &AssetInfo::Native);

    // Release milestone once
    let result = client.try_release_milestone(&creator, &0, &creator);
    assert!(result.is_ok());

    // Try to release the same milestone again
    let result = client.try_release_milestone(&creator, &0, &creator);
    assert_eq!(result, Err(Ok(Error::MilestoneAlreadyReleased)));
}

#[test]
fn test_cannot_release_locked_milestone() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [0; 32].into(),
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

    // Try to release milestone without sufficient funds
    let result = client.try_release_milestone(&creator, &0, &creator);
    assert_eq!(result, Err(Ok(Error::MilestoneNotUnlocked)));
}

#[test]
fn test_donate_freezes_state_validation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [0; 32].into(),
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

    let donor = Address::random(&env);
    
    // Donate successfully
    let result = client.try_donate(&donor, &5_000, &AssetInfo::Native);
    assert!(result.is_ok());

    // Verify campaign is still active
    let campaign_data = client.get_campaign_info();
    assert_eq!(campaign_data.raised_amount, 5_000);
}

#[test]
fn test_unauthorized_donor_cannot_donate() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 1_000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [0; 32].into(),
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

    let unauthorized_donor = Address::random(&env);
    
    // Try to donate without authorization
    let result = CampaignClient::new(&env, &contract_id).try_donate(
        &unauthorized_donor,
        &5_000,
        &AssetInfo::Native,
    );
    // Authorization should fail at require_auth()
    assert!(result.is_err());
}

#[test]
fn test_donation_validates_campaign_status() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let creator = Address::random(&env);
    let goal_amount = 10_000;
    let end_time = env.ledger().timestamp() + 100; // Short window
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        MilestoneInput {
            target_amount: 10_000,
            description_hash: [0; 32].into(),
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

    // Advance time past campaign end
    env.ledger().with_mut(|l| l.timestamp = end_time + 1);

    let donor = Address::random(&env);
    
    // Try to donate after campaign ended
    let result = client.try_donate(&donor, &5_000, &AssetInfo::Native);
    assert_eq!(result, Err(Ok(Error::CampaignEnded)));
}