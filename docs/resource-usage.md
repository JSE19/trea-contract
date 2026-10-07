# Contract Storage Resource Usage

This table counts contract storage API operations on the successful path for
each function. `get` and `has` each count as one read; `set` and `remove` each
count as one write operation. Counts include the `Paused` instance-storage read
performed by `ensure_not_paused` where that helper is called.

These are operation counts, not Soroban resource-fee estimates. Actual fees also
depend on the size and durability of entries, ledger footprint, and host
execution. Token balance and transfer calls are external token-contract
operations and are not included in contract storage counts. Error paths stop
when validation fails and therefore may perform fewer operations.

| Function | Contract storage reads | Contract storage writes | Notes |
| --- | ---: | ---: | --- |
| `init` | 1 | 2 | Checks whether `Admin` exists; sets `Admin` and `Paused`. |
| `pause` | 1 | 1 | Reads `Admin`; sets `Paused` to true. |
| `unpause` | 1 | 1 | Reads `Admin`; sets `Paused` to false. |
| `create_event` | 1 | 1 | Reads `Paused`; sets the event. Existing event IDs are overwritten without a read. |
| `get_event` | 1 | 0 | Reads the event. |
| `update_event_terms` | 2 | 1 | Reads `Paused` and the event; writes the updated event. |
| `update_capacity` | 2 | 1 | Reads `Paused` and the event; writes the updated event. |
| `check_in` (first check-in) | 4 | 1 | Reads `Paused`, event, `CheckedIn`, and `Registered`; sets `CheckedIn`. |
| `check_in` (duplicate) | 3 | 0 | Reads `Paused`, event, and `CheckedIn`; the existing check-in proves the registration is still active, so no `Registered` lookup is needed. |
| `register` | 3 | 2 | Reads `Paused`, event, and whether `Registered` exists; sets event and registration. |
| `refund` | 3 | 3 | Reads `Paused`, event, and payment from `Registered`; sets event and removes `Registered` and `CheckedIn`. |
| `transfer_registration` | 3 | 3 | Reads `Paused`, source `Registered`, and whether destination `Registered` exists; removes source registration and check-in, then sets destination registration. |
| `payout` | 2 | 0 | Reads `Paused` and event. It checks token balances and transfers funds through external token contracts. |

`check_in` relies on the invariant that a check-in record is created only for a
registered attendee and that both `refund` and `transfer_registration` remove
that check-in record whenever they remove the corresponding registration. The
unit test `test_refund_and_transfer_clear_check_in_state` exercises both
invalidation paths.
