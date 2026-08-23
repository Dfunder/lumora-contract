.PHONY: build test deploy-testnet clean check fmt clippy test-unit test-integration test-fuzz test-coverage

build:
	cargo build --release --target wasm32-unknown-unknown

test:
	cargo test

test-unit:
	cargo test --lib -p campaign
	cargo test --lib -p common

test-integration:
	cargo test --test '*' -p campaign

test-fuzz:
	cargo test fuzz_ -p campaign -- --nocapture

test-coverage:
	cargo test -p campaign -- --nocapture 2>&1 | tail -20

cargo-test-all:
	cargo test --workspace

check:
	cargo check --workspace

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

deploy-testnet:
	./scripts/deploy.sh

clean:
	cargo clean
