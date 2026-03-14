use anyhow::{Context, Result};
use arch_sdk::arch_program::pubkey::Pubkey;
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
    /// Whirlpools config address (hex). If omitted, uses the default.
    pub whirlpools_config: Option<String>,
    /// Polling interval in seconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
}

fn default_poll_interval() -> u64 {
    5
}

pub fn parse_hex_pubkey(hex_str: &str) -> Result<Pubkey> {
    let bytes = hex::decode(hex_str).context("invalid hex for pubkey")?;
    Ok(Pubkey::from_slice(&bytes))
}
