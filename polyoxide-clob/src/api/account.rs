use std::collections::HashMap;

use alloy::primitives::Address;
use polyoxide_core::{HttpClient, QueryBuilder};
use serde::{Deserialize, Serialize};

use crate::{
    account::{Credentials, Signer, Wallet},
    error::ClobError,
    request::{AuthMode, Request},
    types::OrderSide,
};

/// Account API namespace for account-related operations
#[derive(Clone)]
pub struct AccountApi {
    pub(crate) http_client: HttpClient,
    pub(crate) wallet: Wallet,
    pub(crate) credentials: Credentials,
    pub(crate) signer: Signer,
    pub(crate) chain_id: u64,
}

impl AccountApi {
    /// Get balance and allowance for a token
    pub fn balance_allowance(
        &self,
        token_id: impl Into<String>,
    ) -> Request<BalanceAllowanceResponse> {
        Request::get(
            self.http_client.clone(),
            "/balance-allowance",
            AuthMode::L2 {
                address: self.wallet.clone().address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
        .query("token_id", token_id.into())
    }

    pub fn usdc_balance(&self) -> Request<BalanceAllowanceResponse> {
        Request::get(
            self.http_client.clone(),
            "/balance-allowance",
            AuthMode::L2 {
                address: self.wallet.clone().address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
        .query("asset_type", "COLLATERAL")
        .query("signature_type", 1)
    }

    /// Update balance allowance for a token
    pub async fn update_balance_allowance(
        &self,
        token_id: impl Into<String>,
    ) -> Result<serde_json::Value, ClobError> {
        #[derive(Serialize)]
        struct Body {
            token_id: String,
        }

        Request::<serde_json::Value>::post(
            self.http_client.clone(),
            "/balance-allowance/update".to_string(),
            AuthMode::L2 {
                address: self.wallet.clone().address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
        .body(&Body {
            token_id: token_id.into(),
        })?
        .send()
        .await
    }

    /// Send a heartbeat to keep the session alive
    pub async fn heartbeat(&self) -> Result<serde_json::Value, ClobError> {
        Request::<serde_json::Value>::post(
            self.http_client.clone(),
            "/v1/heartbeats".to_string(),
            AuthMode::L2 {
                address: self.wallet.clone().address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        )
        .send()
        .await
    }

    /// Get builder trades with optional filtering
    pub fn builder_trades(&self) -> ListBuilderTrades {
        let request = Request::get(
            self.http_client.clone(),
            "/builder/trades",
            AuthMode::L2 {
                address: self.wallet.clone().address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        );
        ListBuilderTrades { request }
    }

    /// Get trades with optional filtering
    pub fn trades(&self) -> ListClobTrades {
        let request = Request::get(
            self.http_client.clone(),
            "/data/trades",
            AuthMode::L2 {
                address: self.wallet.clone().address(),
                credentials: self.credentials.clone(),
                signer: self.signer.clone(),
            },
            self.chain_id,
        );
        ListClobTrades { request }
    }
}

/// Request builder for listing CLOB trades with optional filters
pub struct ListClobTrades {
    request: Request<ListTradesResponse>,
}

impl ListClobTrades {
    /// Filter by specific trade ID
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.request = self.request.query("id", id.into());
        self
    }

    /// Filter by maker address
    pub fn maker_address(mut self, address: impl Into<String>) -> Self {
        self.request = self.request.query("maker_address", address.into());
        self
    }

    /// Filter by market (condition ID)
    pub fn market(mut self, condition_id: impl Into<String>) -> Self {
        self.request = self.request.query("market", condition_id.into());
        self
    }

    /// Filter by asset (token ID)
    pub fn asset_id(mut self, token_id: impl Into<String>) -> Self {
        self.request = self.request.query("asset_id", token_id.into());
        self
    }

    /// Filter trades before this timestamp
    pub fn before(mut self, timestamp: impl Into<String>) -> Self {
        self.request = self.request.query("before", timestamp.into());
        self
    }

    /// Filter trades after this timestamp
    pub fn after(mut self, timestamp: impl Into<String>) -> Self {
        self.request = self.request.query("after", timestamp.into());
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<ListTradesResponse, ClobError> {
        self.request.send().await
    }
}

/// Request builder for listing builder trades with optional filters
pub struct ListBuilderTrades {
    request: Request<ListBuilderTradesResponse>,
}

impl ListBuilderTrades {
    /// Filter trades after this cursor
    pub fn after(mut self, cursor: impl Into<String>) -> Self {
        self.request = self.request.query("after", cursor.into());
        self
    }

    /// Filter by maker address
    pub fn maker_address(mut self, address: impl Into<String>) -> Self {
        self.request = self.request.query("maker_address", address.into());
        self
    }

    /// Filter by market (condition ID)
    pub fn market(mut self, condition_id: impl Into<String>) -> Self {
        self.request = self.request.query("market", condition_id.into());
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<ListBuilderTradesResponse, ClobError> {
        self.request.send().await
    }
}

/// Trade information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub taker_order_id: String,
    pub market: String,
    pub asset_id: String,
    pub side: OrderSide,
    pub size: String,
    pub fee_rate_bps: String,
    pub price: String,
    pub status: String,
    pub match_time: String,
    #[serde(default)]
    pub last_update: Option<String>,
    pub outcome: String,
    #[serde(default)]
    pub bucket_index: Option<u32>,
    pub owner: Address,
    pub maker_address: Option<String>,
    #[serde(default)]
    pub maker_orders: Vec<MakerOrder>,
    pub transaction_hash: String,
    pub trader_side: Option<String>,
}

/// Individual maker order within a trade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakerOrder {
    pub order_id: String,
    pub owner: String,
    pub maker_address: String,
    pub matched_amount: String,
    pub price: String,
    pub fee_rate_bps: String,
    pub asset_id: String,
    pub outcome: String,
    pub side: OrderSide,
}

/// Paginated response from `GET /data/trades`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListTradesResponse {
    pub data: Vec<Trade>,
    pub next_cursor: Option<String>,
}

/// Builder trade from `GET /builder/trades`
///
/// Different from [`Trade`] — uses camelCase field names and has
/// builder-specific fields (`trade_type`, `builder`, `size_usdc`, `fee`, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuilderTrade {
    pub id: String,
    pub trade_type: String,
    pub taker_order_hash: String,
    pub builder: String,
    pub market: String,
    pub asset_id: String,
    pub side: String,
    pub size: String,
    pub size_usdc: String,
    pub price: String,
    pub status: String,
    pub outcome: String,
    pub outcome_index: u32,
    pub owner: String,
    pub maker: String,
    pub transaction_hash: String,
    pub match_time: String,
    #[serde(default)]
    pub bucket_index: Option<u32>,
    pub fee: String,
    pub fee_usdc: String,
    #[serde(rename = "err_msg")]
    pub err_msg: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Paginated response from `GET /builder/trades`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListBuilderTradesResponse {
    pub data: Vec<BuilderTrade>,
    pub next_cursor: Option<String>,
}

/// Balance and allowance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceAllowanceResponse {
    pub balance: String,
    pub allowances: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trade_deserialization() {
        let json = r#"{
            "id": "trade-123",
            "taker_order_id": "order-456",
            "market": "0xcondition",
            "asset_id": "0xtoken",
            "side": "BUY",
            "size": "100.5",
            "fee_rate_bps": "0",
            "price": "0.55",
            "status": "MATCHED",
            "match_time": "1700000000",
            "last_update": null,
            "outcome": "Yes",
            "bucket_index": null,
            "owner": "0x0000000000000000000000000000000000000001",
            "transaction_hash": "0xhash123"
        }"#;
        let trade: Trade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.id, "trade-123");
        assert_eq!(trade.side, OrderSide::Buy);
        assert_eq!(trade.price, "0.55");
        assert!(trade.last_update.is_none());
        assert!(trade.bucket_index.is_none());
        // New fields default to None/empty when absent
        assert!(trade.maker_address.is_none());
        assert!(trade.maker_orders.is_empty());
        assert!(trade.trader_side.is_none());
    }

    #[test]
    fn trade_with_optional_fields() {
        let json = r#"{
            "id": "t1",
            "taker_order_id": "o1",
            "market": "0xcond",
            "asset_id": "0xasset",
            "side": "SELL",
            "size": "50",
            "fee_rate_bps": "100",
            "price": "0.72",
            "status": "MATCHED",
            "match_time": "1700001000",
            "last_update": "1700002000",
            "outcome": "No",
            "bucket_index": 3,
            "owner": "0x0000000000000000000000000000000000000002",
            "maker_address": "0xmaker",
            "maker_orders": [{
                "order_id": "mo-1",
                "owner": "0xowner",
                "maker_address": "0xmaker",
                "matched_amount": "50",
                "price": "0.72",
                "fee_rate_bps": "100",
                "asset_id": "0xasset",
                "outcome": "No",
                "side": "BUY"
            }],
            "transaction_hash": "0xhash456",
            "trader_side": "TAKER"
        }"#;
        let trade: Trade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.side, OrderSide::Sell);
        assert_eq!(trade.last_update.as_deref(), Some("1700002000"));
        assert_eq!(trade.bucket_index, Some(3));
        assert_eq!(trade.maker_address.as_deref(), Some("0xmaker"));
        assert_eq!(trade.maker_orders.len(), 1);
        assert_eq!(trade.maker_orders[0].order_id, "mo-1");
        assert_eq!(trade.maker_orders[0].matched_amount, "50");
        assert_eq!(trade.maker_orders[0].side, OrderSide::Buy);
        assert_eq!(trade.trader_side.as_deref(), Some("TAKER"));
    }

    #[test]
    fn list_trades_response_deserializes() {
        let json = r#"{
            "data": [{
                "id": "t1",
                "taker_order_id": "o1",
                "market": "0xcond",
                "asset_id": "0xasset",
                "side": "BUY",
                "size": "100",
                "fee_rate_bps": "0",
                "price": "0.55",
                "status": "MATCHED",
                "match_time": "1700000000",
                "outcome": "Yes",
                "owner": "0x0000000000000000000000000000000000000001",
                "transaction_hash": "0xhash"
            }],
            "next_cursor": "abc123"
        }"#;
        let resp: ListTradesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].id, "t1");
        assert_eq!(resp.next_cursor.as_deref(), Some("abc123"));
    }

    #[test]
    fn list_trades_response_empty() {
        let json = r#"{"data": [], "next_cursor": null}"#;
        let resp: ListTradesResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
        assert!(resp.next_cursor.is_none());
    }

    #[test]
    fn builder_trade_deserialization() {
        let json = r#"{
            "id": "bt-1",
            "tradeType": "LIMIT",
            "takerOrderHash": "0xhash",
            "builder": "0xbuilder",
            "market": "0xcond",
            "assetId": "0xtoken",
            "side": "BUY",
            "size": "100",
            "sizeUsdc": "55.00",
            "price": "0.55",
            "status": "MATCHED",
            "outcome": "Yes",
            "outcomeIndex": 0,
            "owner": "0xowner",
            "maker": "0xmaker",
            "transactionHash": "0xtxhash",
            "matchTime": "1700000000",
            "bucketIndex": 5,
            "fee": "0.01",
            "feeUsdc": "0.55",
            "err_msg": null,
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-01-01T00:00:01Z"
        }"#;
        let bt: BuilderTrade = serde_json::from_str(json).unwrap();
        assert_eq!(bt.id, "bt-1");
        assert_eq!(bt.trade_type, "LIMIT");
        assert_eq!(bt.builder, "0xbuilder");
        assert_eq!(bt.size_usdc, "55.00");
        assert_eq!(bt.outcome_index, 0);
        assert_eq!(bt.bucket_index, Some(5));
        assert!(bt.err_msg.is_none());
        assert_eq!(bt.created_at.as_deref(), Some("2024-01-01T00:00:00Z"));
    }

    #[test]
    fn builder_trade_with_error() {
        let json = r#"{
            "id": "bt-2",
            "tradeType": "MARKET",
            "takerOrderHash": "0x",
            "builder": "0x",
            "market": "0x",
            "assetId": "0x",
            "side": "SELL",
            "size": "0",
            "sizeUsdc": "0",
            "price": "0",
            "status": "FAILED",
            "outcome": "No",
            "outcomeIndex": 1,
            "owner": "0x",
            "maker": "0x",
            "transactionHash": "0x",
            "matchTime": "0",
            "fee": "0",
            "feeUsdc": "0",
            "err_msg": "insufficient balance",
            "createdAt": null,
            "updatedAt": null
        }"#;
        let bt: BuilderTrade = serde_json::from_str(json).unwrap();
        assert_eq!(bt.err_msg.as_deref(), Some("insufficient balance"));
        assert!(bt.bucket_index.is_none());
        assert!(bt.created_at.is_none());
    }

    #[test]
    fn list_builder_trades_response_deserializes() {
        let json = r#"{
            "data": [{
                "id": "bt-1",
                "tradeType": "LIMIT",
                "takerOrderHash": "0x",
                "builder": "0x",
                "market": "0x",
                "assetId": "0x",
                "side": "BUY",
                "size": "100",
                "sizeUsdc": "55",
                "price": "0.55",
                "status": "MATCHED",
                "outcome": "Yes",
                "outcomeIndex": 0,
                "owner": "0x",
                "maker": "0x",
                "transactionHash": "0x",
                "matchTime": "0",
                "fee": "0",
                "feeUsdc": "0",
                "createdAt": null,
                "updatedAt": null
            }],
            "next_cursor": "cursor123"
        }"#;
        let resp: ListBuilderTradesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].id, "bt-1");
        assert_eq!(resp.next_cursor.as_deref(), Some("cursor123"));
    }

    #[test]
    fn balance_allowance_response_deserializes() {
        let json = r#"{"balance": "141171137", "allowances": {"0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E": "999999"}}"#;
        let resp: BalanceAllowanceResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.balance, "141171137");
        assert_eq!(
            resp.allowances.get("0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E").unwrap(),
            "999999"
        );
    }
}
