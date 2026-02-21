FROM rust:1.92-bookworm AS builder

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

WORKDIR /app
EXPOSE 62776

CMD ["autara-server", \
     "--tokens", "tokens.json", \
     "--signer", "keys/autara-cli-signer.key", \
     "--program-id", "53def2dc8516302842b10e356914d2a5f6b33425ba42aec684f706aa1cf64192", \
     "--oracle-program-id", "eee682c27db375bebbc17ed9a76aaa935c8b72bc7de50d736f03e2dfbed84b15", \
     "--listen", "0.0.0.0:62776", \
     "--network", "testnet"]
