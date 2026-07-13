# Railway hardening checklist (Arch Lend)

Railway hosts the Autara **server** and **pusher** until a longer-term home
replaces it. Same image (`Dockerfile` + `entrypoint.sh`); role selected by env.

## Services

| Service | `ROLE` | Env template |
|---------|--------|--------------|
| API / indexer / Prometheus | `server` | `autara-deploy/scripts/autara.server.mainnet.env` |
| Oracle price pusher | `pusher` | `autara-deploy/scripts/autara.pusher.mainnet.env` |

Keep **testnet** as separate Railway services with
`autara.pusher.testnet.env` / a testnet server env — never mix networks on one
service.

## Must match deployed artifacts

After `deployments/mainnet.json` exists:

- [ ] `PROGRAM_ID` == artifact `program_id`
- [ ] `ORACLE_PROGRAM_ID` == artifact `oracle_id` (also in pusher env)
- [ ] Token mints in `TOKENS_JSON_B64` == mainnet aUSD / aBTC (not testnet)
- [ ] `NETWORK=mainnet` and `ARCH_RPC_URL=https://rpc.mainnet.arch.network`

## No staging leftovers

- [ ] Server has `DISABLE_PRICE_PUSHER=1` when a dedicated pusher runs
- [ ] No stage key defaults relied upon — inject `SIGNER_KEY_B64` /
      `PROGRAM_ID` / `ORACLE_PROGRAM_ID` via Railway secrets
- [ ] Image `keys/` copy is override-only; production must set env secrets
- [ ] Entrypoint refuses mainnet server/pusher without program/oracle ids

## Env separation

- [ ] Mainnet and testnet are different Railway projects or clearly named services
- [ ] Secrets are per-environment (not shared across networks)
- [ ] Pusher signer is a dedicated low-value hot key (not admin/curator/deployer)
