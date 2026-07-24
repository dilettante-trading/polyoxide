//! Sports channel message types.
//!
//! The sports channel differs from market and user in two ways: it lives on
//! the `sports-api` host rather than `ws-subscriptions-clob`, and it takes no
//! subscription payload — connecting is enough to start receiving updates.

use serde::{Deserialize, Serialize};

/// A live sports state update (`sports_update`).
///
/// Upstream types only `event_type` and `timestamp`; the rest of the payload
/// varies by sport and market type and is left as raw JSON in
/// [`data`](Self::data) rather than guessed at.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SportsUpdateMessage {
    /// Event discriminator.
    pub event_type: String,
    /// Timestamp reported by the server, when present.
    #[serde(default)]
    pub timestamp: Option<String>,
    /// Remaining payload fields, unparsed.
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Sports channel message types
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum SportsMessage {
    /// A live game state update.
    Update(SportsUpdateMessage),
}

impl SportsMessage {
    /// Parse a sports channel message from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        Ok(SportsMessage::Update(serde_json::from_str(json)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sports_update_and_retains_unknown_fields() {
        let json = r#"{
            "event_type": "sports_update",
            "timestamp": "1700000000000",
            "game_id": "g-1",
            "period": 2,
            "score": {"home": 21, "away": 14}
        }"#;

        let SportsMessage::Update(msg) = SportsMessage::from_json(json).unwrap();
        assert_eq!(msg.event_type, "sports_update");
        assert_eq!(msg.timestamp.as_deref(), Some("1700000000000"));
        // Sport-specific fields survive in `data` instead of being dropped.
        assert_eq!(msg.data["game_id"], "g-1");
        assert_eq!(msg.data["score"]["home"], 21);
    }

    #[test]
    fn tolerates_missing_timestamp() {
        let json = r#"{"event_type": "sports_update", "game_id": "g-2"}"#;
        let SportsMessage::Update(msg) = SportsMessage::from_json(json).unwrap();
        assert_eq!(msg.timestamp, None);
        assert_eq!(msg.data["game_id"], "g-2");
    }
}
