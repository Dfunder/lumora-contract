# Deployment Guide

## WASM Deployment and Size Budget

This repository ships Soroban smart contracts as WebAssembly (WASM) binaries. Production deployments must use optimized WASM artifacts.

### WASM Optimization

- Build release WASM with:
  ```bash
  cargo build --release --target wasm32-unknown-unknown
  ```
- Optimize the generated WASM with Binaryen's `wasm-opt`:
  ```bash
  scripts/wasm/optimize-wasm.sh campaign
  scripts/wasm/optimize-wasm.sh factory
  ```
- The optimizer produces `target/wasm32-unknown-unknown/release/<crate>.opt.wasm`.
- Deployment pipelines must use the `.opt.wasm` artifact instead of the unoptimized `.wasm` file.

### Size budget

- Target: **under 100 KB (102400 bytes)** per contract.
- Build will fail if optimized contract size exceeds the budget.
- The CI pipeline reports pre- and post-optimization sizes as an artifact.

#### What to do if size exceeds budget

1. Review contract code for unused dependencies.
2. Remove or refactor large helper crates and avoid heap-heavy abstractions.
3. Prefer `#[inline(always)]` only when it reduces generated code size.
4. Use wire-level packing and remove runtime debug instrumentation.
5. If the size increase is legitimate, document the reason in a PR and update this guide with the feature justification.

## Testnet / Mainnet Deployments

### Required environment variables

- `SOROBAN_NETWORK` — `testnet` or `mainnet`
- `SOROBAN_TOKEN` — API token for the target Soroban RPC provider
- `CONTRACT_ID` — the deployed contract ID
- `SHELLY_DEPLOY_KEY` — encrypted deploy key or environment-specific signer token

### Testnet deployment

1. Ensure `SOROBAN_NETWORK=testnet`
2. Build and optimize contracts.
3. Use the optimized contract artifact for deploy:
   ```bash
   soroban contract deploy --wasm target/wasm32-unknown-unknown/release/campaign.opt.wasm
   soroban contract deploy --wasm target/wasm32-unknown-unknown/release/factory.opt.wasm
   ```

### Mainnet deployment

1. Ensure `SOROBAN_NETWORK=mainnet`
2. Verify wallet and fund account balances.
3. Use the same optimized artifact path for deploy.

## Initialization steps

After deployment:

- Record contract IDs and update environment configuration.
- Initialize contract state using `soroban contract invoke` or a specialized deployment script.
- Confirm the contract responds on the target network.

## Common deploy errors

### `invalid wasm` or `failed to parse wasm`

- Ensure the deployed artifact is `.opt.wasm`.
- Rebuild and run `wasm-opt -Oz` again on the release artifact.

### `insufficient funds`

- Fund the deployer account on the target network.
- Check the account balance with `soroban account get-balance`.

### `network unreachable`

- Confirm `SOROBAN_NETWORK` and RPC provider are correct.
- Verify network connectivity and provider status.

## Build command summary

```bash
cargo build --release --target wasm32-unknown-unknown
scripts/wasm/optimize-wasm.sh campaign
scripts/wasm/optimize-wasm.sh factory
```
