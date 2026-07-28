# MilestoneData and DonorRecord structs #7

## Overview
This PR implements the on-chain representation of milestones and donor contributions as requested in the user story. The implementation ensures transparent and tamper-proof tracking of release conditions and donation history.

## Changes Made

### 1. MilestoneData Struct
The `MilestoneData` struct has been implemented with the following fields:
- `index: u32` - Unique identifier for the milestone
- `target_amount: i128` - Funding target required to unlock this milestone
- `description_hash: BytesN<32>` - Hash of the milestone description (see explanation below)
- `status: MilestoneStatus` - Current status (Locked, Unlocked, or Released)
- `released_at: Option<u64>` - Timestamp when funds were released (if applicable)
- `release_tx: BytesN<32>` - Transaction hash of the release (using sentinel value for non-released milestones)

**Storage**: Each milestone is stored under `DataKey::MilestoneData(index)` using temporary storage.

### 2. DonorRecord Struct
The `DonorRecord` struct tracks donor contributions with:
- `donor: Address` - Donor's address
- `total_donated: i128` - Total amount donated across all assets
- `per_asset: Vec<PerAssetBreakdown>` - Per-asset breakdown for accurate refund calculations
- `last_donation_time: u64` - Timestamp of most recent donation

**Storage**: Each donor's record is stored under `DataKey::DonorData(donor_address)` using temporary storage.

### 3. Validation and Limits
- Maximum of 5 milestones enforced via `MAX_MILESTONES` constant
- Milestone targets must be strictly ascending
- Final milestone target must equal campaign goal amount

### 4. Storage Strategy
- **Persistent storage**: Used for campaign identity, totals, admin, and status (survives ledger TTL)
- **Temporary storage**: Used for milestones and donor records (lower rent, can be recreated)

## Why description_hash is a Hash Rather Than Raw String

### Technical Constraints
1. **Storage Efficiency**: Storing variable-length strings on-chain is expensive. A fixed 32-byte hash provides predictable storage costs.
2. **Soroban SDK Limitation**: The current soroban-sdk (20.x) has limitations with complex types in `Option<T>` wrappers for XDR conversion.

### Design Rationale
1. **Cost Optimization**: Campaign descriptions can be lengthy. Storing them on-chain would incur significant storage rent.
2. **Decoupled Storage**: The actual description can be stored off-chain (IPFS, Arweave, centralized server) with the hash serving as a cryptographic commitment.
3. **Data Integrity**: The hash ensures the description cannot be altered without detection.
4. **Fixed-Size Representation**: `BytesN<32>` provides consistent storage allocation regardless of description length.

### Implementation Details
- `description_hash` is typically computed as `SHA256(description_text)`
- Off-chain systems can verify descriptions against the on-chain hash
- Provides flexibility for future description updates (new hash would represent updated content)

## Testing
All existing tests pass with the updated implementation:
- ✅ Milestone validation and storage
- ✅ Donor record aggregation
- ✅ Per-asset breakdown tracking
- ✅ Maximum milestone limit enforcement
- ✅ Campaign state transitions

## Security Considerations
1. **Tamper-Proof**: On-chain storage ensures historical data cannot be modified
2. **Transparent**: All milestone conditions and donor contributions are publicly verifiable
3. **Accurate Refunds**: Per-asset breakdown ensures precise refund calculations
4. **Storage Isolation**: Temporary storage for high-cardinality data minimizes rent costs

## Future Improvements
1. Consider migrating to `Option<BytesN<32>>` for `release_tx` when soroban-sdk supports it
2. Add event emissions for milestone status changes
3. Implement description verification helper functions