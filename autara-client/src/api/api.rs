use jsonrpsee::{core::RpcResult, core::SubscriptionResult, proc_macros::rpc};

use crate::{
    api::{market::FullMarket, types::*},
    client::read::UserPositionItem,
};

#[rpc(server, client)]
pub trait AutaraServerApi {
    /// Initialize a user account with faucet and airdrop
    #[method(name = "create_account")]
    async fn create_account(&self, request: UserParams) -> RpcResult<TransactionToSignResponse>;

    /// Initialize a user account with faucet and airdrop
    #[method(name = "initialize")]
    async fn initialize(&self, request: UserParams) -> RpcResult<()>;

    /// Get all market IDs
    #[method(name = "get_all_market_ids")]
    async fn get_all_market_ids(&self) -> RpcResult<GetAllMarketsResponse>;

    /// Get a single market by ID
    #[method(name = "get_market_by_id")]
    async fn get_market_by_id(&self, request: GetMarketRequest) -> RpcResult<Option<FullMarket>>;

    /// Get all markets streamed one by one over WebSocket (one-shot, not a live feed)
    #[subscription(name = "get_all_markets_streamed" => "market", unsubscribe = "unsubscribe_all_markets_streamed", item = FullMarket)]
    async fn get_all_markets_streamed(&self) -> SubscriptionResult;

    /// Get all user positions streamed one by one over WebSocket (one-shot, not a live feed)
    #[subscription(name = "get_all_user_positions_streamed" => "user_position", unsubscribe = "unsubscribe_all_user_positions_streamed", item = UserPositionItem)]
    async fn get_all_user_positions_streamed(&self) -> SubscriptionResult;

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
