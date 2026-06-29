# Autara deploy runbook

`autara-deploy` is an env-driven tool that deploys the Autara lending stack to
Arch Network: the `autara-program` and `autara-oracle` ELFs, the global config,
and the lending markets. It mirrors the CLAMM `clamm-deploy` crate.

`localnet`, `testnet`, and `mainnet` are all wired up. Mainnet defaults to the
public Arch mainnet RPC (`https://rpc.mainnet.arch.network`) and the Bitcoin
mainnet signing network. A **real** mainnet run is additionally gated in CI by
the typed confirmation `DEPLOY MAINNET` (see `_autara-action.yml`) and the
`mainnet` GitHub Environment; this tool never generates mainnet keypairs.

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
4. **Token setup** — ensures every configured `TOKENS` mint exists on-chain
   (idempotent; fails loudly if a mint is missing — create mints out-of-band via
   `autara-cli token setup`, this tool holds no mint authority).
5. **create_market** — creates one lending market per `MARKET_PAIRS` entry
   (curator = admin, default config mirrors `autara-server`; idempotent).
6. **Artifact** — writes `deployments/<network>.json` (addresses + tx ids only),
   including the created/ensured markets.

Each step is gated by `STEP_DEPLOY_PROGRAM`, `STEP_DEPLOY_ORACLE`,
`STEP_INIT_CONFIG`, `STEP_TOKEN_SETUP`, `STEP_CREATE_MARKET` (all default
`true`). In CI these are set explicitly per action: `deploy` (programs),
`initialize` (global config), `upgrade` (program re-upload), and `setup-markets`
(token setup + markets — see `autara-setup-markets.yml`).

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
files, `TOKENS`, and `MARKET_PAIRS`. Everything else lives in the env file. See
`autara.mainnet.env` for the mainnet template (paths only, `.keys-mainnet/`).
