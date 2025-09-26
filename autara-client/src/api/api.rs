use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use crate::api::{market::FullMarket, types::*};

#[rpc(server, client)]
pub trait AutaraServerApi {
    /// Initialize a user account with faucet and airdrop
    #[method(name = "initialize")]
    async fn initialize(&self, request: UserParams) -> RpcResult<()>;

    /// Get all available markets
    #[method(name = "get_all_markets")]
    async fn get_all_markets(&self) -> RpcResult<Vec<FullMarket>>;

    /// Get user positions for a specific user
    #[method(name = "get_user_positions")]
    async fn get_user_positions(
        &self,
        request: GetUserPositionsRequest,
    ) -> RpcResult<GetUserPositionsResponse>;

    /// Get user positions for a specific user
    #[method(name = "get_user_balances")]
    async fn get_user_balances(&self, request: UserParams) -> RpcResult<GetUserBalancesResponse>;

    /// Build an Autara transaction
    #[method(name = "build_autara_transaction")]
    async fn build_autara_transaction(
        &self,
        request: AutaraTransactionRequest,
    ) -> RpcResult<TransactionToSignResponse>;

    /// Send a signed transaction
    #[method(name = "send_transaction")]
    async fn send_transaction(
        &self,
        request: SendTransactionRequest,
    ) -> RpcResult<SendTransactionResponse>;
}
