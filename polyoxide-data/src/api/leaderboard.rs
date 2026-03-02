use polyoxide_core::{HttpClient, QueryBuilder, Request};
use serde::{Deserialize, Serialize};

use crate::{error::DataApiError, types::TimePeriod};

/// Leaderboard namespace for trader leaderboard operations
#[derive(Clone)]
pub struct LeaderboardApi {
    pub(crate) http_client: HttpClient,
}

impl LeaderboardApi {
    /// Get the trader leaderboard
    pub fn get(&self) -> GetLeaderboard {
        let request = Request::new(self.http_client.clone(), "/v1/leaderboard");
        GetLeaderboard { request }
    }
}

/// Request builder for getting the trader leaderboard
pub struct GetLeaderboard {
    request: Request<Vec<TraderRanking>, DataApiError>,
}

impl GetLeaderboard {
    /// Filter by category (default: OVERALL)
    pub fn category(mut self, category: LeaderboardCategory) -> Self {
        self.request = self.request.query("category", category);
        self
    }

    /// Set the aggregation time period (default: ALL)
    pub fn time_period(mut self, period: TimePeriod) -> Self {
        self.request = self.request.query("timePeriod", period);
        self
    }

    /// Set the ordering field (default: PNL)
    pub fn order_by(mut self, order_by: LeaderboardOrderBy) -> Self {
        self.request = self.request.query("orderBy", order_by);
        self
    }

    /// Set maximum number of results (1-50, default: 25)
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set pagination offset (0-1000, default: 0)
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Filter by user wallet address
    pub fn user(mut self, address: impl Into<String>) -> Self {
        self.request = self.request.query("user", address.into());
        self
    }

    /// Filter by username
    pub fn user_name(mut self, name: impl Into<String>) -> Self {
        self.request = self.request.query("userName", name.into());
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<TraderRanking>, DataApiError> {
        self.request.send().await
    }
}

/// Leaderboard category for filtering
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum LeaderboardCategory {
    /// Overall ranking (default)
    #[default]
    Overall,
    /// Politics category
    Politics,
    /// Sports category
    Sports,
    /// Crypto category
    Crypto,
    /// Culture category
    Culture,
    /// Mentions category
    Mentions,
    /// Weather category
    Weather,
    /// Economics category
    Economics,
    /// Technology category
    Tech,
    /// Finance category
    Finance,
}

impl std::fmt::Display for LeaderboardCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Overall => write!(f, "OVERALL"),
            Self::Politics => write!(f, "POLITICS"),
            Self::Sports => write!(f, "SPORTS"),
            Self::Crypto => write!(f, "CRYPTO"),
            Self::Culture => write!(f, "CULTURE"),
            Self::Mentions => write!(f, "MENTIONS"),
            Self::Weather => write!(f, "WEATHER"),
            Self::Economics => write!(f, "ECONOMICS"),
            Self::Tech => write!(f, "TECH"),
            Self::Finance => write!(f, "FINANCE"),
        }
    }
}

/// Order-by field for leaderboard sorting
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum LeaderboardOrderBy {
    /// Order by profit and loss (default)
    #[default]
    Pnl,
    /// Order by volume
    Vol,
}

impl std::fmt::Display for LeaderboardOrderBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pnl => write!(f, "PNL"),
            Self::Vol => write!(f, "VOL"),
        }
    }
}

/// Trader ranking entry in the leaderboard
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraderRanking {
    /// Trader's ranking position
    pub rank: String,
    /// Proxy wallet address
    pub proxy_wallet: String,
    /// Display username
    pub user_name: Option<String>,
    /// Trading volume
    pub vol: f64,
    /// Profit and loss
    pub pnl: f64,
    /// Profile image URL
    pub profile_image: Option<String>,
    /// Twitter/X handle
    pub x_username: Option<String>,
    /// Verified badge status
    pub verified_badge: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataApi;

    fn client() -> DataApi {
        DataApi::new().unwrap()
    }

    #[test]
    fn test_get_leaderboard_full_chain() {
        let _builder = client()
            .leaderboard()
            .get()
            .category(LeaderboardCategory::Politics)
            .time_period(TimePeriod::Week)
            .order_by(LeaderboardOrderBy::Vol)
            .limit(25)
            .offset(0)
            .user("0x1234")
            .user_name("trader");
    }

    #[test]
    fn leaderboard_category_display_matches_serde() {
        let variants = [
            LeaderboardCategory::Overall,
            LeaderboardCategory::Politics,
            LeaderboardCategory::Sports,
            LeaderboardCategory::Crypto,
            LeaderboardCategory::Culture,
            LeaderboardCategory::Mentions,
            LeaderboardCategory::Weather,
            LeaderboardCategory::Economics,
            LeaderboardCategory::Tech,
            LeaderboardCategory::Finance,
        ];
        for variant in variants {
            let serialized = serde_json::to_value(variant).unwrap();
            let display = variant.to_string();
            assert_eq!(
                format!("\"{}\"", display),
                serialized.to_string(),
                "Display mismatch for {:?}",
                variant
            );
        }
    }

    #[test]
    fn leaderboard_order_by_display_matches_serde() {
        let variants = [LeaderboardOrderBy::Pnl, LeaderboardOrderBy::Vol];
        for variant in variants {
            let serialized = serde_json::to_value(variant).unwrap();
            let display = variant.to_string();
            assert_eq!(
                format!("\"{}\"", display),
                serialized.to_string(),
                "Display mismatch for {:?}",
                variant
            );
        }
    }

    #[test]
    fn leaderboard_category_default_is_overall() {
        assert_eq!(LeaderboardCategory::default(), LeaderboardCategory::Overall);
    }

    #[test]
    fn leaderboard_order_by_default_is_pnl() {
        assert_eq!(LeaderboardOrderBy::default(), LeaderboardOrderBy::Pnl);
    }

    #[test]
    fn deserialize_trader_ranking() {
        let json = r#"{
            "rank": "1",
            "proxyWallet": "0xabc123",
            "userName": "top_trader",
            "vol": 5000000.50,
            "pnl": 250000.75,
            "profileImage": "https://example.com/pic.png",
            "xUsername": "top_trader_x",
            "verifiedBadge": true
        }"#;
        let ranking: TraderRanking = serde_json::from_str(json).unwrap();
        assert_eq!(ranking.rank, "1");
        assert_eq!(ranking.proxy_wallet, "0xabc123");
        assert_eq!(ranking.user_name.as_deref(), Some("top_trader"));
        assert!((ranking.vol - 5000000.50).abs() < f64::EPSILON);
        assert!((ranking.pnl - 250000.75).abs() < f64::EPSILON);
        assert_eq!(ranking.verified_badge, Some(true));
    }

    #[test]
    fn deserialize_trader_ranking_minimal() {
        let json = r#"{
            "rank": "50",
            "proxyWallet": "0xdef456",
            "userName": null,
            "vol": 100.0,
            "pnl": -10.0,
            "profileImage": null,
            "xUsername": null,
            "verifiedBadge": null
        }"#;
        let ranking: TraderRanking = serde_json::from_str(json).unwrap();
        assert_eq!(ranking.rank, "50");
        assert!(ranking.user_name.is_none());
        assert!(ranking.profile_image.is_none());
        assert!((ranking.pnl - (-10.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn deserialize_trader_ranking_list() {
        let json = r#"[
            {"rank": "1", "proxyWallet": "0xa", "userName": null, "vol": 100.0, "pnl": 50.0, "profileImage": null, "xUsername": null, "verifiedBadge": null},
            {"rank": "2", "proxyWallet": "0xb", "userName": null, "vol": 80.0, "pnl": 30.0, "profileImage": null, "xUsername": null, "verifiedBadge": null}
        ]"#;
        let rankings: Vec<TraderRanking> = serde_json::from_str(json).unwrap();
        assert_eq!(rankings.len(), 2);
        assert_eq!(rankings[0].rank, "1");
        assert_eq!(rankings[1].rank, "2");
    }
}
