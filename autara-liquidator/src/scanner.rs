use arch_sdk::arch_program::{bitcoin::key::Keypair, bitcoin::Network, pubkey::Pubkey};
use arch_sdk::AsyncArchRpcClient;
use autara_client::client::blockhash_cache::BlockhashCache;
use autara_client::client::read::AutaraReadClient;
use autara_client::client::single_thread_client::AutaraReadClientImpl;
use autara_client::client::tx_broadcast::AutaraTxBroadcast;
use autara_client::client::tx_builder::AutaraTransactionBuilder;

use crate::config::{min_collateral_after_slippage, TokenFilter};
use crate::preflight::{lamport_balance, token_amount};

#[derive(Debug, Default)]
pub struct ScanStats {
    pub liquidatable: u64,
    pub skipped_stale_oracle: u64,
    pub skipped_insufficient_supply: u64,
    pub skipped_insufficient_gas: u64,
    pub live_success: u64,
    pub live_failure: u64,
}

/// Scan borrow positions and optionally liquidate unhealthy ones.
///
/// CLAMM callback swaps are not wired yet (arch SDK version gap vs CLAMM).
/// Live mode liquidates without a swap callback — the liquidator must already
/// hold enough supply tokens to repay debt.
pub async fn scan_liquidatable_positions(
    client: &AutaraReadClientImpl,
    token_filter: &TokenFilter,
    arch_client: &AsyncArchRpcClient,
    autara_program_id: Pubkey,
    keypair: &Keypair,
    signer: Pubkey,
    blockhash_cache: &BlockhashCache,
    network: Network,
    dry_run: bool,
    slippage_bps: u16,
    min_lamports: u64,
) -> ScanStats {
    let mut stats = ScanStats::default();
    let mut biggest_borrow: Option<(Pubkey, Pubkey, u64)> = None;
    let mut highest_ltv: Option<(Pubkey, Pubkey, autara_lib::math::ifixed_point::IFixedPoint)> =
        None;

    let gas = lamport_balance(arch_client, signer).await;

    let tx_builder = AutaraTransactionBuilder {
        arch_client,
        autara_read_client: client,
        autara_program_id,
        authority_key: signer,
        blockhash_cache: Some(blockhash_cache),
    };

    let tx_broadcast = AutaraTxBroadcast {
        program_id: &autara_program_id,
        arch_client,
    };

    for (position_key, borrow_position) in client.all_borrow_position() {
        let market_key = borrow_position.market();
        let Some(market_wrapper) = client.get_market(market_key) else {
            // Validated oracle load failed — typically stale/confidence. Visible skip.
            stats.skipped_stale_oracle += 1;
            tracing::warn!(
                position = %position_key,
                market = %market_key,
                "SKIP stale_or_invalid_oracle (market wrapper unavailable)"
            );
            continue;
        };

        let supply_mint = market_wrapper.market().supply_token_info().mint;
        let collateral_mint = market_wrapper.market().collateral_token_info().mint;

        if !token_filter.allows_market(&supply_mint, &collateral_mint) {
            continue;
        }

        let Ok(health) = market_wrapper.borrow_position_health(&borrow_position) else {
            continue;
        };

        match &biggest_borrow {
            None => biggest_borrow = Some((position_key, *market_key, health.borrowed_atoms)),
            Some((_, _, prev_atoms)) if health.borrowed_atoms > *prev_atoms => {
                biggest_borrow = Some((position_key, *market_key, health.borrowed_atoms));
            }
            _ => {}
        }
        match &highest_ltv {
            None => highest_ltv = Some((position_key, *market_key, health.ltv)),
            Some((_, _, prev_ltv)) if health.ltv > *prev_ltv => {
                highest_ltv = Some((position_key, *market_key, health.ltv));
            }
            _ => {}
        }

        let unhealthy_ltv = market_wrapper.market().config().ltv_config().unhealthy_ltv;
        if health.ltv < unhealthy_ltv {
            continue;
        }

        // Size the liquidation using on-chain math; haircut collateral min for slippage.
        let (repay_atoms, min_collateral) =
            match market_wrapper.compute_liquidation_result_with_fee(&borrow_position, u64::MAX) {
                Ok((_health_after, liq)) => {
                    let total_coll = liq
                        .total_collateral_atoms_to_liquidate()
                        .unwrap_or(liq.collateral_atoms_to_liquidate);
                    (
                        liq.borrowed_atoms_to_repay,
                        min_collateral_after_slippage(total_coll, slippage_bps),
                    )
                }
                Err(e) => {
                    tracing::warn!(
                        position = %position_key,
                        error = %e,
                        "failed to size liquidation; falling back to full repay / min_collateral=0"
                    );
                    (health.borrowed_atoms, 0)
                }
            };

        stats.liquidatable += 1;
        tracing::info!(
            position = %position_key,
            authority = %borrow_position.authority(),
            market = %market_key,
            ltv = %health.ltv,
            unhealthy_ltv = %unhealthy_ltv,
            borrowed_atoms = health.borrowed_atoms,
            collateral_atoms = health.collateral_atoms,
            repay_atoms,
            min_collateral,
            slippage_bps,
            dry_run,
            "LIQUIDATABLE"
        );

        if dry_run {
            continue;
        }

        if gas < min_lamports {
            stats.skipped_insufficient_gas += 1;
            tracing::error!(
                gas,
                min_lamports,
                "SKIP insufficient gas — refill liquidator lamports"
            );
            continue;
        }

        let supply_ata = market_wrapper
            .market()
            .supply_token_info()
            .get_associated_token_address(&signer);
        let supply_bal = match token_amount(arch_client, supply_ata).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "failed to read supply ATA balance");
                stats.live_failure += 1;
                continue;
            }
        };
        if supply_bal < repay_atoms {
            stats.skipped_insufficient_supply += 1;
            tracing::warn!(
                position = %position_key,
                supply_bal,
                repay_atoms,
                %supply_mint,
                "SKIP insufficient supply inventory for repay"
            );
            continue;
        }

        let tx_to_sign = match tx_builder
            .liquidate(
                market_key,
                &position_key,
                Some(repay_atoms),
                Some(min_collateral),
                None,
            )
            .await
        {
            Ok(tx) => tx,
            Err(e) => {
                tracing::error!(position = %position_key, error = %e, "failed to build liquidate tx");
                stats.live_failure += 1;
                continue;
            }
        };

        let signed_tx = tx_to_sign.sign(&[*keypair], network);
        match tx_broadcast.broadcast_transaction(signed_tx).await {
            Ok(events) => {
                stats.live_success += 1;
                tracing::info!(
                    position = %position_key,
                    market = %market_key,
                    repay_atoms,
                    min_collateral,
                    ?events,
                    "Liquidation SUCCESS"
                );
            }
            Err(e) => {
                stats.live_failure += 1;
                tracing::error!(
                    position = %position_key,
                    market = %market_key,
                    error = %e,
                    "Liquidation FAILED"
                );
            }
        }
    }

    if stats.liquidatable > 0 {
        tracing::info!(
            liquidatable = stats.liquidatable,
            skipped_stale_oracle = stats.skipped_stale_oracle,
            skipped_insufficient_supply = stats.skipped_insufficient_supply,
            skipped_insufficient_gas = stats.skipped_insufficient_gas,
            live_success = stats.live_success,
            live_failure = stats.live_failure,
            "Scan summary"
        );
    } else {
        tracing::info!(
            skipped_stale_oracle = stats.skipped_stale_oracle,
            "No liquidatable positions found"
        );
    }

    if let Some((pos, market, atoms)) = biggest_borrow {
        tracing::info!(
            position = %pos,
            market = %market,
            borrowed_atoms = atoms,
            "STATS biggest_borrow"
        );
    }
    if let Some((pos, market, ltv)) = highest_ltv {
        tracing::info!(
            position = %pos,
            market = %market,
            %ltv,
            "STATS highest_ltv"
        );
    }

    stats
}
