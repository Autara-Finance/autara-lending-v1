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
