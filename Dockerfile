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
COPY entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

WORKDIR /app
EXPOSE 62776

# Keys are NEVER baked into the image. entrypoint.sh decodes them at runtime from
# base64 env vars (SIGNER_KEY_B64, PROGRAM_KEY_B64, ORACLE_KEY_B64,
# TOKEN_AUTHORITY_KEY_B64, optional TOKENS_JSON_B64) into /app/keys/, mirroring the
# CI/Railway secrets pattern. These secrets MUST be provided at runtime.
ENTRYPOINT ["/app/entrypoint.sh"]
