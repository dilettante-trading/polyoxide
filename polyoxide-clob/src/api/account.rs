use alloy::primitives::Address;
use polyoxide_core::{HttpClient, QueryBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    /// Get trades with optional filtering
    pub fn trades(&self) -> ListClobTrades {
        let request = Request::get(
            self.http_client.clone(),
            "/trades",
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
    request: Request<Vec<Trade>>,
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
    pub async fn send(self) -> Result<Vec<Trade>, ClobError> {
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
    pub transaction_hash: String,
}

/// Balance and allowance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceAllowanceResponse {
    pub balance: String,
    pub allowances: HashMap<Address, String>,
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
            "transaction_hash": "0xhash456"
        }"#;
        let trade: Trade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.side, OrderSide::Sell);
        assert_eq!(trade.last_update.as_deref(), Some("1700002000"));
        assert_eq!(trade.bucket_index, Some(3));
    }
}
