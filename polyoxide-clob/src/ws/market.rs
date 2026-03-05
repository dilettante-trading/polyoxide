//! Market channel message types.
//!
//! The market channel provides real-time order book and price updates.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Order summary in the order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSummary {
    /// Price level
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Size at this price level
    #[serde(with = "rust_decimal::serde::str")]
    pub size: Decimal,
}

/// Book message - full order book snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMessage {
    /// Event type (always "book")
    pub event_type: String,
    /// Asset ID (token ID)
    pub asset_id: String,
    /// Market condition ID
    pub market: String,
    /// Timestamp in milliseconds (as string)
    pub timestamp: String,
    /// Order book hash
    pub hash: String,
    /// Buy orders (bids)
    pub bids: Vec<OrderSummary>,
    /// Sell orders (asks)
    pub asks: Vec<OrderSummary>,
    /// Last trade price
    pub last_trade_price: Option<String>,
}

/// Price change entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceChange {
    /// Asset ID (token ID)
    pub asset_id: String,
    /// Price level
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Size at this price level
    #[serde(with = "rust_decimal::serde::str")]
    pub size: Decimal,
    /// Order side (BUY or SELL)
    pub side: String,
    /// Order book hash
    pub hash: String,
    /// Best bid price
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub best_bid: Option<Decimal>,
    /// Best ask price
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub best_ask: Option<Decimal>,
}

/// Price change message - incremental order book update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceChangeMessage {
    /// Event type (always "price_change")
    pub event_type: String,
    /// Market condition ID
    pub market: String,
    /// List of price changes
    pub price_changes: Vec<PriceChange>,
    /// Timestamp in milliseconds (as string)
    pub timestamp: String,
}

/// Tick size change message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickSizeChangeMessage {
    /// Event type (always "tick_size_change")
    pub event_type: String,
    /// Asset ID (token ID)
    pub asset_id: String,
    /// Market condition ID
    pub market: String,
    /// Old tick size
    pub old_tick_size: String,
    /// New tick size
    pub new_tick_size: String,
    /// Side (BUY or SELL)
    pub side: String,
    /// Timestamp in milliseconds (as string)
    pub timestamp: String,
}

/// Last trade price message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastTradePriceMessage {
    /// Event type (always "last_trade_price")
    pub event_type: String,
    /// Asset ID (token ID)
    pub asset_id: String,
    /// Market condition ID
    pub market: String,
    /// Trade price
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Trade side (BUY or SELL)
    pub side: String,
    /// Trade size
    #[serde(with = "rust_decimal::serde::str")]
    pub size: Decimal,
    /// Fee rate
    pub fee_rate_bps: Option<String>,
    /// Timestamp in milliseconds (as string)
    pub timestamp: String,
}

/// Market channel message types
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MarketMessage {
    /// Full order book snapshot
    Book(BookMessage),
    /// Incremental price change
    PriceChange(PriceChangeMessage),
    /// Tick size change
    TickSizeChange(TickSizeChangeMessage),
    /// Last trade price
    LastTradePrice(LastTradePriceMessage),
}

impl MarketMessage {
    /// Parse a market channel message from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        // Book messages come as an array with a single element
        if json.starts_with('[') {
            let books: Vec<BookMessage> = serde_json::from_str(json)?;
            if let Some(book) = books.into_iter().next() {
                return Ok(MarketMessage::Book(book));
            }
            return Err(serde::de::Error::custom("Empty book array"));
        }

        #[derive(Deserialize)]
        struct RawMessage {
            event_type: String,
        }

        let raw: RawMessage = serde_json::from_str(json)?;
        match raw.event_type.as_str() {
            "book" => Ok(MarketMessage::Book(serde_json::from_str(json)?)),
            "price_change" => Ok(MarketMessage::PriceChange(serde_json::from_str(json)?)),
            "tick_size_change" => Ok(MarketMessage::TickSizeChange(serde_json::from_str(json)?)),
            "last_trade_price" => Ok(MarketMessage::LastTradePrice(serde_json::from_str(json)?)),
            _ => Err(serde::de::Error::custom(format!(
                "Unknown market event type: {}",
                raw.event_type
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn deserialize_book_message_array_format() {
        let json = r#"[{
            "event_type": "book",
            "asset_id": "abc123",
            "market": "0xcond",
            "timestamp": "1700000000000",
            "hash": "0xhash",
            "bids": [{"price": "0.55", "size": "100"}],
            "asks": [{"price": "0.60", "size": "200"}],
            "last_trade_price": "0.57"
        }]"#;

        let msg = MarketMessage::from_json(json).unwrap();
        match msg {
            MarketMessage::Book(book) => {
                assert_eq!(book.event_type, "book");
                assert_eq!(book.asset_id, "abc123");
                assert_eq!(book.market, "0xcond");
                assert_eq!(book.hash, "0xhash");
                assert_eq!(book.bids.len(), 1);
                assert_eq!(book.bids[0].price, dec!(0.55));
                assert_eq!(book.bids[0].size, dec!(100));
                assert_eq!(book.asks.len(), 1);
                assert_eq!(book.asks[0].price, dec!(0.60));
                assert_eq!(book.asks[0].size, dec!(200));
                assert_eq!(book.last_trade_price.as_deref(), Some("0.57"));
            }
            _ => panic!("Expected Book variant"),
        }
    }

    #[test]
    fn deserialize_book_message_object_format() {
        let json = r#"{
            "event_type": "book",
            "asset_id": "abc123",
            "market": "0xcond",
            "timestamp": "1700000000000",
            "hash": "0xhash",
            "bids": [],
            "asks": [],
            "last_trade_price": null
        }"#;

        let msg = MarketMessage::from_json(json).unwrap();
        match msg {
            MarketMessage::Book(book) => {
                assert_eq!(book.event_type, "book");
                assert!(book.bids.is_empty());
                assert!(book.asks.is_empty());
                assert!(book.last_trade_price.is_none());
            }
            _ => panic!("Expected Book variant"),
        }
    }

    #[test]
    fn deserialize_book_empty_array_errors() {
        let json = "[]";
        let err = MarketMessage::from_json(json).unwrap_err();
        assert!(err.to_string().contains("Empty book array"));
    }

    #[test]
    fn deserialize_price_change_message() {
        let json = r#"{
            "event_type": "price_change",
            "market": "0xcond",
            "price_changes": [{
                "asset_id": "abc123",
                "price": "0.55",
                "size": "100",
                "side": "BUY",
                "hash": "0xhash",
                "best_bid": "0.54",
                "best_ask": "0.56"
            }],
            "timestamp": "1700000000000"
        }"#;

        let msg = MarketMessage::from_json(json).unwrap();
        match msg {
            MarketMessage::PriceChange(pc) => {
                assert_eq!(pc.market, "0xcond");
                assert_eq!(pc.price_changes.len(), 1);
                let change = &pc.price_changes[0];
                assert_eq!(change.price, dec!(0.55));
                assert_eq!(change.size, dec!(100));
                assert_eq!(change.side, "BUY");
                assert_eq!(change.best_bid, Some(dec!(0.54)));
                assert_eq!(change.best_ask, Some(dec!(0.56)));
            }
            _ => panic!("Expected PriceChange variant"),
        }
    }

    #[test]
    fn deserialize_price_change_optional_fields() {
        let json = r#"{
            "event_type": "price_change",
            "market": "0xcond",
            "price_changes": [{
                "asset_id": "abc123",
                "price": "0.55",
                "size": "100",
                "side": "SELL",
                "hash": "0xhash"
            }],
            "timestamp": "1700000000000"
        }"#;

        let msg = MarketMessage::from_json(json).unwrap();
        match msg {
            MarketMessage::PriceChange(pc) => {
                let change = &pc.price_changes[0];
                assert!(change.best_bid.is_none());
                assert!(change.best_ask.is_none());
            }
            _ => panic!("Expected PriceChange variant"),
        }
    }

    #[test]
    fn deserialize_tick_size_change_message() {
        let json = r#"{
            "event_type": "tick_size_change",
            "asset_id": "abc123",
            "market": "0xcond",
            "old_tick_size": "0.01",
            "new_tick_size": "0.001",
            "side": "BUY",
            "timestamp": "1700000000000"
        }"#;

        let msg = MarketMessage::from_json(json).unwrap();
        match msg {
            MarketMessage::TickSizeChange(tsc) => {
                assert_eq!(tsc.asset_id, "abc123");
                assert_eq!(tsc.old_tick_size, "0.01");
                assert_eq!(tsc.new_tick_size, "0.001");
                assert_eq!(tsc.side, "BUY");
            }
            _ => panic!("Expected TickSizeChange variant"),
        }
    }

    #[test]
    fn deserialize_last_trade_price_message() {
        let json = r#"{
            "event_type": "last_trade_price",
            "asset_id": "abc123",
            "market": "0xcond",
            "price": "0.72",
            "side": "SELL",
            "size": "50",
            "fee_rate_bps": "200",
            "timestamp": "1700000000000"
        }"#;

        let msg = MarketMessage::from_json(json).unwrap();
        match msg {
            MarketMessage::LastTradePrice(ltp) => {
                assert_eq!(ltp.price, dec!(0.72));
                assert_eq!(ltp.side, "SELL");
                assert_eq!(ltp.size, dec!(50));
                assert_eq!(ltp.fee_rate_bps.as_deref(), Some("200"));
            }
            _ => panic!("Expected LastTradePrice variant"),
        }
    }

    #[test]
    fn deserialize_last_trade_price_optional_fee() {
        let json = r#"{
            "event_type": "last_trade_price",
            "asset_id": "abc123",
            "market": "0xcond",
            "price": "0.72",
            "side": "BUY",
            "size": "50",
            "timestamp": "1700000000000"
        }"#;

        let msg = MarketMessage::from_json(json).unwrap();
        match msg {
            MarketMessage::LastTradePrice(ltp) => {
                assert!(ltp.fee_rate_bps.is_none());
            }
            _ => panic!("Expected LastTradePrice variant"),
        }
    }

    #[test]
    fn from_json_unknown_event_type_errors() {
        let json = r#"{"event_type": "unknown_event", "market": "0x1"}"#;
        let err = MarketMessage::from_json(json).unwrap_err();
        assert!(err.to_string().contains("Unknown market event type"));
        assert!(err.to_string().contains("unknown_event"));
    }

    #[test]
    fn from_json_invalid_json_errors() {
        let err = MarketMessage::from_json("not json at all").unwrap_err();
        assert!(err.is_data() || err.is_syntax());
    }

    #[test]
    fn from_json_missing_event_type_errors() {
        let json = r#"{"market": "0xcond"}"#;
        let err = MarketMessage::from_json(json).unwrap_err();
        assert!(err.is_data());
    }

    #[test]
    fn order_summary_decimal_precision() {
        let json = r#"{"price": "0.123456789", "size": "999999.999"}"#;
        let summary: OrderSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.price, dec!(0.123456789));
        assert_eq!(summary.size, dec!(999999.999));
    }

    #[test]
    fn book_message_multiple_bids_asks() {
        let json = r#"[{
            "event_type": "book",
            "asset_id": "abc123",
            "market": "0xcond",
            "timestamp": "1700000000000",
            "hash": "0xhash",
            "bids": [
                {"price": "0.55", "size": "100"},
                {"price": "0.54", "size": "200"},
                {"price": "0.53", "size": "300"}
            ],
            "asks": [
                {"price": "0.60", "size": "50"},
                {"price": "0.61", "size": "75"}
            ]
        }]"#;

        let msg = MarketMessage::from_json(json).unwrap();
        match msg {
            MarketMessage::Book(book) => {
                assert_eq!(book.bids.len(), 3);
                assert_eq!(book.asks.len(), 2);
                assert_eq!(book.bids[2].price, dec!(0.53));
                assert_eq!(book.bids[2].size, dec!(300));
            }
            _ => panic!("Expected Book variant"),
        }
    }
}
