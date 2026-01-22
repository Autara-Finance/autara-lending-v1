program-build:
	cargo-build-sbf --features entrypoint

program-test:
	cargo nextest run --no-fail-fast -j 24 -p autara-integration-tests

lib-test:
	cargo nextest run --no-fail-fast -p autara-lib

deploy: program-build
	cargo run --bin deploy
