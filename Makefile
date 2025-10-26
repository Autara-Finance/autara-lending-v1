program-build:
	cargo-build-sbf --features entrypoint

program-test: program-build
	cargo nextest run --no-fail-fast -j 24

deploy: program-build
	cargo run --bin deploy
