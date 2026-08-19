#[test]
fn test_release_milestone() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let owner = Address::random(&env);
    let goal_amount = 1000;
    let deadline = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        Milestone {
            name: "Milestone 1".into_val(&env),
            description: "First milestone".into_val(&env),
            target_amount: 500,
            deadline: deadline - 500,
            status: MilestoneStatus::Locked,
            released_at: None,
        },
    ];

    client.initialize(
        &owner,
        &"Test Campaign".into_val(&env),
        &"A campaign for testing".into_val(&env),
        &goal_amount,
        &deadline,
        &accepted_assets,
        &milestones,
    );

    let donor = Address::random(&env);
    client.donate(&donor, &500, &AssetInfo::Native);

    let recipient = Address::random(&env);
    client.release_milestone(&0, &recipient);

    let campaign_data = client.get_campaign_data();
    assert_eq!(campaign_data.released_amount, 500);
    assert_eq!(campaign_data.next_releasable_milestone, 1);

    let milestone_data = client.get_milestone_data(&0);
    assert_eq!(milestone_data.status, MilestoneStatus::Released);
    assert!(milestone_data.released_at.is_some());
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")]
fn test_release_milestone_overflow() {
    let env = Env::default();
    let contract_id = env.register_contract(None, Campaign);
    let client = CampaignClient::new(&env, &contract_id);

    let owner = Address::random(&env);
    let goal_amount = i128::MAX;
    let deadline = env.ledger().timestamp() + 1000;
    let accepted_assets = soroban_sdk::vec![&env, AssetInfo::Native];
    let milestones = soroban_sdk::vec![
        &env,
        Milestone {
            name: "Milestone 1".into_val(&env),
            description: "First milestone".into_val(&env),
            target_amount: i128::MAX,
            deadline: deadline - 500,
            status: MilestoneStatus::Locked,
            released_at: None,
        },
    ];

    client.initialize(
        &owner,
        &"Test Campaign".into_val(&env),
        &"A campaign for testing".into_val(&env),
        &goal_amount,
        &deadline,
        &accepted_assets,
        &milestones,
    );

    let donor = Address::random(&env);
    client.donate(&donor, &i128::MAX, &AssetInfo::Native);

    let recipient = Address::random(&env);
    client.release_milestone(&0, &recipient);
}