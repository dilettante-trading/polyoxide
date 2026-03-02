use polyoxide_core::HttpClient;
use serde::{Deserialize, Serialize};

use crate::{
    account::{Credentials, Signer, Wallet},
    request::{AuthMode, Request},
};

/// Rewards namespace for liquidity reward operations
#[derive(Clone)]
pub struct Rewards {
    pub(crate) http_client: HttpClient,
    pub(crate) wallet: Wallet,
    pub(crate) credentials: Credentials,
    pub(crate) signer: Signer,
    pub(crate) chain_id: u64,
}

impl Rewards {
    fn l2_auth(&self) -> AuthMode {
        AuthMode::L2 {
            address: self.wallet.address(),
            credentials: self.credentials.clone(),
            signer: self.signer.clone(),
        }
    }

    /// Get user earnings, optionally filtered by day via `.query("day", "YYYY-MM-DD")`
    pub fn earnings(&self) -> Request<RewardEarnings> {
        Request::get(
            self.http_client.clone(),
            "/rewards/user",
            self.l2_auth(),
            self.chain_id,
        )
    }

    /// Get user total accumulated earnings
    pub fn total_earnings(&self) -> Request<RewardTotalEarnings> {
        Request::get(
            self.http_client.clone(),
            "/rewards/user/total",
            self.l2_auth(),
            self.chain_id,
        )
    }

    /// Get user reward percentages
    pub fn percentages(&self) -> Request<RewardPercentages> {
        Request::get(
            self.http_client.clone(),
            "/rewards/user/percentages",
            self.l2_auth(),
            self.chain_id,
        )
    }

    /// Get user earnings broken down by market
    pub fn market_earnings(&self) -> Request<Vec<RewardMarketEarning>> {
        Request::get(
            self.http_client.clone(),
            "/rewards/user/markets",
            self.l2_auth(),
            self.chain_id,
        )
    }

    /// Get currently active reward markets
    pub fn current_markets(&self) -> Request<Vec<RewardMarket>> {
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
    fn reward_earnings_empty_object_deserializes() {
        let json = r#"{}"#;
        let resp: RewardEarnings = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_object());
    }
}
