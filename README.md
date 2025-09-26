# Autara Lending V1

Autara Lending V1 is a decentralized lending protocol built on the Arch blockchain. It allows users to lend and borrow permissionlessly assets.
The protocol is designed to be secure, efficient, and user-friendly, providing a seamless experience for both lenders and borrowers.

### Setup

- Install [Rust](https://www.rust-lang.org/tools/install)
- Install [Solana](https://docs.anza.xyz/cli/install) (stable version, not latest)
- Install [Docker](https://docs.docker.com/engine/install/)
- Install [nextest](https://nexte.st/book/installation.html)
- Install [llvm-cov](https://nexte.st/docs/integrations/test-coverage/?h=cove)


### Repository

This repository contains 7 crates:

- `autara-lib`: Main library of the Autara protocol, defining the data structures and logic
- `autara-client`: A client to interact with the Autara protocol
- `autara-program-lib`: Library for Arch smart contracts
- `autara-program`: Autara smart contract
- `autara-integration-tests`: Standalone crate for the Autara smart contracts integration tests
- `autara-pyth`: A naive client to push Pyth price feeds to autara-oracle smart contracts
- `autara-oracle`: A naive oracle smart contract to store Pyth price feeds without any validation

### Build

To build smart contracts, run the following command:

```bash
cargo-build-sbf --features entrypoint
```

To build workspace, run:

```bash
cargo build
```

### Local environment

Setup an arch local environment:

```bash
docker pull --platform linux/amd64 baptisteeb/arigato-node:latest
docker run -d --platform linux/amd64 --name arigato-node -p 18443:18443 -p 3030:3030 -p 9002:9002 baptisteeb/arigato-node:latest
```

Then, deploy the smart contracts:

```bash
cargo run --bin deploy
```

### Run Pyth Pusher

```bash
cargo run --bin autara-pyth
```

### Run demo

```bash
cargo run --bin autara-client
```

### Autara Server

```bash
cargo run --bin autara-server
```

### Test

```bash
cargo nextest run --no-fail-fast -j 24
```

If you dont have a local arch node running, you can run the tests with the following command:

```bash
cargo nextest run --exclude autara-integration-tests --workspace
```

Run tests with coverage:

```bash
cargo llvm-cov nextest --exclude autara-integration-tests --workspace
```
