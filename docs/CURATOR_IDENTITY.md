# Curator identity (mainnet)

Dedicated market-owner key — **not** the protocol admin / deployer / pusher.

| Field | Value |
|-------|-------|
| Role | Market curator (`create_market`, `update_config`, `socialize_loss`, `redeem_curator_fees`) |
| Key file (gitignored) | `autara-deploy/.keys-mainnet-prod/curator.json` |
| Arch pubkey (hex) | `d790d01e8ff4835a12e16e89a95963c251cfe7d5efdb3390b7d2ce3248dafb77` |
| Env | `CURATOR_KEY_PATH` (see `autara.mainnet.env`) |
| Generated | 2026-07-13 via `autara-client` `keygen` example |

## Custody

- Back up `curator.json` offline (password manager / hardware / sealed ops store).
- Never reuse this key as Railway pusher signer or deploy payer.
- Register `CURATOR_KEYPAIR_B64` on the GitHub `mainnet` Environment via
  `autara-deploy/scripts/set-github-secrets.sh --env mainnet --from-dir autara-deploy/.keys-mainnet-prod --apply`.

## Note

Market PDAs derive from the curator pubkey. This key must be used at
**first** `create_market` for the production aUSD/aBTC market. Changing curator
later requires a new market PDA (or an on-chain transfer path if/when added).
