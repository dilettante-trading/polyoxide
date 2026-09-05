//! Subscription frames.
//!
//! The venue's `filters` field is a JSON document embedded in a JSON string,
//! and it is unforgiving: `{"symbol": "btc/usd"}` — one space — still delivers
//! the subscribe backfill and then never sends another update, with no error
//! of any kind. This module therefore never accepts a caller-supplied filter
//! string; it builds one with [`serde_json`], which is compact by
//! construction.

use serde::Serialize;

use crate::topic::Topic;

/// The symbol filter, serialised as compact JSON into the `filters` string.
#[derive(Debug, Serialize)]
struct SymbolFilter<'a> {
    symbol: &'a str,
}

/// One topic subscription, optionally narrowed to a single symbol.
///
/// The venue accepts at most one symbol per entry — an array of symbols
/// returns zero frames — so several symbols means several entries. Build them
/// with [`Subscription::symbols`], which fans out for you.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    topic: Topic,
    symbol: Option<String>,
}

impl Subscription {
    /// Subscribe to every symbol on a topic.
    pub fn for_topic(topic: Topic) -> Self {
        Self {
            topic,
            symbol: None,
        }
    }

    /// Narrow this subscription to one symbol.
    ///
    /// Symbol format is topic-specific: `btcusdt` for
    /// [`Topic::BinanceSpot`], `btc/usd` for the Chainlink topics. Matching is
    /// case-insensitive despite the upstream documentation's claim otherwise.
    pub fn symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    /// Fan out into one subscription per symbol.
    ///
    /// Returns an empty vector for an empty input, which is deliberate: an
    /// empty list must not collapse into an unfiltered subscription for every
    /// symbol on the topic. Discards any symbol set by [`symbol`](Self::symbol)
    /// — this fans out from the topic, not from an existing single-symbol
    /// subscription.
    pub fn symbols<I, S>(self, symbols: I) -> Vec<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        symbols
            .into_iter()
            .map(|symbol| Self {
                topic: self.topic,
                symbol: Some(symbol.into()),
            })
            .collect()
    }

    /// The topic this subscription covers.
    pub fn topic(&self) -> Topic {
        self.topic
    }

    /// The symbol this subscription is narrowed to, if any.
    pub fn symbol_filter(&self) -> Option<&str> {
        self.symbol.as_deref()
    }
}

impl Serialize for Subscription {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        // serde_derive computes an exact length from its skip predicates
        // rather than trusting `skip_field` to patch a wrong one, because a
        // definite-length format (MessagePack, CBOR, postcard) writes the
        // header before the fields and never revises it. JSON ignores the
        // hint, which is exactly why getting this wrong stays invisible here.
        let len = 2 + usize::from(self.symbol.is_some());
        let mut entry = serializer.serialize_struct("Subscription", len)?;
        entry.serialize_field("topic", self.topic.as_wire())?;
        entry.serialize_field("type", "update")?;
        if let Some(symbol) = &self.symbol {
            // `to_string` never emits spaces, which is exactly the property
            // the venue requires and hand-written filters get wrong.
            let filters = serde_json::to_string(&SymbolFilter { symbol })
                .map_err(serde::ser::Error::custom)?;
            entry.serialize_field("filters", &filters)?;
        } else {
            entry.skip_field("filters")?;
        }
        entry.end()
    }
}

/// The frame sent immediately after connecting.
#[derive(Debug, Serialize)]
pub struct SubscriptionRequest {
    action: &'static str,
    subscriptions: Vec<Subscription>,
}

impl SubscriptionRequest {
    /// Build a subscribe frame from a set of subscriptions.
    pub fn new(subscriptions: impl IntoIterator<Item = Subscription>) -> Self {
        Self {
            action: "subscribe",
            subscriptions: subscriptions.into_iter().collect(),
        }
    }

    /// The subscriptions carried by this frame.
    pub fn subscriptions(&self) -> &[Subscription] {
        &self.subscriptions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topic::TwapWindow;

    #[test]
    fn filter_is_compact_json_with_no_spaces() {
        // A single space here makes the venue deliver the snapshot and then
        // fall silent forever. The encoding must be exact.
        let sub =
            Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty)).symbol("btc/usd");
        let request = SubscriptionRequest::new([sub]);
        let json = serde_json::to_string(&request).unwrap();

        assert!(
            json.contains(r#""filters":"{\"symbol\":\"btc/usd\"}""#),
            "filter must be compact JSON, got: {json}"
        );
        assert!(
            !json.contains(r#"{\"symbol\": \"btc/usd\"}"#),
            "a space in the filter silently kills updates: {json}"
        );
    }

    #[test]
    fn request_matches_the_documented_envelope() {
        let sub = Subscription::for_topic(Topic::ChainlinkSpot).symbol("eth/usd");
        let value = serde_json::to_value(SubscriptionRequest::new([sub])).unwrap();

        assert_eq!(value["action"], "subscribe");
        assert_eq!(
            value["subscriptions"][0]["topic"],
            "crypto_prices_chainlink"
        );
        assert_eq!(value["subscriptions"][0]["type"], "update");
        assert_eq!(
            value["subscriptions"][0]["filters"],
            r#"{"symbol":"eth/usd"}"#
        );
    }

    #[test]
    fn an_unfiltered_subscription_omits_filters_entirely() {
        let value = serde_json::to_value(SubscriptionRequest::new([Subscription::for_topic(
            Topic::BinanceSpot,
        )]))
        .unwrap();

        assert!(
            value["subscriptions"][0]
                .as_object()
                .unwrap()
                .get("filters")
                .is_none(),
            "omit filters to receive every symbol; null is not the same request"
        );
    }

    #[test]
    fn multiple_symbols_fan_out_into_separate_entries() {
        // `{"symbol":["btc/usd","eth/usd"]}` returns zero frames. One symbol
        // per subscription entry is the only form the venue accepts.
        let subs = Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Sixty))
            .symbols(["btc/usd", "eth/usd"]);
        assert_eq!(subs.len(), 2);

        let value = serde_json::to_value(SubscriptionRequest::new(subs)).unwrap();
        let entries = value["subscriptions"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["filters"], r#"{"symbol":"btc/usd"}"#);
        assert_eq!(entries[1]["filters"], r#"{"symbol":"eth/usd"}"#);
        assert_eq!(entries[0]["topic"], "crypto_prices_twap_sixty");
        assert_eq!(entries[1]["topic"], "crypto_prices_twap_sixty");
    }

    #[test]
    fn symbols_with_no_arguments_yields_no_subscriptions() {
        let subs = Subscription::for_topic(Topic::BinanceSpot).symbols(Vec::<String>::new());
        assert!(
            subs.is_empty(),
            "an empty symbol list must not silently become an unfiltered subscription"
        );
    }

    #[test]
    fn a_symbol_needing_escaping_survives_the_nested_encoding() {
        // `filters` is JSON inside a JSON string, so a symbol containing a
        // quote or backslash gets escaped twice. No real Polymarket symbol
        // looks like this; the test exists so that anyone tempted to build
        // the filter with `format!` instead of serde_json breaks a test
        // rather than the wire format.
        let sub = Subscription::for_topic(Topic::ChainlinkSpot).symbol(r#"weird"symbol\x"#);
        let value = serde_json::to_value(SubscriptionRequest::new([sub])).unwrap();

        let filters = value["subscriptions"][0]["filters"].as_str().unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(filters).unwrap();
        assert_eq!(reparsed["symbol"], r#"weird"symbol\x"#);
    }
}
