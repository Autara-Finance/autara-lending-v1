pub mod api;
pub mod client;

/// Re-export of the co-signer SDK so consumers use the exact same crate
/// version (and therefore the exact same `ArchSignerT` trait / `ArchMessage`
/// types) as the [`client::tx_builder::TransactionToSign::sign_with`] seam.
pub use cosigner_client;
pub mod config;
pub mod filter;
pub mod prometheus;
pub mod rpc_ext;
pub mod test;
pub mod token_mint;
