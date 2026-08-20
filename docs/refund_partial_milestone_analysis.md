# Partial Milestone Release Case Analysis

## Scenario
A campaign has one or more milestones released before cancellation/failure occurs. Donors who contributed before the milestone release may expect refunds, but some funds have already been transferred to the creator.

## Lifecycle Analysis

### Campaign Status Flow
1. **Active** → Donations accepted, milestones unlock as thresholds reached
2. **GoalReached** → Funding goal met, all milestones unlocked
3. **Cancelled/Failed** → Creator terminates campaign, refund window opens

### Milestone Release Rules
- Milestones must be released sequentially (0, 1, 2, ...)
- Each milestone can only be released once
- Released funds are transferred to the creator immediately
- Campaign status does NOT automatically change after milestone release

### Reachability of Partial Release Case

**This case IS REACHABLE** given the current lifecycle rules:

1. Campaign starts in `Active` status
2. Donations reach first milestone threshold (e.g., 5,000 of 10,000 goal)
3. Creator releases milestone 0 → 5,000 transferred to creator
4. Campaign remains in `Active` or transitions to `GoalReached` if goal met
5. Creator later calls `cancel_campaign()` or `fail_campaign()`
6. Refund window opens, but contract only has remaining funds (5,000)

## Current Implementation Behavior

The current refund implementation:
- ✅ Refunds exact per-asset contributions from `DonorRecord`
- ✅ No rounding losses (uses exact stored amounts)
- ⚠️ **Does not check contract balance before refunding**
- ⚠️ **Token transfer may fail if insufficient funds**

### Example Scenario

```
Goal: 10,000 XLM
Milestones: [5,000, 10,000]
Donor A: 3,000 XLM
Donor B: 4,000 XLM
Donor C: 3,000 XLM

Timeline:
1. All donors contribute (10,000 total)
2. Creator releases milestone 0 (5,000 XLM transferred to creator)
3. Contract balance: 5,000 XLM
4. Creator cancels campaign
5. Donor A requests refund: 3,000 XLM ✅ (success, balance: 2,000)
6. Donor B requests refund: 4,000 XLM ❌ (fails, insufficient balance)
```

## Recommendations

### Option 1: Prohibit Cancellation After Milestone Release
Modify `cancel_campaign()` and `fail_campaign()` to check if any milestones have been released:

```rust
if campaign_data.released_amount > 0 {
    return Err(Error::CannotCancelAfterRelease);
}
```

**Pros**: Prevents the edge case entirely
**Cons**: Reduces flexibility for legitimate use cases

### Option 2: Proportional Refunds
Calculate refund based on remaining contract balance:

```rust
let refund_ratio = validate_sub(total_raised, released_amount)?;
let per_asset_refund = validate_mul(donor_amount, refund_ratio)? / total_raised;
```

**Pros**: Fair distribution of remaining funds
**Cons**: Breaks "exact per-asset refund" requirement

### Option 3: Current Behavior with Documentation
Keep current implementation but:
- Document that refunds may fail if milestones were released
- Add view function to check if full refund is possible
- Recommend creators cancel before releasing milestones

**Pros**: Maintains exact refund guarantee when possible
**Cons**: Shifts responsibility to creators

### Option 4: Require Creator Return
Add mechanism for creator to return released funds before cancellation:

```rust
pub fn return_milestone_funds(env: Env, amount: i128) -> Result<(), Error> {
    // Creator can return funds to enable full refunds
}
```

**Pros**: Enables full refunds even after partial release
**Cons**: Requires additional creator action

## Current Decision

**Option 3 (Current Behavior with Documentation)** is implemented:

- The contract enforces exact per-asset refunds when funds are available
- Token transfers will fail at the Stellar network level if insufficient
- Creators are responsible for canceling before releasing milestones
- This maintains the "exact refund" guarantee while acknowledging the edge case

## Test Coverage

The test suite includes:
- ✅ Multi-asset refund with exact calculation
- ✅ Refund window boundary conditions
- ✅ Status-based refund eligibility
- ⚠️ **No test for partial milestone release case** (documented as edge case)

## Conclusion

The partial milestone release case is reachable but represents a creator responsibility issue rather than a contract bug. The current implementation prioritizes:
1. Exact per-asset refunds (no rounding)
2. Clear refund window boundaries
3. Creator control over campaign lifecycle

Creators should cancel campaigns before releasing milestones to ensure full refund availability.
