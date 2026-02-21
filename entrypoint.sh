#!/bin/sh
set -e

# Decode secrets from Railway environment variables
if [ -n "$TOKENS_JSON_B64" ]; then
  echo "$TOKENS_JSON_B64" | base64 -d > /app/tokens.json
fi

if [ -n "$SIGNER_KEY_B64" ]; then
  mkdir -p /app/keys
  echo "$SIGNER_KEY_B64" | base64 -d > /app/keys/signer.key
  export AUTARA_SIGNER_KEY=/app/keys/signer.key
fi

if [ -n "$PROGRAM_KEY_B64" ]; then
  mkdir -p /app/keys
  echo "$PROGRAM_KEY_B64" | base64 -d > /app/keys/autara-stage.key
fi

if [ -n "$ORACLE_KEY_B64" ]; then
  mkdir -p /app/keys
  echo "$ORACLE_KEY_B64" | base64 -d > /app/keys/autara-pyth-stage.key
fi

if [ -n "$TOKEN_AUTHORITY_KEY_B64" ]; then
  mkdir -p /app/keys
  echo "$TOKEN_AUTHORITY_KEY_B64" | base64 -d > /app/keys/autara-token-authority.key
fi

# Railway injects PORT; default to 62776
LISTEN_ADDR="0.0.0.0:${PORT:-62776}"

exec autara-server \
  --tokens /app/tokens.json \
  --listen "$LISTEN_ADDR" \
  --network "${NETWORK:-testnet}" \
  ${ARCH_NODE:+--arch-node "$ARCH_NODE"}
