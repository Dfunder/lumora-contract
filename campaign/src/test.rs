use crate::{
    Campaign, CampaignClient, CampaignData, CampaignStatus, Error, MilestoneData, MilestoneInput,
    MilestoneStatus,
};
use common::{AssetInfo, ErrorCode};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, BytesN, Env, IntoVal, Symbol,
};

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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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

    let result = CampaignClient::new(&env, &contract_id).try_initialize(
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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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

    let result = CampaignClient::new(&env, &contract_id).try_release_milestone(
        &non_creator,
        &0,
        &non_creator,
    );
    assert!(result.is_err());
}

#[test]
fn test_release_milestones_in_order_enforced() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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

    let result = client.try_release_milestone(&creator, &2, &creator);
    assert_eq!(result, Err(Ok(Error::PreviousMilestoneNotReleased)));

    let result = client.try_release_milestone(&creator, &1, &creator);
    assert_eq!(result, Err(Ok(Error::PreviousMilestoneNotReleased)));

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

    let result = client.try_release_milestone(&creator, &0, &creator);
    assert!(result.is_ok());

    let result = client.try_release_milestone(&creator, &0, &creator);
    assert_eq!(result, Err(Ok(Error::MilestoneAlreadyReleased)));
}

#[test]
fn test_cannot_release_locked_milestone() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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

    let result = client.try_release_milestone(&creator, &0, &creator);
    assert_eq!(result, Err(Ok(Error::MilestoneNotUnlocked)));
}

#[test]
fn test_donate_freezes_state_validation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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

    let result = CampaignClient::new(&env, &contract_id).try_donate(
        &unauthorized_donor,
        &5_000,
        &AssetInfo::Native,
    );
    assert!(result.is_err());
}

#[test]
fn test_donation_validates_campaign_status() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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

    client.release_milestone(&creator, &0, &creator);

    let data = client.get_campaign_info();
    assert_eq!(data.released_amount, 10_000);

    let milestone = client.get_milestone(&0).unwrap();
    assert_eq!(milestone.status, MilestoneStatus::Released);
    assert!(milestone.released_at.is_some());
}

/// Multi-milestone campaign: verify that each release amount is the delta
/// between consecutive milestone targets. Specifically checks the final
/// milestone release amount = target[n] - target[n-1].
#[test]
fn test_release_amount_final_milestone_delta() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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

    client.release_milestone(&creator, &0, &creator);
    assert_eq!(client.get_campaign_info().released_amount, 5_000);

    client.release_milestone(&creator, &1, &creator);
    assert_eq!(client.get_campaign_info().released_amount, 10_000);

    client.release_milestone(&creator, &2, &creator);
    assert_eq!(client.get_campaign_info().released_amount, 15_000);
}

/// Verifies that `milestone_released` is emitted once per asset transferred,
/// with the correct event name and payload shape:
/// { milestone_index, amount, asset, recipient, timestamp }
#[test]
fn test_milestone_released_event_per_asset() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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

    let pre_release_ts = env.ledger().timestamp();
    env.ledger().with_mut(|l| l.timestamp = pre_release_ts + 1);

    client.release_milestone(&creator, &0, &creator);

    let post_release_ts = env.ledger().timestamp();

    let events = env.events().all();
    let milestone_released_sym = Symbol::new(&env, "milestone_released");
    let mut release_events = soroban_sdk::Vec::new(&env);
    for i in 0..events.len() {
        let event = events.get(i).unwrap();
        let topics: soroban_sdk::Vec<Symbol> = event.1.try_into().unwrap();
        if topics.get(0) == Some(milestone_released_sym.clone()) {
            release_events.push_back(event);
        }
    }

    assert_eq!(
        release_events.len(),
        1,
        "expected exactly one milestone_released event for single-asset release"
    );

    let event = release_events.get(0).unwrap();
    assert_eq!(event.0, contract_id);

    let data: soroban_sdk::Vec<soroban_sdk::Val> = event.2.try_into().unwrap();
    assert_eq!(data.len(), 5, "event must have 5 fields");

    assert_eq!(data.get(0).unwrap(), 0_u32.into_val(&env));
    assert_eq!(data.get(1).unwrap(), 10_000_i128.into_val(&env));
    assert_eq!(data.get(2).unwrap(), AssetInfo::Native.into_val(&env));
    assert_eq!(data.get(3).unwrap(), creator.into_val(&env));
    let ts: u64 = data.get(4).unwrap().try_into().unwrap();
    assert!(
        ts >= pre_release_ts && ts <= post_release_ts,
        "timestamp {} should be in range [{},{}]",
        ts,
        pre_release_ts,
        post_release_ts
    );
}

/// Verifies that a multi-asset release emits one milestone_released event
/// per accepted asset that has a non-zero balance (not a single combined event).
#[test]
fn test_milestone_released_event_one_per_asset_multi_asset() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

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

    let donor = Address::generate(&env);
    client.donate(&donor, &6_000, &asset_a);
    client.donate(&donor, &5_000, &asset_b);

    let events_before = env.events().all().len();

    client.release_milestone(&creator, &0, &creator);

    let all_events = env.events().all();
    let milestone_released_sym = Symbol::new(&env, "milestone_released");
    let mut release_events = soroban_sdk::Vec::new(&env);
    for i in events_before..all_events.len() {
        let event = all_events.get(i).unwrap();
        let topics: soroban_sdk::Vec<Symbol> = event.1.try_into().unwrap();
        if topics.get(0) == Some(milestone_released_sym.clone()) {
            release_events.push_back(event);
        }
    }

    assert_eq!(
        release_events.len(),
        2,
        "expected one milestone_released event per asset"
    );

    let asset_0: AssetInfo = {
        let data: soroban_sdk::Vec<soroban_sdk::Val> =
            release_events.get(0).unwrap().2.try_into().unwrap();
        data.get(2).unwrap().try_into().unwrap()
    };
    let asset_1: AssetInfo = {
        let data: soroban_sdk::Vec<soroban_sdk::Val> =
            release_events.get(1).unwrap().2.try_into().unwrap();
        data.get(2).unwrap().try_into().unwrap()
    };
    assert_ne!(asset_0, asset_1, "events should be for different assets");

    let data = client.get_campaign_info();
    assert_eq!(data.released_amount, 10_000);
}
