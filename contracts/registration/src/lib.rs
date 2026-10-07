#![no_std]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_borrows_for_generic_args)]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Env, Map, Symbol,
};

#[contracttype]
#[derive(Clone)]
pub struct Event {
    pub organizer: Address,
    pub token_prices: Map<Address, i128>,
    pub capacity: u32,
    pub registered: u32,
    pub self_refund_allowed: bool,
    pub refund_deadline: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    EventNotFound = 1,
    EventFull = 2,
    NotRegistered = 3,
    RefundNotAllowed = 4,
    DeadlinePassed = 5,
    NotOrganizer = 6,
    OnlyAttendeeOrOrganizer = 7,
    Paused = 8,
    NotInitialized = 9,
    NotAdmin = 10,
    UnsupportedToken = 11,
    AlreadyRegistered = 12,
    EventHasRegistrations = 13,
    CapacityBelowCurrentRegistrations = 14,
}

#[contracttype]
#[derive(Clone)]
pub struct Payment {
    pub token: Address,
    pub amount: i128,
}

#[contracttype]
pub enum DataKey {
    Event(u32),
    Registered(u32, Address),
    CheckedIn(u32, Address),
    Admin,
    Paused,
}

#[contract]
pub struct EventRegistration;

#[contractimpl]
#[allow(deprecated)]
impl EventRegistration {
    pub fn init(env: Env, admin: Address) {
        admin.require_auth();
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "already initialized"
        );
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    pub fn pause(env: Env, admin: Address) -> Result<(), ContractError> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)?;
        if admin != stored_admin {
            return Err(ContractError::NotAdmin);
        }
        env.storage().instance().set(&DataKey::Paused, &true);
        Ok(())
    }

    pub fn unpause(env: Env, admin: Address) -> Result<(), ContractError> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)?;
        if admin != stored_admin {
            return Err(ContractError::NotAdmin);
        }
        env.storage().instance().set(&DataKey::Paused, &false);
        Ok(())
    }

    pub fn create_event(
        env: Env,
        organizer: Address,
        event_id: u32,
        token_prices: Map<Address, i128>,
        capacity: u32,
        self_refund_allowed: bool,
        refund_deadline: u64,
    ) -> Result<(), ContractError> {
        organizer.require_auth();
        ensure_not_paused(&env)?;
        let event = Event {
            organizer: organizer.clone(),
            token_prices,
            capacity,
            registered: 0,
            self_refund_allowed,
            refund_deadline,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Event(event_id), &event);
        env.events()
            .publish((Symbol::new(&env, "create_event"), event_id), organizer);
        Ok(())
    }

    pub fn get_event(env: Env, event_id: u32) -> Event {
        env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("event not found")
    }

    pub fn update_event_terms(
        env: Env,
        organizer: Address,
        event_id: u32,
        token_prices: Map<Address, i128>,
        self_refund_allowed: bool,
        refund_deadline: u64,
    ) -> Result<(), ContractError> {
        organizer.require_auth();
        ensure_not_paused(&env)?;
        let mut event: Event = env
            .storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .ok_or(ContractError::EventNotFound)?;

        if !caller_is_organizer(&event, &organizer) {
            return Err(ContractError::NotOrganizer);
        }
        if event.registered != 0 {
            return Err(ContractError::EventHasRegistrations);
        }

        event.token_prices = token_prices;
        event.self_refund_allowed = self_refund_allowed;
        event.refund_deadline = refund_deadline;

        env.storage()
            .persistent()
            .set(&DataKey::Event(event_id), &event);
        Ok(())
    }

    pub fn update_capacity(
        env: Env,
        organizer: Address,
        event_id: u32,
        new_capacity: u32,
    ) -> Result<(), ContractError> {
        organizer.require_auth();
        ensure_not_paused(&env)?;
        let mut event: Event = env
            .storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .ok_or(ContractError::EventNotFound)?;

        if !caller_is_organizer(&event, &organizer) {
            return Err(ContractError::NotOrganizer);
        }

        if new_capacity < event.registered {
            return Err(ContractError::CapacityBelowCurrentRegistrations);
        }

        event.capacity = new_capacity;
        env.storage()
            .persistent()
            .set(&DataKey::Event(event_id), &event);
        Ok(())
    }

    // check_in is intentionally idempotent: repeated check-ins for the same attendee
    // are treated as a successful no-op so retrying the organizer action or handling
    // duplicate client requests does not fail the check-in flow.
    pub fn check_in(
        env: Env,
        organizer: Address,
        event_id: u32,
        attendee: Address,
    ) -> Result<(), ContractError> {
        organizer.require_auth();
        ensure_not_paused(&env)?;

        let event: Event = env
            .storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .ok_or(ContractError::EventNotFound)?;

        if !caller_is_organizer(&event, &organizer) {
            return Err(ContractError::NotOrganizer);
        }
        if !env.storage().persistent().has(&DataKey::Registered(event_id, attendee.clone())) {
            return Err(ContractError::NotRegistered);
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::CheckedIn(event_id, attendee.clone()))
        {
            return Ok(());
        }

        env.storage()
            .persistent()
            .set(&DataKey::CheckedIn(event_id, attendee.clone()), &true);
        env.events()
            .publish((Symbol::new(&env, "check_in"), event_id), attendee);
        Ok(())
    }

    pub fn register(
        env: Env,
        attendee: Address,
        event_id: u32,
        payment_token: Address,
    ) -> Result<(), ContractError> {
        ensure_not_paused(&env)?;

        attendee.require_auth();
        let mut event: Event = env
            .storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .ok_or(ContractError::EventNotFound)?;

        if env.storage().persistent().has(&DataKey::Registered(event_id, attendee.clone())) {
            return Err(ContractError::AlreadyRegistered);
        }

        if event.registered >= event.capacity {
            return Err(ContractError::EventFull);
        }

        let price = event
            .token_prices
            .get(payment_token.clone())
            .ok_or(ContractError::UnsupportedToken)?;

        if price > 0 {
            let client = token::Client::new(&env, &payment_token);
            let contract_address = env.current_contract_address();
            client.transfer(&attendee, &contract_address, &price);
        }

        event.registered = event
            .registered
            .checked_add(1)
            .expect("registered overflow");
        env.storage()
            .persistent()
            .set(&DataKey::Event(event_id), &event);

        let payment = Payment {
            token: payment_token,
            amount: price,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Registered(event_id, attendee), &payment);

        Ok(())
    }

    pub fn refund(
        env: Env,
        caller: Address,
        event_id: u32,
        attendee: Address,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        ensure_not_paused(&env)?;

        let mut event: Event = env
            .storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .ok_or(ContractError::EventNotFound)?;

        let payment: Payment = env
            .storage()
            .persistent()
            .get(&DataKey::Registered(event_id, attendee.clone()))
            .ok_or(ContractError::NotRegistered)?;

        let is_organizer = caller == event.organizer;
        let is_self = caller == attendee;

        if is_self {
            if !event.self_refund_allowed {
                return Err(ContractError::RefundNotAllowed);
            }
            if event.refund_deadline > 0 && env.ledger().timestamp() >= event.refund_deadline {
                return Err(ContractError::DeadlinePassed);
            }
        } else if !is_organizer {
            return Err(ContractError::OnlyAttendeeOrOrganizer);
        }

        if payment.amount > 0 {
            let client = token::Client::new(&env, &payment.token);
            let contract_address = env.current_contract_address();
            client.transfer(&contract_address, &attendee, &payment.amount);
        }

        if event.registered == 0 {
            return Err(ContractError::NotRegistered);
        }

        event.registered = event
            .registered
            .checked_sub(1)
            .expect("registered underflow");
        env.storage()
            .persistent()
            .set(&DataKey::Event(event_id), &event);
        env.storage()
            .persistent()
            .remove(&DataKey::Registered(event_id, attendee.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::CheckedIn(event_id, attendee.clone()));
        env.events()
            .publish((Symbol::new(&env, "refund"), event_id), attendee);

        Ok(())
    }

    pub fn transfer_registration(
        env: Env,
        from: Address,
        event_id: u32,
        to: Address,
    ) -> Result<(), ContractError> {
        from.require_auth();
        ensure_not_paused(&env)?;

        let payment: Payment = env
            .storage()
            .persistent()
            .get(&DataKey::Registered(event_id, from.clone()))
            .ok_or(ContractError::NotRegistered)?;

        if env.storage().persistent().has(&DataKey::Registered(event_id, to.clone())) {
            return Err(ContractError::AlreadyRegistered);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::Registered(event_id, from.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::CheckedIn(event_id, from.clone()));
        env.storage()
            .persistent()
            .set(&DataKey::Registered(event_id, to), &payment);

        Ok(())
    }

    pub fn payout(env: Env, organizer: Address, event_id: u32) -> Result<(), ContractError> {
        organizer.require_auth();
        ensure_not_paused(&env)?;
        let event: Event = env
            .storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .ok_or(ContractError::EventNotFound)?;

        if !caller_is_organizer(&event, &organizer) {
            return Err(ContractError::NotOrganizer);
        }

        for token in event.token_prices.keys() {
            let client = token::Client::new(&env, &token);
            let contract_address = env.current_contract_address();
            let balance = client.balance(&contract_address);
            if balance > 0 {
                client.transfer(&contract_address, &organizer, &balance);
            }
        }
        Ok(())
    }
}

fn ensure_not_paused(env: &Env) -> Result<(), ContractError> {
    let is_paused: bool = env.storage().instance().get(&DataKey::Paused).unwrap_or(false);
    if is_paused {
        Err(ContractError::Paused)
    } else {
        Ok(())
    }
}

fn caller_is_organizer(event: &Event, caller: &Address) -> bool {
    &event.organizer == caller
}

mod test;
