//! User channel message types.
//!
//! The user channel provides real-time order and trade updates for authenticated users.

use serde::{Deserialize, Serialize};

/// Maker order in a trade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakerOrder {
    /// Order ID
    pub order_id: String,
    /// Maker address
    pub maker_address: String,
    /// Matched amount
    pub matched_amount: String,
    /// Fee rate
    pub fee_rate_bps: Option<String>,
    /// Asset ID
    pub asset_id: String,
    /// Price
    pub price: String,
}

/// Trade status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TradeStatus {
    /// Trade matched
    Matched,
    /// Trade mined on chain
    Mined,
    /// Trade confirmed
    Confirmed,
    /// Trade retrying
    Retrying,
    /// Trade failed
    Failed,
}

/// Trade message - user trade update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeMessage {
    /// Event type (always "trade")
    pub event_type: String,
    /// Trade ID
    pub id: String,
    /// Asset ID (token ID)
    pub asset_id: String,
    /// Market condition ID
    pub market: String,
    /// Outcome (YES or NO)
    pub outcome: String,
    /// Trade price
    pub price: String,
    /// Trade size
    pub size: String,
    /// Trade side (BUY or SELL)
    pub side: String,
    /// Trade status
    pub status: TradeStatus,
    /// Taker order ID
    pub taker_order_id: String,
    /// Maker orders involved in the trade
    pub maker_orders: Vec<MakerOrder>,
    /// Trade owner address
    pub owner: Option<String>,
    /// Transaction hash (when mined/confirmed)
    pub transaction_hash: Option<String>,
    /// Timestamp
    pub timestamp: String,
}

/// Order event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderEventType {
    /// New order placement
    Placement,
    /// Order updated (partial fill)
    Update,
    /// Order cancelled
    Cancellation,
}

/// Order message - user order update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderMessage {
    /// Event type (always "order")
    pub event_type: String,
    /// Order ID
    pub id: String,
    /// Asset ID (token ID)
    pub asset_id: String,
    /// Market condition ID
    pub market: String,
    /// Outcome (YES or NO)
    pub outcome: String,
    /// Order price
    pub price: String,
    /// Order side (BUY or SELL)
    pub side: String,
    /// Original order size
    pub original_size: String,
    /// Size matched so far
    pub size_matched: String,
    /// Order event type
    #[serde(rename = "type")]
    pub order_type: OrderEventType,
    /// Order owner address
    pub order_owner: Option<String>,
    /// Timestamp
    pub timestamp: String,
}

/// User channel message types
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum UserMessage {
    /// Trade update
    Trade(TradeMessage),
    /// Order update
    Order(OrderMessage),
}

impl UserMessage {
    /// Parse a user channel message from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct RawMessage {
            event_type: String,
        }

        let raw: RawMessage = serde_json::from_str(json)?;
        match raw.event_type.as_str() {
            "trade" => Ok(UserMessage::Trade(serde_json::from_str(json)?)),
            "order" => Ok(UserMessage::Order(serde_json::from_str(json)?)),
            _ => Err(serde::de::Error::custom(format!(
                "Unknown user event type: {}",
                raw.event_type
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade_json(status: &str) -> String {
        format!(
            r#"{{
                "event_type": "trade",
                "id": "trade-1",
                "asset_id": "abc123",
                "market": "0xcond",
                "outcome": "YES",
                "price": "0.55",
                "size": "100",
                "side": "BUY",
                "status": "{}",
                "taker_order_id": "order-1",
                "maker_orders": [{{
                    "order_id": "maker-1",
                    "maker_address": "0xmaker",
                    "matched_amount": "100",
                    "fee_rate_bps": "200",
                    "asset_id": "abc123",
                    "price": "0.55"
                }}],
                "owner": "0xowner",
                "transaction_hash": "0xtxhash",
                "timestamp": "1700000000000"
            }}"#,
            status
        )
    }

    #[test]
    fn deserialize_trade_message_matched() {
        let json = trade_json("MATCHED");
        let msg = UserMessage::from_json(&json).unwrap();
        match msg {
            UserMessage::Trade(trade) => {
                assert_eq!(trade.id, "trade-1");
                assert_eq!(trade.status, TradeStatus::Matched);
                assert_eq!(trade.side, "BUY");
                assert_eq!(trade.outcome, "YES");
                assert_eq!(trade.maker_orders.len(), 1);
                assert_eq!(trade.maker_orders[0].order_id, "maker-1");
                assert_eq!(trade.owner.as_deref(), Some("0xowner"));
                assert_eq!(trade.transaction_hash.as_deref(), Some("0xtxhash"));
            }
            _ => panic!("Expected Trade variant"),
        }
    }

    #[test]
    fn deserialize_trade_all_statuses() {
        for (status_str, expected) in [
            ("MATCHED", TradeStatus::Matched),
            ("MINED", TradeStatus::Mined),
            ("CONFIRMED", TradeStatus::Confirmed),
            ("RETRYING", TradeStatus::Retrying),
            ("FAILED", TradeStatus::Failed),
        ] {
            let json = trade_json(status_str);
            let msg = UserMessage::from_json(&json).unwrap();
            match msg {
                UserMessage::Trade(trade) => assert_eq!(trade.status, expected),
                _ => panic!("Expected Trade variant for status {}", status_str),
            }
        }
    }

    #[test]
    fn deserialize_trade_optional_fields_absent() {
        let json = r#"{
            "event_type": "trade",
            "id": "trade-1",
            "asset_id": "abc123",
            "market": "0xcond",
            "outcome": "NO",
            "price": "0.45",
            "size": "50",
            "side": "SELL",
            "status": "CONFIRMED",
            "taker_order_id": "order-1",
            "maker_orders": [],
            "timestamp": "1700000000000"
        }"#;

        let msg = UserMessage::from_json(json).unwrap();
        match msg {
            UserMessage::Trade(trade) => {
                assert!(trade.owner.is_none());
                assert!(trade.transaction_hash.is_none());
                assert!(trade.maker_orders.is_empty());
            }
            _ => panic!("Expected Trade variant"),
        }
    }

    fn order_json(order_type: &str) -> String {
        format!(
            r#"{{
                "event_type": "order",
                "id": "order-1",
                "asset_id": "abc123",
                "market": "0xcond",
                "outcome": "YES",
                "price": "0.60",
                "side": "BUY",
                "original_size": "100",
                "size_matched": "50",
                "type": "{}",
                "order_owner": "0xowner",
                "timestamp": "1700000000000"
            }}"#,
            order_type
        )
    }

    #[test]
    fn deserialize_order_message_placement() {
        let json = order_json("PLACEMENT");
        let msg = UserMessage::from_json(&json).unwrap();
        match msg {
            UserMessage::Order(order) => {
                assert_eq!(order.id, "order-1");
                assert_eq!(order.order_type, OrderEventType::Placement);
                assert_eq!(order.original_size, "100");
                assert_eq!(order.size_matched, "50");
                assert_eq!(order.order_owner.as_deref(), Some("0xowner"));
            }
            _ => panic!("Expected Order variant"),
        }
    }

    #[test]
    fn deserialize_order_all_event_types() {
        for (type_str, expected) in [
            ("PLACEMENT", OrderEventType::Placement),
            ("UPDATE", OrderEventType::Update),
            ("CANCELLATION", OrderEventType::Cancellation),
        ] {
            let json = order_json(type_str);
            let msg = UserMessage::from_json(&json).unwrap();
            match msg {
                UserMessage::Order(order) => assert_eq!(order.order_type, expected),
                _ => panic!("Expected Order variant for type {}", type_str),
            }
        }
    }

    #[test]
    fn order_type_field_renamed_from_type() {
        // Verify that "type" in JSON maps to order_type in struct
        let json = order_json("UPDATE");
        let order: OrderMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(order.order_type, OrderEventType::Update);

        // Serialize back and verify it uses "type"
        let serialized = serde_json::to_string(&order).unwrap();
        assert!(serialized.contains(r#""type":"UPDATE""#));
    }

    #[test]
    fn deserialize_order_optional_owner_absent() {
        let json = r#"{
            "event_type": "order",
            "id": "order-1",
            "asset_id": "abc123",
            "market": "0xcond",
            "outcome": "NO",
            "price": "0.40",
            "side": "SELL",
            "original_size": "200",
            "size_matched": "0",
            "type": "CANCELLATION",
            "timestamp": "1700000000000"
        }"#;

        let msg = UserMessage::from_json(json).unwrap();
        match msg {
            UserMessage::Order(order) => {
                assert!(order.order_owner.is_none());
                assert_eq!(order.order_type, OrderEventType::Cancellation);
            }
            _ => panic!("Expected Order variant"),
        }
    }

    #[test]
    fn from_json_unknown_event_type_errors() {
        let json = r#"{"event_type": "unknown_event"}"#;
        let err = UserMessage::from_json(json).unwrap_err();
        assert!(err.to_string().contains("Unknown user event type"));
        assert!(err.to_string().contains("unknown_event"));
    }

    #[test]
    fn from_json_invalid_json_errors() {
        let err = UserMessage::from_json("not json").unwrap_err();
        assert!(err.is_data() || err.is_syntax());
    }

    #[test]
    fn from_json_missing_event_type_errors() {
        let json = r#"{"id": "order-1"}"#;
        let err = UserMessage::from_json(json).unwrap_err();
        assert!(err.is_data());
    }

    #[test]
    fn trade_status_serde_roundtrip() {
        let statuses = [
            TradeStatus::Matched,
            TradeStatus::Mined,
            TradeStatus::Confirmed,
            TradeStatus::Retrying,
            TradeStatus::Failed,
        ];
        for status in statuses {
            let serialized = serde_json::to_string(&status).unwrap();
            let deserialized: TradeStatus = serde_json::from_str(&serialized).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn order_event_type_serde_roundtrip() {
        let types = [
            OrderEventType::Placement,
            OrderEventType::Update,
            OrderEventType::Cancellation,
        ];
        for t in types {
            let serialized = serde_json::to_string(&t).unwrap();
            let deserialized: OrderEventType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(t, deserialized);
        }
    }

    #[test]
    fn trade_status_rejects_unknown_value() {
        let err = serde_json::from_str::<TradeStatus>(r#""UNKNOWN""#).unwrap_err();
        assert!(err.is_data());
    }

    #[test]
    fn order_event_type_rejects_unknown_value() {
        let err = serde_json::from_str::<OrderEventType>(r#""INVALID""#).unwrap_err();
        assert!(err.is_data());
    }

    #[test]
    fn trade_status_rejects_lowercase() {
        let err = serde_json::from_str::<TradeStatus>(r#""matched""#).unwrap_err();
        assert!(err.is_data());
    }

    #[test]
    fn trade_message_rejects_missing_required_field() {
        // Missing "id" field
        let json = r#"{
            "event_type": "trade",
            "asset_id": "abc",
            "market": "0xcond",
            "outcome": "YES",
            "price": "0.55",
            "size": "100",
            "side": "BUY",
            "status": "MATCHED",
            "taker_order_id": "o1",
            "maker_orders": [],
            "timestamp": "1700000000000"
        }"#;
        let err = serde_json::from_str::<TradeMessage>(json).unwrap_err();
        assert!(err.is_data());
    }

    #[test]
    fn maker_order_optional_fee_rate() {
        let json = r#"{
            "order_id": "maker-1",
            "maker_address": "0xmaker",
            "matched_amount": "50",
            "asset_id": "abc123",
            "price": "0.55"
        }"#;
        let maker: MakerOrder = serde_json::from_str(json).unwrap();
        assert!(maker.fee_rate_bps.is_none());
        assert_eq!(maker.matched_amount, "50");
    }
}
