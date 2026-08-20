.PHONY: build test deploy-testnet clean check fmt clippy

build:
	cargo build --release --target wasm32-unknown-unknown

test:
	cargo test

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
