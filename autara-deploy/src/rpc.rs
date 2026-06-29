//! Thin wrapper around the Arch SDK clients with the helpers the deploy flow
//! needs: preflight reachability/balance reads, faucet funding, program
//! deployment (via `ProgramDeployer`), and send-and-confirm for instructions.

use anyhow::{anyhow, bail, Result};
use arch_program::bitcoin::Network as BitcoinNetwork;
use arch_program::{bitcoin::key::Keypair, instruction::Instruction, pubkey::Pubkey};
use arch_sdk::{
    arch_program::sanitized::ArchMessage, build_and_sign_transaction, with_secret_key_file,
    AsyncArchRpcClient, Config, ProgramDeployer, Status,
};

/// Load a secp256k1 keypair from a file written by `arch-cli` (a hex secret-key
/// string or a JSON byte array). Returns the keypair and its on-chain `Pubkey`
/// (x-only serialization).
pub fn load_keypair(path: &str) -> Result<(Keypair, Pubkey)> {
    with_secret_key_file(path).map_err(|e| anyhow!("failed to load keypair from {path}: {e}"))
}

/// Holds the async RPC client (reads, sends) plus the SDK config used by the
/// synchronous `ProgramDeployer`.
pub struct RpcContext {
    pub rpc: AsyncArchRpcClient,
    config: Config,
    network: BitcoinNetwork,
    payer: Keypair,
    payer_pubkey: Pubkey,
}

impl RpcContext {
    pub fn new(config: Config, payer: Keypair, payer_pubkey: Pubkey) -> Self {
        Self {
            rpc: AsyncArchRpcClient::new(&config),
            network: config.network,
            config,
            payer,
            payer_pubkey,
        }
    }

    pub fn payer_pubkey(&self) -> Pubkey {
        self.payer_pubkey
    }

    /// Best-effort RPC reachability probe (read-only).
    pub async fn rpc_reachable(&self) -> Result<()> {
        self.rpc
            .get_best_block_hash()
            .await
            .map(|_| ())
            .map_err(|e| {
                anyhow!(
                    "Arch RPC at '{}' unreachable: {e}",
                    self.config.arch_node_url
                )
            })
    }

    /// Best-effort lamport balance for preflight reporting. Returns `None` if
    /// the account does not exist yet.
    pub async fn balance(&self, pubkey: Pubkey) -> Option<u64> {
        self.rpc
            .read_account_info(pubkey)
            .await
            .ok()
            .map(|a| a.lamports)
    }

    /// Read-only check: does an account exist on-chain? Used to make the token
    /// and market steps idempotent (mirrors the client's `account_exists`).
    pub async fn account_exists(&self, pubkey: Pubkey) -> bool {
        self.rpc.read_account_info(pubkey).await.is_ok()
    }

    /// Read-only check: is the program account present and `is_executable`?
    pub async fn is_executable(&self, program_pubkey: Pubkey) -> bool {
        matches!(
            self.rpc.read_account_info(program_pubkey).await,
            Ok(info) if info.is_executable
        )
    }

    /// Fund an account via the faucet (localnet/testnet). The program ELFs are
    /// large, so callers typically request several airdrops.
    pub async fn fund_with_faucet(&self, keypair: &Keypair) -> Result<()> {
        self.rpc
            .create_and_fund_account_with_faucet(keypair)
            .await
            .map_err(|e| anyhow!("faucet funding failed: {e}"))
    }

    /// Deploy (or resume the deploy of) a program ELF using the SDK's
    /// `ProgramDeployer`, mirroring the repo's existing deploy binary.
    ///
    /// `ProgramDeployer` is SYNCHRONOUS and drives its own blocking client, so
    /// this MUST be called from outside any active async runtime (the caller
    /// runs it between `block_on` sections). It is idempotent: an
    /// already-deployed program is treated as success.
    pub fn deploy_program(
        &self,
        program_name: String,
        program_kp: Keypair,
        authority_kp: Keypair,
        elf_path: String,
    ) -> Result<()> {
        match ProgramDeployer::new(&self.config).try_deploy_program(
            program_name,
            program_kp,
            authority_kp,
            &elf_path,
        ) {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("already exists") || msg.contains("already deployed") {
                    Ok(())
                } else {
                    Err(anyhow!("program deployment failed: {msg}"))
                }
            }
        }
    }

    /// Build, sign (extra signers + payer) and confirm a transaction. Returns
    /// the txid string on success.
    pub async fn send(
        &self,
        instructions: Vec<Instruction>,
        extra_signers: Vec<Keypair>,
    ) -> Result<String> {
        let blockhash = self
            .rpc
            .get_best_block_hash()
            .await
            .map_err(|e| anyhow!("get_best_block_hash failed: {e}"))?;

        let message = ArchMessage::new(&instructions, Some(self.payer_pubkey), blockhash);

        let mut signers = extra_signers;
        signers.push(self.payer);

        let tx = build_and_sign_transaction(message, signers, self.network)
            .map_err(|e| anyhow!("failed to build/sign transaction: {e}"))?;

        let txids = self
            .rpc
            .send_transactions(vec![tx])
            .await
            .map_err(|e| anyhow!("send_transactions failed: {e}"))?;
        let txid = txids
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("send_transactions returned no txid"))?;

        let processed = self
            .rpc
            .wait_for_processed_transactions(vec![txid])
            .await
            .map_err(|e| anyhow!("waiting for transaction failed: {e}"))?;
        let processed = processed
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no processed transaction returned"))?;

        match processed.status {
            Status::Processed => Ok(txid.to_string()),
            Status::Failed(e) => bail!(
                "transaction {txid} failed: {e}\nlogs:\n{}",
                processed.logs.join("\n")
            ),
            Status::Queued => bail!("transaction {txid} still queued after wait"),
        }
    }
}
