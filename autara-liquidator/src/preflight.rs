use anyhow::Result;
use apl_token::state::Account as TokenAccount;
use arch_sdk::arch_program::program_pack::Pack;
use arch_sdk::arch_program::pubkey::Pubkey;
use arch_sdk::AsyncArchRpcClient;

/// Read lamport balance for a pubkey (0 if account missing).
pub async fn lamport_balance(rpc: &AsyncArchRpcClient, pubkey: Pubkey) -> u64 {
    match rpc.read_account_info(pubkey).await {
        Ok(info) => info.lamports,
        Err(_) => 0,
    }
}

/// Read SPL/APL token amount for an ATA (0 if missing / unpack fails).
pub async fn token_amount(rpc: &AsyncArchRpcClient, ata: Pubkey) -> Result<u64> {
    match rpc.read_account_info(ata).await {
        Ok(info) => {
            let account = TokenAccount::unpack(&info.data)
                .map_err(|e| anyhow::anyhow!("unpack token account {ata}: {e}"))?;
            Ok(account.amount)
        }
        Err(_) => Ok(0),
    }
}
