use std::collections::HashSet;

use anyhow::{Context, Result};
use arch_sdk::arch_program::{bitcoin::key::Keypair, bitcoin::Network, pubkey::Pubkey};
use clap::Parser;
use serde::Deserialize;

#[derive(Parser)]
#[command(name = "autara-liquidator")]
#[command(about = "Liquidator bot for the Autara Lending protocol")]
pub struct Args {
    /// Path to the config file
    #[arg(long, default_value = "liquidator-config.json")]
    pub config: String,
}

#[derive(Debug, Deserialize)]
pub struct LiquidatorConfig {
    /// RPC URL for the Arch node
    pub rpc_url: String,
    /// Autara lending program ID (hex)
    pub autara_program_id: String,
    /// Path to the liquidator keypair file (dedicated key — not admin/curator/pusher)
    pub liquidator_keypair: String,
    /// Signing network: regtest | testnet | testnet4 | mainnet
    #[serde(default = "default_network")]
    pub network: String,
    /// When true (default), scan and log only — never submit txs.
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
    /// Polling interval in seconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// Slippage tolerance applied to expected collateral out → min_collateral.
    /// Basis points (100 = 1%). Default 100.
    #[serde(default = "default_slippage_bps")]
    pub slippage_bps: u16,
    /// Minimum lamports required on the liquidator before submitting a live tx.
    #[serde(default = "default_min_lamports")]
    pub min_lamports: u64,
    /// Halt the process after this many consecutive live-tx failures (0 = disabled).
    #[serde(default = "default_max_consecutive_failures")]
    pub max_consecutive_failures: u32,
    /// Optional set of token addresses (hex) to restrict scanning to.
    #[serde(default)]
    pub restrict_tokens: Vec<String>,
}

impl LiquidatorConfig {
    pub fn load_keypair(&self) -> Result<(Keypair, Pubkey)> {
        arch_sdk::with_secret_key_file(&self.liquidator_keypair)
            .context("failed to load liquidator keypair")
    }

    pub fn bitcoin_network(&self) -> Result<Network> {
        match self.network.to_lowercase().as_str() {
            "regtest" => Ok(Network::Regtest),
            "testnet" | "testnet4" => Ok(Network::Testnet4),
            "mainnet" | "bitcoin" => Ok(Network::Bitcoin),
            other => anyhow::bail!(
                "unknown network '{other}' (use regtest|testnet|testnet4|mainnet)"
            ),
        }
    }
}

fn default_poll_interval() -> u64 {
    5
}

fn default_dry_run() -> bool {
    true
}

fn default_network() -> String {
    "testnet".to_string()
}

fn default_slippage_bps() -> u16 {
    100
}

fn default_min_lamports() -> u64 {
    50_000
}

fn default_max_consecutive_failures() -> u32 {
    5
}

pub fn parse_hex_pubkey(hex_str: &str) -> Result<Pubkey> {
    let bytes = hex::decode(hex_str).context("invalid hex for pubkey")?;
    Ok(Pubkey::from_slice(&bytes))
}

/// Apply slippage_bps haircut to an expected collateral amount.
pub fn min_collateral_after_slippage(expected: u64, slippage_bps: u16) -> u64 {
    let bps = u64::from(slippage_bps).min(10_000);
    expected.saturating_mul(10_000 - bps) / 10_000
}

/// Optional token filter that restricts which markets/tokens the liquidator considers.
#[derive(Debug, Clone)]
pub struct TokenFilter {
    tokens: HashSet<Pubkey>,
}

impl TokenFilter {
    pub fn from_config(hex_list: &[String]) -> Result<Self> {
        let tokens = hex_list
            .iter()
            .map(|hex| parse_hex_pubkey(hex))
            .collect::<Result<_>>()?;
        Ok(Self { tokens })
    }

    pub fn is_active(&self) -> bool {
        !self.tokens.is_empty()
    }

    pub fn allows_market(&self, supply_mint: &Pubkey, collateral_mint: &Pubkey) -> bool {
        self.tokens.is_empty()
            || self.tokens.contains(supply_mint)
            || self.tokens.contains(collateral_mint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slippage_haircut() {
        assert_eq!(min_collateral_after_slippage(1_000_000, 100), 990_000);
        assert_eq!(min_collateral_after_slippage(100, 10_000), 0);
        assert_eq!(min_collateral_after_slippage(100, 0), 100);
    }

    #[test]
    fn token_filter_empty_allows_all() {
        let f = TokenFilter::from_config(&[]).unwrap();
        let a = Pubkey::from_slice(&[1; 32]);
        let b = Pubkey::from_slice(&[2; 32]);
        assert!(f.allows_market(&a, &b));
        assert!(!f.is_active());
    }

    #[test]
    fn token_filter_requires_match() {
        let a = Pubkey::from_slice(&[1; 32]);
        let b = Pubkey::from_slice(&[2; 32]);
        let c = Pubkey::from_slice(&[3; 32]);
        let f = TokenFilter {
            tokens: [a].into_iter().collect(),
        };
        assert!(f.allows_market(&a, &b));
        assert!(f.allows_market(&c, &a));
        assert!(!f.allows_market(&b, &c));
    }
}
