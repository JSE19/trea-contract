# Trea Contract

Smart contract powering **Trea**, an event registration and ticketing platform built on **Stellar** and **Soroban**. This repo contains only the on-chain logic — registration, payment escrow, refunds, and attendance check-in. The backend and frontend live in separate repos.

> Status: core contract logic implemented and unit tested against a local Soroban test environment. Testnet deployment is the next step.

---

## Why Soroban

- **Payments** settle fast and cheap on Stellar — useful for ticket sales, especially across borders.
- Registration, payment, and refund logic run in a single auditable contract instead of a backend database anyone with server access could quietly edit.
- Funds are **escrowed in the contract** at registration time and only released to the organizer via an explicit `payout` call — refunds don't depend on the organizer being online or willing to co-sign every request.

## What this contract does

- **Event creation** — organizers set price (or free), payment token, capacity, and refund policy per event.
- **Registration with escrow** — paid registrations transfer funds into the contract, not directly to the organizer.
- **Refunds** — either the attendee (if the organizer allows self-refund, optionally before a deadline) or the organizer (anytime, as an override) can trigger a refund.
- **Check-in / attendance verification** — organizers mark attendees as checked in on-chain.
- **Payout** — organizer withdraws escrowed funds from the contract after the event.

All state-changing calls require on-chain authorization (`require_auth()`) from the relevant address — nobody can register, refund, or check in on behalf of someone else without their signature, except the organizer's explicit overrides described below.

## Repo structure

```
.
├── contracts/
│   └── registration/
│       ├── src/
│       │   ├── lib.rs      # contract logic
│       │   └── test.rs     # unit tests
│       ├── Cargo.toml
│       └── Makefile
├── Cargo.toml               # Rust workspace root
└── README.md
```

## Contract reference

Contract: `EventRegistration` — `contracts/registration/src/lib.rs`

| Function | Caller | Description |
|---|---|---|
| `create_event(organizer, event_id, price, token, capacity, self_refund_allowed, refund_deadline)` | organizer | Registers a new event. `price = 0` marks it free. `refund_deadline = 0` means no deadline on self-refunds. |
| `register(attendee, event_id)` | attendee | Registers for an event. If `price > 0`, transfers payment from the attendee into contract escrow. |
| `refund(caller, event_id, attendee)` | attendee or organizer | Refunds an attendee from escrow. Self-refund requires `self_refund_allowed` and (if set) must be before `refund_deadline`. Organizer can always refund. |
| `check_in(organizer, event_id, attendee)` | organizer | Marks a registered attendee as checked in; walk-ins are rejected. |
| `payout(organizer, event_id)` | organizer | Withdraws the event's escrowed balance to the organizer. |

## Getting started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) v1.84.0 or higher
- The `wasm32v1-none` target: `rustup target add wasm32v1-none`
- [Stellar CLI](https://developers.stellar.org/docs/tools/cli): `cargo install --locked stellar-cli`

### Build and test

```bash
cd contracts/registration
cargo test              # run unit tests
stellar contract build  # build the .wasm artifact
```

The built contract is output to `target/wasm32v1-none/release/registration.wasm`.

### Deploy to Testnet

```bash
stellar keys generate --global trea-deployer --network testnet --fund
stellar contract deploy \
  --wasm target/wasm32v1-none/release/registration.wasm \
  --source trea-deployer \
  --network testnet
```

This returns a contract ID you can call directly via the CLI or wire up to a frontend.

## Related repos

- Backend (API, event metadata, photo uploads) — https://github.com/Trea-Hub/trea-backend/
- Frontend (event discovery, registration flow, wallet connect) — https://github.com/Trea-Hub/trea-frontend

## Roadmap

- [x] Core contract: create event, register, refund, check-in, payout
- [x] Unit test coverage for auth paths, escrow, and refund policy branches
- [ ] Testnet deployment + CLI walkthrough
- [ ] Security review before Mainnet

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for setup details, coding conventions, and how to pick up an issue.

## License

MIT — see [LICENSE](./LICENSE). 
## Links

- Stellar Developer Docs: https://developers.stellar.org
- Soroban SDK: https://docs.rs/soroban-sdk
- Stellar Community Fund: https://communityfund.stellar.org