/// Safety & Attack Scenario Tests
///
/// This module contains adversarial test cases designed to break the protocol's
/// safety invariants, plus property-based tests (proptests) to verify those
/// invariants hold across wide input ranges.
///
/// Vulnerability categories tested:
/// 1. Fair value overflow with realistic token decimals (6/8)
/// 2. Vault inflation / donation attack (ERC-4626 style first depositor attack)
/// 3. Oracle confidence edge cases (confidence >= rate rejected at construction)
/// 4. Interest rate compounding overflow (unsync'd markets, extreme rates)
/// 5. Rounding exploitation (tiny borrows/repays rounding in attacker's favor)
/// 6. Liquidation edge cases (extreme LTV, boundary conditions)
/// 7. Multi-depositor fairness after interest accrual
#[cfg(test)]
mod fair_value_overflow {
    use crate::{
        error::LendingError,
        math::{ifixed_point::IFixedPoint, safe_math::SafeMath},
        oracle::oracle_price::OracleRate,
    };

    /// 6-decimal tokens (USDC) should handle even u64::MAX atoms safely
    #[test]
    fn no_overflow_6_decimal_token_max_amount() {
        let oracle = OracleRate::new(
            IFixedPoint::from_num(1.0),
            IFixedPoint::from_num(0.001),
        );
        let amount = u64::MAX;
        let decimals: u8 = 6;

        let result = oracle.borrow_value(amount, decimals);
        assert!(result.is_ok(), "6-decimal token should not overflow even at u64::MAX");

        let value = result.unwrap().to_float();
        let expected = (u64::MAX as f64) * 1.001 / 1_000_000.0;
        assert!(
            (value - expected).abs() / expected < 0.0001,
            "Value mismatch: got {}, expected {}",
            value,
            expected
        );
    }

    /// 8-decimal tokens (BTC) at extreme prices should be safe
    #[test]
    fn no_overflow_btc_at_million_dollar_price() {
        let oracle = OracleRate::new(
            IFixedPoint::from_num(1_000_000.0),
            IFixedPoint::from_num(1_000.0),
        );
        let amount: u64 = 100_000 * 10u64.pow(8);
        let decimals: u8 = 8;

        let result = oracle.borrow_value(amount, decimals);
        assert!(result.is_ok());

        let value = result.unwrap().to_float();
        let expected = 100_000.0 * 1_001_000.0;
        assert!(
            (value - expected).abs() / expected < 0.0001,
            "Expected ~${}, got ${}",
            expected,
            value
        );
    }

    /// Verify that multiplication overflow is actually caught by safe_mul
    #[test]
    fn safe_mul_catches_overflow() {
        let large = IFixedPoint::from_num(1e15_f64);
        let also_large = IFixedPoint::from_num(1e15_f64);
        let result = large.safe_mul(also_large);
        assert_eq!(
            result.unwrap_err(),
            LendingError::MultiplicationOverflow,
            "safe_mul must catch overflow"
        );
    }
}

#[cfg(test)]
mod vault_inflation_attack {
    use crate::math::{
        rounding::RoundingMode,
        shares_tracker::SharesTracker,
        ufixed_point::UFixedPoint,
    };

    /// Classic ERC-4626 vault inflation attack attempt:
    /// 1. Attacker deposits 1 atom → gets 1 share
    /// 2. Attacker donates X atoms → atoms_per_share becomes (X+1)
    /// 3. Victim deposits Y atoms where Y < (X+1)
    ///
    /// KEY FINDING: Unlike integer-shares vaults (ERC-4626), this protocol uses
    /// UFixedPoint (U64F64) shares with 64 fractional bits. So the victim gets
    /// FRACTIONAL shares (e.g., 0.999998) and can withdraw their deposit back.
    /// The classic inflation attack is largely mitigated by fractional shares!
    #[test]
    fn first_depositor_inflation_attack_mitigated_by_fractional_shares() {
        let mut tracker = SharesTracker::new();

        // Step 1: Attacker deposits 1 atom
        let attacker_shares = tracker.deposit_atoms(1).unwrap();
        assert_eq!(attacker_shares, UFixedPoint::from_u64(1));

        // Step 2: Attacker donates 1_000_000 atoms to inflate atoms_per_share
        tracker.donate_atoms(1_000_000).unwrap();
        assert_eq!(
            tracker.atoms_per_share(),
            UFixedPoint::from_u64(1_000_001)
        );

        // Step 3: Victim deposits 999_999 atoms (just under atoms_per_share)
        let victim_shares = tracker.deposit_atoms(999_999).unwrap();
        // Unlike ERC-4626, victim gets FRACTIONAL shares: 999_999/1_000_001 ≈ 0.999998
        assert!(
            !victim_shares.is_zero(),
            "U64F64 fractional shares protect against inflation attack: victim got {:?} shares",
            victim_shares
        );

        // Victim can withdraw and gets back almost all their deposit
        let victim_withdrawn = tracker
            .withdraw_shares(victim_shares, RoundingMode::RoundDown)
            .unwrap();
        // Loss should be minimal (at most 1 atom from rounding)
        let loss = 999_999u64.saturating_sub(victim_withdrawn);
        assert!(
            loss <= 1,
            "Victim lost {} atoms from inflation attack (mitigated by fractional shares)",
            loss
        );
    }

    /// Even with modest inflation, fractional shares protect the victim
    #[test]
    fn modest_inflation_mitigated_by_fractional_shares() {
        let mut tracker = SharesTracker::new();

        let _attacker_shares = tracker.deposit_atoms(1).unwrap();
        tracker.donate_atoms(100).unwrap();

        // Victim deposits 100 atoms → gets fractional shares (100/101 ≈ 0.9901)
        let victim_shares = tracker.deposit_atoms(100).unwrap();
        assert!(
            !victim_shares.is_zero(),
            "Fractional shares protect victim: got {:?} shares (not zero)",
            victim_shares
        );

        let victim_withdrawn = tracker
            .withdraw_shares(victim_shares, RoundingMode::RoundDown)
            .unwrap();
        let loss = 100u64.saturating_sub(victim_withdrawn);
        assert!(
            loss <= 1,
            "Victim loss bounded to {} atom(s) thanks to fractional shares",
            loss
        );
    }

    /// Show that larger initial deposits protect against the attack
    #[test]
    fn large_initial_deposit_mitigates_inflation() {
        let mut tracker = SharesTracker::new();

        // Protocol seeds the vault with 1000 atoms
        let _seed_shares = tracker.deposit_atoms(1000).unwrap();

        // Attacker tries to donate 1_000_000 atoms
        tracker.donate_atoms(1_000_000).unwrap();
        // atoms_per_share = (1000 + 1_000_000) / 1000 = 1001

        // Victim deposits 1_000_000 atoms
        let victim_shares = tracker.deposit_atoms(1_000_000).unwrap();
        assert!(
            !victim_shares.is_zero(),
            "With 1000 initial shares, the inflation attack is mitigated"
        );

        // Victim can withdraw reasonable amount
        let victim_withdrawn = tracker
            .withdraw_shares(victim_shares, RoundingMode::RoundDown)
            .unwrap();
        let loss = 1_000_000u64.saturating_sub(victim_withdrawn);
        assert!(
            loss <= 1001, // Loss bounded by atoms_per_share
            "Victim loss should be bounded by atoms_per_share. Lost {} atoms",
            loss
        );
    }

    /// The supply-side uses the same SharesTracker with fractional U64F64 shares.
    /// Verify that the inflation attack is mitigated there too.
    #[test]
    fn supply_side_inflation_mitigated() {
        let mut supply_tracker = SharesTracker::new();

        let attacker_shares = supply_tracker.deposit_atoms(1).unwrap();
        supply_tracker.donate_atoms(1_000_000).unwrap();

        let victim_shares = supply_tracker.deposit_atoms(999_999).unwrap();
        // Victim gets fractional shares — NOT zero
        assert!(
            !victim_shares.is_zero(),
            "Supply-side: fractional shares protect victim. Got {:?} shares",
            victim_shares
        );

        let victim_withdrawn = supply_tracker
            .withdraw_shares(victim_shares, RoundingMode::RoundDown)
            .unwrap();
        let loss = 999_999u64.saturating_sub(victim_withdrawn);
        assert!(
            loss <= 1,
            "Victim loss is minimal ({} atoms) thanks to U64F64 fractional shares",
            loss
        );

        // Attacker's profit from the attack is negligible
        let attacker_withdrawn = supply_tracker
            .withdraw_shares(attacker_shares, RoundingMode::RoundDown)
            .unwrap();
        // Attacker should only get back their deposit + donation (1 + 1M = 1_000_001)
        // NOT the victim's funds
        assert!(
            attacker_withdrawn <= 1_000_002,
            "Attacker should not profit significantly. Got {}",
            attacker_withdrawn
        );
    }

    /// Even at extreme atoms_per_share (2^62), fractional shares must still
    /// protect the victim — if victim gets zero shares, the attack succeeded.
    #[test]
    fn inflation_attack_at_u64f64_precision_boundary() {
        let mut tracker = SharesTracker::new();

        tracker.deposit_atoms(1).unwrap();

        let huge_donation: u64 = 1u64 << 62; // 2^62 = ~4.6×10^18
        tracker.donate_atoms(huge_donation).unwrap();

        // Victim deposits 1 atom at very high atoms_per_share
        let victim_shares = tracker.deposit_atoms(1).unwrap();
        assert!(
            !victim_shares.is_zero(),
            "Inflation attack succeeded: victim got zero shares at atoms_per_share=2^62. \
             Fractional precision exhausted — attacker can steal victim deposits."
        );

        let victim_withdrawn = tracker
            .withdraw_shares(victim_shares, RoundingMode::RoundDown)
            .unwrap();
        assert!(
            victim_withdrawn <= 1,
            "Victim should get at most their 1 atom back"
        );
    }
}

#[cfg(test)]
mod oracle_confidence_edge_cases {
    use crate::{
        error::LendingError,
        math::{ifixed_point::IFixedPoint, safe_math::SafeMath},
        oracle::oracle_price::OracleRate,
    };

    /// confidence == rate is now rejected at construction
    #[test]
    fn confidence_equals_rate_rejected() {
        let result = OracleRate::try_new(
            IFixedPoint::from_num(100.0),
            IFixedPoint::from_num(100.0),
        );
        assert_eq!(
            result.unwrap_err(),
            LendingError::OracleConfidenceExceedsRate,
        );
    }

    /// confidence > rate is now rejected at construction
    #[test]
    fn confidence_exceeds_rate_rejected() {
        let result = OracleRate::try_new(
            IFixedPoint::from_num(100.0),
            IFixedPoint::from_num(150.0),
        );
        assert_eq!(
            result.unwrap_err(),
            LendingError::OracleConfidenceExceedsRate,
        );
    }

    /// Very wide confidence relative to price makes collateral appear very cheap
    /// and borrows very expensive — massive LTV impact
    #[test]
    fn wide_confidence_extreme_ltv_impact() {
        let oracle = OracleRate::new(
            IFixedPoint::from_num(100_000.0),
            IFixedPoint::from_num(10_000.0),
        );

        let amount: u64 = 100_000_000; // 1 BTC (8 decimals)
        let decimals: u8 = 8;

        let collateral = oracle.collateral_value(amount, decimals).unwrap();
        let borrow = oracle.borrow_value(amount, decimals).unwrap();

        let spread = borrow.safe_sub(collateral).unwrap().to_float() / collateral.to_float();
        assert!(
            (spread - 0.2222).abs() < 0.01,
            "With 10% confidence, there's a ~22% gap between borrow and collateral value. \
             Spread: {:.4}",
            spread
        );
    }

    /// Near-max price must still compute upper_bound without panic
    #[test]
    fn extreme_price_upper_bound_returns_error_not_panic() {
        let extreme_price = IFixedPoint::from_num(5e23_f64);
        let small_conf = IFixedPoint::from_num(1.0);
        let oracle = OracleRate::new(extreme_price, small_conf);

        // Must not panic — overflow must be caught by safe_add and returned as Err
        let result = oracle.upper_bound_rate();
        assert!(
            result.is_ok(),
            "upper_bound at 5e23 + 1 should not overflow: {:?}",
            result.err()
        );
    }

    /// Zero rate is rejected
    #[test]
    fn zero_rate_rejected() {
        let result = OracleRate::try_new(
            IFixedPoint::from_num(0.0),
            IFixedPoint::from_num(0.0),
        );
        assert_eq!(result.unwrap_err(), LendingError::OracleRateIsNull);
    }

    /// Negative rate is rejected
    #[test]
    fn negative_rate_rejected() {
        let result = OracleRate::try_new(
            IFixedPoint::from_num(-100.0),
            IFixedPoint::from_num(1.0),
        );
        assert_eq!(result.unwrap_err(), LendingError::OracleRateIsNull);
    }
}

#[cfg(test)]
mod interest_rate_overflow {
    use crate::{
        constant::SECONDS_PER_YEAR,
        error::LendingError,
        interest_rate::interest_rate_per_second::InterestRatePerSecond,
        math::ifixed_point::IFixedPoint,
    };

    /// Very high APY (1000%) compounded over 1 year should work
    #[test]
    fn extreme_apy_1000_percent_one_year() {
        let rate = InterestRatePerSecond::approximate_from_apy(10.0);
        let result = rate.coumpounding_interest_rate_during_elapsed_seconds(SECONDS_PER_YEAR);
        assert!(
            result.is_ok(),
            "1000% APY over 1 year should not overflow. Error: {:?}",
            result.err()
        );
        let interest = result.unwrap();
        let apy = interest.rate().to_float();
        assert!(
            (apy - 10.0).abs() < 0.01,
            "1000% APY should yield ~10x interest, got {}",
            apy
        );
    }

    /// exp() overflow: rate_per_second × elapsed must stay below ~55.26
    /// 1000% APY over 24 years must overflow (rate×time exceeds exp domain max ~55.26)
    #[test]
    fn exp_overflow_at_extreme_duration() {
        let rate = InterestRatePerSecond::approximate_from_apy(10.0);
        let twenty_four_years = 24 * SECONDS_PER_YEAR;
        let result = rate.coumpounding_interest_rate_during_elapsed_seconds(twenty_four_years);
        assert!(
            result.is_err(),
            "1000%% APY over 24 years should overflow exp(). \
             If it doesn't, the market could brick with extreme accumulated interest."
        );
        assert_eq!(result.unwrap_err(), LendingError::InvalidExpArg);
    }

    /// At 100% APY over 80 years, the rate × time ≈ 55.4 → overflow
    #[test]
    fn moderate_rate_extremely_long_unsync() {
        let rate = InterestRatePerSecond::approximate_from_apy(1.0);
        let eighty_years = 80 * SECONDS_PER_YEAR;
        let result = rate.coumpounding_interest_rate_during_elapsed_seconds(eighty_years);
        assert!(
            result.is_err(),
            "100% APY compounded over 80 years without sync should overflow exp(). \
             The market would be permanently bricked if left unsynced this long."
        );
    }

    /// Even a modest 5% APY is safe for centuries
    #[test]
    fn low_rate_safe_for_centuries() {
        let rate = InterestRatePerSecond::approximate_from_apy(0.05);
        let hundred_years = 100 * SECONDS_PER_YEAR;
        let result = rate.coumpounding_interest_rate_during_elapsed_seconds(hundred_years);
        assert!(result.is_ok(), "5% APY over 100 years should be safe");
    }

    /// Negative interest rate compounding
    #[test]
    fn negative_rate_compounding() {
        let neg_rate = InterestRatePerSecond::new(IFixedPoint::from_num(-0.0000001_f64));
        let one_year = SECONDS_PER_YEAR;
        let result = neg_rate.coumpounding_interest_rate_during_elapsed_seconds(one_year);
        assert!(result.is_ok(), "Negative rate compounding should not overflow");
        let rate = result.unwrap().rate();
        assert!(
            rate.is_negative(),
            "Negative rate should produce negative interest"
        );
    }
}

#[cfg(test)]
mod rounding_exploitation {
    use crate::math::{
        ifixed_point::IFixedPoint,
        rounding::RoundingMode,
        shares_tracker::SharesTracker,
        ufixed_point::UFixedPoint,
    };
    use crate::interest_rate::interest_rate::InterestRate;

    /// Attack: borrow 1 atom repeatedly to accumulate rounding errors.
    /// Each 1-atom borrow gets rounded shares. After interest,
    /// shares_to_atoms rounds UP for repayment → hurts borrower, not protocol.
    #[test]
    fn tiny_borrow_rounding_hurts_borrower() {
        let mut borrow_tracker = SharesTracker::new();

        borrow_tracker.deposit_atoms(1_000_000).unwrap();
        let rate = InterestRate::new(IFixedPoint::lit("0.5"));
        borrow_tracker.apply_interest_rate(rate).unwrap();

        let aps = borrow_tracker.atoms_per_share();
        assert!(aps > UFixedPoint::from_u64(1));

        for _ in 0..100 {
            let shares = borrow_tracker.deposit_atoms(1).unwrap();

            let debt_atoms = borrow_tracker
                .shares_to_atoms(shares, RoundingMode::RoundUp)
                .unwrap();
            assert!(
                debt_atoms >= 1,
                "Protocol should never round debt DOWN. Debt atoms: {}",
                debt_atoms
            );
        }
    }

    /// Attack: repeatedly deposit and withdraw tiny amounts to exploit rounding.
    /// Protocol should never lose atoms from this.
    #[test]
    fn tiny_deposit_withdraw_rounding_never_profits() {
        let mut tracker = SharesTracker::new();

        tracker.deposit_atoms(1_000_000).unwrap();
        let rate = InterestRate::new(IFixedPoint::lit("0.3333"));
        tracker.apply_interest_rate(rate).unwrap();

        let initial_total = tracker.total_atoms(RoundingMode::RoundDown).unwrap();

        for _ in 0..100 {
            let shares = tracker.deposit_atoms(1).unwrap();
            if !shares.is_zero() {
                let withdrawn = tracker
                    .withdraw_shares(shares, RoundingMode::RoundDown)
                    .unwrap();
                assert!(withdrawn <= 1, "Withdrew {} atoms for 1 atom deposit!", withdrawn);
            }
        }

        let final_total = tracker.total_atoms(RoundingMode::RoundDown).unwrap();
        assert!(
            final_total >= initial_total,
            "Protocol lost atoms from rounding: before={}, after={}",
            initial_total,
            final_total
        );
    }

    /// Verify that repay rounding favors the protocol (round up)
    #[test]
    fn repay_rounding_favors_protocol() {
        let mut borrow_tracker = SharesTracker::new();

        borrow_tracker.deposit_atoms(3).unwrap();
        let rate = InterestRate::new(IFixedPoint::lit("0.5"));
        borrow_tracker.apply_interest_rate(rate).unwrap();

        let one_share = UFixedPoint::from_u64(1);
        let atoms_round_up = borrow_tracker
            .shares_to_atoms(one_share, RoundingMode::RoundUp)
            .unwrap();
        let atoms_round_down = borrow_tracker
            .shares_to_atoms(one_share, RoundingMode::RoundDown)
            .unwrap();

        assert!(atoms_round_up >= atoms_round_down);
        assert_eq!(atoms_round_up, 2);
        assert_eq!(atoms_round_down, 1);
    }
}

#[cfg(test)]
mod liquidation_edge_cases {
    use crate::{
        math::ifixed_point::IFixedPoint,
        operation::liquidation::compute_liquidation_with_fee,
        oracle::oracle_price::OracleRate,
        state::{
            collateral_vault::tests::{BTC, BTC_DECIMALS},
            supply_vault::tests::{USDC, USDC_DECIMALS},
        },
    };

    /// Liquidation with zero confidence (exact prices)
    #[test]
    fn liquidation_zero_confidence() {
        let supply_oracle = OracleRate::new(
            IFixedPoint::from_num(1.0),
            IFixedPoint::from_num(0.0),
        );
        let collateral_oracle = OracleRate::new(
            IFixedPoint::from_num(100_000.0),
            IFixedPoint::from_num(0.0),
        );

        let borrow_value = supply_oracle.borrow_value(USDC(1000.), USDC_DECIMALS as u8).unwrap();
        let collateral_value = supply_oracle.collateral_value(USDC(1000.), USDC_DECIMALS as u8).unwrap();
        assert_eq!(
            borrow_value, collateral_value,
            "With zero confidence, borrow and collateral values should match"
        );

        let result = compute_liquidation_with_fee(
            USDC(80_000.),
            USDC_DECIMALS as u8,
            &supply_oracle,
            BTC(1.),
            BTC_DECIMALS as u8,
            &collateral_oracle,
            IFixedPoint::lit("0.5"),
            IFixedPoint::lit("0.05"),
            u64::MAX,
        );
        assert!(
            result.is_ok(),
            "Liquidation should work with zero confidence oracles"
        );
    }

    /// Liquidation with very small amounts (dust positions) must not panic or
    /// produce nonsensical results
    #[test]
    fn liquidation_dust_position() {
        let supply_oracle = OracleRate::new(
            IFixedPoint::from_num(1.0),
            IFixedPoint::from_num(0.001),
        );
        let collateral_oracle = OracleRate::new(
            IFixedPoint::from_num(100_000.0),
            IFixedPoint::from_num(100.0),
        );

        let result = compute_liquidation_with_fee(
            1,
            USDC_DECIMALS as u8,
            &supply_oracle,
            1,
            BTC_DECIMALS as u8,
            &collateral_oracle,
            IFixedPoint::lit("0.5"),
            IFixedPoint::lit("0.05"),
            u64::MAX,
        );

        assert!(
            result.is_ok(),
            "Liquidation of dust position should not fail: {:?}",
            result.err()
        );
        let liq = result.unwrap();
        assert!(
            liq.borrowed_atoms_to_repay <= 1,
            "Cannot repay more than borrowed"
        );
    }

    /// Liquidation where position is exactly at LTV=1 (underwater boundary)
    #[test]
    fn liquidation_exactly_at_ltv_one() {
        let supply_oracle = OracleRate::new(
            IFixedPoint::from_num(1.0),
            IFixedPoint::from_num(0.0),
        );
        let collateral_oracle = OracleRate::new(
            IFixedPoint::from_num(100_000.0),
            IFixedPoint::from_num(0.0),
        );

        // Borrow exactly equals collateral value: LTV = 1.0
        // collateral: 1 BTC = $100,000
        // borrow: $100,000 USDC
        let result = compute_liquidation_with_fee(
            USDC(100_000.),
            USDC_DECIMALS as u8,
            &supply_oracle,
            BTC(1.),
            BTC_DECIMALS as u8,
            &collateral_oracle,
            IFixedPoint::lit("0.5"),
            IFixedPoint::lit("0.05"),
            u64::MAX,
        );

        // This should trigger the value_to_liquidate computation
        // At LTV=1.0 with target 0.5: should liquidate significant portion
        assert!(result.is_ok(), "Liquidation at LTV=1 should work");
        let liq = result.unwrap();
        assert!(liq.borrowed_atoms_to_repay > 0);
        assert!(liq.total_collateral_atoms_to_liquidate().unwrap() <= BTC(1.));
    }

    /// Liquidation with maximum fee (10%) and high LTV
    #[test]
    fn liquidation_max_fee_high_ltv() {
        let supply_oracle = OracleRate::new(
            IFixedPoint::from_num(1.0),
            IFixedPoint::from_num(0.0),
        );
        let collateral_oracle = OracleRate::new(
            IFixedPoint::from_num(100_000.0),
            IFixedPoint::from_num(0.0),
        );

        // LTV ≈ 95%: borrow $95K against $100K collateral
        let result = compute_liquidation_with_fee(
            USDC(95_000.),
            USDC_DECIMALS as u8,
            &supply_oracle,
            BTC(1.),
            BTC_DECIMALS as u8,
            &collateral_oracle,
            IFixedPoint::lit("0.81"), // target LTV after liquidation
            IFixedPoint::lit("0.1"),  // 10% bonus
            u64::MAX,
        );

        assert!(result.is_ok());
        let liq = result.unwrap();
        // Total collateral taken should not exceed available
        assert!(liq.total_collateral_atoms_to_liquidate().unwrap() <= BTC(1.));
        assert!(liq.borrowed_atoms_to_repay <= USDC(95_000.));
    }
}

#[cfg(test)]
mod multi_depositor_fairness {
    use crate::{
        interest_rate::interest_rate::InterestRate,
        math::{
            ifixed_point::IFixedPoint,
            rounding::RoundingMode,
            shares_tracker::SharesTracker,
        },
    };

    /// After interest accrual, early depositors should gain proportionally
    /// the same as late depositors (from the point they entered)
    #[test]
    fn fairness_after_interest_accrual() {
        let mut tracker = SharesTracker::new();

        let alice_shares = tracker.deposit_atoms(1_000_000).unwrap();

        let rate = InterestRate::new(IFixedPoint::lit("0.5"));
        tracker.apply_interest_rate(rate).unwrap();

        let bob_shares = tracker.deposit_atoms(1_000_000).unwrap();

        assert!(alice_shares > bob_shares, "Alice entered earlier, should have more shares");

        tracker.apply_interest_rate(rate).unwrap();

        let alice_atoms = tracker
            .shares_to_atoms(alice_shares, RoundingMode::RoundDown)
            .unwrap();
        let bob_atoms = tracker
            .shares_to_atoms(bob_shares, RoundingMode::RoundDown)
            .unwrap();

        // Alice: 1M → 1.5M → 2.25M
        // Bob: 1M → 1.5M
        assert!(
            (alice_atoms as f64 - 2_250_000.0).abs() < 2.0,
            "Alice should have ~2.25M, got {}",
            alice_atoms
        );
        assert!(
            (bob_atoms as f64 - 1_500_000.0).abs() < 2.0,
            "Bob should have ~1.5M, got {}",
            bob_atoms
        );
    }

    /// After loss socialization, all depositors lose proportionally
    #[test]
    fn loss_socialization_is_fair() {
        let mut tracker = SharesTracker::new();

        let alice_shares = tracker.deposit_atoms(3_000_000).unwrap();
        let bob_shares = tracker.deposit_atoms(1_000_000).unwrap();

        tracker.socialize_loss_atoms(400_000).unwrap();

        let alice_atoms = tracker
            .shares_to_atoms(alice_shares, RoundingMode::RoundDown)
            .unwrap();
        let bob_atoms = tracker
            .shares_to_atoms(bob_shares, RoundingMode::RoundDown)
            .unwrap();

        assert!(
            (alice_atoms as f64 - 2_700_000.0).abs() < 2.0,
            "Alice should have ~2.7M, got {}",
            alice_atoms
        );
        assert!(
            (bob_atoms as f64 - 900_000.0).abs() < 2.0,
            "Bob should have ~900K, got {}",
            bob_atoms
        );
    }
}

#[cfg(test)]
mod shares_tracker_extreme {
    use crate::{
        interest_rate::interest_rate::InterestRate,
        math::{
            ifixed_point::IFixedPoint,
            rounding::RoundingMode,
            shares_tracker::SharesTracker,
        },
    };

    /// Extremely high atoms_per_share after repeated interest compounding
    /// eventually causes overflow in shares_to_atoms
    #[test]
    fn atoms_per_share_overflow_from_compounding() {
        let mut tracker = SharesTracker::new();
        tracker.deposit_atoms(1_000_000).unwrap();

        // Apply 100% interest 50 times: atoms_per_share ≈ 2^50 ≈ 10^15
        let rate = InterestRate::new(IFixedPoint::lit("1.0"));
        let mut overflow_at = None;
        for i in 0..70 {
            let result = tracker.apply_interest_rate(rate);
            if result.is_err() {
                overflow_at = Some(i);
                break;
            }
        }

        // UFixedPoint max integer = 2^64 ≈ 1.8×10^19
        // After ~63 doublings, atoms_per_share ≈ 2^63 → overflows
        assert!(
            overflow_at.is_some(),
            "Expected atoms_per_share overflow after repeated 100% interest, \
             but it survived all 70 rounds. Current aps: {:?}",
            tracker.atoms_per_share()
        );
    }

    /// Many depositors withdrawing in different orders should never
    /// leave the tracker in a negative state
    #[test]
    fn multi_depositor_withdrawal_ordering() {
        let mut tracker = SharesTracker::new();

        // 10 depositors, each with different amounts
        let deposits: Vec<u64> = vec![100, 1_000, 10_000, 50_000, 100_000, 500, 7_777, 33_333, 1, 999_999];
        let mut shares_list = Vec::new();
        for &d in &deposits {
            shares_list.push(tracker.deposit_atoms(d).unwrap());
        }

        // Apply some interest
        let rate = InterestRate::new(IFixedPoint::lit("0.25"));
        tracker.apply_interest_rate(rate).unwrap();

        // Withdraw in reverse order
        for shares in shares_list.into_iter().rev() {
            let result = tracker.withdraw_shares(shares, RoundingMode::RoundDown);
            assert!(result.is_ok(), "Withdrawal should succeed");
        }

        // Remaining atoms should be >= 0 (guaranteed by u64) and very small (rounding dust)
        let remaining = tracker.total_atoms(RoundingMode::RoundDown).unwrap();
        assert!(
            remaining <= deposits.len() as u64,
            "Remaining dust should be minimal: {}",
            remaining
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  PROPERTY-BASED TESTS (PROPTESTS)
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod prop_tests_safety {
    use proptest::prelude::*;

    use crate::{
        constant::SECONDS_PER_YEAR,
        interest_rate::interest_rate::InterestRate,
        interest_rate::interest_rate_per_second::InterestRatePerSecond,
        math::{
            ifixed_point::IFixedPoint,
            rounding::RoundingMode,
            safe_math::SafeMath,
            shares_tracker::SharesTracker,
        },
        oracle::oracle_price::OracleRate,
        state::{
            collateral_vault::tests::BTC,
            supply_vault::tests::USDC,
        },
    };

    proptest! {
        /// INVARIANT: No depositor can withdraw more atoms than their proportional
        /// fair share (deposit × (1 + rate)).
        #[test]
        fn no_depositor_profits_from_rounding(
            deposit_a in 1u64..1_000_000_000u64,
            deposit_b in 1u64..1_000_000_000u64,
            rate_bps in 1u64..5000u64,
        ) {
            let mut tracker = SharesTracker::new();
            let shares_a = tracker.deposit_atoms(deposit_a).unwrap();
            let shares_b = tracker.deposit_atoms(deposit_b).unwrap();

            let rate = InterestRate::new(IFixedPoint::from_ratio(rate_bps, 10_000).unwrap());
            tracker.apply_interest_rate(rate).unwrap();

            let withdrawn_a = tracker
                .withdraw_shares(shares_a, RoundingMode::RoundDown)
                .unwrap();
            let withdrawn_b = tracker
                .withdraw_shares(shares_b, RoundingMode::RoundDown)
                .unwrap();

            let expected_a = (deposit_a as f64) * (1.0 + rate_bps as f64 / 10_000.0);
            let expected_b = (deposit_b as f64) * (1.0 + rate_bps as f64 / 10_000.0);
            prop_assert!(withdrawn_a as f64 <= expected_a + 1.0);
            prop_assert!(withdrawn_b as f64 <= expected_b + 1.0);
        }

        /// INVARIANT: atoms_per_share never decreases from positive interest
        #[test]
        fn atoms_per_share_monotonically_increases(
            initial in 1_000u64..1_000_000_000u64,
            rate_bps in 1u64..10_000u64,
            rounds in 1usize..20usize,
        ) {
            let mut tracker = SharesTracker::new();
            tracker.deposit_atoms(initial).unwrap();
            let rate = InterestRate::new(IFixedPoint::from_ratio(rate_bps, 10_000).unwrap());

            let mut prev_aps = tracker.atoms_per_share();
            for _ in 0..rounds {
                if tracker.apply_interest_rate(rate).is_err() {
                    break; // overflow is fine, just stop
                }
                let new_aps = tracker.atoms_per_share();
                prop_assert!(new_aps >= prev_aps, "atoms_per_share decreased!");
                prev_aps = new_aps;
            }
        }

        /// INVARIANT: For any valid oracle price, borrow_value >= collateral_value
        /// for the same amount (because borrow uses upper bound, collateral uses lower)
        #[test]
        fn borrow_value_gte_collateral_value(
            price_f in 0.01f64..1_000_000.0,
            conf_pct in 0.0f64..50.0,
            amount in 1u64..1_000_000_000u64,
            decimals in 0u8..15u8,
        ) {
            let price = IFixedPoint::from_num(price_f);
            let confidence = IFixedPoint::from_num(price_f * conf_pct / 100.0);
            let oracle = match OracleRate::try_new(price, confidence) {
                Ok(o) => o,
                Err(_) => return Ok(()), // skip invalid oracle params
            };

            if let (Ok(borrow_val), Ok(coll_val)) = (
                oracle.borrow_value(amount, decimals),
                oracle.collateral_value(amount, decimals),
            ) {
                prop_assert!(
                    borrow_val >= coll_val,
                    "borrow_value ({:?}) should always >= collateral_value ({:?})",
                    borrow_val,
                    coll_val
                );
            }
        }

        /// INVARIANT: Interest compounding should be monotonic in elapsed time
        #[test]
        fn interest_monotonic_in_time(
            apy_pct in 1u64..200u64,
            t1 in 1u64..SECONDS_PER_YEAR,
            delta in 1u64..SECONDS_PER_YEAR,
        ) {
            let rate = InterestRatePerSecond::approximate_from_apy(apy_pct as f64 / 100.0);
            let t2 = t1.saturating_add(delta);
            if let (Ok(r1), Ok(r2)) = (
                rate.coumpounding_interest_rate_during_elapsed_seconds(t1),
                rate.coumpounding_interest_rate_during_elapsed_seconds(t2),
            ) {
                prop_assert!(
                    r2.rate() >= r1.rate(),
                    "More time should mean more interest: r1={:?}, r2={:?}",
                    r1.rate(),
                    r2.rate()
                );
            }
        }

        /// INVARIANT: Donation attack bounded — victim loss is at most atoms_per_share
        #[test]
        fn donation_attack_loss_bounded(
            seed_deposit in 1u64..1_000u64,
            donation in 1u64..10_000_000u64,
            victim_deposit in 1u64..10_000_000u64,
        ) {
            let mut tracker = SharesTracker::new();
            let _attacker_shares = tracker.deposit_atoms(seed_deposit).unwrap();
            tracker.donate_atoms(donation).unwrap();

            let victim_shares = tracker.deposit_atoms(victim_deposit).unwrap();
            if victim_shares.is_zero() {
                let aps = tracker.atoms_per_share().as_u64_rounded_down().unwrap();
                prop_assert!(
                    victim_deposit < aps,
                    "Victim deposited {} but atoms_per_share is only {}.",
                    victim_deposit,
                    aps
                );
            } else {
                let withdrawn = tracker
                    .withdraw_shares(victim_shares, RoundingMode::RoundDown)
                    .unwrap();
                let aps = tracker.atoms_per_share().as_u64_rounded_down().unwrap();
                let max_loss = aps;
                if victim_deposit > max_loss {
                    prop_assert!(
                        withdrawn >= victim_deposit - max_loss,
                        "Victim lost more than atoms_per_share: deposited={}, withdrawn={}, aps={}",
                        victim_deposit,
                        withdrawn,
                        aps
                    );
                }
            }
        }

        /// INVARIANT: For realistic oracle prices and token amounts,
        /// collateral_value and borrow_value should not overflow
        #[test]
        fn realistic_oracle_no_overflow(
            price_usd in 0.001f64..1_000_000.0,
            conf_pct in 0.01f64..5.0,
            amount_tokens in 0.001f64..1_000_000.0,
            decimals in 6u8..9u8,
        ) {
            let price = IFixedPoint::from_num(price_usd);
            let confidence = IFixedPoint::from_num(price_usd * conf_pct / 100.0);
            let oracle = match OracleRate::try_new(price, confidence) {
                Ok(o) => o,
                Err(_) => return Ok(()),
            };
            let amount = (amount_tokens * 10f64.powi(decimals as i32)) as u64;

            let borrow_result = oracle.borrow_value(amount, decimals);
            let collateral_result = oracle.collateral_value(amount, decimals);

            prop_assert!(
                borrow_result.is_ok(),
                "borrow_value overflow for price={}, amount={}, decimals={}",
                price_usd, amount, decimals
            );
            prop_assert!(
                collateral_result.is_ok(),
                "collateral_value overflow for price={}, amount={}, decimals={}",
                price_usd, amount, decimals
            );
        }

        /// INVARIANT: Liquidation always reduces LTV (or fully liquidates)
        #[test]
        fn liquidation_never_increases_ltv(
            borrowed_usdc in 60_000u64..99_000u64,
            fee_thousandths in 5u64..100u64,
        ) {
            let supply_oracle = OracleRate::new(
                IFixedPoint::from_num(1.0),
                IFixedPoint::from_num(0.0),
            );
            let collateral_oracle = OracleRate::new(
                IFixedPoint::from_num(100_000.0),
                IFixedPoint::from_num(0.0),
            );

            let borrowed_atoms = USDC(borrowed_usdc as f64);
            let collateral_atoms = BTC(1.);
            let fee = IFixedPoint::from_ratio(fee_thousandths, 1000).unwrap();

            // Only test valid fee range
            if fee > IFixedPoint::lit("0.1") || fee < IFixedPoint::lit("0.001") {
                return Ok(());
            }

            let liq = crate::operation::liquidation::compute_liquidation_with_fee(
                borrowed_atoms,
                6,
                &supply_oracle,
                collateral_atoms,
                8,
                &collateral_oracle,
                IFixedPoint::lit("0.5"),
                fee,
                u64::MAX,
            ).expect("Liquidation should not fail for valid inputs");

            prop_assert!(
                liq.total_collateral_atoms_to_liquidate().unwrap() <= collateral_atoms,
                "Total liquidated collateral exceeds available"
            );
            prop_assert!(
                liq.borrowed_atoms_to_repay <= borrowed_atoms,
                "Repaid more than borrowed"
            );

            if liq.borrowed_atoms_to_repay < borrowed_atoms {
                let remaining_borrow = borrowed_atoms - liq.borrowed_atoms_to_repay;
                let remaining_collateral = collateral_atoms
                    - liq.total_collateral_atoms_to_liquidate().unwrap();
                if remaining_collateral > 0 {
                    let ltv_before = supply_oracle.borrow_value(borrowed_atoms, 6).unwrap()
                        .safe_div(collateral_oracle.collateral_value(collateral_atoms, 8).unwrap())
                        .unwrap();
                    let ltv_after = supply_oracle.borrow_value(remaining_borrow, 6).unwrap()
                        .safe_div(collateral_oracle.collateral_value(remaining_collateral, 8).unwrap())
                        .unwrap();
                    prop_assert!(
                        ltv_after <= ltv_before,
                        "LTV increased after liquidation: before={:?}, after={:?}",
                        ltv_before, ltv_after
                    );
                }
            }
        }

        /// INVARIANT: Multiple depositors withdrawing after interest should
        /// never extract more total atoms than deposited + earned interest
        #[test]
        fn total_withdrawn_bounded_by_deposits_plus_interest(
            deposits in prop::collection::vec(1u64..1_000_000u64, 2..10),
            rate_bps in 1u64..5000u64,
        ) {
            let mut tracker = SharesTracker::new();
            let mut all_shares = Vec::new();
            let total_deposited: u64 = deposits.iter().sum();

            for &d in &deposits {
                all_shares.push(tracker.deposit_atoms(d).unwrap());
            }

            let rate = InterestRate::new(IFixedPoint::from_ratio(rate_bps, 10_000).unwrap());
            tracker.apply_interest_rate(rate).unwrap();

            let mut total_withdrawn = 0u64;
            for shares in all_shares {
                total_withdrawn += tracker
                    .withdraw_shares(shares, RoundingMode::RoundDown)
                    .unwrap();
            }

            let max_expected = (total_deposited as f64) * (1.0 + rate_bps as f64 / 10_000.0);
            prop_assert!(
                (total_withdrawn as f64) <= max_expected + 1.0,
                "Withdrew {} but max should be {}",
                total_withdrawn,
                max_expected
            );

            let remaining = tracker.total_atoms(RoundingMode::RoundDown).unwrap();
            prop_assert!(remaining <= deposits.len() as u64 + 1);
        }

        /// INVARIANT: After socialization, atoms_per_share strictly decreases
        #[test]
        fn socialization_decreases_atoms_per_share(
            deposit in 10_000u64..1_000_000_000u64,
            loss_pct in 1u64..90u64,
        ) {
            let mut tracker = SharesTracker::new();
            tracker.deposit_atoms(deposit).unwrap();
            let aps_before = tracker.atoms_per_share();

            let loss = deposit * loss_pct / 100;
            if loss > 0 {
                tracker.socialize_loss_atoms(loss).unwrap();
                let aps_after = tracker.atoms_per_share();
                prop_assert!(
                    aps_after < aps_before,
                    "atoms_per_share should decrease after loss: before={:?}, after={:?}",
                    aps_before, aps_after
                );
            }
        }

        /// INVARIANT: Multiple depositors withdrawing in any order should never
        /// leave the tracker with more than depositor-count atoms of rounding dust
        #[test]
        fn multi_depositor_withdrawal_ordering_no_negative(
            deposits in prop::collection::vec(1u64..1_000_000u64, 2..10),
            rate_bps in 0u64..5000u64,
            reverse in proptest::bool::ANY,
        ) {
            let mut tracker = SharesTracker::new();
            let mut shares_list = Vec::new();
            for &d in &deposits {
                shares_list.push(tracker.deposit_atoms(d).unwrap());
            }

            if rate_bps > 0 {
                let rate = InterestRate::new(IFixedPoint::from_ratio(rate_bps, 10_000).unwrap());
                if tracker.apply_interest_rate(rate).is_err() {
                    return Ok(());
                }
            }

            let iter: Box<dyn Iterator<Item = _>> = if reverse {
                Box::new(shares_list.into_iter().rev())
            } else {
                Box::new(shares_list.into_iter())
            };

            for shares in iter {
                let result = tracker.withdraw_shares(shares, RoundingMode::RoundDown);
                prop_assert!(result.is_ok(), "Withdrawal should succeed");
            }

            let remaining = tracker.total_atoms(RoundingMode::RoundDown).unwrap();
            prop_assert!(
                remaining <= deposits.len() as u64,
                "Remaining dust should be minimal: {}",
                remaining
            );
        }
    }
}
