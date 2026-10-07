#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::Map;

fn create_token_contract<'a>(
    env: &Env,
    admin: &Address,
) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let address = sac.address();
    (
        TokenClient::new(env, &address),
        StellarAssetClient::new(env, &address),
    )
}

fn create_token_prices(env: &Env, token: &Address, price: i128) -> Map<Address, i128> {
    let mut map = Map::new(env);
    map.set(token.clone(), price);
    map
}

#[test]
fn test_free_event_register_and_checkin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _) = create_token_contract(&env, &token_admin);

    let prices = create_token_prices(&env, &token.address, 0);

    client.create_event(&organizer, &1, &prices, &100, &false, &0);
    client.register(&attendee, &1, &token.address);

    client.check_in(&organizer, &1, &attendee);

    let checked_in: bool = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::CheckedIn(1, attendee.clone()))
            .unwrap()
    });
    assert!(checked_in);
}

#[test]
fn test_check_in_is_idempotent() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _) = create_token_contract(&env, &token_admin);

    let prices = create_token_prices(&env, &token.address, 0);

    client.create_event(&organizer, &1, &prices, &100, &false, &0);
    client.register(&attendee, &1, &token.address);

    client.check_in(&organizer, &1, &attendee);
    client.check_in(&organizer, &1, &attendee);

    let checked_in: bool = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::CheckedIn(1, attendee.clone()))
            .unwrap()
    });
    assert!(checked_in);
}

#[test]
fn test_paid_event_register_transfers_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &0);
    client.register(&attendee, &1, &token.address);

    assert_eq!(token.balance(&attendee), 800);
    assert_eq!(token.balance(&contract_id), 200);
    assert_eq!(token.balance(&organizer), 0);
}

#[test]
fn test_payout_moves_escrowed_funds_to_organizer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &0);
    client.register(&attendee, &1, &token.address);
    assert_eq!(token.balance(&contract_id), 200);

    client.payout(&organizer, &1);

    assert_eq!(token.balance(&contract_id), 0);
    assert_eq!(token.balance(&organizer), 200);
}

#[test]
fn test_duplicate_registration_fails_without_double_charge() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);
    client.create_event(&organizer, &1, &prices, &100, &true, &0);
    client.register(&attendee, &1, &token.address);

    assert_eq!(token.balance(&attendee), 800);
    assert_eq!(token.balance(&contract_id), 200);

    let err = client
        .try_register(&attendee, &1, &token.address)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::AlreadyRegistered);

    assert_eq!(token.balance(&attendee), 800);
    assert_eq!(token.balance(&contract_id), 200);

    env.as_contract(&contract_id, || {
        let event: Event = env.storage().persistent().get(&DataKey::Event(1)).unwrap();
        assert_eq!(event.registered, 1);
    });
}

#[test]
fn test_capacity_zero_rejects_registration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);
    client.create_event(&organizer, &1, &prices, &0, &true, &0);

    let err = client
        .try_register(&attendee, &1, &token.address)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::EventFull);

    assert_eq!(token.balance(&attendee), 1000);
    assert_eq!(token.balance(&contract_id), 0);
}

#[test]
fn test_capacity_one_allows_one_registration_and_rejects_second() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee_a = Address::generate(&env);
    let attendee_b = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee_a, &1000);
    token_admin_client.mint(&attendee_b, &1000);

    let prices = create_token_prices(&env, &token.address, 200);
    client.create_event(&organizer, &1, &prices, &1, &true, &0);

    client.register(&attendee_a, &1, &token.address);
    assert_eq!(token.balance(&attendee_a), 800);
    assert_eq!(token.balance(&contract_id), 200);

    env.as_contract(&contract_id, || {
        let event: Event = env.storage().persistent().get(&DataKey::Event(1)).unwrap();
        assert_eq!(event.registered, 1);
    });

    let err = client
        .try_register(&attendee_b, &1, &token.address)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::EventFull);

    assert_eq!(token.balance(&attendee_b), 1000);
    assert_eq!(token.balance(&contract_id), 200);
}

#[test]
fn test_get_event_returns_stored_event() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _) = create_token_contract(&env, &token_admin);

    let prices = create_token_prices(&env, &token.address, 200);
    client.create_event(&organizer, &42, &prices, &10, &true, &9_999_999_999);

    let event = client.get_event(&42);
    assert_eq!(event.organizer, organizer);
    assert_eq!(event.capacity, 10);
    assert_eq!(event.self_refund_allowed, true);
    assert_eq!(event.refund_deadline, 9_999_999_999);
    assert_eq!(event.registered, 0);
}

#[test]
fn test_get_event_not_found_panics_with_clear_error() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.get_event(&999);
    }));

    assert!(panic.is_err(), "missing event should panic");
}

#[test]
fn test_check_in_requires_registered_attendee() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _) = create_token_contract(&env, &token_admin);

    let prices = create_token_prices(&env, &token.address, 200);
    client.create_event(&organizer, &1, &prices, &100, &true, &0);

    let err = client
        .try_check_in(&organizer, &1, &attendee)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::NotRegistered);
}

#[test]
fn test_self_refund_before_deadline_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &9_999_999_999);
    client.register(&attendee, &1, &token.address);
    assert_eq!(token.balance(&attendee), 800);

    client.refund(&attendee, &1, &attendee);
    assert_eq!(token.balance(&attendee), 1000);
}

#[test]
fn test_refunding_twice_panics_without_double_payment() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);
    client.create_event(&organizer, &1, &prices, &100, &true, &0);
    client.register(&attendee, &1, &token.address);
    assert_eq!(token.balance(&attendee), 800);

    client.refund(&attendee, &1, &attendee);
    assert_eq!(token.balance(&attendee), 1000);

    let second_refund = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.refund(&attendee, &1, &attendee);
    }));

    assert!(second_refund.is_err(), "a second refund should panic");
    assert_eq!(token.balance(&attendee), 1000);
}

#[test]
fn test_stranger_cannot_refund() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let stranger = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &100);
    client.register(&attendee, &1, &token.address);

    let err = client
        .try_refund(&stranger, &1, &attendee)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::OnlyAttendeeOrOrganizer);
}

#[test]
fn test_self_refund_after_deadline_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &100);
    client.register(&attendee, &1, &token.address);

    env.ledger().with_mut(|li| li.timestamp = 200);

    let err = client
        .try_refund(&attendee, &1, &attendee)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::DeadlinePassed);
}

#[test]
fn test_transfer_registration_success() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let receiver = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &0);
    client.register(&attendee, &1, &token.address);

    client.transfer_registration(&attendee, &1, &receiver);

    client.refund(&receiver, &1, &receiver);

    assert_eq!(token.balance(&receiver), 200);
}

#[test]
fn test_transfer_registration_not_registered() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let from = Address::generate(&env);
    let to = Address::generate(&env);

    let err = client
        .try_transfer_registration(&from, &1, &to)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::NotRegistered);
}

#[test]
fn test_transfer_registration_from_no_longer_registered() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let receiver = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &false, &100);
    client.register(&attendee, &1, &token.address);

    client.transfer_registration(&attendee, &1, &receiver);

    let err = client
        .try_refund(&attendee, &1, &attendee)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::NotRegistered);
}

#[test]
fn test_transfer_registration_already_registered() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let receiver = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);
    token_admin_client.mint(&receiver, &1000);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &0);
    client.register(&attendee, &1, &token.address);
    client.register(&receiver, &1, &token.address);

    let err = client
        .try_transfer_registration(&attendee, &1, &receiver)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::AlreadyRegistered);
}

#[test]
fn test_update_capacity_increases_and_decreases() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _) = create_token_contract(&env, &token_admin);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &0);
    client.update_capacity(&organizer, &1, &200);
    client.update_capacity(&organizer, &1, &50);
}

#[test]
fn test_update_capacity_fails_below_registered() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &100);
    client.register(&attendee, &1, &token.address);

    let err = client
        .try_update_capacity(&organizer, &1, &0)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::CapacityBelowCurrentRegistrations);
}

#[test]
fn test_multiple_tokens() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee_a = Address::generate(&env);
    let attendee_b = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token_a, token_a_admin) = create_token_contract(&env, &token_admin);
    let (token_b, token_b_admin) = create_token_contract(&env, &token_admin);

    token_a_admin.mint(&attendee_a, &1000);
    token_b_admin.mint(&attendee_b, &1000);

    let mut prices = Map::new(&env);
    prices.set(token_a.address.clone(), 100);
    prices.set(token_b.address.clone(), 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &0);

    client.register(&attendee_a, &1, &token_a.address);
    client.register(&attendee_b, &1, &token_b.address);

    assert_eq!(token_a.balance(&contract_id), 100);
    assert_eq!(token_b.balance(&contract_id), 200);

    client.payout(&organizer, &1);
    assert_eq!(token_a.balance(&organizer), 100);
    assert_eq!(token_b.balance(&organizer), 200);
}

#[test]
fn test_unsupported_token_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_a, _) = create_token_contract(&env, &token_admin);
    let (token_b, _) = create_token_contract(&env, &token_admin);

    let prices = create_token_prices(&env, &token_a.address, 100);

    client.create_event(&organizer, &1, &prices, &100, &true, &0);

    let err = client
        .try_register(&attendee, &1, &token_b.address)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::UnsupportedToken);
}

#[test]
fn test_update_event_terms_before_registration_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    let initial_prices = create_token_prices(&env, &token.address, 200);
    client.create_event(&organizer, &1, &initial_prices, &100, &false, &0);

    let new_prices = create_token_prices(&env, &token.address, 300);
    client.update_event_terms(&organizer, &1, &new_prices, &true, &1000);

    let attendee = Address::generate(&env);
    token_admin_client.mint(&attendee, &1000);

    client.register(&attendee, &1, &token.address);

    assert_eq!(token.balance(&attendee), 700);
    assert_eq!(token.balance(&contract_id), 300);

    client.refund(&attendee, &1, &attendee);
    assert_eq!(token.balance(&attendee), 1000);
}

#[test]
fn test_update_event_terms_after_registration_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &true, &100);
    client.register(&attendee, &1, &token.address);

    let err = client
        .try_update_event_terms(&organizer, &1, &prices, &true, &1000)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::EventHasRegistrations);
}

#[test]
fn test_update_event_terms_not_organizer_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let stranger = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, _) = create_token_contract(&env, &token_admin);

    let prices = create_token_prices(&env, &token.address, 200);

    client.create_event(&organizer, &1, &prices, &100, &false, &0);

    let err = client
        .try_update_event_terms(&stranger, &1, &prices, &true, &1000)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::NotOrganizer);
}

#[test]
fn test_init_requires_admin_auth() {
    let env = Env::default();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    let result = client.try_init(&admin);
    assert!(result.is_err(), "init must require auth from admin");
}

#[test]
fn test_pause_and_unpause() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    client.init(&admin);

    let prices = create_token_prices(&env, &token.address, 200);
    client.create_event(&organizer, &1, &prices, &100, &false, &0);

    // Pause the contract
    client.pause(&admin);

    // Registration should fail
    let err = client
        .try_register(&attendee, &1, &token.address)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::Paused);

    // Unpause the contract
    client.unpause(&admin);

    // Registration should succeed
    client.register(&attendee, &1, &token.address);
    assert_eq!(token.balance(&attendee), 800);
}

#[test]
fn test_registered_underflow_returns_not_registered() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EventRegistration, ());
    let client = EventRegistrationClient::new(&env, &contract_id);

    let organizer = Address::generate(&env);
    let attendee = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let (token, token_admin_client) = create_token_contract(&env, &token_admin);

    token_admin_client.mint(&attendee, &1000);

    let prices = create_token_prices(&env, &token.address, 200);
    client.create_event(&organizer, &1, &prices, &100, &true, &0);

    client.register(&attendee, &1, &token.address);

    env.as_contract(&contract_id, || {
        let mut event: Event = env.storage().persistent().get(&DataKey::Event(1)).unwrap();
        event.registered = 0;
        env.storage().persistent().set(&DataKey::Event(1), &event);
    });

    let err = client
        .try_refund(&attendee, &1, &attendee)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ContractError::NotRegistered);
}
