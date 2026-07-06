# Shared Loss / Loss Socialization — Mechanism, Test, and Operator Runbook

Status: verified on testnet against a fresh, isolated market. This document maps
what the code **actually does** (with `file:line` references), gives a
re-runnable proof, and provides operator instructions for a mainnet incident.

> TL;DR for reviewers (Matt/Amine): the code matches the "curator sweeps the
> collateral without paying the pool, liquidates off-chain, and adds back an
> arbitrary amount" description **almost exactly**. The only material difference
> vs. the informal description is the **trigger condition**: socialization is
> gated purely on `LTV >= 1.0` (position underwater). It is **not** gated on a
> "recovery mode" flag, and it does **not** require that an on-chain liquidation
> was attempted or proven impossible first. The add-back (`donate_supply`) is
> fully discretionary — nothing on-chain forces the curator to return anything,
> or a fair amount.

---

## 1. The two instructions

The feature is two separate instructions, both defined in the instruction enum
at `autara-lib/src/ixs/types.rs:57-63` (`SocializeLoss` = tag 18,
`DonateSupply` = tag 19).

### 1a. `SocializeLoss` — the pool eats the loss, curator takes the collateral

| Property | Value |
|---|---|
| Instruction tag | `18` (`AurataInstructionTag::SocializeLoss`, `autara-lib/src/ixs/types.rs:57`) |
| Builder | `socialize_loss_ix(...)` `autara-lib/src/ixs/liquidation.rs:79` |
| Client (lib) | `tx_builder().socialize_loss(market, position)` `autara-client/src/client/tx_builder.rs:566`; `client.socialize_loss(...)` `autara-client/src/client/client_with_signer.rs:369` |
| CLI | **Not exposed.** The `cli` binary has `Liquidate` and `DonateSupply` but no `SocializeLoss` (`autara-client/src/bin/cli.rs`). Operators must call it via the client library / an example. |
| Must sign | **Curator only** — `market.config().curator() != curator.key` ⇒ `InvalidMarketAuthority` (`programs/autara-program/src/ixs/socialize_loss.rs:56-58`). Not the global admin, not a liquidator, not anyone else. |

**Accounts** (`programs/autara-program/src/ixs/socialize_loss.rs:16-25`): market,
borrow_position, curator (signer), `receiver_collateral_ata`,
`market_collateral_vault`, apl token program, supply oracle, collateral oracle.
The client wires `receiver_collateral_ata` to the **curator's own ATA**
(`autara-client/src/client/tx_builder.rs:591-595`).

**Trigger condition enforced on-chain**
(`autara-lib/src/state/market.rs:414-425`):

```414:433:autara-lib/src/state/market.rs
    pub(super) fn socialize_loss(
        &mut self,
        borrow_position: &mut BorrowPosition,
        collateral_oracle: &OracleRate,
        supply_oracle: &OracleRate,
    ) -> LendingResult<(u64, u64)> {
        let health = self
            .borrow_position_health(borrow_position, collateral_oracle, supply_oracle)
            .track_caller()?;
        if health.ltv < IFixedPoint::one() {
            return Err(LendingError::CannotSocializeDebtForHealthyPosition.into());
        }
        let debt = self
            .supply_vault
            .socialize_loss(borrow_position.borrowed_shares())?;
        let collateral_to_withdraw = borrow_position.collateral_deposited_atoms();
        borrow_position.repay_all();
        borrow_position.withdraw_collateral(collateral_to_withdraw)?;
        Ok((debt, collateral_to_withdraw))
    }
```

- `LTV >= 1.0` means the position is **fully underwater** (debt value ≥ collateral
  value). LTV is computed conservatively: borrow value uses the oracle **upper
  bound**, collateral value uses the **lower bound**
  (`autara-lib/src/oracle/oracle_price.rs:81-102`, `market.rs:88-118`). This is a
  *stricter* bar than liquidation, which only needs `LTV >= unhealthy_ltv` (0.9 in
  the standard config) — see `market.rs:386`.
- There is **no** "recovery mode" flag on the market, and **no** check that a
  liquidation was attempted first. The single gate is `LTV >= 1`.

**What it does to state:**
1. **Suppliers eat the loss, pro-rata.** `supply_vault.socialize_loss(debt_shares)`
   (`autara-lib/src/state/supply_vault.rs:279-285`) removes the borrow shares and
   calls `SharesTracker::socialize_loss_atoms` (`autara-lib/src/math/shares_tracker.rs:159-167`),
   which lowers `atoms_per_share` by `debt / total_shares`. Every supplier's
   redeemable balance drops proportionally — this is a share-price writedown, not
   a targeting of any single supplier.
2. **The bad position is zeroed.** `repay_all()` clears the debt and
   `withdraw_collateral(all)` zeroes the collateral (`market.rs:430-431`).
3. **The curator receives ALL the collateral, paying nothing.** The processor
   transfers the full collateral balance from the market vault to the curator's
   ATA (`programs/autara-program/src/processor/socialize_loss.rs:49-60`). No
   tokens move into the supply vault in this instruction.
4. Emits `SocializeLossEvent { debt_socialized, collateral_liquidated }`
   (`processor/socialize_loss.rs:33-38`).

### 1b. `DonateSupply` — the (discretionary) add-back

| Property | Value |
|---|---|
| Instruction tag | `19` (`autara-lib/src/ixs/types.rs:61`) |
| Builder | `donate_supply_ix(...)` `autara-lib/src/ixs/supply.rs:133` |
| Client (lib) | `tx_builder().donate_supply(market, atoms)` `autara-client/src/client/tx_builder.rs:603`; `client.donate_supply(...)` `autara-client/src/client/client_with_signer.rs:356` |
| CLI | `TxCommands::DonateSupply { market, amount }` `autara-client/src/bin/cli.rs:236,734` |
| Must sign | **Anyone.** The account validation only checks the vault/mint match — there is **no curator or admin check** (`programs/autara-program/src/ixs/donate_supply.rs:38-47`). |

Behavior (`programs/autara-program/src/processor/donate_supply.rs:11-48`):
transfers `amount` supply tokens from the caller's ATA into the market supply
vault and calls `donate_atoms` (`autara-lib/src/math/shares_tracker.rs:149-157`),
which **raises** `atoms_per_share` by `amount / total_shares`. No supply shares
are minted, so the donation accrues entirely to existing suppliers pro-rata. The
amount is arbitrary and caller-chosen.

---

## 2. Code vs. the informal descriptions

**Daniel/Matt's description:** *"if there isn't enough on-chain liquidity for
liquidation, the market creator (curator) can sweep the collateral WITHOUT
paying the pool, liquidate off-chain, and arbitrarily decide how much to add
back."*

| Claim | Verdict | Evidence |
|---|---|---|
| Curator sweeps the collateral | ✅ Matches | curator ATA receives all collateral, `processor/socialize_loss.rs:49-60` |
| Without paying the pool | ✅ Matches | `socialize_loss` moves no supply tokens in; curator's supply balance is unchanged (proven in the test) |
| Off-chain liquidation, arbitrary add-back | ✅ Matches | `donate_supply` is a separate, discretionary instruction with a caller-chosen `amount`; nothing forces it |
| Triggered by "not enough liquidity to liquidate" | ⚠️ **Differs** | The on-chain gate is purely `LTV >= 1`. There is no liquidity check and no requirement that liquidation was attempted/failed first. A curator can socialize any position the moment it goes underwater, even if a liquidator could have cleared it. |
| A "recovery mode" flag on the market | ⚠️ **Not in code** | No such flag exists; the market state has no recovery mode toggle. |

**Deepanshu's description:** *"make a bad loan, and when the loan hits a certain
threshold, the operator calls a feature that shares loss amongst everyone on the
pool."* ✅ Matches — the "certain threshold" is `LTV >= 1.0`, and the loss is
shared pro-rata across all suppliers via the share-price writedown. Note the
operator that must sign is specifically the **curator** of that market.

**Is there a normal liquidation path with a bonus?** Yes —
`Liquidate` (tag 10). A liquidator repays debt and receives collateral plus a
`liquidation_bonus` (`autara-lib/src/state/market.rs:376-411`), and it only
requires `LTV >= unhealthy_ltv`. `socialize_loss` is the *fallback* for when the
position is already underwater (`LTV >= 1`, i.e. the collateral no longer covers
the debt even without a bonus) and no liquidator has stepped in — but the code
does **not** enforce that liquidation was tried first.

### Points requiring a human decision (Matt/Amine)
1. **No liquidation-first / liquidity check.** Confirm it's intended that the
   curator can socialize purely on `LTV >= 1` without proving liquidation was
   impossible. As written, a position at exactly `LTV = 1` can be swept.
2. **Fully trusted curator on the add-back.** The collateral leaves to the
   curator with zero on-chain accounting tying it to a repayment. Suppliers rely
   entirely on the curator's honesty to (a) sell the collateral and (b) donate a
   fair amount back. Consider whether an escrow/attestation or a minimum-add-back
   is wanted before mainnet.
3. **`donate_supply` is permissionless.** Fine for "anyone can top up the pool",
   but worth an explicit acknowledgement.
4. **No CLI command for `socialize_loss`.** The emergency instruction is only
   reachable via the client library. For incident response, either add a CLI
   subcommand or rely on the committed example below. (Not fixed here — no code
   behavior changed.)

---

## 3. Existing test coverage

- **Unit tests (`autara-lib`)**: `market.rs:1137` (supplier writedown),
  `market_wrapper.rs:581/599/703` (healthy position rejected; underwater
  accepted; crash-to-near-zero), `supply_vault.rs:556/569`, `shares_tracker.rs:422/432`.
- **Program account-validation tests**: `programs/autara-program/src/ixs/socialize_loss.rs:75-186`
  (curator-must-sign, market/owner mismatches).
- **Live integration tests (testnet, fresh market)**:
  `autara-integration-tests/tests/autara/socialize_loss.rs` —
  `only_curator_can_socialize_loss`, `curator_can_socialize_loss`, `can_donate`.
  All three pass (see §4).

Before this task these paths had never been exercised end-to-end by the team;
they now are (both the integration tests and the new example below).

---

## 4. Runnable proof

Two ways to reproduce, both against **testnet with fresh isolated markets/feeds**
(they never touch the shared aUSD/aBTC market):

### 4a. Existing integration tests

```bash
cargo test -p autara-integration-tests --test tests socialize_loss -- --nocapture --test-threads=1
```

Result: `3 passed` (`only_curator_can_socialize_loss`,
`curator_can_socialize_loss`, `can_donate`).

### 4b. New env-driven example with txids

`autara-client/examples/shared_loss_flow.rs` builds a fresh market with fresh
oracle feeds (via `AutaraTestEnv`), manufactures a bad loan, socializes it, and
adds back — printing txids and before/after numbers with per-step PASS/FAIL.

```bash
cargo run -p autara-client --example shared_loss_flow
```

Optional env knobs (defaults give a 70% LTV loan hit by a 60% collateral crash):
`SL_SUPPLY_UNITS`, `SL_COLLATERAL_UNITS`, `SL_BORROW_UNITS`,
`SL_COLLATERAL_PRICE`, `SL_CRASH_PRICE`, `SL_DONATE_UNITS`.

**Verified live run (all 6 assertions PASS):**

- market `2d70d161e224b2e38a6ea806ea2bafe8a9799a6ec1f57dd5330d1bcad8e80376`
- supply — `33f3810458ba78a886a698dabd944acd0d5bc42fa38dfacb74be0259bca40b2f`
- deposit collateral — `51b7e7362bf2ef168a48c06f8bf80fe46304dd414f77f3869b6bce3954ad05e1`
- borrow (LTV 0.7141) — `38a3cdc04f798af838f73333ebedce04fda12b412c4f6c26c04eafb1b77828f0`
- **socialize_loss** — `9ede916bc78ef7c6d53052c126ce1cc9a7b02959d56883815eedef6a737f06d4`
  - supplier A written down by `70000000050425` atoms (≈ the debt)
  - bad position debt → `0`
  - curator swept `1000000000` collateral atoms (the whole position)
  - curator supply balance **unchanged** (paid nothing into the pool)
- **donate_supply** (add-back `40000` units) — `19a5f01dafc05fbb0dda5f0df69e4585093527a8fb1c562f800aa4d86de0639d`
  - supplier A recovered `39999999999134` atoms
  - **net supplier A loss = `30000000051291` atoms** (the shortfall the curator did not add back)

This demonstrates the whole flow: the pool takes the loss pro-rata, the curator
takes the collateral for free, and the add-back is whatever the curator chooses
(here 40k of a 70k debt, leaving a 30k net loss to suppliers).

---

## 5. Operator runbook (mainnet incident response)

**When:** a borrow position is underwater (`LTV >= 1`) and no liquidator cleared
it, so the market is carrying bad debt. Socialization writes that loss down
across all suppliers of **that isolated market only**.

**Who:** the transaction must be signed by the **curator** of the affected
market. For the shared market the curator is `autara-deploy/.keys-testnet/admin.json`
on testnet; use the corresponding mainnet curator key.

**Preconditions / checklist:**
1. Confirm the position is genuinely underwater and cannot be liquidated
   normally (a normal `Liquidate` with a bonus is preferable when possible — it
   costs suppliers nothing). Only socialize as a last resort.
2. Confirm the oracle is fresh (feeds pushed within `max_age`, default 60s),
   otherwise the tx fails with an oracle-staleness error.
3. Identify the market pubkey and the bad borrow position pubkey.

**Step 1 — socialize the loss (curator-signed).** There is no CLI subcommand, so
use the client library. Minimal call:

```rust
// signer = curator keypair; program_id, market, position as pubkeys
let events = curator_client.socialize_loss(&market, &position).await?;
```

Effect: suppliers of that market are written down pro-rata by the outstanding
debt; the position's debt and collateral are zeroed; **all** the collateral lands
in the curator's ATA.

**Step 2 — liquidate the swept collateral off-chain.** The curator now holds the
collateral. Sell/convert it through whatever venue recovers the most value.

**Step 3 — add the recovered value back to the pool.** Return proceeds to
suppliers via `DonateSupply` (this one *is* on the CLI):

```bash
cargo run -p autara-client --bin cli -- \
  --network <mainnet> tx donate-supply \
  --market <MARKET_PUBKEY> --amount <ATOMS_RECOVERED>
```

The donation raises every supplier's redeemable balance pro-rata. Donating the
full recovered amount minimizes suppliers' net loss; the shortfall between the
socialized debt and the add-back is the loss suppliers ultimately bear.

**Safety notes:**
- Socialization is irreversible and directly reduces supplier balances — treat it
  as a break-glass action.
- It only affects the single isolated market; other markets are untouched.
- Because the collateral leaves to the curator with no on-chain repayment link,
  the add-back is a manual, trust-based step. Track the swept collateral and the
  add-back carefully and publish the accounting.
