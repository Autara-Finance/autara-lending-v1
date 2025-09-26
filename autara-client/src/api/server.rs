use std::{ops::Deref, sync::Arc};

use arch_sdk::{arch_program::sanitized::ArchMessage, AsyncArchRpcClient, RuntimeTransaction};
use jsonrpsee::{
    core::RpcResult,
    types::{error::INTERNAL_ERROR_CODE, ErrorObjectOwned},
    RpcModule,
};

use crate::{
    api::{api::AutaraServerApiServer, market::FullMarket, types::*},
    client::{
        blockhash_cache::BlockhashCache, client_without_signer::AutaraFullClientWithoutSigner,
        read::AutaraReadClient, shared_autara_state::AutaraSharedState,
    },
    rpc_ext::ArchAsyncRpcExt,
    test::AutaraTestEnv,
};

pub struct AutataServerContext {
    pub client: AutaraFullClientWithoutSigner<Arc<AutaraSharedState>>,
    pub env: AutaraTestEnv,
}

impl Deref for AutataServerContext {
    type Target = AutaraFullClientWithoutSigner<Arc<AutaraSharedState>>;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

#[jsonrpsee::core::async_trait]
impl AutaraServerApiServer for AutataServerContext {
    async fn initialize(&self, request: UserParams) -> RpcResult<()> {
        let client = self.async_arch_client();
        client
            .call_method_with_params_raw::<_>("create_account_with_faucet", request.user)
            .await
            .internal("Failed to create account with faucet")?;
        client
            .call_method_with_params_raw::<_>("request_airdrop", request.user)
            .await
            .internal("Failed to request airdrop")?;
        self.env
            .supply_minter
            .credit_to(&request.user, 100_000_000_000)
            .await
            .internal("Failed to credit supply tokens")?;
        self.env
            .collateral_minter
            .credit_to(&request.user, 100_000_000_000)
            .await
            .internal("Failed to credit collateral tokens")?;
        Ok(())
    }

    async fn get_all_markets(&self) -> RpcResult<Vec<FullMarket>> {
        let markets = self
            .read_client()
            .all_markets()
            .map(|(id, market)| FullMarket::new_from_market(id, market.owned()))
            .collect::<Vec<_>>();
        Ok(markets)
    }

    async fn get_user_positions(
        &self,
        request: GetUserPositionsRequest,
    ) -> RpcResult<GetUserPositionsResponse> {
        let positions = self.read_client().user_positions(&request.user);
        Ok(GetUserPositionsResponse { positions })
    }

    async fn build_autara_transaction(
        &self,
        request: AutaraTransactionRequest,
    ) -> RpcResult<TransactionToSignResponse> {
        request.build_tx_tracing_params().trace();
        let tx = match request {
            AutaraTransactionRequest::Supply(request) => {
                self.tx_builder(&request.user)
                    .supply(&request.market_id, request.amount)
                    .await
            }
            AutaraTransactionRequest::DepositCollateral(request) => {
                self.tx_builder(&request.user)
                    .deposit_collateral(&request.market_id, request.amount)
                    .await
            }
            AutaraTransactionRequest::Withdraw(request) => {
                self.tx_builder(&request.user)
                    .withdraw_supply(&request.market_id, request.amount)
                    .await
            }
            AutaraTransactionRequest::WithdrawCollateral(request) => {
                self.tx_builder(&request.user)
                    .withdraw_collateral(&request.market_id, request.amount)
                    .await
            }
            AutaraTransactionRequest::Borrow(request) => {
                self.tx_builder(&request.user)
                    .borrow(&request.market_id, request.amount)
                    .await
            }
            AutaraTransactionRequest::Repay(request) => {
                self.tx_builder(&request.user)
                    .repay(&request.market_id, request.amount)
                    .await
            }
            AutaraTransactionRequest::UpdateConfig(request) => {
                self.tx_builder(&request.user)
                    .update_config(&request.market_id, request.config)
                    .await
            }
            AutaraTransactionRequest::UpdateGlobalConfig(request) => {
                self.tx_builder(&request.user)
                    .update_global_config(request.config)
                    .await
            }
            AutaraTransactionRequest::LiquidatePosition(request) => {
                self.tx_builder(&request.user)
                    .liquidate(
                        &request.market,
                        &request.liquidatee_position,
                        Some(request.liquidate.max_borrowed_atoms_to_repay),
                        Some(request.liquidate.min_collateral_atoms_to_receive),
                        request.liquidate.ix_callback,
                    )
                    .await
            }
            AutaraTransactionRequest::RedeemCuratorFees(request) => {
                self.tx_builder(&request.user)
                    .redeem_curator_fees(&request.market_id)
                    .await
            }
            AutaraTransactionRequest::RedeemProtocolFees(request) => {
                self.tx_builder(&request.user)
                    .redeem_protocol_fees(&request.market_id)
                    .await
            }
            AutaraTransactionRequest::CreateMarket(request) => self
                .tx_builder(&request.payer)
                .create_market(
                    request.curator,
                    request.payer,
                    request.params,
                    request.supply_mint,
                    request.collateral_mint,
                )
                .await
                .map(|x| x.1),
            AutaraTransactionRequest::WithdrawAndRepay(request) => {
                self.tx_builder(&request.user)
                    .withdraw_repay(&request.market_id, request.params)
                    .await
            }
            AutaraTransactionRequest::BorrowAndDeposit(request) => {
                self.tx_builder(&request.user)
                    .borrow_deposit(&request.market_id, request.params)
                    .await
            }
        }
        .internal("Failed to build transaction")?;
        Ok(TransactionToSignResponse {
            transaction_to_sign: tx.message_hash,
            message: tx.message.serialize(),
        })
    }

    async fn send_transaction(
        &self,
        request: SendTransactionRequest,
    ) -> RpcResult<SendTransactionResponse> {
        let message =
            ArchMessage::deserialize(&request.message).internal("Failed to deserialize message")?;
        let events = self
            .tx_broadcast()
            .broadcast_transaction(RuntimeTransaction {
                version: 0,
                signatures: vec![request.signature.clone()],
                message,
            })
            .await
            .internal("Failed to broadcast transaction")?;
        Ok(SendTransactionResponse {
            signature: request.signature,
            events,
        })
    }

    async fn get_user_balances(&self, request: UserParams) -> RpcResult<GetUserBalancesResponse> {
        let balances = self
            .async_arch_client()
            .get_all_balances(&request.user)
            .await
            .internal("Failed to get user balances")?;
        Ok(GetUserBalancesResponse {
            balances: balances
                .into_iter()
                .filter_map(|(mint, amount)| {
                    self.client
                        .read_client()
                        .get_token_decimals(&mint)
                        .map(|token_decimals| UserBalance {
                            mint,
                            amount,
                            ui_amount: (amount as f64 / 10f64.powi(token_decimals as i32))
                                .to_string(),
                        })
                })
                .collect(),
        })
    }
}

pub async fn build_autara_server(
    read_client: Arc<AutaraSharedState>,
    test_env: AutaraTestEnv,
    arch_client: AsyncArchRpcClient,
) -> anyhow::Result<RpcModule<AutataServerContext>> {
    let context = AutataServerContext {
        client: AutaraFullClientWithoutSigner::new(
            read_client,
            arch_client.clone(),
            BlockhashCache::new(arch_client, None).await?,
        ),
        env: test_env,
    };
    Ok(context.into_rpc())
}

trait ErrorMapper<T> {
    fn internal(self, context: &str) -> RpcResult<T>;
}

impl<T, E: std::fmt::Display> ErrorMapper<T> for Result<T, E> {
    fn internal(self, context: &str) -> RpcResult<T> {
        self.map_err(|e| {
            ErrorObjectOwned::owned(
                INTERNAL_ERROR_CODE,
                format!("context: {context}, error: {e}"),
                None::<()>,
            )
        })
    }
}
