# Lumora Contract Event Schema Documentation

This document describes all events emitted by the Lumora crowdfunding contract, including their topics, data fields, and firing conditions. These events are designed to be parseable by Horizon event filters for event-driven backend logic.

## Event List

- [campaign_initialized](#campaign_initialized)
- [donation_received](#donation_received)
- [milestone_unlocked](#milestone_unlocked)
- [milestone_released](#milestone_released)
- [campaign_ended](#campaign_ended)
- [campaign_cancelled](#campaign_cancelled)
- [refund_issued](#refund_issued)
- [deadline_extended](#deadline_extended)

---

## campaign_initialized

Emitted when a new campaign is successfully deployed and initialized.

**Topics:**

- `campaign_initialized` (Symbol)
- `contract_address` (Address) - The campaign contract address
- `creator` (Address) - The campaign creator's address

**Data:**

- `goal_amount` (i128) - The funding goal amount
- `end_time` (u64) - The campaign deadline timestamp
- `accepted_assets` (Vec<AssetInfo>) - List of accepted asset types
- `milestones` (Vec<MilestoneInput>) - Milestone configuration with target amounts and description hashes

**Firing Condition:**

- Emitted exactly once during the `initialize()` function call
- Only after all validation checks pass (goal amount > 0, end time in future, accepted assets provided, valid milestones)
- After campaign data is stored in contract storage

**Horizon Event Filter Example:**

```javascript
// Filter for campaign_initialized events
events.filter({
  contract: [CONTRACT_ADDRESS],
  topics: [["campaign_initialized"]],
});
```

---

## donation_received

Emitted when a donation is successfully processed and stored.

**Topics:**

- `donation_received` (Symbol)
- `contract_address` (Address) - The campaign contract address

**Data:**

- `donor` (Address) - The donor's address
- `amount` (i128) - The donation amount
- `asset_code` (String) - The asset code as a string ("XLM" for native, or token contract address string)
- `raised_total` (i128) - The total amount raised after this donation
- `timestamp` (u64) - The ledger timestamp when the donation was received

**Firing Condition:**

- Emitted only after a successful token transfer from donor to contract
- Only after storage updates are complete:
  - Campaign raised_amount is incremented
  - Donor record is updated with new total and per-asset breakdown
  - Per-asset raised total is updated
  - Milestone status updates (if thresholds crossed)
  - Campaign status update to GoalReached (if goal just met)
- Only emitted if all validations pass (amount > 0, minimum donation met, asset accepted, campaign active, before deadline)

**Horizon Event Filter Example:**

```javascript
// Filter for donation_received events
events.filter({
  contract: [CONTRACT_ADDRESS],
  topics: [["donation_received"]],
});

// Parse data fields from event body
```

---

## milestone_unlocked

Emitted when a milestone becomes unlocked due to sufficient funds being raised.

**Topics:**

- `milestone_unlocked` (Symbol)
- `contract_address` (Address) - The campaign contract address

**Data:**

- `milestone_index` (u32) - The index of the unlocked milestone
- `target_amount` (i128) - The target amount for this milestone
- `raised_total` (i128) - The total amount raised at time of unlocking

**Firing Condition:**

- Emitted when raised_amount crosses a milestone's target_amount threshold
- Only emitted once per milestone (does not repeat for subsequent donations)
- Only after milestone status is updated from Locked to Unlocked in storage
- Occurs during the `donate()` function after successful transfer and storage updates

**Horizon Event Filter Example:**

```javascript
// Filter for milestone_unlocked events
events.filter({
  contract: [CONTRACT_ADDRESS],
  topics: [["milestone_unlocked"]],
});
```

---

## milestone_released

Emitted when a milestone is released and funds are transferred to the recipient. One event is emitted per asset type accepted by the campaign.

**Topics:**

- `milestone_released` (Symbol)
- `contract_address` (Address) - The campaign contract address

**Data:**

- `milestone_index` (u32) - The index of the released milestone
- `amount` (i128) - The amount released for this specific asset
- `asset` (AssetInfo) - The asset type being released
- `recipient` (Address) - The recipient address
- `timestamp` (u64) - The ledger timestamp when released

**Firing Condition:**

- Emitted during `release_milestone()` function
- Only after successful token transfer from contract to recipient
- Only after milestone status is updated to Released in storage
- Only after released_at timestamp is recorded
- One event per asset type with non-zero release amount
- Only emitted if milestone is in Unlocked status and released in sequential order

**Horizon Event Filter Example:**

```javascript
// Filter for milestone_released events
events.filter({
  contract: [CONTRACT_ADDRESS],
  topics: [["milestone_released"]],
});
```

---

## campaign_ended

Emitted when a campaign ends. This can happen via explicit end by creator or automatically when a donation is attempted past the deadline.

**Topics:**

- `campaign_ended` (Symbol)

**Data:**

- `creator` (Address) - The campaign creator's address
- `timestamp` (u64) - The ledger timestamp when campaign ended

**Firing Condition:**

- Emitted in two scenarios:
  1. When creator calls `end_campaign()` explicitly
  2. When a donation is attempted past the deadline (automatic transition from Active to Ended)
- Only after campaign status is updated to Ended in storage
- Does not start the refund window (ending is not a failure mode)

**Horizon Event Filter Example:**

```javascript
// Filter for campaign_ended events
events.filter({
  contract: [CONTRACT_ADDRESS],
  topics: [["campaign_ended"]],
});
```

---

## campaign_cancelled

Emitted when a campaign is cancelled by the creator.

**Topics:**

- `campaign_cancelled` (Symbol)

**Data:**

- `creator` (Address) - The campaign creator's address
- `timestamp` (u64) - The ledger timestamp when cancelled

**Firing Condition:**

- Emitted during `cancel_campaign()` function
- Only after campaign status is updated to Cancelled in storage
- Only permitted while no funds have been raised (raised_amount == 0)
- Starts the refund window (campaign_end_time is set to current time)
- Only callable by campaign creator while campaign is Active or GoalReached

**Horizon Event Filter Example:**

```javascript
// Filter for campaign_cancelled events
events.filter({
  contract: [CONTRACT_ADDRESS],
  topics: [["campaign_cancelled"]],
});
```

---

## refund_issued

Emitted when a refund is successfully processed for a donor. One event is emitted per asset type refunded.

**Topics:**

- `refund` (Symbol) - Note: Event name is "refund" in the contract

**Data:**

- `donor` (Address) - The donor's address receiving the refund
- `amount` (i128) - The refund amount for this specific asset
- `asset` (AssetInfo) - The asset type being refunded

**Firing Condition:**

- Emitted during `refund()` function
- Only after successful token transfer from contract to donor
- Only after donor record is cleared (total_donated set to 0, per_asset cleared)
- Only if campaign is in Cancelled or Failed status
- Only within the refund window (30 days from campaign end)
- One event per asset type with non-zero refund amount
- Only callable by the donor themselves

**Horizon Event Filter Example:**

```javascript
// Filter for refund events
events.filter({
  contract: [CONTRACT_ADDRESS],
  topics: [["refund"]],
});
```

---

## deadline_extended

Emitted when a campaign deadline is extended by the creator.

**Topics:**

- `deadline_extended` (Symbol)

**Data:**

- `creator` (Address) - The campaign creator's address
- `new_end_time` (u64) - The new extended deadline timestamp

**Firing Condition:**

- Emitted during `extend_deadline()` function
- Only after campaign end_time is updated in storage
- Only if new_end_time is strictly later than current end_time
- Only if new_end_time is within 90 days of the ORIGINAL end time (even across repeated extensions)
- Only callable by campaign creator while campaign is Active or GoalReached
- Does not affect the refund window calculation (based on original end time)

**Horizon Event Filter Example:**

```javascript
// Filter for deadline_extended events
events.filter({
  contract: [CONTRACT_ADDRESS],
  topics: [["deadline_extended"]],
});
```

---

## Additional Events (Not in Original Requirements)

### goal_rch (Goal Reached)

Emitted when the campaign goal is reached for the first time.

**Topics:**

- `goal_rch` (Symbol)

**Data:**

- `raised_amount` (i128) - The total amount raised
- `goal_amount` (i128) - The campaign goal amount

**Firing Condition:**

- Emitted during `donate()` when raised_amount first reaches or exceeds goal_amount
- Only after campaign status is updated to GoalReached
- Only emitted once (does not repeat for subsequent donations)

### failed

Emitted when a campaign is marked as failed by the creator.

**Topics:**

- `failed` (Symbol)

**Data:**

- `creator` (Address) - The campaign creator's address
- `timestamp` (u64) - The ledger timestamp when failed

**Firing Condition:**

- Emitted during `fail_campaign()` function
- Only after campaign status is updated to Failed in storage
- Starts the refund window (campaign_end_time is set to current time)
- Only callable by campaign creator while campaign is Active or GoalReached

---

## Data Types Reference

### AssetInfo

Enum representing asset types:

- `Native` - Stellar native asset (XLM)
- `Token(Address)` - Custom token identified by contract address

### MilestoneInput

- `target_amount` (i128) - Amount required to unlock this milestone
- `description_hash` (BytesN<32>) - Hash of milestone description

### CampaignStatus

Enum values: `Active`, `Successful`, `Failed`, `GoalReached`, `Ended`, `Cancelled`

### MilestoneStatus

Enum values: `Locked`, `Unlocked`, `Released`

---

## Horizon Event Filtering Best Practices

1. **Filter by contract address** to get events from a specific campaign
2. **Filter by topic** to get specific event types
3. **Parse data fields** from the event body to extract relevant information
4. **Use pagination** for campaigns with many events
5. **Handle multiple asset events** for milestone_released and refund_issued

Example Horizon API call:

```
GET /events?contract={CONTRACT_ID}&topics=["donation"]
```
