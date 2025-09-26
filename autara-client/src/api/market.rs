use crate::api::serde_helper::*;
use arch_sdk::arch_program::pubkey::Pubkey;
use autara_lib::{
    interest_rate::lending_interest_rate::{
        LendingInterestRateCurveMut, MarketBorrowRateParameters,
    },
    math::{
        bps::{percent_to_bps, ONE_IN_BPS},
        ifixed_point::IFixedPoint,
    },
    state::{
        market::Market,
        market_wrapper::{MarketWrapper, OwnedMarket},
        supply_vault::SupplyVaultSummary,
    },
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullMarket {
    #[serde(with = "serde_pubkey")]
    pub market_id: Pubkey,
    pub market: MarketWrapper<OwnedMarket>,
    pub total_collateral_atoms: u64,
    pub supply_vault_summary: SupplyVaultSummary,
    pub interest_rate_curve_shape: InterestRateCurveShape,
}

impl FullMarket {
    pub fn new_from_market(market_id: Pubkey, market: MarketWrapper<OwnedMarket>) -> Self {
        let total_collateral_atoms = market.market().collateral_vault().total_collateral_atoms();
        let supply_vault_summary = market
            .market()
            .supply_vault()
            .get_summary()
            .unwrap_or_default();
        let interest_rate_curve_shape = InterestRateCurveShape::from_market(market.market());
        Self {
            market_id,
            market,
            total_collateral_atoms,
            supply_vault_summary,
            interest_rate_curve_shape,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterestRateCurveShape {
    pub points: Vec<InterestRateCurveShapePoint>,
}

impl InterestRateCurveShape {
    pub fn from_market(market: &Market) -> Self {
        let curve = market.supply_vault().interest_rate_curve();
        let points = (0..=percent_to_bps(100))
            .step_by(5)
            .filter_map(|utilization_in_bps| {
                let utilization =
                    IFixedPoint::from_i64_u64_ratio(utilization_in_bps as _, ONE_IN_BPS as _);
                curve
                    .clone()
                    .interest_rate_kind_mut()
                    .borrow_rate_per_second(MarketBorrowRateParameters {
                        utilisation_rate: &utilization,
                        elapsed_seconds_since_last_update: 0,
                    })
                    .ok()
                    .and_then(|rate| {
                        let apy_borrow_rate = rate.approximate_apy().ok()?;
                        let apy_lending_rate = rate
                            .adjust_for_utilisation_rate(utilization)
                            .ok()?
                            .approximate_apy()
                            .ok()?;
                        Some(InterestRateCurveShapePoint {
                            utilization: utilization.to_float(),
                            apy_borrow_rate,
                            apy_lending_rate,
                        })
                    })
            })
            .collect();
        Self { points }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterestRateCurveShapePoint {
    pub utilization: f64,
    pub apy_borrow_rate: f64,
    pub apy_lending_rate: f64,
}
