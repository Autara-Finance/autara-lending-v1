// Live end-to-end proof of Autara's "shared loss" / loss-socialization feature.
//
// WHAT THIS PROVES (see docs/SHARED_LOSS.md for the full mechanism map):
//   1. A borrow position can be driven underwater (LTV >= 1) by an oracle price
//      crash, so a normal liquidation would leave bad debt.
//   2. The market curator (and ONLY the curator) can call `socialize_loss`,
//      which:
//        - writes down every supplier's redeemable balance pro-rata by the
//          full outstanding debt (the pool eats the loss),
//        - clears the bad position's debt and collateral to zero,
//        - transfers ALL of the position's collateral to the CURATOR's wallet
//          WITHOUT the curator paying anything into the pool.
//   3. The curator can later (at its sole discretion, any amount) call
//      `donate_supply` to add value back to the pool, which raises every
//      supplier's redeemable balance pro-rata. Nothing on-chain forces the
//      curator to add anything back, or to add back a "fair" amount.
//
// This is a FRESH, SELF-CONTAINED market with FRESH oracle feeds created by the
// test harness (AutaraTestEnv). It NEVER touches the shared aUSD/aBTC market.
//
// TARGET (defaults reproduce the original *stage* run):
//   By default it runs against the Autara stage program on testnet with a
//   harness-generated curator, signing with Regtest (same as before). Env knobs
//   let it target ANY deployment (e.g. our fresh testnet stack) without editing
//   code:
//     SL_PROGRAM_ID   lending program id (hex)         [default: stage program]
//     SL_ORACLE_ID    oracle program id (hex)          [default: stage oracle]
//     SL_CURATOR_KEY  path to a secret-key file; this signer creates the market
//                     and calls socialize_loss/donate  [default: harness authority]
//     SL_NETWORK      signing network testnet4|testnet|regtest|mainnet [default: regtest]
//     SL_RPC          arch rpc url                     [default: testnet rpc]
//     SL_VERIFY_ONLY  =1 -> run only the permissionless-push pre-flight and exit
//
// Run (stage, unchanged):
//   cargo run -p autara-client --example shared_loss_flow
//
// Amount knobs (all optional; defaults give a realistic 70% LTV loan hit by a
// 60% collateral crash):
//   SL_SUPPLY_UNITS      supplier A deposit, whole supply units   (default 200000)
//   SL_COLLATERAL_UNITS  borrower B collateral, whole units       (default 1)
//   SL_BORROW_UNITS      borrower B borrow, whole supply units    (default 70000)
//   SL_COLLATERAL_PRICE  healthy collateral price in USD          (default 100000)
//   SL_CRASH_PRICE       crashed collateral price in USD          (default 40000)
//   SL_DONATE_UNITS      curator add-back after off-chain sale    (default 40000)
//
// Token mints created by the harness use 9 decimals, so 1 unit = 1e9 atoms.

use anyhow::{anyhow, Context, Result};
use arch_sdk::{
    arch_program::{bitcoin::key::Keypair, bitcoin::Network, pubkey::Pubkey},
    generate_new_keypair, with_secret_key_file, AsyncArchRpcClient, Config, Status,
};
use autara_client::{
    client::{
        client_with_signer::AutaraFullClientWithSigner, read::AutaraReadClient,
        single_thread_client::AutaraReadClientImpl, tx_builder::TransactionToSign,
    },
    config::{autara_oracle_stage_program_id, autara_stage_program_id, ArchConfig},
    rpc_ext::ArchAsyncRpcExt,
    test::AutaraTestEnv,
};
use autara_lib::{
    interest_rate::interest_rate_kind::InterestRateCurveKind, ixs::CreateMarketInstruction,
    math::ifixed_point::IFixedPoint, oracle::pyth::PythPrice, pda::find_borrow_position_pda,
    state::market_config::LtvConfig,
};
use autara_pyth::{get_pyth_account, AutaraPythPusherClient};

const DECIMALS_POW: u64 = 1_000_000_000; // 9 decimals => 1 unit = 1e9 atoms

// SAFETY: real BTC/USDC feed ids that MUST NEVER be pushed by this proof.
const REAL_BTC_FEED: &str = "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";
const REAL_USDC_FEED: &str = "eaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a";
// SAFETY: the shared aUSD/aBTC market that MUST NEVER be touched.
const SHARED_MARKET: &str = "d8d679b946aafb22322f477cd5f196700f181aa3f712ca09e486fc77cedc0cce";

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_network(s: &str) -> Network {
    match s.to_ascii_lowercase().as_str() {
        "testnet4" => Network::Testnet4,
        "testnet" => Network::Testnet,
        "regtest" => Network::Regtest,
        "mainnet" | "bitcoin" => Network::Bitcoin,
        other => panic!("unknown SL_NETWORK={other} (use testnet4|testnet|regtest|mainnet)"),
    }
}

fn pk_from_hex(h: &str) -> Pubkey {
    Pubkey::from_slice(&hex::decode(h).expect("valid hex pubkey"))
}

fn pv(b: bool) -> &'static str {
    if b {
        "PASS"
    } else {
        "FAIL"
    }
}

/// Sign + send + wait, returning the txid string. Fails loudly on a failed tx.
async fn send_tx(
    rpc: &arch_sdk::AsyncArchRpcClient,
    network: Network,
    kp: &Keypair,
    tx: TransactionToSign,
    label: &str,
) -> Result<String> {
    let signed = tx.sign(std::slice::from_ref(kp), network);
    let txid = rpc.send_transaction(signed).await?;
    let txid_str = txid.to_string();
    let processed = rpc.wait_for_processed_transaction(&txid).await?;
    match &processed.status {
        Status::Processed => {
            println!("    [{label}] PROCESSED tx {txid_str}");
            Ok(txid_str)
        }
        other => Err(anyhow!(
            "[{label}] tx {txid_str} not processed: status={other:?} logs={:?}",
            processed.logs
        )),
    }
}

fn supply_redeemable(
    client: &AutaraFullClientWithSigner<AutaraReadClientImpl>,
    market: &Pubkey,
    holder: Keypair,
) -> u64 {
    let scoped = client.with_signer(holder);
    let Some(sp) = scoped.get_supply_position(market) else {
        return 0;
    };
    client
        .read_client()
        .get_market(market)
        .and_then(|m| m.market().supply_position_info(&sp).ok())
        .unwrap_or(0)
}

/// Pre-flight: confirm that price pushes to this oracle are PERMISSIONLESS by
/// creating a FRESH fake feed (random 32-byte id) and pushing to it with an
/// arbitrary, unrelated signer. Reports the on-chain feed account size (legacy
/// pre-authority feeds are exactly one `PythPrice` = 120 bytes). Returns the
/// feed account data length on success.
async fn verify_permissionless_push(
    rpc: &AsyncArchRpcClient,
    oracle_id: &Pubkey,
    network: Network,
) -> Result<usize> {
    println!("== PRE-FLIGHT: verify permissionless push + feed layout ==");
    // Arbitrary signer unrelated to curator/admin.
    let (signer, signer_pk, _) = generate_new_keypair(network);
    rpc.create_and_fund_account_with_faucet(&signer)
        .await
        .context("faucet-fund arbitrary push signer")?;
    // Fresh random fake feed id (a brand new keypair's pubkey bytes).
    let (_k, feed_pk, _) = generate_new_keypair(network);
    let feed_id = feed_pk.0;
    let feed_hex = hex::encode(feed_id);
    // Guard: never collide with the real BTC/USDC feeds.
    assert!(
        feed_hex != REAL_BTC_FEED && feed_hex != REAL_USDC_FEED,
        "generated feed id collides with a REAL feed id -- aborting"
    );
    println!(
        "    arbitrary signer = {}",
        hex::encode(signer_pk.serialize())
    );
    println!("    fresh fake feed id = {feed_hex}");

    let price = PythPrice::from_dummy(feed_id, 123.45);
    AutaraPythPusherClient {
        client: rpc.clone(),
        autara_oracle_program_id: *oracle_id,
        network,
    }
    .push_pyth_price(&signer, feed_id, &price)
    .await
    .context("push to fresh fake feed with arbitrary signer")?;

    let pyth_account = get_pyth_account(oracle_id, feed_id);
    let acc = rpc
        .read_account_info(pyth_account)
        .await
        .context("read back fresh feed account")?;
    let owner_ok = acc.owner == *oracle_id;
    let len = acc.data.len();
    let legacy_120 = len == core::mem::size_of::<PythPrice>();
    // decode the price (legacy layout is exactly one PythPrice at the front)
    let stored: Option<&PythPrice> = bytemuck::try_from_bytes::<PythPrice>(
        &acc.data[..core::mem::size_of::<PythPrice>().min(len)],
    )
    .ok();
    let id_ok = stored.map(|p| p.id == feed_id).unwrap_or(false);
    println!(
        "    feed account = {} owner_ok={owner_ok} size={len} bytes (legacy120={legacy_120}) id_ok={id_ok}",
        hex::encode(pyth_account.serialize())
    );
    if !(owner_ok && id_ok) {
        return Err(anyhow!(
            "permissionless push verification FAILED (owner_ok={owner_ok} id_ok={id_ok}); \
             the oracle may be authority-gated -- STOP and escalate (redeploy is a human decision)"
        ));
    }
    println!("    => PERMISSIONLESS PUSH CONFIRMED");
    Ok(len)
}

#[tokio::main]
async fn main() -> Result<()> {
    let supply_atoms = env_u64("SL_SUPPLY_UNITS", 200_000) * DECIMALS_POW;
    let collateral_atoms = env_u64("SL_COLLATERAL_UNITS", 1) * DECIMALS_POW;
    let borrow_atoms = env_u64("SL_BORROW_UNITS", 70_000) * DECIMALS_POW;
    let donate_atoms = env_u64("SL_DONATE_UNITS", 40_000) * DECIMALS_POW;
    let collateral_price = env_f64("SL_COLLATERAL_PRICE", 100_000.0);
    let crash_price = env_f64("SL_CRASH_PRICE", 40_000.0);

    // Target selection (defaults reproduce the stage run).
    let network = parse_network(&env_or("SL_NETWORK", "regtest"));
    let program_id = std::env::var("SL_PROGRAM_ID")
        .map(|h| pk_from_hex(&h))
        .unwrap_or_else(|_| autara_stage_program_id());
    let oracle_id = std::env::var("SL_ORACLE_ID")
        .map(|h| pk_from_hex(&h))
        .unwrap_or_else(|_| autara_oracle_stage_program_id());

    // Build the rpc client honoring SL_NETWORK so faucet funding matches the
    // network used to sign every other transaction.
    let base = ArchConfig::testnet();
    let config = Config {
        arch_node_url: env_or("SL_RPC", &base.arch_node_url),
        node_endpoint: base.bitcoin_node_endpoint.clone(),
        node_username: base.bitcoin_node_username.clone(),
        node_password: base.bitcoin_node_password.clone(),
        network,
        titan_url: String::new(),
    };
    let arch_client = AsyncArchRpcClient::new(&config);

    println!("== TARGET ==");
    println!("    program = {}", hex::encode(program_id.serialize()));
    println!("    oracle  = {}", hex::encode(oracle_id.serialize()));
    println!("    network = {network:?}");
    println!("    rpc     = {}", config.arch_node_url);

    // ---- PRE-FLIGHT: permissionless-push verification (always) ----
    let feed_size = verify_permissionless_push(&arch_client, &oracle_id, network).await?;

    if env_or("SL_VERIFY_ONLY", "0") == "1" {
        println!("\nSL_VERIFY_ONLY=1 -> feed_size={feed_size}; skipping the full flow.");
        return Ok(());
    }

    println!("\n== SETUP: fresh market + fresh oracle feeds ==");
    let env = AutaraTestEnv::new_with_network(arch_client.clone(), program_id, oracle_id, network)
        .await
        .context("create test env (fund keypairs, mint tokens, create feeds)")?;

    // Roles. The curator creates the market, socializes, and donates. By default
    // it is the harness authority (stage behavior); override with SL_CURATOR_KEY
    // (e.g. our admin.json) to prove against a real deployment's admin.
    let (curator, curator_overridden) = match std::env::var("SL_CURATOR_KEY") {
        Ok(path) => {
            let (kp, _pk) = with_secret_key_file(&path)
                .map_err(|e| anyhow!("load SL_CURATOR_KEY {path}: {e}"))?;
            (kp, true)
        }
        Err(_) => (env.authority_keypair, false),
    };
    let supplier_a = env.user_two_keypair; // the lender who eats the loss
    let borrower_b = env.user_keypair; // the bad borrower
    let curator_pk = Pubkey::from_slice(&curator.x_only_public_key().0.serialize());
    let borrower_pk = env.user_pubkey;
    let supply_mint = env.supply_mint;
    let collateral_mint = env.collateral_mint;

    // SAFETY guards: we must never operate on the shared market or its mints.
    // The market PDA is derived from (curator, supply_mint, collateral_mint,
    // index); harness mints are freshly generated so this can never be the
    // shared market, but assert the mints are fresh regardless.
    assert!(hex::encode(supply_mint.serialize()) != SHARED_MARKET);
    assert!(hex::encode(collateral_mint.serialize()) != SHARED_MARKET);

    // If the curator is an external key (admin.json) it will not hold harness
    // tokens: fund it and mint enough supply tokens for the donate step.
    if curator_overridden {
        if let Err(e) = arch_client
            .create_and_fund_account_with_faucet(&curator)
            .await
        {
            println!("    curator faucet note: {e}");
        }
        env.supply_minter
            .mint_to(
                &curator_pk,
                donate_atoms.saturating_mul(2).max(donate_atoms),
            )
            .await
            .context("mint supply tokens to overridden curator for donate_supply")?;
    }

    // Healthy starting prices: supply asset = $1, collateral = $collateral_price.
    tokio::try_join!(
        env.push_supply_price(1.0),
        env.push_collateral_price(collateral_price)
    )
    .context("push initial prices")?;

    let mut client =
        AutaraFullClientWithSigner::new_simple(arch_client.clone(), network, program_id, curator);
    client.full_reload().await?;

    // ---- create the fresh market (curator-signed) ----
    let market = client
        .with_signer(curator)
        .create_market(
            CreateMarketInstruction {
                market_bump: 0,
                index: 0,
                ltv_config: LtvConfig {
                    max_ltv: IFixedPoint::from_i64_u64_ratio(8, 10),
                    unhealthy_ltv: IFixedPoint::from_i64_u64_ratio(9, 10),
                    liquidation_bonus: IFixedPoint::from_i64_u64_ratio(5, 100),
                },
                max_utilisation_rate: IFixedPoint::from_i64_u64_ratio(9, 10),
                supply_oracle_config: env.supply_oracle_config(),
                collateral_oracle_config: env.collateral_oracle_config(),
                interest_rate: InterestRateCurveKind::new_adaptive(),
                lending_market_fee_in_bps: 100,
            },
            supply_mint,
            collateral_mint,
        )
        .await
        .context("create_market")?;
    client.full_reload().await?;
    // Final safety assertion: the market we created is NOT the shared market.
    assert!(
        hex::encode(market.serialize()) != SHARED_MARKET,
        "refusing to operate on the shared aUSD/aBTC market"
    );
    println!("    market = {}", hex::encode(market.serialize()));
    println!(
        "    curator (creator) = {}",
        hex::encode(curator_pk.serialize())
    );
    println!("    supply_mint = {}", hex::encode(supply_mint.serialize()));
    println!(
        "    collateral_mint = {}",
        hex::encode(collateral_mint.serialize())
    );
    println!("    supply_feed = {}", hex::encode(env.supply_feed_id));
    println!(
        "    collateral_feed = {}",
        hex::encode(env.collateral_feed_id)
    );

    let position = find_borrow_position_pda(&program_id, &market, &borrower_pk).0;
    let rpc = arch_client.clone();
    let mut results: Vec<(String, bool, String)> = Vec::new();

    // ---- STEP 1: supplier A supplies liquidity ----
    println!("\n== STEP 1: supplier A supplies {supply_atoms} atoms ==");
    let tx = client
        .with_signer(supplier_a)
        .tx_builder()
        .supply(&market, supply_atoms)
        .await?;
    let tx1 = send_tx(&rpc, network, &supplier_a, tx, "supply").await?;
    client.full_reload().await?;
    let redeemable_start = supply_redeemable(&client, &market, supplier_a);
    let pass1 = redeemable_start >= supply_atoms - 2;
    println!(
        "    ASSERT supplier A redeemable ~= supplied -> {redeemable_start} => {}",
        pv(pass1)
    );
    results.push((
        format!("1 supply (tx {tx1})"),
        pass1,
        format!("redeemable={redeemable_start}"),
    ));

    // ---- STEP 2: borrower B deposits collateral ----
    println!("\n== STEP 2: borrower B deposits {collateral_atoms} collateral atoms ==");
    let tx = client
        .with_signer(borrower_b)
        .tx_builder()
        .deposit_collateral(&market, collateral_atoms)
        .await?;
    let tx2 = send_tx(&rpc, network, &borrower_b, tx, "deposit").await?;
    client.full_reload().await?;
    let coll_deposited = client
        .with_signer(borrower_b)
        .get_borrow_position_health(&market)
        .map(|h| h.collateral_atoms)
        .unwrap_or(0);
    let pass2 = coll_deposited == collateral_atoms;
    println!(
        "    ASSERT collateral_atoms == {collateral_atoms} -> {coll_deposited} => {}",
        pv(pass2)
    );
    results.push((
        format!("2 deposit_collateral (tx {tx2})"),
        pass2,
        format!("collateral={coll_deposited}"),
    ));

    // ---- STEP 3: borrower B borrows near max LTV ----
    println!("\n== STEP 3: borrower B borrows {borrow_atoms} atoms (near max LTV) ==");
    // refresh oracle so the borrow health check reads a non-stale price
    env.push_collateral_price(collateral_price).await?;
    let tx = client
        .with_signer(borrower_b)
        .tx_builder()
        .borrow(&market, borrow_atoms)
        .await?;
    let tx3 = send_tx(&rpc, network, &borrower_b, tx, "borrow").await?;
    client.full_reload().await?;
    let health_healthy = client
        .with_signer(borrower_b)
        .get_borrow_position_health(&market)?;
    let ltv_healthy = health_healthy.ltv.to_float();
    let pass3 = health_healthy.borrowed_atoms >= borrow_atoms && ltv_healthy < 0.8;
    println!(
        "    ASSERT borrowed>=req && ltv<0.8 -> borrowed={} ltv={ltv_healthy:.4} => {}",
        health_healthy.borrowed_atoms,
        pv(pass3)
    );
    results.push((
        format!("3 borrow (tx {tx3})"),
        pass3,
        format!("ltv={ltv_healthy:.4}"),
    ));

    // snapshot BEFORE the crash/socialize
    let redeemable_before = supply_redeemable(&client, &market, supplier_a);
    let curator_bal_before = rpc.get_all_balances(&curator_pk).await.unwrap_or_default();
    let curator_coll_before = curator_bal_before
        .get(&collateral_mint)
        .copied()
        .unwrap_or(0);
    let curator_supply_before = curator_bal_before.get(&supply_mint).copied().unwrap_or(0);

    // ---- STEP 4: crash the collateral price so the position is underwater ----
    println!("\n== STEP 4: crash collateral price {collateral_price} -> {crash_price} ==");
    env.push_collateral_price(crash_price).await?;
    client.full_reload().await?;
    let health_crashed = client
        .with_signer(borrower_b)
        .get_borrow_position_health(&market)?;
    let ltv_crashed = health_crashed.ltv.to_float();
    let pass4 = health_crashed.ltv > IFixedPoint::one();
    println!(
        "    ASSERT ltv > 1.0 (underwater) -> ltv={ltv_crashed:.4} => {}",
        pv(pass4)
    );
    results.push((
        format!("4 price crash"),
        pass4,
        format!("ltv={ltv_crashed:.4}"),
    ));

    // ---- STEP 5a: NON-curator socialize_loss MUST be rejected ----
    println!("\n== STEP 5a: non-curator socialize_loss must be REJECTED ==");
    env.push_collateral_price(crash_price).await?;
    env.push_supply_price(1.0).await?;
    let non_curator_rejected = match client
        .with_signer(borrower_b)
        .tx_builder()
        .socialize_loss(&market, &position)
        .await
    {
        Ok(tx) => {
            // Built ok; sending it must fail on-chain (curator check).
            let signed = tx.sign(std::slice::from_ref(&borrower_b), network);
            match rpc.send_transaction(signed).await {
                Ok(txid) => {
                    let processed = rpc.wait_for_processed_transaction(&txid).await?;
                    !matches!(processed.status, Status::Processed)
                }
                Err(_) => true,
            }
        }
        Err(_) => true,
    };
    println!(
        "    ASSERT non-curator socialize_loss rejected -> {}",
        pv(non_curator_rejected)
    );
    results.push((
        "5a non-curator socialize_loss rejected".to_string(),
        non_curator_rejected,
        format!("rejected={non_curator_rejected}"),
    ));

    // ---- STEP 5b: curator socializes the loss ----
    println!("\n== STEP 5b: curator calls socialize_loss ==");
    env.push_collateral_price(crash_price).await?; // keep feed fresh for the tx
    env.push_supply_price(1.0).await?;
    let tx = client
        .with_signer(curator)
        .tx_builder()
        .socialize_loss(&market, &position)
        .await?;
    let tx5 = send_tx(&rpc, network, &curator, tx, "socialize_loss").await?;
    client.full_reload().await?;

    let redeemable_after = supply_redeemable(&client, &market, supplier_a);
    let debt_after = client
        .with_signer(borrower_b)
        .get_borrow_position_health(&market)
        .map(|h| h.borrowed_atoms)
        .unwrap_or(0);
    let curator_bal_after = rpc.get_all_balances(&curator_pk).await.unwrap_or_default();
    let curator_coll_after = curator_bal_after
        .get(&collateral_mint)
        .copied()
        .unwrap_or(0);
    let curator_supply_after = curator_bal_after.get(&supply_mint).copied().unwrap_or(0);

    let writedown = redeemable_before.saturating_sub(redeemable_after);
    let coll_swept = curator_coll_after.saturating_sub(curator_coll_before);

    // 5b-i: supplier A written down by ~ the socialized debt
    let pass5a = writedown >= borrow_atoms.saturating_sub(2);
    println!(
        "    ASSERT supplier A written down by ~debt -> before={redeemable_before} after={redeemable_after} writedown={writedown} (debt~{borrow_atoms}) => {}",
        pv(pass5a)
    );
    // 5b-ii: bad position's debt cleared to zero
    let pass5b = debt_after == 0;
    println!(
        "    ASSERT bad position debt == 0 -> {debt_after} => {}",
        pv(pass5b)
    );
    // 5b-iii: curator received ALL the collateral
    let pass5c = coll_swept == collateral_atoms;
    println!(
        "    ASSERT curator swept collateral == {collateral_atoms} -> {coll_swept} => {}",
        pv(pass5c)
    );
    // 5b-iv: curator paid NOTHING into the pool (supply balance unchanged)
    let pass5d = curator_supply_after == curator_supply_before;
    println!(
        "    ASSERT curator supply balance unchanged (paid nothing) -> before={curator_supply_before} after={curator_supply_after} => {}",
        pv(pass5d)
    );
    results.push((
        format!("5b socialize_loss (tx {tx5})"),
        pass5a && pass5b && pass5c && pass5d,
        format!("writedown={writedown} debt_after={debt_after} coll_swept={coll_swept}"),
    ));

    // ---- STEP 6: curator adds back (discretionary) after off-chain sale ----
    println!("\n== STEP 6: curator donates {donate_atoms} back (simulated off-chain recovery) ==");
    let tx = client
        .with_signer(curator)
        .tx_builder()
        .donate_supply(&market, donate_atoms)
        .await?;
    let tx6 = send_tx(&rpc, network, &curator, tx, "donate_supply").await?;
    client.full_reload().await?;
    let redeemable_recovered = supply_redeemable(&client, &market, supplier_a);
    let recovery = redeemable_recovered.saturating_sub(redeemable_after);
    // Share-price accounting rounds down: crediting `donate_atoms` over a large
    // share pool loses sub-atom dust per share, so allow a small proportional
    // tolerance (1 ppm + a floor) rather than exact equality.
    let recovery_tol = donate_atoms / 1_000_000 + 100;
    let pass6 = recovery + recovery_tol >= donate_atoms;
    println!(
        "    ASSERT supplier A recovers by ~donation -> after_socialize={redeemable_after} after_donate={redeemable_recovered} recovery={recovery} (donated={donate_atoms}) => {}",
        pv(pass6)
    );
    let net_loss = redeemable_before.saturating_sub(redeemable_recovered);
    println!("    NET supplier A loss (debt - add-back) = {net_loss} atoms");
    results.push((
        format!("6 donate_supply (tx {tx6})"),
        pass6,
        format!("recovery={recovery} net_loss={net_loss}"),
    ));

    // ---- summary ----
    println!("\n================ SUMMARY ================");
    let mut all = true;
    for (name, pass, detail) in &results {
        all &= pass;
        println!("  {} :: {} :: {}", pv(*pass), name, detail);
    }
    println!("=========================================");
    println!("market = {}", hex::encode(market.serialize()));
    println!("FINAL VERDICT: {}", if all { "PASS" } else { "FAIL" });
    if !all {
        std::process::exit(1);
    }
    Ok(())
}
