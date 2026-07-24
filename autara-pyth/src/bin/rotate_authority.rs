//! One-shot feed-authority rotation for the autara-oracle program.
//!
//! For every live feed PDA, sends `UpdateAuthority(new_authority)` signed by
//! the CURRENT (old) pusher key, then re-reads the account to verify the
//! handover. Resumable: feeds already pinned to the new authority are
//! skipped, so the tool can be re-run after partial failures. Exits non-zero
//! when any feed fails to rotate.
//!
//! Requires the upgraded oracle program (with the UpdateAuthority
//! instruction) to be deployed first — against the old program this
//! instruction fails with InvalidInstructionData and no state changes.
//!
//! Example (mainnet cutover, step c of the runbook):
//!
//! ```text
//! cargo run -p autara-pyth --bin rotate_authority -- \
//!     --rpc https://<mainnet-rpc> --network bitcoin \
//!     --program-id <oracle-program-id-hex> \
//!     --signer /path/to/old-pusher.key \
//!     --new-authority f399400270abc64302b837bd358e93ebf67b3ecf63627202f91d49a2df58c843
//! ```
//!
//! Omit `--signer` to sign with the environment-selected signer instead
//! (COSIGNER_*/ARCH_KEY_PATH, intent "rotate-authority") — the ROLLBACK path
//! once feeds are pinned to the co-signer key: the proxy signs the rotation
//! back to a local key given as `--new-authority`.

use anyhow::Context;
use arch_program::bitcoin::Network;
use arch_program::pubkey::Pubkey;
use arch_sdk::{ArchRpcClient, Config};
use autara_lib::oracle::pyth::PythPriceAccount;
use autara_oracle::update_authority_instruction;
use autara_pyth::{
    build_and_send_tx, get_pyth_account, parse_feed_id, BTC_FEED, ETH_FEED, USDC_FEED,
};
use cosigner_client::{ArchSigner, ArchSignerT};

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(long, default_value = "http://localhost:9002")]
    rpc: String,
    #[clap(long, default_value = "regtest")]
    network: Network,
    /// autara-oracle program id, hex.
    #[clap(long)]
    program_id: String,
    /// Path to the CURRENT (old) pusher key file — the key every live feed is
    /// pinned to today. Rotation transactions are signed locally with it.
    /// Omit to sign with the environment-selected signer instead
    /// (COSIGNER_*/ARCH_KEY_PATH — the rollback path once feeds are pinned
    /// to the co-signer key).
    #[clap(long)]
    signer: Option<String>,
    /// The new authority (the co-signer role's Arch pubkey), 32 bytes hex.
    #[clap(long)]
    new_authority: String,
    /// Feeds to rotate. Defaults to the BTC/USDC/ETH feeds the pusher serves.
    #[clap(long, value_delimiter = ',')]
    feeds: Option<Vec<String>>,
}

fn make_config(rpc: &str, network: Network) -> Config {
    Config {
        arch_node_url: rpc.to_string(),
        node_endpoint: String::new(),
        node_username: String::new(),
        node_password: String::new(),
        network,
        titan_url: String::new(),
    }
}

/// Reads the trailing authority pubkey out of a feed account. Anything that
/// is not the current (authority-carrying) layout is an error — legacy
/// feeds have nothing to rotate.
fn feed_authority(data: &[u8]) -> anyhow::Result<Pubkey> {
    anyhow::ensure!(
        data.len() == std::mem::size_of::<PythPriceAccount>(),
        "unexpected account size {} — not an authority-carrying pyth feed account",
        data.len()
    );
    Ok(Pubkey::from_slice(&data[data.len() - 32..]))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = <Args as clap::Parser>::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::Level::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let program_id = Pubkey::from_slice(
        &hex::decode(args.program_id.trim_start_matches("0x")).context("program id hex")?,
    );
    let new_authority: [u8; 32] = hex::decode(args.new_authority.trim_start_matches("0x"))
        .context("new authority hex")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("new authority must be 32 bytes"))?;
    let new_authority = Pubkey::from_slice(&new_authority);

    let signer: ArchSigner = match &args.signer {
        Some(path) => ArchSigner::local_from_key_file(path)
            .map_err(|e| anyhow::anyhow!("loading old pusher key: {e}"))?
            .with_network(args.network),
        None => ArchSigner::from_env()
            .map_err(|e| anyhow::anyhow!("no --signer given and no signer in environment: {e}"))?
            .with_network(args.network)
            .with_intent("rotate-authority"),
    };
    let old_pubkey = signer.pubkey();
    tracing::info!(
        "rotating authority {} -> {}",
        hex::encode(old_pubkey.serialize()),
        hex::encode(new_authority.serialize())
    );

    let client = ArchRpcClient::new(&make_config(&args.rpc, args.network));
    let feeds = args.feeds.unwrap_or_else(|| {
        vec![
            BTC_FEED.to_string(),
            USDC_FEED.to_string(),
            ETH_FEED.to_string(),
        ]
    });

    let mut rotated = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    for feed in &feeds {
        let result: anyhow::Result<bool> = async {
            let feed_id = parse_feed_id(feed)?;
            let pda = get_pyth_account(&program_id, feed_id);
            let info = client
                .read_account_info(pda)
                .await
                .with_context(|| format!("feed PDA {pda:?} not readable (feed not created?)"))?;
            let current = feed_authority(&info.data)?;
            if current == new_authority {
                tracing::info!("{feed}: already pinned to the new authority — skipping");
                return Ok(false);
            }
            let ix =
                update_authority_instruction(&program_id, feed_id, &old_pubkey, &new_authority);
            build_and_send_tx(&client, &signer, &[ix])
                .await
                .context("rotation transaction failed")?;
            // verify the handover landed before counting it
            let info = client
                .read_account_info(pda)
                .await
                .context("post-rotation re-read failed")?;
            let now = feed_authority(&info.data)?;
            anyhow::ensure!(
                now == new_authority,
                "post-rotation verification failed: authority is {}",
                hex::encode(now.serialize())
            );
            tracing::info!("{feed}: rotated and verified");
            Ok(true)
        }
        .await;
        match result {
            Ok(true) => rotated += 1,
            Ok(false) => skipped += 1,
            Err(err) => {
                tracing::error!("{feed}: {err:#}");
                failed += 1;
            }
        }
    }

    tracing::info!("done: {rotated} rotated, {skipped} skipped, {failed} failed");
    if failed > 0 {
        anyhow::bail!("{failed} feed(s) failed to rotate");
    }
    Ok(())
}
