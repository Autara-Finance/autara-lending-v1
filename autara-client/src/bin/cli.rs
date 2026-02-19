use std::{str::FromStr, time::Duration};

use anyhow::{anyhow, Context, Result};
use arch_sdk::{
    arch_program::{bitcoin::Network, pubkey::Pubkey},
    with_secret_key_file,
};
use autara_client::{
    client::{client_with_signer::AutaraFullClientWithSigner, read::AutaraReadClient},
    config::{autara_oracle_stage_program_id, autara_stage_program_id, ArchConfig},
};
use clap::{Parser, Subcommand};
use tracing_subscriber::{util::SubscriberInitExt, EnvFilter};

#[derive(Parser)]
#[command(name = "autara-cli")]
#[command(about = "CLI to interact with the Autara Lending protocol", long_about = None)]
struct Cli {
    /// Arch node URL
    #[arg(long, default_value = "https://rpc.testnet.arch.network")]
    arch_node: String,

    /// Path to the signer key file. Can also be set via AUTARA_SIGNER_KEY env var.
    #[arg(long)]
    signer: Option<String>,

    /// Network to use (regtest, testnet, mainnet)
    #[arg(long, default_value = "regtest")]
    network: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Read operations
    #[command(subcommand)]
    Read(ReadCommands),

    /// Transaction operations
    #[command(subcommand)]
    Tx(TxCommands),
}

#[derive(Subcommand)]
enum ReadCommands {
    /// List all markets
    Markets,

    /// Get market details
    Market {
        /// Market pubkey
        #[arg(long)]
        market: String,
    },

    /// Get user positions for a specific authority
    Positions {
        /// Authority pubkey (defaults to signer)
        #[arg(long)]
        authority: Option<String>,
    },

    /// Get supply position for a market
    SupplyPosition {
        /// Market pubkey
        #[arg(long)]
        market: String,

        /// Authority pubkey (defaults to signer)
        #[arg(long)]
        authority: Option<String>,
    },

    /// Get borrow position health for a market
    BorrowHealth {
        /// Market pubkey
        #[arg(long)]
        market: String,

        /// Authority pubkey (defaults to signer)
        #[arg(long)]
        authority: Option<String>,
    },

    /// Get global config
    GlobalConfig,
}

#[derive(Subcommand)]
enum TxCommands {
    /// Supply assets to a market
    Supply {
        /// Market pubkey
        #[arg(long)]
        market: String,

        /// Amount in atoms (base units)
        #[arg(long)]
        amount: u64,
    },

    /// Withdraw supply from a market
    WithdrawSupply {
        /// Market pubkey
        #[arg(long)]
        market: String,

        /// Amount in atoms (None = withdraw all)
        #[arg(long)]
        amount: Option<u64>,
    },

    /// Deposit collateral to a market
    DepositCollateral {
        /// Market pubkey
        #[arg(long)]
        market: String,

        /// Amount in atoms
        #[arg(long)]
        amount: u64,
    },

    /// Withdraw collateral from a market
    WithdrawCollateral {
        /// Market pubkey
        #[arg(long)]
        market: String,

        /// Amount in atoms (None = withdraw all)
        #[arg(long)]
        amount: Option<u64>,
    },

    /// Borrow assets from a market
    Borrow {
        /// Market pubkey
        #[arg(long)]
        market: String,

        /// Amount in atoms
        #[arg(long)]
        amount: u64,
    },

    /// Repay borrowed assets
    Repay {
        /// Market pubkey
        #[arg(long)]
        market: String,

        /// Amount in atoms (None = repay all)
        #[arg(long)]
        amount: Option<u64>,
    },

    /// Liquidate an unhealthy position
    Liquidate {
        /// Market pubkey
        #[arg(long)]
        market: String,

        /// Borrow position pubkey to liquidate
        #[arg(long)]
        position: String,

        /// Max borrowed atoms to repay
        #[arg(long)]
        max_repay: Option<u64>,

        /// Min collateral atoms to receive
        #[arg(long)]
        min_collateral: Option<u64>,
    },

    /// Redeem curator fees
    RedeemCuratorFees {
        /// Market pubkey
        #[arg(long)]
        market: String,
    },

    /// Redeem protocol fees
    RedeemProtocolFees {
        /// Market pubkey
        #[arg(long)]
        market: String,
    },

    /// Donate supply to a market
    DonateSupply {
        /// Market pubkey
        #[arg(long)]
        market: String,

        /// Amount in atoms
        #[arg(long)]
        amount: u64,
    },
}

fn parse_network(network: &str) -> Result<Network> {
    match network.to_lowercase().as_str() {
        "regtest" => Ok(Network::Regtest),
        "testnet" => Ok(Network::Testnet),
        "mainnet" | "bitcoin" => Ok(Network::Bitcoin),
        _ => anyhow::bail!(
            "Invalid network: {}. Use regtest, testnet, or mainnet",
            network
        ),
    }
}

fn parse_pubkey(s: &str) -> Result<Pubkey> {
    // Try hex decode first (64 chars = 32 bytes)
    if s.len() == 64 {
        let bytes: [u8; 32] = hex::decode(s)
            .context("Invalid hex pubkey")?
            .try_into()
            .map_err(|_| anyhow!("Invalid pubkey length"))?;
        return Ok(Pubkey::from(bytes));
    }
    // Try base58
    Pubkey::from_str(s).map_err(|e| anyhow!("Invalid pubkey format: {}", e))
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::INFO.into())
        .from_env_lossy();
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(filter)
        .finish()
        .init();

    let cli = Cli::parse();

    let signer_path = cli
        .signer
        .or_else(|| std::env::var("AUTARA_SIGNER_KEY").ok());

    // Setup config
    let config = ArchConfig {
        arch_node_url: cli.arch_node.clone(),
        bitcoin_node_endpoint: String::new(),
        bitcoin_node_password: String::new(),
        bitcoin_node_username: String::new(),
    };

    let network = parse_network(&cli.network)?;
    let arch_client = config.arch_rpc_client();

    // Load signer keypair
    let signer = if let Some(ref path) = signer_path {
        with_secret_key_file(path)
            .context("Failed to load signer key")?
            .0
    } else {
        // Use default stage admin key for development
        autara_client::config::autara_stage_admin()
    };

    let signer_pubkey = Pubkey::from_slice(&signer.x_only_public_key().0.serialize());
    tracing::info!("Using signer: {:?}", signer_pubkey);

    // Create client
    let mut client = AutaraFullClientWithSigner::new_simple(
        arch_client,
        network,
        autara_stage_program_id(),
        autara_oracle_stage_program_id(),
        signer,
    );

    // Load state
    tracing::info!("Loading protocol state...");
    tokio::time::timeout(Duration::from_secs(60), client.full_reload())
        .await
        .context("could not load autara state with RPC")??;
    tracing::info!("Loaded protocol state...");
    match cli.command {
        Commands::Read(read_cmd) => handle_read_command(&client, read_cmd).await,
        Commands::Tx(tx_cmd) => handle_tx_command(&client, tx_cmd).await,
    }
}

async fn handle_read_command(
    client: &AutaraFullClientWithSigner<
        autara_client::client::single_thread_client::AutaraReadClientImpl,
    >,
    cmd: ReadCommands,
) -> Result<()> {
    match cmd {
        ReadCommands::Markets => {
            println!("=== Markets ===");
            let mut count = 0;
            for (pubkey, market) in client.read_client().all_markets() {
                count += 1;
                let m = market.market();
                println!("\nMarket: {:?}", pubkey);
                println!("  Supply Token: {:?}", m.supply_token_info().mint);
                println!("  Supply Decimals: {}", m.supply_token_info().decimals);
                println!("  Collateral Token: {:?}", m.collateral_token_info().mint);
                println!(
                    "  Collateral Decimals: {}",
                    m.collateral_token_info().decimals
                );
                println!("  Max LTV: {}", m.config().ltv_config().max_ltv);
                println!("  Unhealthy LTV: {}", m.config().ltv_config().unhealthy_ltv);
                println!(
                    "  Liquidation Bonus: {}",
                    m.config().ltv_config().liquidation_bonus
                );
                println!(
                    "  Max Utilisation Rate: {}",
                    m.config().max_utilisation_rate()
                );
            }
            if count == 0 {
                println!("No markets found");
            } else {
                println!("\nTotal markets: {}", count);
            }
        }

        ReadCommands::Market { market } => {
            let market_key = parse_pubkey(&market)?;
            let market_wrapper = client.read_client().get_market(&market_key);
            if let Some(mw) = market_wrapper {
                let m = mw.market();
                println!("=== Market Details ===");
                println!("Pubkey: {:?}", market_key);
                println!("\nSupply Vault:");
                println!("  Token: {:?}", m.supply_token_info().mint);
                println!("  Decimals: {}", m.supply_token_info().decimals);
                println!("\nCollateral Vault:");
                println!("  Token: {:?}", m.collateral_token_info().mint);
                println!("  Decimals: {}", m.collateral_token_info().decimals);
                println!("\nRisk Config:");
                println!("  Max LTV: {}", m.config().ltv_config().max_ltv);
                println!("  Unhealthy LTV: {}", m.config().ltv_config().unhealthy_ltv);
                println!(
                    "  Liquidation Bonus: {}",
                    m.config().ltv_config().liquidation_bonus
                );
                println!("  Max Utilisation: {}", m.config().max_utilisation_rate());
                println!("  Curator: {:?}", m.config().curator());
            } else {
                println!("Market not found: {:?}", market_key);
            }
        }

        ReadCommands::Positions { authority } => {
            let auth = if let Some(ref auth_str) = authority {
                parse_pubkey(auth_str)?
            } else {
                *client.signer_pubkey()
            };
            let positions = client.read_client().user_positions(&auth);

            println!("=== User Positions for {:?} ===", auth);

            println!("\n--- Supply Positions ---");
            if positions.supply_positions.is_empty() {
                println!("  No supply positions");
            } else {
                for pos in &positions.supply_positions {
                    println!("  Market: {:?}", pos.supply_position.market());
                    println!("    Shares: {}", pos.supply_position.shares());
                    println!("    Owned Atoms: {}", pos.owned_atoms);
                    println!();
                }
            }

            println!("--- Borrow Positions ---");
            if positions.borrow_positions.is_empty() {
                println!("  No borrow positions");
            } else {
                for pos in &positions.borrow_positions {
                    println!("  Market: {:?}", pos.borrow_position.market());
                    println!(
                        "    Collateral: {} atoms",
                        pos.borrow_position.collateral_deposited_atoms()
                    );
                    println!(
                        "    Borrowed Shares: {}",
                        pos.borrow_position.borrowed_shares()
                    );
                    println!("    LTV: {}", pos.health.ltv);
                    println!("    Borrow Value: {}", pos.health.borrow_value);
                    println!("    Collateral Value: {}", pos.health.collateral_value);
                    println!();
                }
            }
        }

        ReadCommands::SupplyPosition { market, authority } => {
            let market_key = parse_pubkey(&market)?;
            let auth = if let Some(ref auth_str) = authority {
                parse_pubkey(auth_str)?
            } else {
                *client.signer_pubkey()
            };

            let (pda, position) = client.read_client().get_supply_position(&market_key, &auth);
            println!("=== Supply Position ===");
            println!("PDA: {:?}", pda);
            if let Some(pos) = position {
                println!("Market: {:?}", pos.market());
                println!("Authority: {:?}", pos.authority());
                println!("Shares: {}", pos.shares());

                if let Some(market_wrapper) = client.read_client().get_market(&market_key) {
                    if let Ok(atoms) = market_wrapper.market().supply_position_info(&pos) {
                        println!("Owned Atoms: {}", atoms);
                    }
                }
            } else {
                println!("No supply position found");
            }
        }

        ReadCommands::BorrowHealth { market, authority } => {
            let market_key = parse_pubkey(&market)?;
            let auth = if let Some(ref auth_str) = authority {
                parse_pubkey(auth_str)?
            } else {
                *client.signer_pubkey()
            };

            match client
                .read_client()
                .get_borrow_position_health(&market_key, &auth)
            {
                Ok(health) => {
                    println!("=== Borrow Position Health ===");
                    println!("Market: {:?}", market_key);
                    println!("Authority: {:?}", auth);
                    println!("LTV: {}", health.ltv);
                    println!("Borrowed Atoms: {}", health.borrowed_atoms);
                    println!("Collateral Atoms: {}", health.collateral_atoms);
                    println!("Borrow Value: {}", health.borrow_value);
                    println!("Collateral Value: {}", health.collateral_value);
                }
                Err(e) => {
                    println!("Could not get borrow position health: {}", e);
                }
            }
        }

        ReadCommands::GlobalConfig => {
            if let Some(config) = client.read_client().get_global_config() {
                println!("=== Global Config ===");
                println!("Admin: {:?}", config.admin());
                println!("Fee Receiver: {:?}", config.fee_receiver());
                println!(
                    "Protocol Fee Share: {} bps",
                    config.protocol_fee_share_in_bps()
                );
            } else {
                println!("Global config not found");
            }
        }
    }
    Ok(())
}

async fn handle_tx_command(
    client: &AutaraFullClientWithSigner<
        autara_client::client::single_thread_client::AutaraReadClientImpl,
    >,
    cmd: TxCommands,
) -> Result<()> {
    match cmd {
        TxCommands::Supply { market, amount } => {
            let market_key = parse_pubkey(&market)?;
            println!("Supplying {} atoms to market {:?}...", amount, market_key);
            let events = client.supply(&market_key, amount).await?;
            println!("Supply successful!");
            println!("Events: {:#?}", events);
        }

        TxCommands::WithdrawSupply { market, amount } => {
            let market_key = parse_pubkey(&market)?;
            println!(
                "Withdrawing {:?} atoms from market {:?}...",
                amount, market_key
            );
            let events = client.withdraw_supply(&market_key, amount).await?;
            println!("Withdraw successful!");
            println!("Events: {:#?}", events);
        }

        TxCommands::DepositCollateral { market, amount } => {
            let market_key = parse_pubkey(&market)?;
            println!(
                "Depositing {} collateral atoms to market {:?}...",
                amount, market_key
            );
            let events = client.deposit_collateral(&market_key, amount).await?;
            println!("Deposit collateral successful!");
            println!("Events: {:#?}", events);
        }

        TxCommands::WithdrawCollateral { market, amount } => {
            let market_key = parse_pubkey(&market)?;
            println!(
                "Withdrawing {:?} collateral atoms from market {:?}...",
                amount, market_key
            );
            let events = client.withdraw_collateral(&market_key, amount).await?;
            println!("Withdraw collateral successful!");
            println!("Events: {:#?}", events);
        }

        TxCommands::Borrow { market, amount } => {
            let market_key = parse_pubkey(&market)?;
            println!("Borrowing {} atoms from market {:?}...", amount, market_key);
            let events = client.borrow(&market_key, amount).await?;
            println!("Borrow successful!");
            println!("Events: {:#?}", events);
        }

        TxCommands::Repay { market, amount } => {
            let market_key = parse_pubkey(&market)?;
            println!("Repaying {:?} atoms to market {:?}...", amount, market_key);
            let events = client.repay(&market_key, amount).await?;
            println!("Repay successful!");
            println!("Events: {:#?}", events);
        }

        TxCommands::Liquidate {
            market,
            position,
            max_repay,
            min_collateral,
        } => {
            let market_key = parse_pubkey(&market)?;
            let position_key = parse_pubkey(&position)?;
            println!(
                "Liquidating position {:?} in market {:?}...",
                position_key, market_key
            );
            let events = client
                .liquidate(&market_key, &position_key, max_repay, min_collateral, None)
                .await?;
            println!("Liquidation successful!");
            println!("Events: {:#?}", events);
        }

        TxCommands::RedeemCuratorFees { market } => {
            let market_key = parse_pubkey(&market)?;
            println!("Redeeming curator fees from market {:?}...", market_key);
            let events = client.reedeem_curator_fees(&market_key).await?;
            println!("Redeem curator fees successful!");
            println!("Events: {:#?}", events);
        }

        TxCommands::RedeemProtocolFees { market } => {
            let market_key = parse_pubkey(&market)?;
            println!("Redeeming protocol fees from market {:?}...", market_key);
            let events = client.reedeem_protocol_fees(&market_key).await?;
            println!("Redeem protocol fees successful!");
            println!("Events: {:#?}", events);
        }

        TxCommands::DonateSupply { market, amount } => {
            let market_key = parse_pubkey(&market)?;
            println!("Donating {} atoms to market {:?}...", amount, market_key);
            let events = client.donate_supply(&market_key, amount).await?;
            println!("Donate supply successful!");
            println!("Events: {:#?}", events);
        }
    }
    Ok(())
}
