FROM rust:1.82-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release --bin autara-server

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/autara-server /usr/local/bin/autara-server
COPY tokens.json /app/tokens.json
COPY keys/ /app/keys/
COPY .env /app/.env

WORKDIR /app
EXPOSE 62776

CMD ["autara-server", "--tokens", "tokens.json", "--signer", "keys/autara-cli-signer.key", "--listen", "0.0.0.0:62776", "--network", "testnet"]
