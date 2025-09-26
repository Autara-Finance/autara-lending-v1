use crate::api::serde_helper::*;
use crate::client::read::UserPositions;
use arch_sdk::{arch_program::pubkey::Pubkey, Signature};
use autara_lib::{
    event::AutaraEvents,
    ixs::{
        BorrowDepositAplInstruction, CreateMarketInstruction, LiquidateInstruction,
        UpdateConfigInstruction, UpdateGlobalConfigInstruction, WithdrawRepayAplInstruction,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "params")]
pub enum AutaraTransactionRequest {
    CreateMarket(CreateMarketParams),
    Supply(MarketUserAmountParams),
    Withdraw(MarketUserOptionalAmountParams),
    DepositCollateral(MarketUserAmountParams),
    WithdrawCollateral(MarketUserOptionalAmountParams),
    Borrow(MarketUserAmountParams),
    Repay(MarketUserOptionalAmountParams),
    UpdateConfig(UpdateConfigParams),
    UpdateGlobalConfig(UpdateGlobalConfigParams),
    LiquidatePosition(LiquidatePositionParams),
    RedeemCuratorFees(MarketUserAmountParams),
    RedeemProtocolFees(MarketUserAmountParams),
    BorrowAndDeposit(BorrowAndDepositRequest),
    WithdrawAndRepay(WithdrawAndRepayRequest),
}

impl AutaraTransactionRequest {
    pub fn build_tx_tracing_params(&self) -> BuildTxTracingParams {
        match self {
            AutaraTransactionRequest::CreateMarket(params) => BuildTxTracingParams {
                instruction: "CreateMarket",
                user: &params.payer,
                market_id: None,
            },
            AutaraTransactionRequest::Supply(params) => BuildTxTracingParams {
                instruction: "Supply",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
            AutaraTransactionRequest::Withdraw(params) => BuildTxTracingParams {
                instruction: "Withdraw",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
            AutaraTransactionRequest::DepositCollateral(params) => BuildTxTracingParams {
                instruction: "DepositCollateral",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
            AutaraTransactionRequest::WithdrawCollateral(params) => BuildTxTracingParams {
                instruction: "WithdrawCollateral",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
            AutaraTransactionRequest::Borrow(params) => BuildTxTracingParams {
                instruction: "Borrow",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
            AutaraTransactionRequest::Repay(params) => BuildTxTracingParams {
                instruction: "Repay",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
            AutaraTransactionRequest::UpdateConfig(params) => BuildTxTracingParams {
                instruction: "UpdateConfig",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
            AutaraTransactionRequest::UpdateGlobalConfig(params) => BuildTxTracingParams {
                instruction: "UpdateGlobalConfig",
                user: &params.user,
                market_id: None,
            },
            AutaraTransactionRequest::LiquidatePosition(params) => BuildTxTracingParams {
                instruction: "LiquidatePosition",
                user: &params.user,
                market_id: Some(&params.market),
            },
            AutaraTransactionRequest::RedeemCuratorFees(params) => BuildTxTracingParams {
                instruction: "RedeemCuratorFees",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
            AutaraTransactionRequest::RedeemProtocolFees(params) => BuildTxTracingParams {
                instruction: "RedeemProtocolFees",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
            AutaraTransactionRequest::BorrowAndDeposit(params) => BuildTxTracingParams {
                instruction: "BorrowAndDeposit",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
            AutaraTransactionRequest::WithdrawAndRepay(params) => BuildTxTracingParams {
                instruction: "WithdrawAndRepay",
                user: &params.user,
                market_id: Some(&params.market_id),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketUserParams {
    #[serde(with = "serde_pubkey")]
    pub market_id: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub user: Pubkey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketUserAmountParams {
    #[serde(with = "serde_pubkey")]
    pub market_id: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub user: Pubkey,
    #[serde(with = "serde_from_str")]
    pub amount: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketUserOptionalAmountParams {
    #[serde(with = "serde_pubkey")]
    pub market_id: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub user: Pubkey,
    /// If None, withdraw/repay the maximum possible amount
    #[serde(default, with = "serde_from_optional_str")]
    pub amount: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConfigParams {
    #[serde(with = "serde_pubkey")]
    pub market_id: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub user: Pubkey,
    pub config: UpdateConfigInstruction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGlobalConfigParams {
    #[serde(with = "serde_pubkey")]
    pub user: Pubkey,
    pub config: UpdateGlobalConfigInstruction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidatePositionParams {
    #[serde(with = "serde_pubkey")]
    pub user: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub liquidatee_position: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub market: Pubkey,
    pub liquidate: LiquidateInstruction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMarketParams {
    #[serde(with = "serde_pubkey")]
    pub payer: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub curator: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub supply_mint: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub collateral_mint: Pubkey,
    pub params: CreateMarketInstruction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserParams {
    #[serde(with = "serde_pubkey")]
    pub user: Pubkey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionToSignResponse {
    pub transaction_to_sign: Vec<u8>,
    pub message: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTransactionRequest {
    pub signature: Signature,
    pub message: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTransactionResponse {
    pub signature: Signature,
    pub events: AutaraEvents,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUserPositionsRequest {
    #[serde(with = "serde_pubkey")]
    pub user: Pubkey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUserPositionsResponse {
    pub positions: UserPositions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUserBalancesResponse {
    pub balances: Vec<UserBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserBalance {
    pub mint: Pubkey,
    pub amount: u64,
    pub ui_amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorrowAndDepositRequest {
    #[serde(with = "serde_pubkey")]
    pub user: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub market_id: Pubkey,
    pub params: BorrowDepositAplInstruction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawAndRepayRequest {
    #[serde(with = "serde_pubkey")]
    pub user: Pubkey,
    #[serde(with = "serde_pubkey")]
    pub market_id: Pubkey,
    pub params: WithdrawRepayAplInstruction,
}

pub struct BuildTxTracingParams<'a> {
    pub instruction: &'static str,
    pub user: &'a Pubkey,
    pub market_id: Option<&'a Pubkey>,
}

impl<'a> BuildTxTracingParams<'a> {
    pub fn trace(&self) {
        if let Some(market_id) = self.market_id {
            tracing::info!(
                instruction = self.instruction,
                user = %self.user,
                market_id = %market_id,
                "Received build tx request"
            );
        } else {
            tracing::info!(instruction = self.instruction, user = %self.user,  "Received build tx request");
        }
    }
}
