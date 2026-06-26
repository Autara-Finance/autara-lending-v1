//! Individual, flag-gated deploy steps. Each step is a thin wrapper that builds
//! the relevant instruction(s) via `autara-lib` and sends them through the
//! shared [`RpcContext`], recording tx ids into the [`DeploymentArtifact`].

use anyhow::Result;
use arch_program::pubkey::Pubkey;

use crate::artifact::DeploymentArtifact;
use crate::rpc::RpcContext;

/// Create the protocol's global config PDA (admin + fee receiver + fee share).
///
/// Idempotent: if the global config already exists on-chain, the existing PDA
/// is returned and no new transaction is recorded.
pub async fn create_global_config(
    ctx: &RpcContext,
    autara_program_id: Pubkey,
    admin: Pubkey,
    fee_receiver: Pubkey,
    protocol_fee_share_bps: u16,
    artifact: &mut DeploymentArtifact,
) -> Result<Pubkey> {
    let payer = ctx.payer_pubkey();
    let (global_config_pda, ix) = autara_lib::ixs::create_global_config_ix(
        autara_program_id,
        payer,
        admin,
        fee_receiver,
        protocol_fee_share_bps,
    );

    match ctx.send(vec![ix], vec![]).await {
        Ok(txid) => {
            artifact.record_tx("create_global_config", txid);
        }
        Err(e) if e.to_string().contains("already exists") => {
            println!("global_config:     already exists ({global_config_pda}) — skipping");
        }
        Err(e) => return Err(e),
    }

    artifact.global_config = Some(global_config_pda.to_string());
    Ok(global_config_pda)
}
