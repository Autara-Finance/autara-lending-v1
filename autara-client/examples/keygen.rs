//! Standalone key generation for sacred (fund-distribution) keypairs.
//!
//! SECURITY MODEL: run this yourself on a machine you trust. The 32-byte
//! secp256k1 secret is written ONLY to the file at `KEYGEN_OUT` (created with
//! mode 0600, never overwriting an existing file) and is NEVER printed to
//! stdout. Only public data is printed: the Arch pubkey (hex) and the
//! Bitcoin P2TR address for the chosen network. The file format is the
//! 64-hex-char secret expected by `arch_sdk::with_secret_key_file`, so the
//! key works with this repo's tooling and arch-cli.
//!
//! Usage:
//!   KEYGEN_NETWORK=mainnet|testnet4|testnet|regtest (default: testnet4)
//!   KEYGEN_OUT=./sacred-treasury.key (required)
//!   cargo run -p autara-client --example keygen

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

use anyhow::{anyhow, bail, Context, Result};
use arch_sdk::arch_program::bitcoin::Network;
use arch_sdk::{generate_new_keypair, with_secret_key_file};

fn main() -> Result<()> {
    let network_str = std::env::var("KEYGEN_NETWORK").unwrap_or_else(|_| "testnet4".to_string());
    let network = match network_str.to_lowercase().as_str() {
        "mainnet" | "bitcoin" => Network::Bitcoin,
        "testnet4" => Network::Testnet4,
        "testnet" => Network::Testnet,
        "regtest" => Network::Regtest,
        other => bail!("unknown KEYGEN_NETWORK '{other}' (use mainnet|testnet4|testnet|regtest)"),
    };
    let out_path = std::env::var("KEYGEN_OUT").context(
        "set KEYGEN_OUT to the output key file path, e.g. KEYGEN_OUT=./sacred-treasury.key",
    )?;

    let (keypair, pubkey, btc_address) = generate_new_keypair(network);
    let secret_hex = keypair.secret_key().display_secret().to_string();

    // create_new(true) refuses to clobber an existing key file; the file is
    // born with mode 0600 so the secret is never world-readable.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&out_path)
        .with_context(|| format!("cannot create '{out_path}' (refusing to overwrite)"))?;
    file.write_all(secret_hex.as_bytes())?;
    file.sync_all()?;
    drop(file);

    // Read-back check: the repo's standard loader must round-trip to the same
    // pubkey, proving the written file is usable.
    let (_reloaded, reloaded_pubkey) =
        with_secret_key_file(&out_path).map_err(|e| anyhow!("readback of {out_path}: {e}"))?;
    if reloaded_pubkey != pubkey {
        bail!("readback pubkey mismatch -- do not use this key file");
    }

    println!("network:          {network_str}");
    println!("secret key file:  {out_path} (mode 0600, 64-hex secret; NEVER share or print it)");
    println!("arch pubkey:      {}", hex::encode(pubkey.serialize()));
    println!("bitcoin address:  {btc_address} (P2TR)");
    println!();
    println!("REMINDER: for mainnet treasury keys, run this on a trusted (ideally offline)");
    println!("machine and back up the key file securely. Consider multisig for large funds.");
    Ok(())
}
