# Oracle price pusher (Arch mainnet + testnet)

The lending program rejects any supply/borrow/liquidate whose oracle is older
than `max_age` (60s) with `OracleRateTooOld` (`0x1b70`). The oracle account's
`publish_time` is stamped with the **on-chain clock at push time**
(`programs/autara-oracle/src/lib.rs`) and compared against the on-chain clock at
read time — same clock, so a feed is fresh only while something keeps writing to
its PDA. Each network therefore needs its own pusher writing to that network's
oracle program.

## Design

One container image, one `entrypoint.sh`, selected by `ROLE`:

- `ROLE=server` (default) — the API/indexer. Unchanged behavior. Set
  `DISABLE_PRICE_PUSHER=1` to hand pushing to a dedicated pusher.
- `ROLE=pusher` — the dedicated `autara-pyth` price pusher, one per network.

Mainnet and testnet run the **same image**; only env differs. Example env in
`autara-deploy/scripts/autara.pusher.{testnet,mainnet}.env`.

## Deploy (two pusher services)

Point a service at this image and set env from the matching file:

| Env | testnet | mainnet |
|-----|---------|---------|
| `ROLE` | `pusher` | `pusher` |
| `NETWORK` | `testnet` | `mainnet` |
| `ARCH_RPC_URL` | `https://rpc.testnet.arch.network` | `https://rpc.mainnet.arch.network` |
| `ORACLE_PROGRAM_ID` | `eee682c2…` (deployed stage oracle) | `a2b2fe9e…` (deploy artifact `oracle_id`) |
| `FEEDS` | BTC,USDC (add ETH if used) | BTC,USDC |
| `SIGNER_KEY_B64` | optional (faucet funds a throwaway key) | **required** — pre-funded key, no faucet |

`NETWORK=mainnet` is translated to `--network bitcoin` for `autara-pyth`
(it parses the raw `bitcoin::Network`).

## Mainnet operational notes

- **Fund + monitor the signer.** There is no faucet and no airdrop loop on the
  standalone pusher. If the signer runs dry, pushes stop and markets fail with
  `0x1b70`. Any key works (the oracle program only needs a signature) — use a
  dedicated low-value key and alert on its balance.
- **Re-funding process:** keep a cold/ops wallet funded; when
  `autara_pusher_balance_lamports` drops below ~24h of fee runway, transfer
  lamports to the pusher signer pubkey. Record the top-up tx in the ops log.
  Prefer a recurring calendar check even if alerts are green.
- **Redundant backup:** run a second pusher only as hot-standby with the same
  `ORACLE_PROGRAM_ID`/`FEEDS` but a *different* signer key, kept stopped or
  rate-limited so it does not double-push. Failover = start standby, fund it,
  stop the primary. Document the DRI who can flip it.
- **Feeds push atomically.** All feeds go in one transaction; a malformed feed
  drops the whole push. Keep the mainnet feed list to what the markets need.
- If you also run `ROLE=server` on mainnet, set `DISABLE_PRICE_PUSHER=1` there so
  it doesn't double-push (and note the server auto-creates markets under its own
  signer — keep that off mainnet unless that's intended).

## Sanity check

A market recovers within one push cycle (~5s) once any write lands on its
oracle PDA. If it stays stale: confirm `ORACLE_PROGRAM_ID` matches the program
the markets were created against, and that pushes reach `Status::Processed`
(the signer is funded), not just log `Sending`.

## Testnet repair: `InvalidPythOracleAccount` / `0x1b69`

Live stage markets (`program 53def2dc…`, `oracle eee682c2…`) still have
**120-byte** pre-authority feed PDAs. The lending program expects
`PythPriceAccount` (**152 bytes**). Symptom: `SupplyApl` fails at
`autara-lib/src/oracle/pyth.rs:48` with `LendingError(InvalidPythOracleAccount)`.

Confirm layout:

```bash
cargo run -p autara-pyth --example check_feed_layout
```

Legacy lines look like `data_len=120 layout=LEGACY(PythPrice)`.

Repair (in order):

1. **Pin a stable pusher signer** on the testnet Railway pusher
   (`SIGNER_KEY_B64`). The first successful post-upgrade push binds feed
   `authority` to that key — a throwaway faucet key will strand the feeds.

   ```bash
   # generate + faucet-fund; copy SIGNER_KEY_B64 to clipboard (secret not printed)
   ./autara-deploy/scripts/provision-pusher-signer.sh --network testnet --fund --copy
   ```

   Paste the clipboard into the Railway testnet pusher's `SIGNER_KEY_B64`, set
   `PUSHER_PUBKEY` from the script's printed arch pubkey (optional, for server
   balance metrics), and redeploy/restart the pusher.
2. **Upgrade only the oracle ELF** at `eee682c2…` (note: `autara-upgrade.yml`
   deliberately does **not** touch the oracle — that is how lending got ahead
   of the feeds). From a commit that includes legacy→152-byte realloc on push:

   ```bash
   # build ELF
   ( cd programs/autara-oracle && cargo-build-sbf --features entrypoint )

   # re-upload oracle only against the live stage keys
   set -a; source autara-deploy/scripts/autara.testnet.env; set +a
   STEP_DEPLOY_PROGRAM=false STEP_DEPLOY_ORACLE=true \
   STEP_INIT_CONFIG=false STEP_TOKEN_SETUP=false STEP_CREATE_MARKET=false \
   DRY_RUN=1 cargo run -p autara-deploy   # preview first
   # then DRY_RUN=0 (or unset) for the real upload
   ```

3. Restart / let the pusher run one cycle. Feeds should become
   `data_len=152 layout=NEW(PythPriceAccount)`.
4. Re-run `check_feed_layout`, then retry Supply on
   `arch-swap-nine.vercel.app`.

Do **not** fix this by relaxing the on-chain lending loader: that would accept
unowned legacy feeds forever. Mainnet feeds are created at the new size and do
not need this path.
