use crate::{CampaignContract, CampaignContractClient, MilestoneInput};
use common::AssetInfo;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    token::StellarAssetClient,
    Address, BytesN, Env, IntoVal, Symbol, Val, Vec,
};

fn milestone(target_amount: i128, env: &Env) -> MilestoneInput {
    MilestoneInput {
        target_amount,
        description_hash: BytesN::from_array(env, &[0; 32]),
    }
}

fn register_funded_token(env: &Env, owner: &Address, amount: i128) -> Address {
    let admin = Address::generate(env);
    let token = env.register_stellar_asset_contract(admin);
    StellarAssetClient::new(env, &token).mint(owner, &amount);
    token
}

#[test]
fn milestone_unlocked_event_has_required_payload_and_does_not_repeat() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let donor = Address::generate(&env);
    let token = register_funded_token(&env, &donor, 1_000);
    let asset = AssetInfo::Token(token);

    client.initialize(
        &creator,
        &1_000,
        &(env.ledger().timestamp() + 1_000),
        &soroban_sdk::vec![&env, asset.clone()],
        &soroban_sdk::vec![&env, milestone(500, &env), milestone(1_000, &env)],
        &1,
    );

    client.donate(&donor, &600, &asset);

    let event_topics: Vec<Val> =
        (Symbol::new(&env, "milestone_unlocked"), contract_id.clone()).into_val(&env);
    let expected_first_data = (0_u32, 500_i128, 600_i128);
    let mut first_unlock_count = 0;

    for event in env.events().all().iter() {
        if event.1 == event_topics {
            first_unlock_count += 1;
            assert_eq!(event.0, contract_id);
            let data: (u32, i128, i128) = event.2.into_val(&env);
            assert_eq!(data, expected_first_data);
        }
    }
    assert_eq!(first_unlock_count, 1);

    // The milestone remains unlocked, so a later donation must not re-emit it.
    client.donate(&donor, &100, &asset);

    let unlock_count_after_later_donation = env
        .events()
        .all()
        .iter()
        .filter(|event| event.1 == event_topics)
        .count();
    assert_eq!(unlock_count_after_later_donation, 1);

    // Crossing the next threshold emits one new event for that milestone only.
    client.donate(&donor, &300, &asset);
    let expected_second_data = (1_u32, 1_000_i128, 1_000_i128);
    let mut first_seen = 0;
    let mut second_seen = 0;
    for event in env.events().all().iter() {
        if event.1 == event_topics {
            let data: (u32, i128, i128) = event.2.into_val(&env);
            if data == expected_first_data {
                first_seen += 1;
            } else if data == expected_second_data {
                second_seen += 1;
            }
        }
    }
    assert_eq!((first_seen, second_seen), (1, 1));
}

#[test]
fn milestone_released_event_has_required_payload_for_each_asset() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let donor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let asset_a = AssetInfo::Token(register_funded_token(&env, &donor, 6_000));
    let asset_b = AssetInfo::Token(register_funded_token(&env, &donor, 4_000));

    client.initialize(
        &creator,
        &10_000,
        &(env.ledger().timestamp() + 1_000),
        &soroban_sdk::vec![&env, asset_a.clone(), asset_b.clone()],
        &soroban_sdk::vec![&env, milestone(10_000, &env)],
        &1,
    );
    client.donate(&donor, &6_000, &asset_a);
    client.donate(&donor, &4_000, &asset_b);

    env.ledger().with_mut(|ledger| ledger.timestamp = 777);
    client.release_milestone(&0, &recipient);

    let event_topics: Vec<Val> =
        (Symbol::new(&env, "milestone_released"), contract_id.clone()).into_val(&env);
    let expected_a = (0_u32, 6_000_i128, asset_a, recipient.clone(), 777_u64);
    let expected_b = (0_u32, 4_000_i128, asset_b, recipient, 777_u64);
    let mut asset_a_events = 0;
    let mut asset_b_events = 0;

    for event in env.events().all().iter() {
        if event.1 == event_topics {
            assert_eq!(event.0, contract_id);
            let data: (u32, i128, AssetInfo, Address, u64) = event.2.into_val(&env);
            if data == expected_a {
                asset_a_events += 1;
            } else if data == expected_b {
                asset_b_events += 1;
            } else {
                panic!("unexpected milestone_released payload");
            }
        }
    }

    assert_eq!((asset_a_events, asset_b_events), (1, 1));
}

#[test]
fn donation_received_event_has_required_payload_with_timestamp() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let donor = Address::generate(&env);
    let token = register_funded_token(&env, &donor, 1_000);
    let asset = AssetInfo::Token(token.clone());

    client.initialize(
        &creator,
        &1_000,
        &(env.ledger().timestamp() + 1_000),
        &soroban_sdk::vec![&env, asset.clone()],
        &soroban_sdk::vec![&env, milestone(1_000, &env)],
        &1,
    );

    env.ledger().with_mut(|ledger| ledger.timestamp = 12345);
    client.donate(&donor, &500, &asset);

    let event_topics: Vec<Val> =
        (Symbol::new(&env, "donation_received"), contract_id.clone()).into_val(&env);
    let expected_data = (
        donor.clone(),
        500_i128,
        token.to_string(),
        500_i128,
        12345_u64,
    );

    let mut donation_events = 0;
    for event in env.events().all().iter() {
        if event.1 == event_topics {
            donation_events += 1;
            assert_eq!(event.0, contract_id);
            let data: (Address, i128, soroban_sdk::String, i128, u64) = event.2.into_val(&env);
            assert_eq!(data, expected_data);
        }
    }
    assert_eq!(donation_events, 1);
}

#[test]
fn refund_issued_event_has_required_payload_for_each_asset() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let donor = Address::generate(&env);
    let asset_a = AssetInfo::Token(register_funded_token(&env, &donor, 3_000));
    let asset_b = AssetInfo::Token(register_funded_token(&env, &donor, 2_000));

    client.initialize(
        &creator,
        &10_000,
        &(env.ledger().timestamp() + 1_000),
        &soroban_sdk::vec![&env, asset_a.clone(), asset_b.clone()],
        &soroban_sdk::vec![&env, milestone(10_000, &env)],
        &1,
    );
    client.donate(&donor, &3_000, &asset_a);
    client.donate(&donor, &2_000, &asset_b);

    client.cancel_campaign();
    client.request_refund(&donor);

    let event_topics: Vec<Val> =
        (Symbol::new(&env, "refund_issued"), contract_id.clone()).into_val(&env);
    let expected_a = (donor.clone(), 3_000_i128, asset_a);
    let expected_b = (donor.clone(), 2_000_i128, asset_b);
    let mut asset_a_events = 0;
    let mut asset_b_events = 0;

    for event in env.events().all().iter() {
        if event.1 == event_topics {
            assert_eq!(event.0, contract_id);
            let data: (Address, i128, AssetInfo) = event.2.into_val(&env);
            if data == expected_a {
                asset_a_events += 1;
            } else if data == expected_b {
                asset_b_events += 1;
            } else {
                panic!("unexpected refund_issued payload");
            }
        }
    }

    assert_eq!((asset_a_events, asset_b_events), (1, 1));
}

#[test]
fn campaign_initialized_event_includes_contract_address() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let token = register_funded_token(&env, &creator, 1_000);
    let asset = AssetInfo::Token(token);

    client.initialize(
        &creator,
        &1_000,
        &(env.ledger().timestamp() + 1_000),
        &soroban_sdk::vec![&env, asset.clone()],
        &soroban_sdk::vec![&env, milestone(1_000, &env)],
        &1,
    );

    let event_topics: Vec<Val> = (
        Symbol::new(&env, "campaign_initialized"),
        contract_id.clone(),
        creator.clone(),
    )
        .into_val(&env);

    let mut init_events = 0;
    for event in env.events().all().iter() {
        if event.1 == event_topics {
            init_events += 1;
            assert_eq!(event.0, contract_id);
        }
    }
    assert_eq!(init_events, 1);
}
