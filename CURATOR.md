# Autara Lending - Curator Guide

## Overview

Autara Lending is a permissionless lending protocol on Arch Network. Each **market** pairs a supply asset (lent by suppliers, borrowed by borrowers) with a collateral asset. As a curator, you manage the risk parameters of your market(s).

## How the Protocol Works

### Core Flow

1. **Suppliers** deposit tokens into the supply vault and receive shares representing their proportional ownership.
2. **Borrowers** deposit collateral, then borrow from the supply vault up to their allowed Loan-to-Value (LTV) ratio.
3. **Interest** accrues on borrows. Interest is split between suppliers (who earn yield on their deposits) and fees (curator + protocol).
4. **Liquidators** can repay part of a borrower's debt when their position becomes unhealthy (LTV >= `unhealthy_ltv`), receiving the borrower's collateral at a discount (`liquidation_bonus`).

### Interest Rate Model

Three curve types are available (set at market creation, immutable after):

- **Fixed**: A constant borrow rate.
- **Polyline**: Up to 8 breakpoints mapping utilisation to borrow rate (strictly increasing).
- **Adaptive** (Morpho Blue IRM): Targets 90% utilisation. The rate adjusts exponentially:
  - Above 90% utilisation: rate increases at speed `50/year * error`
  - Below 90% utilisation: rate decreases at the same speed
  - Bounded between **1% APR** (floor) and **200% APR** (ceiling)
  - Initial rate at target: **4% APR**
  - Curve steepness factor: **4x** (at 100% utilisation, borrow rate = 4x the rate at target)

### Oracle Pricing

Each asset uses an oracle (Chaos or Pyth (testnet only for now)) providing `price +/- confidence`.

- **Collateral** is valued conservatively at `price - confidence` (lower bound).
- **Borrow debt** is valued at `price + confidence` (upper bound).
- Default oracle staleness: **60 seconds**.
- Default max relative confidence: **5%**.

### Shares and Rounding

All accounting uses a share-based system. Rounding always favors the protocol:
- Deposits/lends: shares rounded **down** (depositor gets slightly fewer shares).
- Withdrawals/repayments: shares rounded **up** (protocol retains dust).

### Liquidation

When a position's LTV reaches `unhealthy_ltv`:
1. Liquidators can repay part of the borrower's debt.
2. They receive collateral worth `repaid_debt * (1 + liquidation_bonus)`.
3. Liquidation targets bringing LTV down to `max(unhealthy_ltv * 0.9, max_ltv)`.
4. If LTV >= 100% (bad debt), full liquidation occurs with no bonus.

## Curator Capabilities

As a curator, you can update these parameters via the `UpdateConfig` instruction (all fields are optional per call):

### LTV Configuration

| Parameter | Description | Constraints |
|-----------|-------------|-------------|
| `max_ltv` | Maximum LTV for new borrows | Must be < `unhealthy_ltv` |
| `unhealthy_ltv` | LTV threshold for liquidation | Can only be **increased**, never decreased |
| `liquidation_bonus` | Bonus paid to liquidators | Between 0.1% and 10% |

Additional constraint: `unhealthy_ltv * (1 + liquidation_bonus) <= 0.99`

**Warning**: `unhealthy_ltv` can never be lowered once set. This is to protect existing borrowers from being immediately liquidated. Choose carefully.

### Fee Configuration

| Parameter | Description | Constraints |
|-----------|-------------|-------------|
| `lending_market_fee_in_bps` | Fee on interest paid by borrowers | Max 2000 bps (20%) |

This fee is taken from the interest that would go to suppliers. It is split between:
- **Curator share**: `(10000 - protocol_fee_share_in_bps) / 10000` of the fee
- **Protocol share**: `protocol_fee_share_in_bps / 10000` of the fee (set by protocol admin, not the curator)

### Supply and Utilisation Caps

| Parameter | Description | Constraints |
|-----------|-------------|-------------|
| `max_utilisation_rate` | Max utilisation after a borrow | Max 0.99 (99%) |
| `max_supply_atoms` | Maximum total supply in the vault | No upper bound |

Note: `max_utilisation_rate` only blocks new borrows. Withdrawals can push utilisation above this cap (but never above 100%).

### Oracle Configuration

| Parameter | Description |
|-----------|-------------|
| `supply_oracle_config` | Oracle for the supply asset |
| `collateral_oracle_config` | Oracle for the collateral asset |

## What to Monitor

### 1. Utilisation Rate

- **Normal range**: Below your `max_utilisation_rate`.
- **Concern**: Sustained high utilisation (>90%) means suppliers may struggle to withdraw.
- **Action**: Consider adjusting `max_utilisation_rate` or `lending_market_fee_in_bps` to incentivize rebalancing.

### 2. Adaptive Curve Bricking Risk

If your market uses the **Adaptive** interest rate curve:

- **Risk**: If the market stays at >90% utilisation with **zero transactions** for ~1.1+ years, the adaptive rate calculation permanently fails (the exponential exceeds the computable domain). The market becomes bricked.
- **Why**: `last_update_unix_timestamp` only advances on successful rate updates. Once the accumulated time is too large, all future updates also fail.
- **Prevention**: Ensure at least one transaction (supply, borrow, repay, withdraw, or liquidation) occurs periodically. Any of these triggers `sync_clock` which updates the rate.
- **Monitoring**: Alert if no transactions have occurred on the market for more than a few weeks, especially at high utilisation.

### 3. Oracle Health

- **Staleness**: Oracle prices older than `max_age` (default 60s) will cause transactions to fail. If the oracle feed goes down, the market effectively pauses.
- **Confidence spread**: If `confidence / price > min_relative_confidence` (default 5%), the oracle is rejected. High volatility or low liquidity assets may trigger this.
- **Action**: If oracle issues persist, investigate the feed provider. Consider if the asset is still suitable for lending.

### 4. Liquidation Activity

- **Healthy sign**: Liquidations happening promptly when positions become unhealthy.
- **Concern**: No liquidations despite positions crossing `unhealthy_ltv` suggests liquidator infrastructure issues.
- **Bad debt**: If LTV reaches 100%+, the protocol enters bad-debt territory. Full liquidation occurs with no bonus, and suppliers may face losses.
- **Action**: Ensure liquidation bots are active. Consider adjusting `liquidation_bonus` to be attractive enough for liquidators.

### 5. LTV Parameters

- **max_ltv too close to unhealthy_ltv**: Small gap means minor price movements can push borrowers into liquidation.
- **liquidation_bonus too low**: Liquidators may not be incentivized to liquidate.
- **liquidation_bonus too high**: Borrowers lose excessive collateral on liquidation.
- **Recommended gap**: At least 5-10% between `max_ltv` and `unhealthy_ltv`.
- **On-chain liquidity matters**: `max_ltv` should account for the on-chain liquidity available for the collateral asset. If a liquidator seizes collateral but cannot sell it on-chain due to thin liquidity or high slippage, the liquidation becomes unprofitable and may not happen at all. For illiquid assets, use a lower `max_ltv` to give more room for price movements and ensure liquidators can profitably close positions before bad debt occurs.

### 6. Supply Caps

- **max_supply_atoms**: Monitor total deposits relative to this cap. If the cap is reached, new suppliers cannot deposit.
- **Scale with on-chain liquidity**: `max_supply_atoms` should be set relative to the on-chain liquidity available for the collateral asset. If total borrows grow too large relative to the collateral asset's on-chain trading volume and depth, liquidators cannot sell seized collateral without excessive slippage. This makes liquidations unprofitable and increases the risk of bad debt. Start conservative and increase `max_supply_atoms` only as on-chain liquidity for the collateral asset grows.
- **Action**: Regularly review on-chain DEX liquidity for both the supply and collateral assets. Decrease `max_supply_atoms` if liquidity deteriorates.

### 7. Fee Revenue

- Curator fees accumulate as shares in the supply vault.
- Redeem via the `RedeemCuratorFees` instruction, which transfers the underlying supply tokens to your account.
- Monitor that `protocol_fee_share_in_bps` (set by the protocol admin) hasn't changed unexpectedly. It syncs to your market on every `UpdateConfig` call.

## Key Safety Invariants

These invariants are enforced by the protocol and verified by property-based tests:

- Total borrowed never exceeds total supplied
- Utilisation rate is always in [0, 1]
- Borrows always respect `max_ltv` and `max_utilisation_rate`
- Liquidations always reduce LTV
- Only unhealthy positions (LTV >= `unhealthy_ltv`) can be liquidated
- Rounding always favors the protocol (no value extraction through dust)
- Fees never exceed total interest collected
- `unhealthy_ltv` can only increase over time

## Parameter Reference

| Constant | Value | Description |
|----------|-------|-------------|
| `MAX_LIQUIDATION_BONUS` | 10% | Maximum liquidation bonus |
| `MIN_LIQUIDATION_BONUS` | 0.1% | Minimum liquidation bonus |
| `MAX_UTILISATION_RATE` | 99% | Maximum configurable utilisation cap |
| `MAX_LENDING_MARKET_FEE` | 2000 bps (20%) | Maximum market fee on interest |
| `TARGET_LTV_LIQUIDATION_MARGIN` | 90% | Liquidation targets `unhealthy_ltv * 0.9` |
| Adaptive curve target utilisation | 90% | Target utilisation for rate adjustment |
| Adaptive curve min rate | 1% APR | Floor for rate at target |
| Adaptive curve max rate | 200% APR | Ceiling for rate at target |
| Adaptive curve initial rate | 4% APR | Starting rate at target |
| Adaptive curve steepness | 4x | Rate multiplier at 100% utilisation |
| Default oracle max age | 60s | Oracle staleness threshold |
| Default oracle max confidence | 5% | Maximum relative confidence |
