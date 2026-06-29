//! Env-driven deploy configuration for `autara-deploy`.
//!
//! Everything the deploy flow needs is parsed once from the environment by
//! [`DeployConfig::from_env`]. The only values that normally differ between
//! networks are `network`, `arch_rpc_url`, the `*_KEY_PATH` files, and the
//! token mints — mirroring the CLAMM deploy tool's convention.
//!
//! `localnet`, `testnet`, and `mainnet` are all wired up. Mainnet defaults to
//! the public Arch mainnet RPC and the Bitcoin mainnet signing network; a real
//! mainnet run is still gated behind the CI typed-confirmation (`DEPLOY MAINNET`
//! in `_autara-action.yml`) and never generates keys here.

use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use arch_program::pubkey::Pubkey;
use arch_sdk::arch_program::bitcoin::Network as BitcoinNetwork;
use arch_sdk::Config as ArchConfig;

/// Target network. Selects the Bitcoin network used for transaction signing and
/// the default Arch RPC endpoint, both of which can still be overridden via env.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Localnet,
    Testnet,
    Mainnet,
}

impl Network {
    pub fn as_str(&self) -> &'static str {
        match self {
            Network::Localnet => "localnet",
            Network::Testnet => "testnet",
            Network::Mainnet => "mainnet",
        }
    }

    /// Default Arch RPC endpoint for the network (overridable via `ARCH_RPC_URL`).
    /// The mainnet endpoint is the public Arch RPC documented in the network's
    /// book (mirrors the testnet `rpc.testnet.arch.network` convention).
    pub fn default_rpc_url(&self) -> Result<String> {
        match self {
            Network::Localnet => Ok("http://localhost:9002/".to_string()),
            Network::Testnet => Ok("https://rpc.testnet.arch.network".to_string()),
            Network::Mainnet => Ok("https://rpc.mainnet.arch.network".to_string()),
        }
    }

    /// Bitcoin network used by `build_and_sign_transaction` / `ProgramDeployer`.
    /// Testnet uses `Testnet4`, matching the repo's existing testnet deploy;
    /// mainnet uses `Bitcoin` (matches `arch_sdk::Config::mainnet`).
    pub fn bitcoin_network(&self) -> Result<BitcoinNetwork> {
        match self {
            Network::Localnet => Ok(BitcoinNetwork::Regtest),
            Network::Testnet => Ok(BitcoinNetwork::Testnet4),
            Network::Mainnet => Ok(BitcoinNetwork::Bitcoin),
        }
    }

    pub fn has_faucet(&self) -> bool {
        matches!(self, Network::Localnet | Network::Testnet)
    }
}

impl FromStr for Network {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_lowercase().as_str() {
            "localnet" | "local" | "regtest" | "dev" => Ok(Network::Localnet),
            "testnet" | "devnet" => Ok(Network::Testnet),
            "mainnet" | "mainnet-beta" => Ok(Network::Mainnet),
            other => bail!("unknown NETWORK '{other}' (expected localnet|testnet|mainnet)"),
        }
    }
}

/// A token mint to record in the deployment artifact and to wire into the
/// market-creation steps. Parsed from `TOKENS=LABEL:MINT_HEX:DECIMALS,...`.
#[derive(Debug, Clone)]
pub struct TokenConfig {
    pub label: String,
    pub mint: Pubkey,
    pub decimals: u8,
}

/// Pyth price-feed ids (32-byte, hex) for the labels Autara markets use. These
/// mirror the constants in `autara-pyth` (`BTC_FEED` / `ETH_FEED` / `USDC_FEED`)
/// and the `pyth_feed_for_token` mapping in `autara-server`; they are duplicated
/// here as small constants so the deploy tool avoids the heavy `autara-pyth`
/// dependency (reqwest etc.). A market can only be created for a token whose
/// label resolves to a feed here.
///
/// DRIFT RISK: these are duplicated by hand from `autara-pyth`. There is no
/// compile-time link, so if the canonical feed ids change upstream they MUST be
/// updated here too. `tests::pyth_feed_constants_are_valid` only guards the
/// constants' shape (valid 32-byte hex, pairwise-distinct) — it cannot detect
/// divergence from `autara-pyth` without pulling that crate in (deliberately
/// avoided). Re-confirm these against `autara-pyth` before a mainnet run.
const BTC_FEED: &str = "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";
const USDC_FEED: &str = "eaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a";
const ETH_FEED: &str = "ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace";

/// Map a token label to its Pyth feed id (32 bytes). Case-insensitive; returns
/// `None` for labels without a known feed (those pairs are skipped).
pub fn pyth_feed_for_label(label: &str) -> Option<[u8; 32]> {
    let hex_str = match label.trim().to_uppercase().as_str() {
        "BTC" => BTC_FEED,
        "USDC" => USDC_FEED,
        "ETH" => ETH_FEED,
        _ => return None,
    };
    let mut id = [0u8; 32];
    hex::decode_to_slice(hex_str, &mut id).ok()?;
    Some(id)
}

/// A market to create: a supply/collateral token pair, identified by the labels
/// used in `TOKENS`. Parsed from `MARKET_PAIRS=SUPPLY/COLLATERAL,...`.
#[derive(Debug, Clone)]
pub struct MarketPair {
    pub supply_label: String,
    pub collateral_label: String,
}

/// Economic parameters applied to every created market. The defaults are the
/// values previously hardcoded in `steps.rs` (and mirror `autara-server`'s
/// `default_market_config`), so leaving the env knobs unset reproduces the old
/// behavior byte-for-byte. Mainnet should set CONFIRMED values via the env file
/// rather than relying on these defaults. The interest-rate curve stays adaptive
/// (not parameterized here).
///
/// Stored as `f64` and converted to `IFixedPoint` via `from_num` at instruction
/// build time, exactly as the old hardcoded literals were — see
/// `steps::build_create_market_instruction`.
#[derive(Debug, Clone, Copy)]
pub struct MarketParams {
    pub max_ltv: f64,
    pub unhealthy_ltv: f64,
    pub liquidation_bonus: f64,
    pub max_utilisation_rate: f64,
}

impl Default for MarketParams {
    fn default() -> Self {
        // EXACTLY the values previously hardcoded in `build_create_market_instruction`.
        MarketParams {
            max_ltv: 0.8,
            unhealthy_ltv: 0.9,
            liquidation_bonus: 0.05,
            max_utilisation_rate: 0.9,
        }
    }
}

/// Fully-resolved deploy configuration, parsed once from the environment.
#[derive(Debug, Clone)]
pub struct DeployConfig {
    pub network: Network,
    pub arch_rpc_url: String,

    // Program + account keypair FILES (paths only; never secrets in env).
    pub program_key_path: String,
    pub oracle_key_path: String,
    /// Program deploy/upgrade authority + payer for the ELF uploads.
    pub deployer_key_path: String,
    /// Global-config admin: also the payer/signer for `create_global_config`.
    pub admin_key_path: String,

    // Compiled program ELFs produced by `cargo-build-sbf --features entrypoint`.
    pub program_elf_path: String,
    pub oracle_elf_path: String,

    /// Global-config admin pubkey (defaults to the admin keypair's pubkey).
    pub admin: Option<Pubkey>,
    /// Protocol-fee receiver pubkey (defaults to the admin pubkey).
    pub fee_receiver: Option<Pubkey>,
    /// Protocol fee share, in basis points.
    pub protocol_fee_share_bps: u16,

    /// Token mints to record and to build markets from.
    pub tokens: Vec<TokenConfig>,

    /// Token pairs (by label) to create markets for. Empty => derive every
    /// ordered pair of configured tokens that has a known Pyth feed.
    pub market_pairs: Vec<MarketPair>,
    /// Curator/lending fee applied to each created market, in basis points.
    pub lending_market_fee_bps: u16,
    /// Economic parameters (LTV / utilisation) applied to every created market.
    pub market_params: MarketParams,

    pub use_faucet: bool,
    pub output_path: String,
}

/// Token mint hexes shipped as PLACEHOLDERS in `autara.mainnet.env` (they are the
/// testnet mints). A REAL mainnet run must replace these with the genuine mainnet
/// APL mints; the mainnet preflight refuses to run while any of them is still
/// configured. Keep this list in sync with the `TOKENS=` line in
/// `autara-deploy/scripts/autara.mainnet.env`.
const PLACEHOLDER_MINT_HEXES: [&str; 3] = [
    "36a97410055bbbdc52b421d0c95f76d85eca066b83db8b14f64665b178c93d8b",
    "7250792453cc3a0bd015778f240dd50b552c48c153b7b83e3ef0c441aff9483c",
    "a80fa79ee82952b0a127f50e7d469dae1a51315d4267ca38d7907ad5df5cb3cb",
];

fn env_opt(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn env_or(key: &str, default: &str) -> String {
    env_opt(key).unwrap_or_else(|| default.to_string())
}

fn env_bool(key: &str, default: bool) -> Result<bool> {
    match env_opt(key) {
        None => Ok(default),
        Some(v) => match v.trim().to_lowercase().as_str() {
            "1" | "true" | "yes" | "y" | "on" => Ok(true),
            "0" | "false" | "no" | "n" | "off" => Ok(false),
            other => bail!("invalid bool for {key}: '{other}'"),
        },
    }
}

fn env_parse<T: FromStr>(key: &str, default: T) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    match env_opt(key) {
        None => Ok(default),
        Some(v) => v
            .trim()
            .parse::<T>()
            .map_err(|e| anyhow!("invalid value for {key}: {e}")),
    }
}

fn parse_pubkey(key: &str, value: &str) -> Result<Pubkey> {
    Pubkey::from_str(value.trim()).map_err(|e| anyhow!("invalid pubkey for {key}: {e:?}"))
}

fn env_pubkey_opt(key: &str) -> Result<Option<Pubkey>> {
    match env_opt(key) {
        None => Ok(None),
        Some(v) => Ok(Some(parse_pubkey(key, &v)?)),
    }
}

/// Parse "BTC:36a9...:8,USDC:a80f...:6" into token configs. Mints are HEX
/// (arch_program `Pubkey::from_str` == hex::decode), not base58.
fn parse_tokens(raw: &str) -> Result<Vec<TokenConfig>> {
    let mut tokens = Vec::new();
    for entry in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let mut parts = entry.split(':');
        let label = parts
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow!("invalid TOKENS entry '{entry}' (expected LABEL:MINT:DECIMALS)")
            })?
            .to_string();
        let mint_raw = parts.next().ok_or_else(|| {
            anyhow!("token '{label}' missing mint (expected LABEL:MINT:DECIMALS)")
        })?;
        let decimals_raw = parts.next().ok_or_else(|| {
            anyhow!("token '{label}' missing decimals (expected LABEL:MINT:DECIMALS)")
        })?;
        let mint = Pubkey::from_str(mint_raw.trim())
            .map_err(|e| anyhow!("invalid mint for token '{label}': {e:?}"))?;
        let decimals: u8 = decimals_raw
            .trim()
            .parse()
            .with_context(|| format!("invalid decimals for token '{label}'"))?;
        tokens.push(TokenConfig {
            label,
            mint,
            decimals,
        });
    }
    Ok(tokens)
}

/// Parse "USDC/BTC,USDC/ETH" into market pairs (supply/collateral labels).
fn parse_market_pairs(raw: &str) -> Result<Vec<MarketPair>> {
    let mut pairs = Vec::new();
    for entry in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (supply, collateral) = entry.split_once('/').ok_or_else(|| {
            anyhow!("invalid MARKET_PAIRS entry '{entry}' (expected SUPPLY/COLLATERAL)")
        })?;
        let supply_label = supply.trim().to_string();
        let collateral_label = collateral.trim().to_string();
        if supply_label.is_empty() || collateral_label.is_empty() {
            bail!("invalid MARKET_PAIRS entry '{entry}' (empty label)");
        }
        pairs.push(MarketPair {
            supply_label,
            collateral_label,
        });
    }
    Ok(pairs)
}

impl DeployConfig {
    pub fn from_env() -> Result<Self> {
        let network: Network = env_or("NETWORK", "localnet").parse()?;

        let arch_rpc_url = match env_opt("ARCH_RPC_URL") {
            Some(url) => url,
            None => network.default_rpc_url()?,
        };

        let tokens = match env_opt("TOKENS") {
            Some(raw) => parse_tokens(&raw)?,
            None => Vec::new(),
        };

        let market_pairs = match env_opt("MARKET_PAIRS") {
            Some(raw) => parse_market_pairs(&raw)?,
            None => Vec::new(),
        };

        let cfg = DeployConfig {
            network,
            arch_rpc_url,

            program_key_path: env_or("PROGRAM_KEY_PATH", "keys/autara-stage.key"),
            oracle_key_path: env_or("ORACLE_KEY_PATH", "keys/autara-pyth-stage.key"),
            deployer_key_path: env_or("DEPLOYER_KEY_PATH", "keys/autara-deployer.key"),
            admin_key_path: env_or("ADMIN_KEY_PATH", "keys/autara-admin-stage.key"),

            program_elf_path: env_or("PROGRAM_ELF_PATH", "target/deploy/autara_program.so"),
            oracle_elf_path: env_or("ORACLE_ELF_PATH", "target/deploy/autara_oracle.so"),

            admin: env_pubkey_opt("ADMIN")?,
            fee_receiver: env_pubkey_opt("FEE_RECEIVER")?,
            protocol_fee_share_bps: env_parse("PROTOCOL_FEE_SHARE_BPS", 5000u16)?,

            tokens,

            market_pairs,
            lending_market_fee_bps: env_parse("LENDING_MARKET_FEE_BPS", 100u16)?,
            market_params: MarketParams {
                max_ltv: env_parse("MARKET_MAX_LTV", MarketParams::default().max_ltv)?,
                unhealthy_ltv: env_parse(
                    "MARKET_UNHEALTHY_LTV",
                    MarketParams::default().unhealthy_ltv,
                )?,
                liquidation_bonus: env_parse(
                    "MARKET_LIQUIDATION_BONUS",
                    MarketParams::default().liquidation_bonus,
                )?,
                max_utilisation_rate: env_parse(
                    "MARKET_MAX_UTILISATION",
                    MarketParams::default().max_utilisation_rate,
                )?,
            },

            use_faucet: env_bool("USE_FAUCET", network.has_faucet())?,
            output_path: env_or(
                "OUTPUT_PATH",
                &format!("deployments/{}.json", network.as_str()),
            ),
        };

        Ok(cfg)
    }

    /// Look up a configured token by (case-insensitive) label.
    pub fn token_by_label(&self, label: &str) -> Option<&TokenConfig> {
        self.tokens
            .iter()
            .find(|t| t.label.eq_ignore_ascii_case(label))
    }

    /// Mainnet-only safety checks that must NEVER pass on a real run with an
    /// unsafe config. Returns a human-readable message per violation (empty on a
    /// safe config or any non-mainnet network). The caller decides severity:
    /// these are hard failures on a REAL run and warnings on `--dry-run`.
    ///
    /// Catches the two mainnet footguns:
    ///   1. `USE_FAUCET=true` — there is no faucet on mainnet; the deployer +
    ///      admin must be funded out-of-band.
    ///   2. token mints still equal to the testnet PLACEHOLDER mints shipped in
    ///      `autara.mainnet.env` — a real run must use the genuine mainnet APL
    ///      mints, never the testnet placeholders.
    pub fn mainnet_safety_violations(&self) -> Vec<String> {
        let mut violations = Vec::new();
        if self.network != Network::Mainnet {
            return violations;
        }

        if self.use_faucet {
            violations.push(
                "USE_FAUCET is true but mainnet has no faucet — set USE_FAUCET=false and fund \
                 the deployer + admin out-of-band"
                    .to_string(),
            );
        }

        let placeholders: Vec<Pubkey> = PLACEHOLDER_MINT_HEXES
            .iter()
            .filter_map(|h| Pubkey::from_str(h).ok())
            .collect();
        for token in &self.tokens {
            if placeholders.contains(&token.mint) {
                violations.push(format!(
                    "token '{}' mint {} is still the TESTNET PLACEHOLDER — set the real mainnet \
                     APL mint for '{}' in autara.mainnet.env (TOKENS=)",
                    token.label, token.mint, token.label
                ));
            }
        }

        violations
    }

    /// The market pairs to actually create. When `MARKET_PAIRS` is unset, derive
    /// every ordered pair of configured tokens that has a known Pyth feed
    /// (mirrors `autara-server`'s all-pairs bootstrap).
    pub fn effective_market_pairs(&self) -> Vec<MarketPair> {
        if !self.market_pairs.is_empty() {
            return self.market_pairs.clone();
        }
        let mut pairs = Vec::new();
        for supply in &self.tokens {
            for collateral in &self.tokens {
                if supply.label == collateral.label {
                    continue;
                }
                if pyth_feed_for_label(&supply.label).is_none()
                    || pyth_feed_for_label(&collateral.label).is_none()
                {
                    continue;
                }
                pairs.push(MarketPair {
                    supply_label: supply.label.clone(),
                    collateral_label: collateral.label.clone(),
                });
            }
        }
        pairs
    }

    /// Build the Arch SDK config for this deploy (RPC url + signing network).
    /// Bitcoin-node credentials are left empty: the deploy flow (ELF upload,
    /// faucet, create_global_config) talks only to the Arch RPC, matching the
    /// repo's existing testnet deploy binary.
    pub fn arch_config(&self) -> Result<ArchConfig> {
        Ok(ArchConfig {
            node_endpoint: String::new(),
            node_username: String::new(),
            node_password: String::new(),
            network: self.network.bitcoin_network()?,
            arch_node_url: self.arch_rpc_url.clone(),
            titan_url: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cheap shape guard for the duplicated Pyth feed constants: each must be
    /// valid 32-byte hex and the three must be pairwise distinct. This catches a
    /// typo'd/truncated constant; it cannot detect drift from `autara-pyth`
    /// (that would require the heavy dependency we deliberately avoid — see the
    /// `BTC_FEED`/`USDC_FEED`/`ETH_FEED` doc comment).
    #[test]
    fn pyth_feed_constants_are_valid() {
        let feeds = [("BTC", BTC_FEED), ("USDC", USDC_FEED), ("ETH", ETH_FEED)];
        let mut decoded = Vec::new();
        for (label, hex_str) in feeds {
            assert_eq!(
                hex_str.len(),
                64,
                "{label} feed must be 64 hex chars (32 bytes)"
            );
            let id =
                pyth_feed_for_label(label).unwrap_or_else(|| panic!("{label} feed must decode"));
            assert!(
                !decoded.contains(&id),
                "{label} feed duplicates another feed"
            );
            decoded.push(id);
        }
    }

    /// The placeholder mint list MUST decode (it is compared against parsed
    /// token mints in `mainnet_safety_violations`); a malformed entry would
    /// silently disable the guard for that mint.
    #[test]
    fn placeholder_mints_decode() {
        for hex_str in PLACEHOLDER_MINT_HEXES {
            assert!(
                Pubkey::from_str(hex_str).is_ok(),
                "placeholder mint {hex_str} must be a valid pubkey hex"
            );
        }
    }

    fn token(label: &str, mint_hex: &str) -> TokenConfig {
        TokenConfig {
            label: label.to_string(),
            mint: Pubkey::from_str(mint_hex).unwrap(),
            decimals: 8,
        }
    }

    fn cfg_with(network: Network, use_faucet: bool, tokens: Vec<TokenConfig>) -> DeployConfig {
        DeployConfig {
            network,
            arch_rpc_url: "http://127.0.0.1:1".to_string(),
            program_key_path: "k".to_string(),
            oracle_key_path: "k".to_string(),
            deployer_key_path: "k".to_string(),
            admin_key_path: "k".to_string(),
            program_elf_path: "p".to_string(),
            oracle_elf_path: "o".to_string(),
            admin: None,
            fee_receiver: None,
            protocol_fee_share_bps: 5000,
            tokens,
            market_pairs: Vec::new(),
            lending_market_fee_bps: 100,
            market_params: MarketParams::default(),
            use_faucet,
            output_path: "out.json".to_string(),
        }
    }

    // A non-placeholder (real-looking) mint hex, distinct from the testnet placeholders.
    const REAL_MINT_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    #[test]
    fn mainnet_guard_flags_faucet_and_placeholder_mints() {
        let cfg = cfg_with(
            Network::Mainnet,
            true,
            vec![
                token("BTC", PLACEHOLDER_MINT_HEXES[0]),
                token("USDC", REAL_MINT_HEX),
            ],
        );
        let v = cfg.mainnet_safety_violations();
        // 1 faucet violation + 1 placeholder-mint violation (BTC); USDC is real.
        assert_eq!(v.len(), 2, "violations: {v:?}");
        assert!(v.iter().any(|m| m.contains("USE_FAUCET")));
        assert!(v
            .iter()
            .any(|m| m.contains("BTC") && m.contains("PLACEHOLDER")));
    }

    #[test]
    fn mainnet_guard_passes_with_real_mints_and_no_faucet() {
        let cfg = cfg_with(Network::Mainnet, false, vec![token("USDC", REAL_MINT_HEX)]);
        assert!(cfg.mainnet_safety_violations().is_empty());
    }

    #[test]
    fn non_mainnet_is_never_flagged() {
        // Even with faucet + placeholder mints, testnet/localnet are unaffected.
        let cfg = cfg_with(
            Network::Testnet,
            true,
            vec![token("BTC", PLACEHOLDER_MINT_HEXES[0])],
        );
        assert!(cfg.mainnet_safety_violations().is_empty());
    }
}
