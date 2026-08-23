# Deployment

## Authorization Model

The campaign contract implements explicit authorization checks at all critical operation boundaries to prevent unauthorized state modifications and concurrent access issues.

### Authorization Principles

1. **Creator-Only Operations**: The campaign creator is the sole entity authorized to release milestones and modify critical contract state. All creator operations require explicit authorization via `require_auth()`.

2. **Donor Authorization**: Donors must authorize their own donations. Each donation requires the donor's explicit cryptographic signature.

3. **No Privilege Escalation**: The contract enforces strict role-based access control. Non-creators cannot release milestones, and non-donors cannot initiate donations on behalf of others.

### State Validation Guards

The contract implements three complementary state validation mechanisms to prevent edge cases and concurrent modification scenarios:

#### 1. Frozen State Guard (`check_contract_not_frozen()`)
- **Purpose**: Prevents all state transitions when the contract is frozen
- **Applied to**: `initialize()`, `release_milestone()`, `donate()`
- **Behavior**: Returns `Unauthorized` error if contract is frozen
- **Use Case**: Protects against operations on expired, cancelled, or suspended contracts

#### 2. Locked State Guard (`check_contract_not_locked()`)
- **Purpose**: Prevents concurrent modifications and re-entrant calls
- **Applied to**: `release_milestone()`, `donate()`
- **Mechanism**: Acquired at operation start, released upon completion
- **Behavior**: Returns `Unauthorized` error if lock is already held
- **Use Case**: Ensures milestone releases execute atomically without concurrent state changes

#### 3. Re-initialization Guard (`check_not_already_initialized()`)
- **Purpose**: Prevents re-initialization attacks that could reset contract state
- **Applied to**: `initialize()`
- **Behavior**: Returns `AlreadyInitialized` error if campaign already exists
- **Invariant**: Campaign can only be initialized exactly once

### Operation-Specific Authorization

#### Initialize
- **Authorization Required**: Creator must call with `require_auth()`
- **State Checks**:
  - Contract must not be frozen
  - Contract must not be already initialized
  - Goal amount must be positive
  - End time must be in the future
  - At least one accepted asset must be specified
  - Milestones must be in strictly increasing order matching the goal amount

#### Release Milestone
- **Authorization Required**: Creator must call with `require_auth()`
- **State Checks**:
  - Contract must not be frozen
  - Contract must not be locked (exclusive execution)
  - Milestone must be next in sequential order (previous milestones must be released)
  - Milestone must be unlocked (sufficient funds raised)
  - Milestone must not already be released
- **Atomicity**: Lock is acquired before state modifications and released after completion

#### Donate
- **Authorization Required**: Donor must call with `require_auth()`
- **State Checks**:
  - Contract must not be frozen
  - Contract must not be locked
  - Donation amount must be positive and meet minimum requirement
  - Campaign must not have ended
  - Campaign must be active or have goal reached (accepting donations)
  - Donated asset must be in the accepted assets list

### Error Codes for Authorization Failures

Authorization failures return specific error codes instead of panicking:

- `Unauthorized` (code 1): Generic authorization failure
  - Creator authentication failed
  - Non-creator attempted to release milestones
  - Non-donor attempted to donate without authorization
  - Contract is frozen or locked
  
- `AlreadyInitialized` (code 2): Re-initialization attempted
- `CampaignNotActive` (code 9): Campaign is not in active state
- `CampaignEnded` (code 10): Campaign deadline has passed
- `PreviousMilestoneNotReleased` (code 15): Milestone release order violation

### Concurrent Modification Prevention

The contract uses a temporary lock mechanism to prevent concurrent modifications during critical operations:

```
acquire_lock()
  ↓
Validate state (frozen, locked, authorization)
  ↓
Perform state modifications
  ↓
release_lock()
```

This ensures that:
- Milestone releases execute atomically
- State invariants cannot be violated by concurrent calls
- Re-entrant calls are rejected with `Unauthorized` error

### Testing Authorization Model

The test suite validates:
1. Unauthorized callers are rejected
2. Re-initialization is prevented
3. Non-creators cannot release milestones
4. Milestones must be released in order
5. The same milestone cannot be released twice
6. Locked milestones cannot be released
7. Frozen contracts reject new operations
8. State validation guards protect contract invariants

## Financial Invariants

The campaign contract maintains several financial invariants to ensure the integrity of the system. These invariants are checked at various points in the code to prevent inconsistencies and potential exploits.

- **`raised_amount`**: This is the total amount of funds raised by the campaign. It is the sum of all donations made to the campaign.
- **`released_amount`**: This is the total amount of funds that have been released to the campaign owner. It is the sum of all milestone releases.
- **`total_donated`**: This is the total amount of funds donated by a specific donor.
- **`raised_per_asset`**: This is the total amount of funds raised for a specific asset.

The following invariants must always hold true:

- `raised_amount` >= `released_amount`
- `raised_amount` = sum of all `raised_per_asset`
- `total_donated` for a donor = sum of all `per_asset` donations for that donor

## Testing Strategy

The campaign contract has a comprehensive test suite covering edge cases, boundary conditions, state machine transitions, and property-based invariants.

### Test Categories

#### 1. Initialization Edge Cases
- Zero and negative goal amounts
- Past end times
- Empty accepted assets list
- Empty milestone list and exceeding MAX_MILESTONES (5)
- Last milestone not equal to goal amount
- Negative minimum donation amount
- Zero minimum donation (accepts any positive amount)
- Duplicate milestone amounts (non-strictly increasing)

#### 2. Donation Edge Cases
- Zero and negative donation amounts
- Donations below the minimum threshold
- Donations with unaccepted assets
- Large i128 amounts (1 trillion stroops)
- Donations after campaign end time (triggers state transition)
- Donations in non-Active states (Ended, Cancelled, Failed)
- Multiple donations from the same donor
- Multiple donors with different assets

#### 3. State Machine Transitions
- **Frozen state**: Blocks `donate()`, `release_milestone()`, `cancel_campaign()`, `extend_deadline()`, `end_campaign()`, `fail_campaign()`
- **Active → GoalReached**: Donation reaches or exceeds goal
- **Active → Ended**: Creator ends early, deadline expires, or `update_status()` called
- **Active → Cancelled**: Creator cancels (only with zero funds raised)
- **Active → Failed**: Creator marks as failed
- **GoalReached → Ended/Failed**: Creator can end or fail from GoalReached
- **Ended → milestone release still works**: Ending doesn't block milestone releases

#### 4. Milestone Ordering Violations
- Skipping milestone indices (must release sequentially)
- Releasing already-released milestones
- Releasing locked (not-yet-unlocked) milestones
- All 5 milestones released in sequence with correct release amounts
- Unequal milestone spreads verify correct incremental amounts

#### 5. Multi-Asset Per-Asset Breakdown
- Single asset: accumulated donations tracked correctly
- Two assets: interleaved donations maintain correct per-asset sums
- Multiple donors: independent per-donor records
- Property: `sum(per_asset.amounts) == total_donated` for every donor

#### 6. Refund Window Tests
- Refund window boundary: exactly 30 days succeeds
- Past 30 days: `RefundWindowClosed` error
- Refund only in Cancelled/Failed status
- Double-refund prevention
- Multi-asset refund exact calculation

#### 7. Property-Based / Invariant Tests
- **`released_amount ≤ raised_amount`** after any operation sequence
- **Milestone monotonicity**: once Unlocked, never returns to Locked
- **Per-asset sum invariant**: `sum(per_asset) == total_donated`
- **Proptest fuzzing**: random valid amounts for `donate()`, random milestone configurations

#### 8. Storage Unit Tests
- All storage roundtrips: campaign data, milestones, donor records, totals, per-asset amounts
- Lock acquire/release cycle and reentrancy prevention
- Frozen state independence from lock state
- XLM token, min donation, and end time storage

### Running Tests

```bash
# Run all tests
make test

# Run with verbose output
cargo test -- --nocapture

# Run specific test by name
cargo test test_frozen_blocks_donate

# Run property-based (proptest) tests
cargo test fuzz_
```

### Test Coverage Targets

| Path | Target | Description |
|------|--------|-------------|
| `initialize()` | 90%+ | All validation branches, edge cases |
| `donate()` | 90%+ | State checks, asset validation, milestone unlocking |
| `release_milestone()` | 90%+ | Ordering, amount calculation, multi-asset splits |
| `refund()` | 85%+ | Window checks, per-asset refund, double-refund |
| Storage layer | 90%+ | All getters/setters, lock/frozen state |
| Common utilities | 95%+ | Arithmetic validation, address checks |

## Rounding Strategy

The campaign contract uses integer arithmetic for all financial calculations. This means that there is no floating-point arithmetic and therefore no rounding errors. When calculating the amount to release for each asset in a milestone, the contract uses a simple division and multiplication strategy. The last asset in the list of accepted assets will receive the remainder of the release amount, ensuring that the full amount is released.
