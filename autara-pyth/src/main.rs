use arch_program::bitcoin::Network;
use arch_sdk::{generate_new_keypair, with_secret_key_file, AsyncArchRpcClient, Config};
use autara_pyth::fetch_and_push_feeds;
use clap::Parser;

#[derive(clap::Parser, Debug)]
struct Args {
    #[clap(long, default_value = "http://localhost:9002")]
    rpc: String,
    #[clap(long, default_value = "regtest")]
    network: Network,
    #[clap(
        long,
        default_value = "1728b8a70a5502e610b7c77320f3efcbbbab0032c7e66e949d518232045cc185"
    )]
    program_id: String,
    #[clap(
        long,
        value_delimiter = ',',
        default_value = DEFAULT_FEEDS
    )]
    feeds: Vec<String>,
    /// Path to a hex secret-key file for the push signer. When set, the pusher
    /// uses this (pre-funded) key and skips faucet funding — required on
    /// mainnet, which has no faucet. When unset, a throwaway key is generated
    /// and funded via the faucet (testnet/localnet only).
    #[clap(long)]
    signer: Option<String>,
}

const DEFAULT_FEEDS:&str = "0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43,0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a";

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

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::Level::INFO.into())
                .from_env_lossy(),
        )
        .init();
    let config = make_config(&args.rpc, args.network);
    let client = AsyncArchRpcClient::new(&config);
    let authority_keypair = match &args.signer {
        // Pre-funded signer: mainnet has no faucet, so the key must already hold funds.
        Some(path) => {
            with_secret_key_file(path)
                .unwrap_or_else(|e| panic!("failed to load signer key from {path}: {e}"))
                .0
        }
        // ponytail: no --signer => throwaway key funded by the faucet (testnet/localnet only).
        None => {
            let (keypair, _, _) = generate_new_keypair(args.network);
            client
                .create_and_fund_account_with_faucet(&keypair)
                .await
                .expect("faucet funding failed (mainnet has no faucet — pass --signer)");
            keypair
        }
    };

    let oracle_program_id =
        arch_program::pubkey::Pubkey::from_slice(&hex::decode(&args.program_id).unwrap());
    fetch_and_push_feeds(
        &client,
        &oracle_program_id,
        &authority_keypair,
        &args.feeds,
        args.network,
    )
    .await
}
