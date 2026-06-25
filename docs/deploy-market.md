# Deploying a Lending Market

A practical runbook for standing up a **new Autara lending market** (a supply +
collateral pair) on Arch. It ties together the CLI, the keys, and the config
template into an ordered sequence.

This is about **markets**, not the smart contracts themselves. The Autara and
oracle programs are deployed/upgraded separately and must already be live on the
target cluster — never redeploy a program just to add a market.

Companion references:

- [`market-config-template.jsonc`](../market-config-template.jsonc) — the full,
  field-by-field config reference (LTV, interest-rate models, oracle config,
  validation rules). This guide points at it rather than repeating it.
- [`CURATOR.md`](../CURATOR.md) — what a curator is responsible for and how to
  pick risk parameters.
- [`README.md`](../README.md) — toolchain setup, key files, and the local Arch
  (Arigato) node.

---

## What a market is

A market pairs a **supply asset** (what lenders deposit and borrowers repay)
with a **collateral asset** (what borrowers post). It is owned by a **curator** —
the keypair that signs `tx create-market` becomes the curator and is the only
key that can later update the market's config or redeem curator fees. Markets are
directional, so the common pattern is to deploy both directions of a pair (see
`markets.json`).

---

## Prerequisites

1. **Toolchain** installed (see README → Setup): Rust, Solana CLI (stable),
   Docker, nextest.
2. **An Arch endpoint** for the target cluster:
   - local Arigato node (see README → Local environment), or
   - testnet: `https://rpc.testnet.arch.network`.
3. **Programs are live** on that cluster. On a fresh environment this is the
   one-time `make deploy` (builds both programs and runs the `deploy` binary,
   which also creates the protocol `GlobalConfig`). On an existing cluster the
   programs are already deployed — leave them alone.
4. **A curator key** under `keys/` (e.g. `keys/curator-conservative.key`). Set it
   per-command with `--signer`, or globally via the `AUTARA_SIGNER_KEY` env var.

The CLI is invoked as:

```bash
cargo run --bin autara-cli -- [GLOBAL FLAGS] <subcommand> [ARGS]
```

Global flags: `--arch-node <RPC>` (defaults to testnet), `--signer <key>` (or
`AUTARA_SIGNER_KEY`), `--network <regtest|testnet|mainnet>` (defaults to
`regtest`), `--tokens <tokens.json>`.

> Set `--arch-node` and `--network` consistently for the cluster you're
> targeting. The examples below use testnet; swap in your local node + `regtest`
> for local work.

---

## Step 1 — Make sure both tokens exist

The supply and collateral **token mints must already exist on-chain**, and their
mints/decimals/authorities must be recorded in `tokens.json` (the CLI resolves
token names from this file).

The standard tokens are created idempotently — it skips any mint that already
exists — and `tokens.json` is (re)written by:

```bash
cargo run --bin autara-cli -- \
  --arch-node https://rpc.testnet.arch.network \
  token setup --output tokens.json
```

Mint test balances when you need them:

```bash
cargo run --bin autara-cli -- \
  --signer keys/<token-authority>.key \
  token mint --token <MINT_PUBKEY> --to <RECIPIENT_PUBKEY> --amount <ATOMS>
```

**Adding a brand-new token:** create a fixed keypair under `keys/`
(`token-<sym>.key` + a `token-<sym>-authority.key`) so the mint address is stable
across redeploys, and extend the `token setup` flow to include it. The aUSD
addition on this branch is the worked example — mirror it.

---

## Step 2 — Make sure the oracle feeds are live

Each side of the market is priced by an oracle (Pyth). Confirm prices are being
pushed on-chain for **both** the supply and collateral feeds before creating the
market, or `create-market`/borrows will fail oracle validation.

Push feeds (continuous; runs until you stop it):

```bash
cargo run --bin autara-cli -- \
  oracle push-feeds --feed 0x<supply-feed-id> --feed 0x<collateral-feed-id>
```

For a one-off test value on a local/dev cluster use `oracle push-price --feed
0x<feed-id> --price <value>`. Sanity-check any feed with `oracle fetch-price
--feed 0x<feed-id>`. (In normal operation `autara-server` pushes feeds for every
token in `tokens.json` automatically, de-duplicating shared feeds.)

---

## Step 3 — Write the market config

Copy the template and fill it in — strip the comments so it is valid JSON:

```bash
cp market-config-template.jsonc my-market.json
```

The template documents every field; the decisions that matter:

- **`ltvConfig`** — `maxLtv` (borrow limit), `unhealthyLtv` (liquidation
  threshold), `liquidationBonus`. `maxLtv` must be `< unhealthyLtv`, and
  `unhealthyLtv * (1 + liquidationBonus) <= 0.99`.
- **`maxUtilisationRate`** — utilization cap above which new borrows are blocked.
- **`interestRate`** — `Adaptive` (recommended), `Fixed`, or `Polyline`.
- **`lendingMarketFeeInBps`** — fee on borrower interest (≤ 2000), split between
  curator and protocol.
- **`supplyOracleConfig` / `collateralOracleConfig`** — the Pyth `feedId` and the
  Pyth `programId` on Arch for each asset, plus staleness/confidence validation.

> **Two fields are effectively permanent:** `unhealthyLtv` can only be raised
> later (never lowered), and the **interest-rate curve cannot be changed at all**
> after creation. Choose them deliberately. See `CURATOR.md` for guidance.

---

## Step 4 — Create the market

The `--signer` is the curator.

```bash
cargo run --bin autara-cli -- \
  --arch-node https://rpc.testnet.arch.network \
  --signer keys/<curator>.key \
  tx create-market \
    --config my-market.json \
    --supply-mint <SUPPLY_MINT_PUBKEY> \
    --collateral-mint <COLLATERAL_MINT_PUBKEY>
```

Use the mint pubkeys from `tokens.json`. To run more than one market for the same
pair + curator, bump `index` in the config and create again.

---

## Step 5 — Verify

```bash
# the new market and its parameters
cargo run --bin autara-cli -- read markets
cargo run --bin autara-cli -- read market --market <MARKET_PUBKEY>

# both oracle feeds resolve for the market
cargo run --bin autara-cli -- oracle market-feeds --market <MARKET_PUBKEY>
```

---

## Step 6 — Post-creation hardening

- **Cap exposure.** `maxSupplyAtoms` defaults to unlimited. Set it conservatively
  via `UpdateConfig` relative to on-chain liquidity for the collateral asset, and
  raise it as liquidity grows.
- **Monitor** utilization, oracle staleness/confidence, and liquidation activity
  (see `CURATOR.md`). For `Adaptive` markets, ensure at least one transaction
  every few weeks while utilization is high, to avoid the rate-curve edge case.

---

## Recommended: declarative, repeatable deploys

For more than a one-off — and for re-running safely — define markets
declaratively in `markets.json` rather than calling `create-market` by hand. Each
entry is one direction of a pair:

```jsonc
{
  "name": "aUSD-BTC (77% LTV)",
  "supply": "aUSD",
  "collateral": "BTC",
  "curatorKeyFile": "keys/curator-conservative.key",
  "index": 0,
  "ltvConfig": { "maxLtv": "0.77", "unhealthyLtv": "0.86", "liquidationBonus": "0.05" }
}
```

`autara-server` reads `tokens.json` + `markets.json` on startup and creates every
market **idempotently** (skipping ones that already exist), funding curators and
ensuring the global config along the way. This is how the current BTC-USDC and
aUSD-BTC markets are deployed, and it's the path to prefer for reproducible
environments.
