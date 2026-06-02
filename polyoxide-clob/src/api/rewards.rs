use polyoxide_core::{HttpClient, QueryBuilder};
use serde::{Deserialize, Serialize};

use crate::{
    account::{Credentials, Signer, Wallet},
    request::{AuthMode, Request},
    types::SignatureType,
};

/// Rewards namespace for liquidity reward operations
#[derive(Clone)]
pub struct Rewards {
    pub(crate) http_client: HttpClient,
    pub(crate) wallet: Wallet,
    pub(crate) credentials: Credentials,
    pub(crate) signer: Signer,
    pub(crate) chain_id: u64,
    pub(crate) signature_type: SignatureType,
}

impl Rewards {
    fn l2_auth(&self) -> AuthMode {
        AuthMode::L2 {
            address: self.wallet.address(),
            credentials: self.credentials.clone(),
            signer: self.signer.clone(),
        }
    }

    /// Get user earnings for a specific day (`GET /rewards/user`).
    ///
    /// `date` must be in `YYYY-MM-DD` format (required by the API). The
    /// `signature_type` query parameter is taken from the client configuration.
    pub fn earnings(&self, date: impl Into<String>) -> Request<RewardEarnings> {
        Request::get(
            self.http_client.clone(),
            "/rewards/user",
            self.l2_auth(),
            self.chain_id,
        )
        .query("date", date.into())
        .query("signature_type", self.signature_type as u8)
    }

    /// Get user total earnings for a specific day (`GET /rewards/user/total`).
    ///
    /// `date` must be in `YYYY-MM-DD` format (required by the API). The endpoint
    /// returns an array of totals grouped by asset address.
    pub fn total_earnings(&self, date: impl Into<String>) -> Request<Vec<RewardTotalEarnings>> {
        Request::get(
            self.http_client.clone(),
            "/rewards/user/total",
            self.l2_auth(),
            self.chain_id,
        )
        .query("date", date.into())
        .query("signature_type", self.signature_type as u8)
    }

    /// Get user reward percentages
    pub fn percentages(&self) -> Request<RewardPercentages> {
        Request::get(
            self.http_client.clone(),
            "/rewards/user/percentages",
            self.l2_auth(),
            self.chain_id,
        )
        .query("signature_type", self.signature_type as u8)
    }

    /// Get user earnings broken down by market (`GET /rewards/user/markets`).
    ///
    /// Returns a paginated envelope; the per-market earnings are in `data`.
    pub fn market_earnings(&self) -> Request<Paginated<RewardMarketEarning>> {
        Request::get(
            self.http_client.clone(),
            "/rewards/user/markets",
            self.l2_auth(),
            self.chain_id,
        )
        .query("signature_type", self.signature_type as u8)
    }

    /// Get currently active reward markets (`GET /rewards/markets/current`).
    ///
    /// Returns a paginated envelope (`data`, `next_cursor`, `limit`, `count`);
    /// the reward markets are in the `data` field.
    pub fn current_markets(&self) -> Request<Paginated<RewardMarket>> {
        Request::get(
            self.http_client.clone(),
            "/rewards/markets/current",
            AuthMode::None,
            self.chain_id,
        )
    }

    /// Get rewards for a specific market
    pub fn market(&self, condition_id: impl Into<String>) -> Request<RewardMarket> {
        Request::get(
            self.http_client.clone(),
            format!(
                "/rewards/markets/{}",
                urlencoding::encode(&condition_id.into())
            ),
            AuthMode::None,
            self.chain_id,
        )
    }
}

/// User earnings response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardEarnings {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// User total accumulated earnings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardTotalEarnings {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// User reward percentages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardPercentages {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Per-market earnings breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardMarketEarning {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Reward market information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardMarket {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Pagination envelope used by the rewards list endpoints
/// (`GET /rewards/markets/current` and `GET /rewards/user/markets`).
///
/// These endpoints wrap results in a pagination object rather than returning a
/// bare array, so the items live in [`Self::data`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paginated<T> {
    /// Items for this page.
    #[serde(default = "Vec::new")]
    pub data: Vec<T>,
    /// Cursor for the next page; a value of `"LTE="` indicates the last page.
    #[serde(default)]
    pub next_cursor: Option<String>,
    /// Page size limit reported by the server.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Number of items in this page.
    #[serde(default)]
    pub count: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_earnings_deserializes() {
        let json = r#"{"amount": "1.5", "day": "2024-01-15"}"#;
        let resp: RewardEarnings = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data["amount"], "1.5");
        assert_eq!(resp.data["day"], "2024-01-15");
    }

    #[test]
    fn reward_total_earnings_deserializes() {
        let json = r#"{"total": "42.0"}"#;
        let resp: RewardTotalEarnings = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data["total"], "42.0");
    }

    #[test]
    fn reward_total_earnings_list_deserializes() {
        // GET /rewards/user/total returns an array of per-asset totals, not a
        // single object. Regression test for that shape (observed live).
        let json = r#"[{"asset_address": "0xabc", "total": "42.0"}]"#;
        let resp: Vec<RewardTotalEarnings> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.len(), 1);
        assert_eq!(resp[0].data["total"], "42.0");
    }

    #[test]
    fn reward_percentages_deserializes() {
        let json = r#"{"maker": "0.5", "taker": "0.3"}"#;
        let resp: RewardPercentages = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data["maker"], "0.5");
    }

    #[test]
    fn reward_market_earning_list_deserializes() {
        let json = r#"[
            {"condition_id": "0xabc", "amount": "10.0"},
            {"condition_id": "0xdef", "amount": "5.0"}
        ]"#;
        let resp: Vec<RewardMarketEarning> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.len(), 2);
        assert_eq!(resp[0].data["condition_id"], "0xabc");
    }

    #[test]
    fn reward_market_deserializes() {
        let json = r#"{"condition_id": "0xabc", "reward_rate": "0.01"}"#;
        let resp: RewardMarket = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data["condition_id"], "0xabc");
    }

    #[test]
    fn reward_market_list_deserializes() {
        let json = r#"[{"condition_id": "0xabc"}, {"condition_id": "0xdef"}]"#;
        let resp: Vec<RewardMarket> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.len(), 2);
    }

    #[test]
    fn current_markets_paginated_response_deserializes() {
        // GET /rewards/markets/current wraps results in a pagination envelope,
        // not a bare array. Regression test for that shape (observed live).
        let json = r#"{
            "limit": 500,
            "count": 1,
            "next_cursor": "LTE=",
            "data": [
                {"condition_id": "0xabc", "rewards_max_spread": 99}
            ]
        }"#;
        let page: Paginated<RewardMarket> =
            serde_json::from_str(json).expect("paginated reward markets should deserialize");
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.count, Some(1));
        assert_eq!(page.next_cursor.as_deref(), Some("LTE="));
        assert_eq!(page.data[0].data["condition_id"], "0xabc");
    }

    #[test]
    fn market_earnings_paginated_response_deserializes() {
        // GET /rewards/user/markets wraps results in the same pagination envelope.
        let json = r#"{
            "limit": 100,
            "count": 1,
            "next_cursor": "LTE=",
            "data": [
                {"condition_id": "0xabc", "earnings": 0.237519}
            ]
        }"#;
        let page: Paginated<RewardMarketEarning> =
            serde_json::from_str(json).expect("paginated market earnings should deserialize");
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.count, Some(1));
        assert_eq!(page.data[0].data["condition_id"], "0xabc");
    }

    #[test]
    fn reward_earnings_empty_object_deserializes() {
        let json = r#"{}"#;
        let resp: RewardEarnings = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_object());
    }
}
