# Shared loss / socialization — mechanism map

This is the operator-facing companion to `autara-client/examples/shared_loss_flow.rs`
and `autara-integration-tests/tests/autara/socialize_loss.rs`.

## When it happens

A borrow position can become **bad debt**: collateral value (conservative oracle
lower bound) is less than outstanding debt, so even a full liquidation cannot
make suppliers whole. Autara does **not** invent tokens. The curator must
`socialize_loss`, which:

1. Writes down every supplier's redeemable balance **pro-rata** by the full
   outstanding debt (the pool eats the loss).
2. Clears the bad position's debt and collateral to zero.
3. Transfers **all** remaining collateral to the **curator** wallet (curator
   pays nothing into the pool).

The curator is then trusted to sell the collateral off-chain / via DEX and
optionally `donate_supply` any recovery back into the vault so suppliers recover
pro-rata. Nothing on-chain forces a fair or complete donate.

## Permissions

| Instruction        | Who can call                         |
|--------------------|--------------------------------------|
| `socialize_loss`   | **Market curator only**              |
| `donate_supply`    | **Anyone** (permissionless)          |

Non-curator `socialize_loss` must fail. Covered by integration test
`only_curator_can_socialize_loss`.

## Live rehearsal (testnet)

```bash
# Fresh self-contained market on the stage program (does NOT touch aUSD/aBTC):
cargo run -p autara-client --example shared_loss_flow
```

Optional knobs: `SL_SUPPLY_UNITS`, `SL_COLLATERAL_UNITS`, `SL_BORROW_UNITS`,
`SL_COLLATERAL_PRICE`, `SL_CRASH_PRICE`, `SL_DONATE_UNITS`.

## Mainnet runbook (curator)

1. Confirm bad debt: position LTV ≥ 100% and liquidators cannot clear it.
2. Snapshot supplier redeemable balances and the position's debt/collateral.
3. Call `socialize_loss` with the curator key (`CURATOR_KEY_PATH`).
4. Receive swept collateral; sell via CLAMM/PropAMM/CEX as needed.
5. `donate_supply` recovery (any amount; ideally ≈ realized sale proceeds).
6. Record txids + before/after balances in the incident log.
7. Alert suppliers / governance per ops policy.

## Signoff evidence Deepanshu should expect

- [ ] Integration tests green: curator-only reject, happy-path socialize, donate.
- [ ] `shared_loss_flow` example PASS on testnet (attach log).
- [ ] This doc reviewed; mainnet runbook steps accepted.
- [ ] Curator key is dedicated (≠ protocol admin) and custody documented.
