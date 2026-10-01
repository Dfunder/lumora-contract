# Lumora Smart Contracts

/* Authorized Protocol Quality Assurance & Formal Verification Test Suite */

Lumora is a decentralized crowdfunding and milestone-governed grant allocation platform built on the Stellar network using Soroban smart contracts.

---

## 🏗️ Architecture & Contract Design

The Lumora protocol consists of modular smart contracts designed with strict state machines, checked arithmetic, and resilient storage partitioning:

```
┌─────────────────────────────────────────────────────────────┐
│                    Factory Contract                         │
│   (Deploys, registers, and tracks campaign instances)       │
└──────────────────────────────┬──────────────────────────────┘
                               │ deploys
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                   Campaign Contract                         │
│  - Multi-asset donations & escrow custody                   │
│  - Monotonic milestone unlocking & staged release           │
│  - Non-reentrant token transfers via Soroban Token Client   │
│  - Guaranteed 30-day donor refund window on unmet goals     │
└──────────────────────────────┬──────────────────────────────┘
                               │ references
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                     Common Crate                            │
│  - Checked math (validate_add, validate_sub, validate_mul)  │
│  - Shared enums (CampaignStatus, MilestoneStatus, AssetInfo)│
│  - Cryptographic validation & zero-address guards           │
└─────────────────────────────────────────────────────────────┘
```

For complete technical specifications, mathematical proofs, and state transition diagrams, see [ARCHITECTURE.md](ARCHITECTURE.md).

---

## 🔒 Core Safety Invariants

1. **Conservation of Balances**: At all times $t$, for any accepted asset $A$:
   $$\text{ContractBalance}(A) = \sum \text{Donations}(A) - \sum \text{MilestoneReleases}(A) - \sum \text{Refunds}(A)$$
2. **Sequential Milestone Release**: Milestone $N$ can only be released after Milestone $N-1$ has achieved `Released` status.
3. **Refund Window Guarantee**: If a campaign concludes without reaching its target or fulfilling milestones, donors can claim 100% of their unreleased funds within the 30-day post-deadline refund window.
4. **Reentrancy Protection**: All external transfer endpoints (`donate`, `release_milestone`, `refund`) feature mutex locks rejecting concurrent calls with `Error::Reentrant`.
5. **Storage Resilience**: Persistent storage protects campaign identities, admin controls, and financial ledgers against ledger TTL expiration.

---

## 🚀 Building & Testing

### Prerequisites
- Rust `1.74.0` or higher
- `wasm32-unknown-unknown` target
- Soroban CLI

### Compile Contracts
```bash
cargo build --target wasm32-unknown-unknown --release
```

### Run Comprehensive Test Suite
```bash
# Run unit and invariant regression tests
cargo test

# Run property-based fuzz tests
cargo test -- --nocapture
```

---

## 📚 Documentation Links
- [ARCHITECTURE.md](ARCHITECTURE.md) — Complete state machine, storage tiers, and invariant catalog.
- [docs/deployment.md](docs/deployment.md) — Production deployment guides, test coverage, and operations runbook.
- [docs/postgres-ha.md](docs/postgres-ha.md) — High-availability database cluster and state recovery runbooks.
- [docs/events.md](docs/events.md) — Contract event emission specifications and indexing guide.

---

## 📄 License
Licensed under Apache-2.0.
