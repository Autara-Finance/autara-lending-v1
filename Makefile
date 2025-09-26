program-build:
	cargo-build-sbf --features entrypoint

program-test: program-build
	cargo nextest run --no-fail-fast -j 24

deploy:
	cargo run --bin deploy
