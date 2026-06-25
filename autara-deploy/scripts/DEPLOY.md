# Autara deploy runbook

`autara-deploy` is an env-driven tool that deploys the Autara lending stack to
Arch Network: the `autara-program` and `autara-oracle` ELFs plus the global
config. It mirrors the CLAMM `clamm-deploy` crate.

**Phase 1 is TESTNET-FIRST.** `localnet` and `testnet` are wired up; `mainnet`
is intentionally unconfigured (the `Network::Mainnet` variant exists so it can
be added later without restructuring).

## TL;DR

```bash
# Always dry-run first (prints derived addresses + preflight, sends NOTHING).
NETWORK=testnet ./autara-deploy/scripts/deploy.sh --dry-run

# Real deploy (sends transactions). Builds the SBF ELFs, then deploys.
NETWORK=testnet ./autara-deploy/scripts/deploy.sh

# Read-only balance check.
NETWORK=testnet cargo run -p autara-deploy -- check-balance
```

You can also drive the binary directly (it reads the same env):

```bash
set -a; source autara-deploy/scripts/autara.testnet.env; set +a
DRY_RUN=1 cargo run -p autara-deploy
```

## What it does

1. **Preflight** — prints program/oracle ids, deployer/admin, RPC url, the
   derived global-config PDA, token mints, ELF presence, RPC reachability, and
   on-chain balances. Includes a **program-id guard** (see below).
2. **Deploy programs** — uploads `target/deploy/autara_program.so` and
   `autara_oracle.so` via the SDK `ProgramDeployer` (idempotent).
3. **create_global_config** — admin + fee receiver + fee share (idempotent).
4. **Artifact** — writes `deployments/<network>.json` (addresses + tx ids only).

Each step is gated by `STEP_DEPLOY_PROGRAM`, `STEP_DEPLOY_ORACLE`,
`STEP_INIT_CONFIG` (all default `true`).

## Program-id guard (important)

The on-chain `autara-program` derives the global-config PDA and runs ownership
checks against a **compiled-in id** (`autara_program::id()` =
`53def2dc...1cf64192`), *not* the runtime program id. The client derives PDAs
from the deployed program key. So the deployed program key's pubkey **must
equal** `autara_program::id()`.

- `keys/autara-stage.key`'s pubkey already equals `autara_program::id()`, so the
  default testnet env is consistent — no sync step is required.
- If you deploy with a different key, the tool warns (fatal on a real run). To
  use a new key you must update `id()` in `programs/autara-program/src/lib.rs`
  and rebuild the ELF.

The `autara-oracle` program is position-independent (it uses the runtime
`program_id` only), so it needs no such guard.

## Secret hygiene

- Env files contain **paths only**, never key material.
- New key directories matching `autara-deploy/.keys-*/` are gitignored. Keep
  rotated/fresh testnet keys in `autara-deploy/.keys-testnet/`.
- The committed `autara.testnet.env` references the existing repo `keys/` so the
  dry-run works immediately; rotate to `.keys-testnet/` for production.

## Per-network differences

Only these should change between networks: `ARCH_RPC_URL`, the `*_KEY_PATH`
files, and `TOKENS`. Everything else lives in the env file.
