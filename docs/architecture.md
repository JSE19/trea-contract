# Storage Key Structure Proposal

## Current structure

Contract storage is keyed by one `DataKey` enum in
`contracts/registration/src/lib.rs`:

```rust
pub enum DataKey {
    Event(u32),
    Registered(u32, Address),
    CheckedIn(u32, Address),
    Admin,
    Paused,
}
```

This keeps all keys in one place and makes storage access straightforward while
the number of keys is small. Its trade-off is that event, attendee, and
administration state share one namespace and the enum will become harder to
navigate as more domains are introduced.

## Proposed structure

If the contract gains additional domains, split keys by responsibility, for
example:

```rust
pub enum EventKey {
    Event(u32),
}

pub enum AttendeeKey {
    Registered(u32, Address),
    CheckedIn(u32, Address),
}

pub enum AdminKey {
    Admin,
    Paused,
}
```

Each key type would be used with the appropriate contract storage API. This
groups related state and lets each domain evolve independently. It also adds
types and names without reducing the number of storage operations, and is
likely unnecessary until the key set grows.

## Recommendation

Keep the current `DataKey` until a concrete feature makes the domain boundaries
meaningfully larger or harder to maintain. At the current size, the single enum
is compact and easy to inspect; splitting it now would add structure without
an immediate functional benefit.

Revisit this proposal when adding a feature such as waitlists, cancellations,
or co-organizers. If adopted, prefer domain-specific names such as
`EventKey`, `AttendeeKey`, and `AdminKey`, and migrate related call sites
together.

## Storage compatibility warning

Soroban storage keys are serialized into ledger state. Replacing a key enum or
changing its variants can change the serialized keys even if the Rust variants
look equivalent. Existing deployed state may then no longer be addressable by
the new code.

Before applying this refactor to a deployed contract, confirm the exact XDR
encoding of the existing keys and prove that the new representation preserves
it. If encoding cannot be preserved, provide an explicit state migration or
deploy a new contract with a migration plan. Unit tests alone cannot establish
compatibility with already persisted ledger entries.

No key refactor is included in this proposal; the current storage layout and
runtime behavior remain unchanged.
