//! Individual, flag-gated deploy steps. Each step is a thin wrapper that builds
//! the relevant instruction(s) via `autara-lib` and sends them through the
//! shared [`RpcContext`], recording tx ids into the [`DeploymentArtifact`].

use anyhow::{bail, Result};
use arch_program::pubkey::Pubkey;

use autara_lib::interest_rate::interest_rate_kind::InterestRateCurveKind;
use autara_lib::ixs::{create_market_ix, CreateMarketInstruction};
use autara_lib::math::ifixed_point::IFixedPoint;
use autara_lib::oracle::oracle_config::OracleConfig;
use autara_lib::pda::find_market_pda;
use autara_lib::state::market_config::LtvConfig;

use crate::artifact::{DeploymentArtifact, MarketRecord};
use crate::config::{pyth_feed_for_label, MarketPair, MarketParams, TokenConfig};
use crate::rpc::RpcContext;

/// Create the protocol's global config PDA (admin + fee receiver + fee share).
///
/// Idempotent: if the global config already exists on-chain, the existing PDA
/// is returned and no new transaction is recorded.
pub async fn create_global_config(
    ctx: &RpcContext,
    autara_program_id: Pubkey,
    admin: Pubkey,
    fee_receiver: Pubkey,
    protocol_fee_share_bps: u16,
    artifact: &mut DeploymentArtifact,
) -> Result<Pubkey> {
    let payer = ctx.payer_pubkey();
    let (global_config_pda, ix) = autara_lib::ixs::create_global_config_ix(
        autara_program_id,
        payer,
        admin,
        fee_receiver,
        protocol_fee_share_bps,
    );

    match ctx.send(vec![ix], vec![]).await {
        Ok(txid) => {
            artifact.record_tx("create_global_config", txid);
        }
        Err(e) if e.to_string().contains("already exists") => {
            println!("global_config:     already exists ({global_config_pda}) — skipping");
        }
        Err(e) => return Err(e),
    }

    artifact.global_config = Some(global_config_pda.to_string());
    Ok(global_config_pda)
}

/// Token-setup step: ENSURE every configured token mint is present on-chain.
///
/// The deploy tool intentionally carries token *pubkeys* only (never the mint
/// authority secret), so it does not create mints — those are produced once by
/// `autara-cli token setup`. This step is the idempotent guard that the mints
/// exist before any market is created; it fails loudly (with a pointer to the
/// CLI) rather than letting `create_market` fail with a confusing on-chain
/// error. Mints are already recorded in the artifact's `tokens` list.
pub async fn ensure_token_mints(ctx: &RpcContext, tokens: &[TokenConfig]) -> Result<()> {
    for token in tokens {
        if ctx.account_exists(token.mint).await {
            println!("token {:<6}     mint {} present", token.label, token.mint);
        } else {
            bail!(
                "token '{}' mint {} is not on-chain — create the APL mints first \
                 (e.g. `autara-cli token setup`); the deploy tool holds no mint authority",
                token.label,
                token.mint
            );
        }
    }
    Ok(())
}

/// Build a `CreateMarketInstruction` from the env-configured market parameters.
///
/// `MarketParams::default()` reproduces `autara-server`'s `default_market_config`
/// (max_ltv 0.8 / unhealthy 0.9 / liquidation bonus 0.05, 90% max utilisation),
/// so an unset env yields byte-for-byte identical instructions (the `f64` values
/// flow through `IFixedPoint::from_num` exactly as the old literals did — see
/// `tests::default_params_match_legacy_hardcoded_values`). The interest-rate
/// curve is always adaptive (not parameterized).
fn build_create_market_instruction(
    index: u8,
    lending_market_fee_bps: u16,
    params: MarketParams,
    supply_oracle: OracleConfig,
    collateral_oracle: OracleConfig,
) -> CreateMarketInstruction {
    CreateMarketInstruction {
        // `market_bump` is overwritten by `create_market_ix` from the derived PDA.
        market_bump: 0,
        index,
        ltv_config: LtvConfig {
            max_ltv: IFixedPoint::from_num(params.max_ltv),
            unhealthy_ltv: IFixedPoint::from_num(params.unhealthy_ltv),
            liquidation_bonus: IFixedPoint::from_num(params.liquidation_bonus),
        },
        max_utilisation_rate: IFixedPoint::from_num(params.max_utilisation_rate),
        supply_oracle_config: supply_oracle,
        collateral_oracle_config: collateral_oracle,
        interest_rate: InterestRateCurveKind::new_adaptive(),
        lending_market_fee_in_bps: lending_market_fee_bps,
    }
}

/// Derive the market PDA for a pair without sending anything (used by the
/// dry-run preview and by [`create_market`]).
pub fn derive_market_pda(
    autara_program_id: Pubkey,
    curator: Pubkey,
    supply: &TokenConfig,
    collateral: &TokenConfig,
    index: u8,
) -> Pubkey {
    find_market_pda(
        &autara_program_id,
        &curator,
        &supply.mint,
        &collateral.mint,
        index,
    )
    .0
}

/// Create a lending market for one supply/collateral pair (curator = admin).
///
/// Idempotent: if the market PDA already exists it is recorded with
/// `created=false` and no transaction is sent. Reuses
/// `autara_lib::ixs::create_market_ix` so the on-chain layout cannot drift.
#[allow(clippy::too_many_arguments)]
pub async fn create_market(
    ctx: &RpcContext,
    autara_program_id: Pubkey,
    oracle_program_id: Pubkey,
    curator: Pubkey,
    pair: &MarketPair,
    supply: &TokenConfig,
    collateral: &TokenConfig,
    lending_market_fee_bps: u16,
    market_params: MarketParams,
    index: u8,
    artifact: &mut DeploymentArtifact,
) -> Result<Pubkey> {
    let supply_feed = pyth_feed_for_label(&supply.label)
        .ok_or_else(|| anyhow::anyhow!("no Pyth feed for supply token '{}'", supply.label))?;
    let collateral_feed = pyth_feed_for_label(&collateral.label).ok_or_else(|| {
        anyhow::anyhow!("no Pyth feed for collateral token '{}'", collateral.label)
    })?;

    let market_pda = derive_market_pda(autara_program_id, curator, supply, collateral, index);
    let label = format!(
        "create_market:{}/{}",
        pair.supply_label, pair.collateral_label
    );

    if ctx.account_exists(market_pda).await {
        println!(
            "market {}/{} already exists ({market_pda}) — skipping",
            pair.supply_label, pair.collateral_label
        );
        artifact.record_market(MarketRecord {
            supply_label: pair.supply_label.clone(),
            collateral_label: pair.collateral_label.clone(),
            supply_mint: supply.mint.to_string(),
            collateral_mint: collateral.mint.to_string(),
            index,
            market: market_pda.to_string(),
            created: false,
        });
        return Ok(market_pda);
    }

    let create_market = build_create_market_instruction(
        index,
        lending_market_fee_bps,
        market_params,
        OracleConfig::new_pyth(supply_feed, oracle_program_id),
        OracleConfig::new_pyth(collateral_feed, oracle_program_id),
    );
    let (derived, ix) = create_market_ix(
        create_market,
        supply.mint,
        collateral.mint,
        autara_program_id,
        curator,
        ctx.payer_pubkey(),
    );
    debug_assert_eq!(derived, market_pda);

    let txid = ctx.send(vec![ix], vec![]).await?;
    artifact.record_tx(&label, txid);
    artifact.record_market(MarketRecord {
        supply_label: pair.supply_label.clone(),
        collateral_label: pair.collateral_label.clone(),
        supply_mint: supply.mint.to_string(),
        collateral_mint: collateral.mint.to_string(),
        index,
        market: market_pda.to_string(),
        created: true,
    });
    Ok(market_pda)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arch_program::pubkey::Pubkey;

    /// Proof that making the market params env-configurable did NOT change
    /// testnet behavior: with `MarketParams::default()` the built instruction is
    /// byte-for-byte identical to the previously hardcoded values
    /// (max_ltv 0.8 / unhealthy 0.9 / liquidation_bonus 0.05 / max_util 0.9).
    #[test]
    fn default_params_match_legacy_hardcoded_values() {
        let oracle = OracleConfig::new_pyth([0u8; 32], Pubkey::new_from_array([0u8; 32]));
        let ix = build_create_market_instruction(0, 100, MarketParams::default(), oracle, oracle);
        assert_eq!(ix.ltv_config.max_ltv, IFixedPoint::from_num(0.8));
        assert_eq!(ix.ltv_config.unhealthy_ltv, IFixedPoint::from_num(0.9));
        assert_eq!(ix.ltv_config.liquidation_bonus, IFixedPoint::from_num(0.05));
        assert_eq!(ix.max_utilisation_rate, IFixedPoint::from_num(0.9));
    }
}
