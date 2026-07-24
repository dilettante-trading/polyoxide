//! WebSocket subscription message types.

use serde::{Deserialize, Serialize};

use super::auth::ApiCredentials;

/// WebSocket endpoint URL for market channel
pub const WS_MARKET_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

/// WebSocket endpoint URL for user channel
pub const WS_USER_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/user";

/// WebSocket endpoint URL for the sports channel
///
/// Note the different host: sports updates are served by `sports-api`, not by
/// `ws-subscriptions-clob` like the market and user channels.
pub const WS_SPORTS_URL: &str = "wss://sports-api.polymarket.com/ws";

/// Channel type for WebSocket subscription
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    /// Market channel for public order book and price updates
    Market,
    /// User channel for authenticated order and trade updates
    User,
    /// Sports channel for live game state updates
    ///
    /// Unlike the other two, this variant is never sent on the wire — the
    /// sports channel takes no subscription payload. It exists so a connected
    /// [`WebSocket`](crate::ws::WebSocket) can report which channel it is on.
    Sports,
}

/// Order book depth level for a market subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub enum SubscriptionLevel {
    /// Level 1 — top of book only.
    One,
    /// Level 2 — aggregated depth (upstream default).
    Two,
    /// Level 3 — full depth.
    Three,
}

impl From<SubscriptionLevel> for u8 {
    fn from(level: SubscriptionLevel) -> Self {
        match level {
            SubscriptionLevel::One => 1,
            SubscriptionLevel::Two => 2,
            SubscriptionLevel::Three => 3,
        }
    }
}

impl TryFrom<u8> for SubscriptionLevel {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            3 => Ok(Self::Three),
            other => Err(format!("invalid subscription level {other}, expected 1-3")),
        }
    }
}

/// Optional settings for a market channel subscription.
///
/// Every field is omitted from the wire payload when left unset, so the
/// default value produces exactly the subscription
/// [`MarketSubscription::new`] has always sent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarketSubscriptionOptions {
    /// Enable the `best_bid_ask`, `new_market`, and `market_resolved` events.
    ///
    /// These are gated server-side: without this flag the server simply never
    /// sends them, so [`MarketMessage::BestBidAsk`](crate::ws::MarketMessage)
    /// and its siblings will never be observed.
    pub custom_feature_enabled: bool,
    /// Whether to send an order book snapshot on subscribe (upstream default:
    /// `true`).
    pub initial_dump: Option<bool>,
    /// Order book depth level (upstream default: [`SubscriptionLevel::Two`]).
    pub level: Option<SubscriptionLevel>,
}

impl MarketSubscriptionOptions {
    /// Enable `best_bid_ask`, `new_market`, and `market_resolved` events.
    pub fn with_custom_features(mut self) -> Self {
        self.custom_feature_enabled = true;
        self
    }

    /// Set whether the server sends an initial order book snapshot.
    pub fn initial_dump(mut self, initial_dump: bool) -> Self {
        self.initial_dump = Some(initial_dump);
        self
    }

    /// Set the order book depth level.
    pub fn level(mut self, level: SubscriptionLevel) -> Self {
        self.level = Some(level);
        self
    }
}

/// Subscription message for market channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSubscription {
    /// Asset IDs (token IDs) to subscribe to
    pub assets_ids: Vec<String>,
    /// Channel type (always "market")
    #[serde(rename = "type")]
    pub channel_type: ChannelType,
    /// Enables `best_bid_ask`, `new_market`, and `market_resolved` events.
    /// Omitted when false, matching the upstream default.
    #[serde(default, skip_serializing_if = "is_false")]
    pub custom_feature_enabled: bool,
    /// Whether to send an initial order book snapshot. Omitted when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_dump: Option<bool>,
    /// Order book depth level. Omitted when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<SubscriptionLevel>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl MarketSubscription {
    /// Create a new market subscription with upstream defaults.
    pub fn new(assets_ids: Vec<String>) -> Self {
        Self::with_options(assets_ids, MarketSubscriptionOptions::default())
    }

    /// Create a market subscription with explicit options.
    pub fn with_options(assets_ids: Vec<String>, options: MarketSubscriptionOptions) -> Self {
        Self {
            assets_ids,
            channel_type: ChannelType::Market,
            custom_feature_enabled: options.custom_feature_enabled,
            initial_dump: options.initial_dump,
            level: options.level,
        }
    }
}

/// Subscription message for user channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSubscription {
    /// Market condition IDs to subscribe to
    pub markets: Vec<String>,
    /// Authentication credentials
    pub auth: ApiCredentials,
    /// Channel type (always "user")
    #[serde(rename = "type")]
    pub channel_type: ChannelType,
}

impl UserSubscription {
    /// Create a new user subscription
    pub fn new(markets: Vec<String>, credentials: ApiCredentials) -> Self {
        Self {
            markets,
            auth: credentials,
            channel_type: ChannelType::User,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_type_serialization() {
        let market = serde_json::to_value(ChannelType::Market).unwrap();
        let user = serde_json::to_value(ChannelType::User).unwrap();

        assert_eq!(market, "market");
        assert_eq!(user, "user");
    }

    #[test]
    fn channel_type_deserialization() {
        let market: ChannelType = serde_json::from_str("\"market\"").unwrap();
        let user: ChannelType = serde_json::from_str("\"user\"").unwrap();

        assert_eq!(market, ChannelType::Market);
        assert_eq!(user, ChannelType::User);
    }

    #[test]
    fn channel_type_rejects_uppercase() {
        let result = serde_json::from_str::<ChannelType>("\"MARKET\"");
        assert!(result.is_err(), "Should reject uppercase channel type");
    }

    #[test]
    fn market_subscription_new_sets_channel_type() {
        let sub = MarketSubscription::new(vec!["asset1".into(), "asset2".into()]);
        assert_eq!(sub.channel_type, ChannelType::Market);
        assert_eq!(sub.assets_ids.len(), 2);
        assert_eq!(sub.assets_ids[0], "asset1");
        assert_eq!(sub.assets_ids[1], "asset2");
    }

    #[test]
    fn market_subscription_serialization() {
        let sub = MarketSubscription::new(vec!["token123".into()]);
        let json = serde_json::to_value(&sub).unwrap();

        assert_eq!(json["type"], "market");
        assert_eq!(json["assets_ids"][0], "token123");
    }

    #[test]
    fn market_subscription_empty_assets() {
        let sub = MarketSubscription::new(vec![]);
        let json = serde_json::to_value(&sub).unwrap();

        assert_eq!(json["type"], "market");
        assert!(json["assets_ids"].as_array().unwrap().is_empty());
    }

    #[test]
    fn user_subscription_new_sets_channel_type() {
        let creds = ApiCredentials::new("key", "secret", "pass");
        let sub = UserSubscription::new(vec!["cond1".into()], creds);
        assert_eq!(sub.channel_type, ChannelType::User);
        assert_eq!(sub.markets.len(), 1);
        assert_eq!(sub.markets[0], "cond1");
    }

    #[test]
    fn user_subscription_serialization() {
        let creds = ApiCredentials::new("my_key", "my_secret", "my_pass");
        let sub = UserSubscription::new(vec!["market1".into(), "market2".into()], creds);
        let json = serde_json::to_value(&sub).unwrap();

        assert_eq!(json["type"], "user");
        assert_eq!(json["markets"][0], "market1");
        assert_eq!(json["markets"][1], "market2");
        assert_eq!(json["auth"]["apiKey"], "my_key");
        assert_eq!(json["auth"]["secret"], "my_secret");
        assert_eq!(json["auth"]["passphrase"], "my_pass");
    }

    #[test]
    fn ws_url_constants() {
        assert!(WS_MARKET_URL.starts_with("wss://"));
        assert!(WS_MARKET_URL.contains("market"));
        assert!(WS_USER_URL.starts_with("wss://"));
        assert!(WS_USER_URL.contains("user"));
    }
}

#[cfg(test)]
mod options_tests {
    use super::*;

    #[test]
    fn default_options_preserve_the_original_wire_payload() {
        // Regression guard: adding the new optional fields must not change the
        // bytes sent by existing callers of MarketSubscription::new.
        let sub = MarketSubscription::new(vec!["token123".into()]);
        let json = serde_json::to_value(&sub).unwrap();

        assert_eq!(json["type"], "market");
        assert_eq!(json["assets_ids"][0], "token123");
        assert_eq!(
            json.as_object().unwrap().len(),
            2,
            "unset options must be omitted entirely, got {json}"
        );
    }

    #[test]
    fn custom_features_flag_is_sent_when_enabled() {
        let sub = MarketSubscription::with_options(
            vec!["t".into()],
            MarketSubscriptionOptions::default().with_custom_features(),
        );
        let json = serde_json::to_value(&sub).unwrap();
        assert_eq!(json["custom_feature_enabled"], true);
    }

    #[test]
    fn initial_dump_and_level_serialize_when_set() {
        let sub = MarketSubscription::with_options(
            vec!["t".into()],
            MarketSubscriptionOptions::default()
                .initial_dump(false)
                .level(SubscriptionLevel::Three),
        );
        let json = serde_json::to_value(&sub).unwrap();
        assert_eq!(json["initial_dump"], false);
        // Level is an integer on the wire, not a string.
        assert_eq!(json["level"], 3);
    }

    #[test]
    fn subscription_level_rejects_out_of_range() {
        assert!(SubscriptionLevel::try_from(0u8).is_err());
        assert!(SubscriptionLevel::try_from(4u8).is_err());
        assert_eq!(
            SubscriptionLevel::try_from(2u8).unwrap(),
            SubscriptionLevel::Two
        );
    }

    #[test]
    fn sports_url_uses_its_own_host() {
        assert!(WS_SPORTS_URL.starts_with("wss://"));
        assert!(
            !WS_SPORTS_URL.contains("ws-subscriptions-clob"),
            "sports is served by sports-api, not the clob subscriptions host"
        );
    }
}
