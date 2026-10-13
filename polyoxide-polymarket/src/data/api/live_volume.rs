use polyoxide_core::{HttpClient, QueryBuilder, Request};
use serde::{Deserialize, Serialize};

use crate::data::error::DataError;

/// LiveVolume namespace for live volume operations
#[derive(Clone)]
pub struct LiveVolumeApi {
    pub(crate) http_client: HttpClient,
}

impl LiveVolumeApi {
    /// Get live volume for an event
    pub async fn get(&self, event_id: u64) -> Result<Vec<LiveVolume>, DataError> {
        Request::<Vec<LiveVolume>, DataError>::new(self.http_client.clone(), "/live-volume")
            .query("id", event_id)
            .send()
            .await
    }
}

/// Live volume for an event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveVolume {
    /// Total aggregated volume
    pub total: f64,
    /// Per-market volume breakdown
    pub markets: Vec<MarketVolume>,
}

/// Volume for a specific market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketVolume {
    /// Market condition ID
    pub market: String,
    /// Volume value
    pub value: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_live_volume() {
        let json = r#"{
            "total": 750000.0,
            "markets": [
                {"market": "cond_001", "value": 500000.0},
                {"market": "cond_002", "value": 250000.0}
            ]
        }"#;

        let vol: LiveVolume = serde_json::from_str(json).unwrap();
        assert!((vol.total - 750000.0).abs() < f64::EPSILON);
        assert_eq!(vol.markets.len(), 2);
        assert_eq!(vol.markets[0].market, "cond_001");
        assert!((vol.markets[0].value - 500000.0).abs() < f64::EPSILON);
        assert_eq!(vol.markets[1].market, "cond_002");
        assert!((vol.markets[1].value - 250000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn deserialize_live_volume_empty_markets() {
        let json = r#"{"total": 0.0, "markets": []}"#;
        let vol: LiveVolume = serde_json::from_str(json).unwrap();
        assert!((vol.total - 0.0).abs() < f64::EPSILON);
        assert!(vol.markets.is_empty());
    }

    #[test]
    fn deserialize_market_volume() {
        let json = r#"{"market": "cond_xyz", "value": 12345.67}"#;
        let mv: MarketVolume = serde_json::from_str(json).unwrap();
        assert_eq!(mv.market, "cond_xyz");
        assert!((mv.value - 12345.67).abs() < f64::EPSILON);
    }
}
