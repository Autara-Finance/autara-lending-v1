build-program-autara:
	cd programs/autara-program && cargo-build-sbf --features entrypoint

build-program-oracle:
	cd programs/autara-oracle && cargo-build-sbf --features entrypoint

program-test:
	cargo nextest run --no-fail-fast -j 24 -p autara-integration-tests

lib-test:
	cargo nextest run --no-fail-fast -p autara-lib

deploy: build-program-autara build-program-oracle
	cargo run --bin deploy
