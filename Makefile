.PHONY: build test deploy-testnet clean

build:
	cargo build --release --target wasm32-unknown-unknown

test:
	cargo test

deploy-testnet:
	./scripts/deploy.sh

clean:
	cargo clean
