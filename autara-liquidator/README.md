# Autara liquidator

Scan Autara borrow positions and (optionally) submit `liquidate` transactions.

Addresses parts of [#26](https://github.com/Autara-Finance/autara-lending-v1/issues/26).

## Status

| Capability | Status |
|---|---|
| Scan unhealthy positions (LTV ≥ `unhealthy_ltv`) | ✅ |
| Dry-run (default) | ✅ |
| Sized repay + `min_collateral` (slippage bps) | ✅ |
| Supply inventory + gas preflight | ✅ |
| Stale-oracle skip visibility | ✅ |
| Circuit breaker (consecutive failures → exit 2) | ✅ |
| Live liquidate (no swap callback) | ✅ — must hold supply tokens |
| Atomic CLAMM collateral→supply callback | ❌ — still open on #26 |
| Prometheus / deploy packaging | ❌ — still open on #26 |

## Run (dry-run)

```bash
cp autara-liquidator/liquidator-config.example.json liquidator-config.json
# edit program_id / keypair / rpc as needed
cargo run -p autara-liquidator -- --config liquidator-config.json
```

Set `"dry_run": false` only after the liquidator wallet is funded with the
market's supply asset (e.g. aUSD) **and** you intend to submit txs.

## Config

See `liquidator-config.example.json`:

| Field | Default | Meaning |
|---|---|---|
| `network` | `testnet` | Signs as Testnet4; also `mainnet` / `regtest` |
| `dry_run` | `true` | Scan only |
| `slippage_bps` | `100` | Haircut on expected collateral → min out |
| `min_lamports` | `50000` | Gas floor before live submit |
| `max_consecutive_failures` | `5` | Circuit breaker (0 disables) |
| `restrict_tokens` | `[]` | Optional mint allowlist |

Use a **dedicated** liquidator key (not admin/curator/pusher).

## Mainnet

Point `autara_program_id` at `deployments/mainnet.json` → `program_id`, keep
dry-run until a controlled unhealthy-position rehearsal succeeds.
