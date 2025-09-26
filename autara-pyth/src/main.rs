use arch_program::bitcoin::Network;
use arch_sdk::{generate_new_keypair, AsyncArchRpcClient};
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
}

const DEFAULT_FEEDS:&str = "0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43,0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a";

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
    let client = AsyncArchRpcClient::new(&args.rpc);
    let (authority_keypair, _, _) = generate_new_keypair(args.network);

    AsyncArchRpcClient::new(&args.rpc)
        .create_and_fund_account_with_faucet(&authority_keypair, Network::Regtest)
        .await
        .unwrap();

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
