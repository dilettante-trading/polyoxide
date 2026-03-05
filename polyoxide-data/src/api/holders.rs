use polyoxide_core::{HttpClient, QueryBuilder, Request};
use serde::{Deserialize, Serialize};

use crate::error::DataApiError;

/// Holders namespace for holder-related operations
#[derive(Clone)]
pub struct Holders {
    pub(crate) http_client: HttpClient,
}

impl Holders {
    /// Get top holders for markets
    pub fn list(&self, markets: impl IntoIterator<Item = impl ToString>) -> ListHolders {
        let market_ids: Vec<String> = markets.into_iter().map(|s| s.to_string()).collect();
        let mut request = Request::new(self.http_client.clone(), "/holders");
        if !market_ids.is_empty() {
            request = request.query("market", market_ids.join(","));
        }

        ListHolders { request }
    }
}

/// Request builder for getting top holders
pub struct ListHolders {
    request: Request<Vec<MarketHolders>, DataApiError>,
}

impl ListHolders {
    /// Set maximum number of results per market (0-500, default: 100)
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set minimum balance filter (0-999999, default: 1)
    pub fn min_balance(mut self, min_balance: u32) -> Self {
        self.request = self.request.query("minBalance", min_balance);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<MarketHolders>, DataApiError> {
        self.request.send().await
    }
}

/// Market holders response containing token and its holders
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct MarketHolders {
    /// Token identifier
    pub token: String,
    /// List of holders for this token
    pub holders: Vec<Holder>,
}

/// Individual holder of a market token
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Holder {
    /// Proxy wallet address
    pub proxy_wallet: String,
    /// User bio
    #[serde(default)]
    pub bio: Option<String>,
    /// Asset identifier (token ID)
    #[serde(default)]
    pub asset: Option<String>,
    /// User pseudonym
    #[serde(default)]
    pub pseudonym: Option<String>,
    /// Amount held
    pub amount: f64,
    /// Whether username is displayed publicly
    #[serde(default)]
    pub display_username_public: Option<bool>,
    /// Outcome index (0 or 1 for binary markets)
    pub outcome_index: u32,
    /// User display name
    #[serde(default)]
    pub name: Option<String>,
    /// User profile image URL
    #[serde(default)]
    pub profile_image: Option<String>,
    /// Optimized profile image URL
    #[serde(default)]
    pub profile_image_optimized: Option<String>,
    /// Whether the user is verified
    #[serde(default)]
    pub verified: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_market_holders() {
        let json = r#"{
            "token": "token_abc",
            "holders": [
                {
                    "proxyWallet": "0xholder1",
                    "bio": "Top trader",
                    "asset": "token_abc",
                    "pseudonym": "whale1",
                    "amount": 50000.0,
                    "displayUsernamePublic": true,
                    "outcomeIndex": 0,
                    "name": "Holder One",
                    "profileImage": "https://example.com/img.png",
                    "profileImageOptimized": "https://example.com/img_opt.png",
                    "verified": true
                },
                {
                    "proxyWallet": "0xholder2",
                    "bio": null,
                    "asset": null,
                    "pseudonym": null,
                    "amount": 1000.0,
                    "displayUsernamePublic": null,
                    "outcomeIndex": 1,
                    "name": null,
                    "profileImage": null,
                    "profileImageOptimized": null,
                    "verified": false
                }
            ]
        }"#;

        let mh: MarketHolders = serde_json::from_str(json).unwrap();
        assert_eq!(mh.token, "token_abc");
        assert_eq!(mh.holders.len(), 2);

        let h1 = &mh.holders[0];
        assert_eq!(h1.proxy_wallet, "0xholder1");
        assert_eq!(h1.bio, Some("Top trader".to_string()));
        assert!((h1.amount - 50000.0).abs() < f64::EPSILON);
        assert_eq!(h1.outcome_index, 0);
        assert_eq!(h1.display_username_public, Some(true));
        assert_eq!(h1.name, Some("Holder One".to_string()));
        assert_eq!(h1.verified, Some(true));

        let h2 = &mh.holders[1];
        assert_eq!(h2.proxy_wallet, "0xholder2");
        assert!(h2.bio.is_none());
        assert!(h2.asset.is_none());
        assert!(h2.pseudonym.is_none());
        assert!((h2.amount - 1000.0).abs() < f64::EPSILON);
        assert_eq!(h2.outcome_index, 1);
        assert!(h2.name.is_none());
        assert_eq!(h2.verified, Some(false));
    }

    #[test]
    fn deserialize_empty_holders_list() {
        let json = r#"{"token": "empty_token", "holders": []}"#;
        let mh: MarketHolders = serde_json::from_str(json).unwrap();
        assert_eq!(mh.token, "empty_token");
        assert!(mh.holders.is_empty());
    }

    #[test]
    fn holder_without_verified_field() {
        let json = r#"{
            "proxyWallet": "0xholder",
            "amount": 100.0,
            "outcomeIndex": 0
        }"#;
        let h: Holder = serde_json::from_str(json).unwrap();
        assert_eq!(h.proxy_wallet, "0xholder");
        assert!(h.verified.is_none());
    }
}
