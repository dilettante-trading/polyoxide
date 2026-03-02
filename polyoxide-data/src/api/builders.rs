use polyoxide_core::{HttpClient, QueryBuilder, Request};
use serde::{Deserialize, Serialize};

use crate::error::DataApiError;

// Re-export for backwards compatibility
pub use crate::types::TimePeriod;

/// Builders namespace for builder-related operations
#[derive(Clone)]
pub struct BuildersApi {
    pub(crate) http_client: HttpClient,
}

impl BuildersApi {
    /// Get the aggregated builder leaderboard
    pub fn leaderboard(&self) -> GetBuilderLeaderboard {
        let request = Request::new(self.http_client.clone(), "/v1/builders/leaderboard");

        GetBuilderLeaderboard { request }
    }

    /// Get daily builder volume time series
    pub fn volume(&self) -> GetBuilderVolume {
        let request = Request::new(self.http_client.clone(), "/v1/builders/volume");

        GetBuilderVolume { request }
    }
}

/// Request builder for getting the builder leaderboard
pub struct GetBuilderLeaderboard {
    request: Request<Vec<BuilderRanking>, DataApiError>,
}

impl GetBuilderLeaderboard {
    /// Set the aggregation time period (default: DAY)
    pub fn time_period(mut self, period: TimePeriod) -> Self {
        self.request = self.request.query("timePeriod", period);
        self
    }

    /// Set maximum number of results (0-50, default: 25)
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set pagination offset (0-1000, default: 0)
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<BuilderRanking>, DataApiError> {
        self.request.send().await
    }
}

/// Builder ranking entry in the leaderboard
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct BuilderRanking {
    /// Builder's ranking position
    pub rank: String,
    /// Builder identifier/name
    pub builder: String,
    /// Trading volume metric
    pub volume: f64,
    /// Count of active users
    pub active_users: u64,
    /// Verification status
    pub verified: bool,
    /// Logo image URL
    pub builder_logo: Option<String>,
}

/// Request builder for getting the builder volume time series
pub struct GetBuilderVolume {
    request: Request<Vec<BuilderVolume>, DataApiError>,
}

impl GetBuilderVolume {
    /// Set the time period filter (default: DAY)
    pub fn time_period(mut self, period: TimePeriod) -> Self {
        self.request = self.request.query("timePeriod", period);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<BuilderVolume>, DataApiError> {
        self.request.send().await
    }
}

/// Builder volume entry in the time series
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct BuilderVolume {
    /// Date/time of the volume record (ISO 8601)
    pub dt: String,
    /// Builder identifier/name
    pub builder: String,
    /// Logo image URL
    pub builder_logo: Option<String>,
    /// Verification status
    pub verified: bool,
    /// Trading volume metric
    pub volume: f64,
    /// Count of active users
    pub active_users: u64,
    /// Builder's ranking position
    pub rank: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_period_display_matches_serde() {
        let variants = [
            TimePeriod::Day,
            TimePeriod::Week,
            TimePeriod::Month,
            TimePeriod::All,
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
    fn time_period_serde_roundtrip() {
        for variant in [
            TimePeriod::Day,
            TimePeriod::Week,
            TimePeriod::Month,
            TimePeriod::All,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: TimePeriod = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn time_period_default_is_day() {
        assert_eq!(TimePeriod::default(), TimePeriod::Day);
    }

    #[test]
    fn time_period_specific_values() {
        assert_eq!(TimePeriod::Day.to_string(), "DAY");
        assert_eq!(TimePeriod::Week.to_string(), "WEEK");
        assert_eq!(TimePeriod::Month.to_string(), "MONTH");
        assert_eq!(TimePeriod::All.to_string(), "ALL");
    }

    #[test]
    fn deserialize_builder_ranking() {
        let json = r#"{
            "rank": "1",
            "builder": "polymarket-app",
            "volume": 1500000.50,
            "activeUsers": 25000,
            "verified": true,
            "builderLogo": "https://example.com/logo.png"
        }"#;

        let ranking: BuilderRanking = serde_json::from_str(json).unwrap();
        assert_eq!(ranking.rank, "1");
        assert_eq!(ranking.builder, "polymarket-app");
        assert!((ranking.volume - 1500000.50).abs() < f64::EPSILON);
        assert_eq!(ranking.active_users, 25000);
        assert!(ranking.verified);
        assert_eq!(
            ranking.builder_logo,
            Some("https://example.com/logo.png".to_string())
        );
    }

    #[test]
    fn deserialize_builder_ranking_null_logo() {
        let json = r#"{
            "rank": "5",
            "builder": "unknown-builder",
            "volume": 100.0,
            "activeUsers": 10,
            "verified": false,
            "builderLogo": null
        }"#;

        let ranking: BuilderRanking = serde_json::from_str(json).unwrap();
        assert_eq!(ranking.rank, "5");
        assert!(!ranking.verified);
        assert!(ranking.builder_logo.is_none());
    }

    #[test]
    fn deserialize_builder_volume() {
        let json = r#"{
            "dt": "2025-01-15T00:00:00Z",
            "builder": "top-builder",
            "builderLogo": null,
            "verified": true,
            "volume": 500000.0,
            "activeUsers": 1200,
            "rank": "3"
        }"#;

        let vol: BuilderVolume = serde_json::from_str(json).unwrap();
        assert_eq!(vol.dt, "2025-01-15T00:00:00Z");
        assert_eq!(vol.builder, "top-builder");
        assert!(vol.verified);
        assert!((vol.volume - 500000.0).abs() < f64::EPSILON);
        assert_eq!(vol.active_users, 1200);
        assert_eq!(vol.rank, "3");
        assert!(vol.builder_logo.is_none());
    }

    #[test]
    fn deserialize_builder_ranking_list() {
        let json = r#"[
            {"rank": "1", "builder": "a", "volume": 100.0, "activeUsers": 5, "verified": true, "builderLogo": null},
            {"rank": "2", "builder": "b", "volume": 50.0, "activeUsers": 3, "verified": false, "builderLogo": null}
        ]"#;

        let rankings: Vec<BuilderRanking> = serde_json::from_str(json).unwrap();
        assert_eq!(rankings.len(), 2);
        assert_eq!(rankings[0].rank, "1");
        assert_eq!(rankings[1].rank, "2");
    }
}
