use super::{
    AssetInfo, CampaignContract, CampaignContractClient, CampaignStatus, Error, MilestoneInput,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, BytesN, Env, Vec,
};

fn setup(now: u64) -> (Env, CampaignContractClient<'static>) {
    let env = Env::default();
    env.ledger().set(LedgerInfo {
        timestamp: now,
        protocol_version: 20,
        sequence_number: 1,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 16,
        max_entry_ttl: 6_312_000,
    });
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);
    (env, client)
}

fn asset() -> AssetInfo {
    AssetInfo::Native
}

fn milestone(env: &Env, target_amount: i128) -> MilestoneInput {
    MilestoneInput {
        target_amount,
        description_hash: BytesN::from_array(env, &[0u8; 32]),
    }
}

fn ascending_milestones(env: &Env, goal_amount: i128) -> Vec<MilestoneInput> {
    let mut milestones = Vec::new(env);
    milestones.push_back(milestone(env, goal_amount / 2));
    milestones.push_back(milestone(env, goal_amount));
    milestones
}

#[test]
fn initialize_with_valid_parameters_succeeds() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let end_time = now + 1_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);
    let milestones = ascending_milestones(&env, goal_amount);

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
    );

    let data = client.get_campaign_info();
    assert_eq!(data.creator, creator);
    assert_eq!(data.goal_amount, goal_amount);
    assert_eq!(data.raised_amount, 0);
    assert_eq!(data.end_time, end_time);
    assert_eq!(data.status, CampaignStatus::Active);
    assert_eq!(data.accepted_assets, accepted_assets);
    assert_eq!(data.milestone_count, 2);
}

#[test]
fn reinitialize_with_different_params_fails() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let end_time = now + 1_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);
    let milestones = ascending_milestones(&env, goal_amount);

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
    );

    let other_creator = Address::generate(&env);
    let other_goal: i128 = 5_000;
    let other_end_time = now + 2_000;
    let other_milestones = ascending_milestones(&env, other_goal);

    let result = client.try_initialize(
        &other_creator,
        &other_goal,
        &other_end_time,
        &accepted_assets,
        &other_milestones,
    );
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));

    // Original campaign data must remain untouched.
    let data = client.get_campaign_info();
    assert_eq!(data.creator, creator);
    assert_eq!(data.goal_amount, goal_amount);
}

#[test]
fn zero_goal_amount_fails() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let end_time = now + 1_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);
    let milestones = ascending_milestones(&env, 0);

    let result = client.try_initialize(&creator, &0, &end_time, &accepted_assets, &milestones);
    assert_eq!(result, Err(Ok(Error::InvalidGoalAmount)));
}

#[test]
fn past_end_time_fails() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);
    let milestones = ascending_milestones(&env, goal_amount);

    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &(now - 1),
        &accepted_assets,
        &milestones,
    );
    assert_eq!(result, Err(Ok(Error::InvalidEndTime)));
}

#[test]
fn end_time_equal_to_now_fails() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);
    let milestones = ascending_milestones(&env, goal_amount);

    let result = client.try_initialize(&creator, &goal_amount, &now, &accepted_assets, &milestones);
    assert_eq!(result, Err(Ok(Error::InvalidEndTime)));
}

#[test]
fn no_accepted_assets_fails() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let end_time = now + 1_000;
    let accepted_assets: Vec<AssetInfo> = Vec::new(&env);
    let milestones = ascending_milestones(&env, goal_amount);

    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
    );
    assert_eq!(result, Err(Ok(Error::NoAcceptedAssets)));
}

#[test]
fn non_ascending_milestones_fails() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let end_time = now + 1_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);

    let mut milestones = Vec::new(&env);
    milestones.push_back(milestone(&env, 6_000));
    milestones.push_back(milestone(&env, 5_000));
    milestones.push_back(milestone(&env, goal_amount));

    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMilestones)));
}

#[test]
fn non_strictly_ascending_milestones_fails() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let end_time = now + 1_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);

    let mut milestones = Vec::new(&env);
    milestones.push_back(milestone(&env, 5_000));
    milestones.push_back(milestone(&env, 5_000));
    milestones.push_back(milestone(&env, goal_amount));

    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMilestones)));
}

#[test]
fn final_milestone_not_equal_to_goal_fails() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let end_time = now + 1_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);

    let mut milestones = Vec::new(&env);
    milestones.push_back(milestone(&env, 4_000));
    milestones.push_back(milestone(&env, 9_000));

    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMilestones)));
}

#[test]
fn single_milestone_equal_to_goal_succeeds() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let end_time = now + 1_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);

    let mut milestones = Vec::new(&env);
    milestones.push_back(milestone(&env, goal_amount));

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
    );

    let data = client.get_campaign_info();
    assert_eq!(data.milestone_count, 1);
    assert_eq!(data.status, CampaignStatus::Active);
}

#[test]
fn zero_milestones_fails() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let end_time = now + 1_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);
    let milestones: Vec<MilestoneInput> = Vec::new(&env);

    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMilestones)));
}

#[test]
fn more_than_five_milestones_fails() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 12_000;
    let end_time = now + 1_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);

    let mut milestones = Vec::new(&env);
    for i in 1..=6 {
        milestones.push_back(milestone(&env, i * 2_000));
    }

    let result = client.try_initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMilestones)));
}

#[test]
fn initialize_requires_creator_auth() {
    let now = 1_000;
    let (env, client) = setup(now);
    let creator = Address::generate(&env);
    let goal_amount: i128 = 10_000;
    let end_time = now + 1_000;
    let accepted_assets = Vec::from_array(&env, [asset()]);
    let milestones = ascending_milestones(&env, goal_amount);

    client.initialize(
        &creator,
        &goal_amount,
        &end_time,
        &accepted_assets,
        &milestones,
    );

    // `setup` mocks all auths, so the call above succeeds regardless of who
    // signed it. What we can verify is that the contract actually demanded
    // the creator's authorization (and no one else's) while doing so.
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, creator);
}
