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

# Program/oracle IDs are PUBLIC and passed explicitly. The server's fallback id
# derivation reads keypairs via a compile-time source path (/app/autara-client/../keys)
# that does not exist in the slim runtime image, so omitting these flags makes it panic
# with NotFound at boot. Passing them keeps production on the OLD program/oracle.
exec autara-server \
  --tokens /app/tokens.json \
  --program-id 53def2dc8516302842b10e356914d2a5f6b33425ba42aec684f706aa1cf64192 \
  --oracle-program-id eee682c27db375bebbc17ed9a76aaa935c8b72bc7de50d736f03e2dfbed84b15 \
  --listen "$LISTEN_ADDR" \
  --network "${NETWORK:-testnet}" \
  ${ARCH_NODE:+--arch-node "$ARCH_NODE"}
