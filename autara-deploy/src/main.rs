//! Autara (lending) deploy tool.
//!
//! Everything is env-driven (see [`DeployConfig::from_env`]). Run with
//! `--dry-run` (or `DRY_RUN=1`) to print every derived address and preflight
//! check without touching the chain. A real run deploys the `autara-program`
//! and `autara-oracle` ELFs, creates the global config, and writes a
//! `DeploymentArtifact` (addresses + tx ids only).
//!
//! Phase 1 is TESTNET-FIRST. The structure mirrors the CLAMM `clamm-deploy`
//! crate so the two stay consistent.

mod artifact;
mod config;
mod rpc;
mod steps;

use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use arch_program::pubkey::Pubkey;
use arch_sdk::{AsyncArchRpcClient, Config as ArchConfig};
use clap::{Args, Parser, Subcommand};
use sha2::{Digest, Sha256};

use artifact::{DeploymentArtifact, TokenRecord};
use config::{DeployConfig, Network};
use rpc::{load_keypair, RpcContext};

#[derive(Parser, Debug)]
#[command(
    name = "autara-deploy",
    about = "Deploy Autara lending to Arch Network"
)]
struct Cli {
    /// Print all derived addresses and preflight checks, then exit without
    /// sending any transaction.
    #[arg(long)]
    dry_run: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Read-only: print the native lamport balance of an owner (defaults to the
    /// admin keypair, then the deployer keypair). Sends NO transaction.
    CheckBalance(CheckBalanceArgs),
    /// Faucet-fund the deployer + admin keypairs for the configured network.
    /// Sends ONLY faucet airdrops — no deploy/init/program transactions. Uses
    /// the same env-driven key paths as the deploy flow.
    Fund(FundArgs),
}

#[derive(Args, Debug)]
struct FundArgs {
    /// Number of faucet airdrops to request for the deployer (program ELFs are
    /// large, so several airdrops are typically needed to cover the upload).
    #[arg(long, default_value_t = 5)]
    deployer_rounds: u32,

    /// Number of faucet airdrops to request for the admin (payer/signer for
    /// create_global_config).
    #[arg(long, default_value_t = 1)]
    admin_rounds: u32,
}

#[derive(Args, Debug)]
struct CheckBalanceArgs {
    /// Owner pubkey (hex). Defaults to the ADMIN_KEY_PATH (then DEPLOYER_KEY_PATH) keypair.
    #[arg(long)]
    owner: Option<String>,

    /// Arch RPC url. Defaults to ARCH_RPC_URL or the network default.
    #[arg(long)]
    rpc_url: Option<String>,

    /// Network (localnet|testnet). Defaults to NETWORK or localnet.
    #[arg(long)]
    network: Option<String>,
}

/// Read a boolean env var, defaulting to `default` when unset/empty.
fn env_flag(key: &str, default: bool) -> bool {
    match std::env::var(key).ok().filter(|v| !v.trim().is_empty()) {
        None => default,
        Some(v) => matches!(
            v.trim().to_lowercase().as_str(),
            "1" | "true" | "yes" | "y" | "on"
        ),
    }
}

/// SHA-256 of a file's bytes, hex-encoded.
fn sha256_file(path: &str) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("reading ELF at {path}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

/// Best-effort short git commit of the working tree.
fn git_short_commit() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Resolve the network from an explicit flag, the `NETWORK` env var, or the
/// localnet default.
fn resolve_network(explicit: Option<String>) -> Result<Network> {
    explicit
        .or_else(|| {
            std::env::var("NETWORK")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| "localnet".to_string())
        .parse()
}

/// `check-balance` subcommand: read-only lamport-balance report.
async fn run_check_balance(args: CheckBalanceArgs) -> Result<()> {
    let network = resolve_network(args.network)?;
    let rpc_url = args
        .rpc_url
        .or_else(|| {
            std::env::var("ARCH_RPC_URL")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .map(Ok)
        .unwrap_or_else(|| network.default_rpc_url())?;

    let owner = match args.owner {
        Some(o) => Pubkey::from_str(o.trim())
            .map_err(|e| anyhow!("invalid --owner pubkey '{o}': {e:?}"))?,
        None => {
            let path = std::env::var("ADMIN_KEY_PATH")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .or_else(|| {
                    std::env::var("DEPLOYER_KEY_PATH")
                        .ok()
                        .filter(|v| !v.trim().is_empty())
                })
                .context("--owner not given and ADMIN_KEY_PATH/DEPLOYER_KEY_PATH are unset")?;
            load_keypair(&path)?.1
        }
    };

    let config = ArchConfig {
        node_endpoint: String::new(),
        node_username: String::new(),
        node_password: String::new(),
        network: network.bitcoin_network()?,
        arch_node_url: rpc_url.clone(),
        titan_url: String::new(),
    };
    let rpc = AsyncArchRpcClient::new(&config);

    println!("network:  {}", network.as_str());
    println!("rpc_url:  {rpc_url}");
    println!("owner:    {owner}");
    match rpc.read_account_info(owner).await {
        Ok(info) => println!("balance:  {} lamports", info.lamports),
        Err(_) => println!("balance:  (account not found)"),
    }
    Ok(())
}

/// `fund` subcommand: faucet-fund the deployer + admin keypairs. Sends ONLY
/// faucet airdrops (no deploy/init transactions), so it is safe to run while
/// preparing a fresh deploy. Funding brand-new addresses is non-destructive.
async fn run_fund(args: FundArgs) -> Result<()> {
    let cfg = DeployConfig::from_env()?;
    if !cfg.network.has_faucet() {
        bail!(
            "network '{}' has no faucet; fund the accounts manually",
            cfg.network.as_str()
        );
    }

    let (deployer_kp, deployer_pubkey) = load_keypair(&cfg.deployer_key_path)?;
    let (admin_kp, admin_pubkey) = load_keypair(&cfg.admin_key_path)?;

    // The admin keypair doubles as the RpcContext payer; `fund_with_faucet`
    // only uses the shared RPC client, so the payer role is irrelevant here.
    let ctx = RpcContext::new(cfg.arch_config()?, admin_kp, admin_pubkey);

    println!("network:  {}", cfg.network.as_str());
    println!("rpc_url:  {}", cfg.arch_rpc_url);
    println!("deployer: {deployer_pubkey}");
    println!("admin:    {admin_pubkey}");

    for i in 0..args.deployer_rounds {
        ctx.fund_with_faucet(&deployer_kp).await?;
        println!(
            "faucet -> deployer (round {}/{})",
            i + 1,
            args.deployer_rounds
        );
    }
    for i in 0..args.admin_rounds {
        ctx.fund_with_faucet(&admin_kp).await?;
        println!("faucet -> admin    (round {}/{})", i + 1, args.admin_rounds);
    }

    match ctx.balance(deployer_pubkey).await {
        Some(b) => println!("deployer_balance: {b} lamports"),
        None => println!("deployer_balance: (account not found)"),
    }
    match ctx.balance(admin_pubkey).await {
        Some(b) => println!("admin_balance:    {b} lamports"),
        None => println!("admin_balance:    (account not found)"),
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // A single-thread runtime drives the async reads/sends. The synchronous
    // `ProgramDeployer` is invoked OUTSIDE `block_on` (it drives its own
    // blocking client), mirroring the repo's existing deploy binary.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    if let Some(command) = cli.command {
        return match command {
            Command::CheckBalance(args) => rt.block_on(run_check_balance(args)),
            Command::Fund(args) => rt.block_on(run_fund(args)),
        };
    }

    let dry_run = cli.dry_run || env_flag("DRY_RUN", false);
    let cfg = DeployConfig::from_env()?;

    // Load keypairs. Keypair is Copy, so the same key can serve multiple roles.
    let (program_kp, program_pubkey) = load_keypair(&cfg.program_key_path)?;
    let (oracle_kp, oracle_pubkey) = load_keypair(&cfg.oracle_key_path)?;
    let (deployer_kp, deployer_pubkey) = load_keypair(&cfg.deployer_key_path)?;
    let (admin_kp, admin_default_pubkey) = load_keypair(&cfg.admin_key_path)?;

    let admin = cfg.admin.unwrap_or(admin_default_pubkey);
    let fee_receiver = cfg.fee_receiver.unwrap_or(admin);

    // The global-config payer/signer is the admin keypair.
    let ctx = RpcContext::new(cfg.arch_config()?, admin_kp, admin_default_pubkey);

    // Step gating (default-on).
    let step_deploy_program = env_flag("STEP_DEPLOY_PROGRAM", true);
    let step_deploy_oracle = env_flag("STEP_DEPLOY_ORACLE", true);
    let step_init_config = env_flag("STEP_INIT_CONFIG", true);

    // ----- Preflight -----
    println!("== Autara deploy preflight ({}) ==", cfg.network.as_str());
    println!("arch_rpc_url:      {}", cfg.arch_rpc_url);
    println!("program_id:        {program_pubkey}");
    println!("oracle_id:         {oracle_pubkey}");
    println!("deployer:          {deployer_pubkey}");
    println!("admin:             {admin}");
    println!("fee_receiver:      {fee_receiver}");
    println!("protocol_fee_bps:  {}", cfg.protocol_fee_share_bps);

    // The on-chain `autara-program` derives the global-config PDA and runs its
    // ownership checks against a COMPILED-IN id (`autara_program::id()`), not
    // the runtime program id. The client, however, derives PDAs from the
    // deployed program key. If the two disagree, create_global_config / market
    // instructions target the wrong PDA and fail. Guard it here — fatal on a
    // real run, a loud warning in dry-run so the preview still completes.
    // (The oracle is position-independent: it uses the runtime program_id only,
    // so it needs no such guard.)
    let compiled_id = autara_program::id();
    if program_pubkey != compiled_id {
        eprintln!("WARNING: program keypair pubkey != compiled autara_program::id()");
        eprintln!("  program keypair      : {program_pubkey}");
        eprintln!("  autara_program::id() : {compiled_id}");
        eprintln!("  Fix: deploy with the keypair whose pubkey == autara_program::id(),");
        eprintln!("       or update id() in programs/autara-program/src/lib.rs and rebuild.");
        if !dry_run {
            bail!(
                "program id mismatch: create_global_config/market would target the wrong PDA \
                 (compiled {compiled_id}, deploying {program_pubkey})"
            );
        }
    } else {
        println!("program_id_guard:  ok (matches autara_program::id())");
    }

    let (global_config_pda, _) = autara_lib::pda::find_global_config_pda(&program_pubkey);
    println!("global_config_pda: {global_config_pda}");

    for t in &cfg.tokens {
        println!(
            "token {:<6}     mint={} decimals={}",
            t.label, t.mint, t.decimals
        );
    }

    for (label, path) in [
        ("program_elf", &cfg.program_elf_path),
        ("oracle_elf", &cfg.oracle_elf_path),
    ] {
        if Path::new(path).exists() {
            println!("{label:<14}   {path} (present)");
        } else {
            println!("{label:<14}   {path} (MISSING — run the SBF build)");
        }
    }

    rt.block_on(async {
        match ctx.rpc_reachable().await {
            Ok(()) => println!("rpc_reachable:     yes"),
            Err(e) => println!("rpc_reachable:     NO ({e})"),
        }
        match ctx.balance(deployer_pubkey).await {
            Some(b) => println!("deployer_balance:  {b} lamports"),
            None => println!("deployer_balance:  (account not found)"),
        }
        match ctx.balance(admin).await {
            Some(b) => println!("admin_balance:     {b} lamports"),
            None => println!("admin_balance:     (account not found)"),
        }
        if step_deploy_program {
            let live = ctx.is_executable(program_pubkey).await;
            println!(
                "program_on_chain:  {}",
                if live { "executable" } else { "not deployed" }
            );
        }
        if step_deploy_oracle {
            let live = ctx.is_executable(oracle_pubkey).await;
            println!(
                "oracle_on_chain:   {}",
                if live { "executable" } else { "not deployed" }
            );
        }
    });

    if dry_run {
        println!("\n[dry-run] No transactions sent. Nothing written on-chain.");
        return Ok(());
    }

    // ----- Real run -----
    let mut artifact = DeploymentArtifact {
        network: cfg.network.as_str().to_string(),
        arch_rpc_url: cfg.arch_rpc_url.clone(),
        deployed_at_unix: DeploymentArtifact::now_unix(),
        program_id: program_pubkey.to_string(),
        oracle_id: oracle_pubkey.to_string(),
        build_commit: git_short_commit(),
        program_elf_path: cfg.program_elf_path.clone(),
        program_elf_sha256: None,
        oracle_elf_path: cfg.oracle_elf_path.clone(),
        oracle_elf_sha256: None,
        deployer: deployer_pubkey.to_string(),
        admin: admin.to_string(),
        fee_receiver: fee_receiver.to_string(),
        protocol_fee_share_bps: cfg.protocol_fee_share_bps,
        global_config: None,
        tokens: cfg
            .tokens
            .iter()
            .map(|t| TokenRecord {
                label: t.label.clone(),
                mint: t.mint.to_string(),
                decimals: t.decimals,
            })
            .collect(),
        transactions: Vec::new(),
    };

    // Faucet-fund the deployer + admin (large ELFs => several airdrops).
    if cfg.use_faucet {
        rt.block_on(async {
            for _ in 0..5 {
                ctx.fund_with_faucet(&deployer_kp).await?;
            }
            ctx.fund_with_faucet(&admin_kp).await?;
            Ok::<_, anyhow::Error>(())
        })?;
        println!("faucet: funded deployer + admin");
    }

    // Deploy the programs (SYNCHRONOUS ProgramDeployer — outside the runtime).
    if step_deploy_program {
        artifact.program_elf_sha256 = Some(sha256_file(&cfg.program_elf_path)?);
        ctx.deploy_program(
            "autara_program".to_string(),
            program_kp,
            deployer_kp,
            cfg.program_elf_path.clone(),
        )?;
        println!("deployed autara-program {program_pubkey}");
    } else if Path::new(&cfg.program_elf_path).exists() {
        artifact.program_elf_sha256 = sha256_file(&cfg.program_elf_path).ok();
    }

    if step_deploy_oracle {
        artifact.oracle_elf_sha256 = Some(sha256_file(&cfg.oracle_elf_path)?);
        ctx.deploy_program(
            "autara_oracle".to_string(),
            oracle_kp,
            deployer_kp,
            cfg.oracle_elf_path.clone(),
        )?;
        println!("deployed autara-oracle {oracle_pubkey}");
    } else if Path::new(&cfg.oracle_elf_path).exists() {
        artifact.oracle_elf_sha256 = sha256_file(&cfg.oracle_elf_path).ok();
    }

    // Create the global config.
    if step_init_config {
        let pda = rt.block_on(steps::create_global_config(
            &ctx,
            program_pubkey,
            admin,
            fee_receiver,
            cfg.protocol_fee_share_bps,
            &mut artifact,
        ))?;
        println!("global config {pda}");
    }

    artifact.write(&cfg.output_path)?;
    println!("\n== Deploy complete ==");
    println!("artifact: {}", cfg.output_path);
    println!("transactions recorded: {}", artifact.transactions.len());

    Ok(())
}
