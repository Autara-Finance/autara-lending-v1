//! Env-driven deploy configuration for `autara-deploy`.
//!
//! Everything the deploy flow needs is parsed once from the environment by
//! [`DeployConfig::from_env`]. The only values that normally differ between
//! networks are `network`, `arch_rpc_url`, the `*_KEY_PATH` files, and the
//! token mints — mirroring the CLAMM deploy tool's convention.
//!
//! Phase 1 is TESTNET-FIRST: `localnet` and `testnet` are wired up; `mainnet`
//! is intentionally left unconfigured (the variant exists so it can be added
//! later without restructuring).

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
    /// Phase 2. The variant exists so mainnet can be wired up later, but the
    /// tool refuses to derive any mainnet defaults today.
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
    pub fn default_rpc_url(&self) -> Result<String> {
        match self {
            Network::Localnet => Ok("http://localhost:9002/".to_string()),
            Network::Testnet => Ok("https://rpc.testnet.arch.network".to_string()),
            Network::Mainnet => bail!("mainnet RPC is not configured yet (Phase 2)"),
        }
    }

    /// Bitcoin network used by `build_and_sign_transaction` / `ProgramDeployer`.
    /// Testnet uses `Testnet4`, matching the repo's existing testnet deploy.
    pub fn bitcoin_network(&self) -> Result<BitcoinNetwork> {
        match self {
            Network::Localnet => Ok(BitcoinNetwork::Regtest),
            Network::Testnet => Ok(BitcoinNetwork::Testnet4),
            Network::Mainnet => bail!("mainnet signing is not supported yet (Phase 2)"),
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
            other => bail!("unknown NETWORK '{other}' (expected localnet|testnet)"),
        }
    }
}

/// A token mint to record in the deployment artifact (and, later, to wire into
/// market-creation steps). Parsed from `TOKENS=LABEL:MINT_HEX:DECIMALS,...`.
#[derive(Debug, Clone)]
pub struct TokenConfig {
    pub label: String,
    pub mint: Pubkey,
    pub decimals: u8,
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

    /// Token mints to record (and, in a later phase, to build markets from).
    pub tokens: Vec<TokenConfig>,

    pub use_faucet: bool,
    pub output_path: String,
}

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

            use_faucet: env_bool("USE_FAUCET", network.has_faucet())?,
            output_path: env_or(
                "OUTPUT_PATH",
                &format!("deployments/{}.json", network.as_str()),
            ),
        };

        Ok(cfg)
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
