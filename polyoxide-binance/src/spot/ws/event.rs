//! Stream payloads, and the update that carries one.
//!
//! The socket uses one-letter keys and different fields from REST, so each
//! stream has its own payload type, with long field names over the wire's keys.
//! Symbols are `String`, as sent. The keys Binance documents as "Ignore", `M`
//! on trades and aggregate trades and `B` on the kline, are not modelled.

use std::fmt;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::spot::{
    types::{Interval, Level},
    ws::{error::SpotWsError, stream::StreamName},
};

/// `<s>@aggTrade`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AggTradeEvent {
    /// `aggTrade`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Aggregate trade id.
    #[serde(rename = "a")]
    pub id: u64,
    /// Price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// First trade id merged.
    #[serde(rename = "f")]
    pub first_trade_id: u64,
    /// Last trade id merged.
    #[serde(rename = "l")]
    pub last_trade_id: u64,
    /// Trade time.
    #[serde(rename = "T")]
    pub trade_time: u64,
    /// Whether the buyer was the maker, so the taker sold.
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// `<s>@trade`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TradeEvent {
    /// `trade`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Trade id.
    #[serde(rename = "t")]
    pub id: u64,
    /// Price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Trade time.
    #[serde(rename = "T")]
    pub trade_time: u64,
    /// Whether the buyer was the maker, so the taker sold.
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// `<s>@kline_<interval>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct KlineEvent {
    /// `kline`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The candle so far.
    #[serde(rename = "k")]
    pub kline: KlineBar,
}

/// The candle in a [`KlineEvent`]. The wire's `B`, documented as "Ignore", is
/// dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct KlineBar {
    /// Candle start.
    #[serde(rename = "t")]
    pub open_time: u64,
    /// Candle end, inclusive.
    #[serde(rename = "T")]
    pub close_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The candle's width.
    #[serde(rename = "i")]
    pub interval: Interval,
    /// First trade id; `-1` when the candle holds no trade.
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    /// Last trade id; `-1` when the candle holds no trade.
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    /// Open price.
    #[serde(rename = "o", with = "rust_decimal::serde::str")]
    pub open: Decimal,
    /// Close price so far.
    #[serde(rename = "c", with = "rust_decimal::serde::str")]
    pub close: Decimal,
    /// High price.
    #[serde(rename = "h", with = "rust_decimal::serde::str")]
    pub high: Decimal,
    /// Low price.
    #[serde(rename = "l", with = "rust_decimal::serde::str")]
    pub low: Decimal,
    /// Base-asset volume.
    #[serde(rename = "v", with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Trades.
    #[serde(rename = "n")]
    pub trade_count: u64,
    /// Whether the candle is closed.
    #[serde(rename = "x")]
    pub is_closed: bool,
    /// Quote-asset volume.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quote_volume: Decimal,
    /// Base-asset volume bought by takers.
    #[serde(rename = "V", with = "rust_decimal::serde::str")]
    pub taker_buy_base_volume: Decimal,
    /// Quote-asset volume bought by takers.
    #[serde(rename = "Q", with = "rust_decimal::serde::str")]
    pub taker_buy_quote_volume: Decimal,
}

/// `<s>@miniTicker` and each row of `!miniTicker@arr`: the 24-hour window's
/// close, open, high, low and volumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MiniTickerEvent {
    /// `24hrMiniTicker`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Close price: the last price.
    #[serde(rename = "c", with = "rust_decimal::serde::str")]
    pub close_price: Decimal,
    /// Price 24 hours ago.
    #[serde(rename = "o", with = "rust_decimal::serde::str")]
    pub open_price: Decimal,
    /// Highest price.
    #[serde(rename = "h", with = "rust_decimal::serde::str")]
    pub high_price: Decimal,
    /// Lowest price.
    #[serde(rename = "l", with = "rust_decimal::serde::str")]
    pub low_price: Decimal,
    /// Base-asset volume.
    #[serde(rename = "v", with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Quote-asset volume.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quote_volume: Decimal,
}

/// `<s>@ticker`: rolling 24-hour statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TickerEvent {
    /// `24hrTicker`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Last price less the open price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price_change: Decimal,
    /// The change as a percentage.
    #[serde(rename = "P", with = "rust_decimal::serde::str")]
    pub price_change_percent: Decimal,
    /// Volume-weighted average price.
    #[serde(rename = "w", with = "rust_decimal::serde::str")]
    pub weighted_avg_price: Decimal,
    /// The price before the window opened.
    #[serde(rename = "x", with = "rust_decimal::serde::str")]
    pub prev_close_price: Decimal,
    /// Last price.
    #[serde(rename = "c", with = "rust_decimal::serde::str")]
    pub last_price: Decimal,
    /// Last quantity.
    #[serde(rename = "Q", with = "rust_decimal::serde::str")]
    pub last_quantity: Decimal,
    /// Best bid price.
    #[serde(rename = "b", with = "rust_decimal::serde::str")]
    pub bid_price: Decimal,
    /// Best bid quantity.
    #[serde(rename = "B", with = "rust_decimal::serde::str")]
    pub bid_quantity: Decimal,
    /// Best ask price.
    #[serde(rename = "a", with = "rust_decimal::serde::str")]
    pub ask_price: Decimal,
    /// Best ask quantity.
    #[serde(rename = "A", with = "rust_decimal::serde::str")]
    pub ask_quantity: Decimal,
    /// Price 24 hours ago.
    #[serde(rename = "o", with = "rust_decimal::serde::str")]
    pub open_price: Decimal,
    /// Highest price.
    #[serde(rename = "h", with = "rust_decimal::serde::str")]
    pub high_price: Decimal,
    /// Lowest price.
    #[serde(rename = "l", with = "rust_decimal::serde::str")]
    pub low_price: Decimal,
    /// Base-asset volume.
    #[serde(rename = "v", with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Quote-asset volume.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quote_volume: Decimal,
    /// Window start.
    #[serde(rename = "O")]
    pub open_time: u64,
    /// Window end.
    #[serde(rename = "C")]
    pub close_time: u64,
    /// First trade id in the window.
    #[serde(rename = "F")]
    pub first_trade_id: i64,
    /// Last trade id in the window.
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    /// Trades in the window.
    #[serde(rename = "n")]
    pub trade_count: u64,
}

/// `<s>@bookTicker`: the best bid and ask. The frame carries no event type
/// and no time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BookTickerEvent {
    /// Book update id.
    #[serde(rename = "u")]
    pub update_id: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Best bid price.
    #[serde(rename = "b", with = "rust_decimal::serde::str")]
    pub bid_price: Decimal,
    /// Best bid quantity.
    #[serde(rename = "B", with = "rust_decimal::serde::str")]
    pub bid_quantity: Decimal,
    /// Best ask price.
    #[serde(rename = "a", with = "rust_decimal::serde::str")]
    pub ask_price: Decimal,
    /// Best ask quantity.
    #[serde(rename = "A", with = "rust_decimal::serde::str")]
    pub ask_quantity: Decimal,
}

/// `<s>@depth<levels>` and `<s>@depth<levels>@100ms`: the top of the book.
///
/// The frame carries `lastUpdateId`, `bids` and `asks` and no symbol, so
/// `symbol` is the envelope's, uppercased as [`Symbol`](crate::spot::types::Symbol)
/// renders it, and it is not serialised back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PartialDepthEvent {
    /// The symbol, from the envelope's stream name.
    #[serde(skip)]
    pub symbol: String,
    /// Book update id of the snapshot.
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    /// Bids, best first.
    pub bids: Vec<Level>,
    /// Asks, best first.
    pub asks: Vec<Level>,
}

/// `<s>@avgPrice`: the average price over `interval`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AvgPriceEvent {
    /// `avgPrice`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The window the average covers, such as `5m`.
    #[serde(rename = "i")]
    pub interval: String,
    /// The average price.
    #[serde(rename = "w", with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Time of the last trade.
    #[serde(rename = "T")]
    pub last_trade_time: u64,
}

/// What a stream carries, by stream kind.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Payload {
    /// `<s>@aggTrade`.
    AggTrade(AggTradeEvent),
    /// `<s>@trade`.
    Trade(TradeEvent),
    /// `<s>@kline_<interval>`.
    Kline(KlineEvent),
    /// `<s>@miniTicker`.
    MiniTicker(MiniTickerEvent),
    /// `!miniTicker@arr`: only the symbols that changed. A row's `e` is not
    /// checked, and one row that does not decode fails the whole frame.
    MiniTickers(Vec<MiniTickerEvent>),
    /// `<s>@ticker`.
    Ticker(TickerEvent),
    /// `<s>@bookTicker`.
    BookTicker(BookTickerEvent),
    /// `<s>@depth<levels>` and `<s>@depth<levels>@100ms`.
    PartialDepth(PartialDepthEvent),
    /// `<s>@avgPrice`.
    AvgPrice(AvgPriceEvent),
    /// A single-symbol stream's object whose event type (`e`) is not the one
    /// its stream carries, kept whole rather than failing the frame. The live
    /// suite fails on one, since it means Binance renamed an event.
    Unknown {
        /// The payload's `e`, or empty when it has none.
        event_type: String,
        /// The payload, re-serialised: its values as sent, its keys sorted.
        raw: String,
    },
}

impl Serialize for Payload {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::AggTrade(event) => event.serialize(serializer),
            Self::Trade(event) => event.serialize(serializer),
            Self::Kline(event) => event.serialize(serializer),
            Self::MiniTicker(event) => event.serialize(serializer),
            Self::MiniTickers(rows) => rows.serialize(serializer),
            Self::Ticker(event) => event.serialize(serializer),
            Self::BookTicker(event) => event.serialize(serializer),
            Self::PartialDepth(event) => event.serialize(serializer),
            Self::AvgPrice(event) => event.serialize(serializer),
            Self::Unknown { raw, .. } => serde_json::from_str::<Value>(raw)
                .map_err(serde::ser::Error::custom)?
                .serialize(serializer),
        }
    }
}

/// One frame of a combined stream: `{"stream": <name>, "data": <payload>}`.
///
/// It serialises back to that envelope, so `serde_json::to_string(&update)`
/// is the frame as the wire sent it, less the fields this crate drops.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Update {
    /// The stream the frame arrived on.
    pub stream: StreamName,
    /// What it carries.
    pub payload: Payload,
}

impl Serialize for Update {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("stream", &self.stream.to_string())?;
        map.serialize_entry("data", &self.payload)?;
        map.end()
    }
}

#[derive(Deserialize)]
struct Envelope {
    stream: String,
    data: Value,
}

impl Update {
    /// Decodes one combined-stream frame.
    ///
    /// Fails with [`SpotWsError::Frame`] when the text is not an envelope, its
    /// stream is not one this crate builds, or its payload does not decode as
    /// the stream's type.
    pub fn from_json(text: &str) -> Result<Self, SpotWsError> {
        let envelope: Envelope =
            serde_json::from_str(text).map_err(|err| frame_error("", text, err))?;
        let stream: StreamName = envelope
            .stream
            .parse()
            .map_err(|err| frame_error(&envelope.stream, text, err))?;
        let payload = decode(&stream, envelope.data)
            .map_err(|err| frame_error(&envelope.stream, text, err))?;
        Ok(Self { stream, payload })
    }
}

fn frame_error(stream: &str, raw: &str, reason: impl fmt::Display) -> SpotWsError {
    SpotWsError::Frame {
        stream: stream.to_owned(),
        raw: raw.to_owned(),
        reason: reason.to_string(),
    }
}

fn decode(stream: &StreamName, data: Value) -> Result<Payload, serde_json::Error> {
    // The streams whose frames carry no event type, and the array stream, decode
    // as they are. A partial-depth frame carries no symbol either: the
    // envelope's stream name supplies it.
    let expected = match stream {
        StreamName::AllMiniTickers => {
            return serde_json::from_value(data).map(Payload::MiniTickers)
        }
        StreamName::BookTicker(_) => return serde_json::from_value(data).map(Payload::BookTicker),
        StreamName::PartialDepth(symbol, ..) => {
            let mut event: PartialDepthEvent = serde_json::from_value(data)?;
            event.symbol = symbol.to_string();
            return Ok(Payload::PartialDepth(event));
        }
        StreamName::AggTrade(_) => "aggTrade",
        StreamName::Trade(_) => "trade",
        StreamName::Kline(..) => "kline",
        StreamName::MiniTicker(_) => "24hrMiniTicker",
        StreamName::Ticker(_) => "24hrTicker",
        StreamName::AvgPrice(_) => "avgPrice",
    };
    let event_type = data.get("e").and_then(Value::as_str).unwrap_or_default();
    if event_type != expected {
        return Ok(Payload::Unknown {
            event_type: event_type.to_owned(),
            raw: data.to_string(),
        });
    }
    Ok(match stream {
        StreamName::AggTrade(_) => Payload::AggTrade(serde_json::from_value(data)?),
        StreamName::Trade(_) => Payload::Trade(serde_json::from_value(data)?),
        StreamName::Kline(..) => Payload::Kline(serde_json::from_value(data)?),
        StreamName::MiniTicker(_) => Payload::MiniTicker(serde_json::from_value(data)?),
        StreamName::Ticker(_) => Payload::Ticker(serde_json::from_value(data)?),
        StreamName::AvgPrice(_) => Payload::AvgPrice(serde_json::from_value(data)?),
        StreamName::AllMiniTickers | StreamName::BookTicker(_) | StreamName::PartialDepth(..) => {
            unreachable!("returned above")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spot::ws::fixtures;
    use serde_json::json;

    fn wire(frame: &str) -> Value {
        serde_json::from_str(frame).unwrap()
    }

    fn data(frame: &str) -> Value {
        wire(frame)["data"].clone()
    }

    fn dec(row: &Value, key: &str) -> Decimal {
        row[key]
            .as_str()
            .unwrap_or_else(|| panic!("{key} is not a string"))
            .parse()
            .unwrap()
    }

    fn int(row: &Value, key: &str) -> u64 {
        row[key]
            .as_u64()
            .unwrap_or_else(|| panic!("{key} is not a u64"))
    }

    fn signed(row: &Value, key: &str) -> i64 {
        row[key]
            .as_i64()
            .unwrap_or_else(|| panic!("{key} is not an i64"))
    }

    fn text(row: &Value, key: &str) -> String {
        row[key]
            .as_str()
            .unwrap_or_else(|| panic!("{key} is not a string"))
            .to_owned()
    }

    fn payload(frame: &str) -> Payload {
        Update::from_json(frame).unwrap().payload
    }

    /// A captured frame with its envelope's stream name replaced.
    fn on_stream(frame: &str, stream: &str) -> String {
        let mut frame = wire(frame);
        frame["stream"] = json!(stream);
        frame.to_string()
    }

    #[test]
    fn every_captured_frame_decodes_as_its_stream() {
        for (name, frame) in fixtures::ALL {
            let update = Update::from_json(frame).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                !matches!(update.payload, Payload::Unknown { .. }),
                "{name} decoded as Unknown"
            );
        }
    }

    #[test]
    fn each_frame_lands_in_the_payload_its_stream_names() {
        assert!(matches!(payload(fixtures::AGG_TRADE), Payload::AggTrade(_)));
        assert!(matches!(payload(fixtures::TRADE), Payload::Trade(_)));
        assert!(matches!(payload(fixtures::KLINE), Payload::Kline(_)));
        assert!(matches!(
            payload(fixtures::MINI_TICKER),
            Payload::MiniTicker(_)
        ));
        assert!(matches!(
            payload(fixtures::ALL_MINI_TICKERS),
            Payload::MiniTickers(_)
        ));
        assert!(matches!(payload(fixtures::TICKER), Payload::Ticker(_)));
        assert!(matches!(
            payload(fixtures::BOOK_TICKER),
            Payload::BookTicker(_)
        ));
        assert!(matches!(
            payload(fixtures::PARTIAL_DEPTH),
            Payload::PartialDepth(_)
        ));
        assert!(matches!(
            payload(fixtures::PARTIAL_DEPTH_100MS),
            Payload::PartialDepth(_)
        ));
        assert!(matches!(payload(fixtures::AVG_PRICE), Payload::AvgPrice(_)));
        assert!(matches!(
            payload(fixtures::NON_ASCII_MINI_TICKER),
            Payload::MiniTicker(_)
        ));
    }

    /// The round trip proves the key set, not which field a key lands in: two
    /// fields of one type swapped would pass it. Each field is compared here
    /// with its own key in the raw frame, as Binance's pages define the keys,
    /// so this holds for any capture. Where two keys carry equal values on
    /// every frame, a swap of those two stays invisible.
    #[test]
    fn each_field_holds_the_value_of_its_own_key() {
        fn mini(e: &MiniTickerEvent, row: &Value) {
            assert_eq!(
                [
                    e.close_price,
                    e.open_price,
                    e.high_price,
                    e.low_price,
                    e.volume,
                    e.quote_volume,
                ],
                ["c", "o", "h", "l", "v", "q"].map(|k| dec(row, k)),
                "{}",
                e.symbol
            );
            assert_eq!(e.event_time, int(row, "E"), "{}", e.symbol);
            assert_eq!(e.symbol, text(row, "s"));
            assert_eq!(e.event_type, text(row, "e"));
        }

        let Payload::MiniTickers(rows) = payload(fixtures::ALL_MINI_TICKERS) else {
            panic!("not MiniTickers");
        };
        assert_eq!(rows.len(), 2);
        for (e, row) in rows
            .iter()
            .zip(data(fixtures::ALL_MINI_TICKERS).as_array().unwrap())
        {
            mini(e, row);
        }
        for frame in [fixtures::MINI_TICKER, fixtures::NON_ASCII_MINI_TICKER] {
            let Payload::MiniTicker(e) = payload(frame) else {
                panic!("not MiniTicker");
            };
            mini(&e, &data(frame));
        }

        let row = data(fixtures::TICKER);
        let Payload::Ticker(e) = payload(fixtures::TICKER) else {
            panic!("not Ticker");
        };
        assert_eq!(
            [
                e.price_change,
                e.price_change_percent,
                e.weighted_avg_price,
                e.prev_close_price,
                e.last_price,
                e.last_quantity,
                e.bid_price,
                e.bid_quantity,
                e.ask_price,
                e.ask_quantity,
                e.open_price,
                e.high_price,
                e.low_price,
                e.volume,
                e.quote_volume,
            ],
            ["p", "P", "w", "x", "c", "Q", "b", "B", "a", "A", "o", "h", "l", "v", "q"]
                .map(|k| dec(&row, k))
        );
        assert_eq!(e.symbol, text(&row, "s"));
        assert_eq!(e.event_type, text(&row, "e"));
        assert_eq!(
            [e.event_time, e.open_time, e.close_time, e.trade_count],
            ["E", "O", "C", "n"].map(|k| int(&row, k))
        );
        assert_eq!(
            [e.first_trade_id, e.last_trade_id],
            ["F", "L"].map(|k| signed(&row, k))
        );

        let row = data(fixtures::AGG_TRADE);
        let Payload::AggTrade(e) = payload(fixtures::AGG_TRADE) else {
            panic!("not AggTrade");
        };
        assert_eq!([e.price, e.quantity], ["p", "q"].map(|k| dec(&row, k)));
        assert_eq!(
            [
                e.event_time,
                e.id,
                e.first_trade_id,
                e.last_trade_id,
                e.trade_time
            ],
            ["E", "a", "f", "l", "T"].map(|k| int(&row, k))
        );
        assert_eq!(e.is_buyer_maker, row["m"].as_bool().unwrap());
        assert_eq!(e.symbol, text(&row, "s"));
        assert_eq!(e.event_type, text(&row, "e"));

        let row = data(fixtures::TRADE);
        let Payload::Trade(e) = payload(fixtures::TRADE) else {
            panic!("not Trade");
        };
        assert_eq!([e.price, e.quantity], ["p", "q"].map(|k| dec(&row, k)));
        assert_eq!(
            [e.event_time, e.id, e.trade_time],
            ["E", "t", "T"].map(|k| int(&row, k))
        );
        assert_eq!(e.is_buyer_maker, row["m"].as_bool().unwrap());
        assert_eq!(e.symbol, text(&row, "s"));
        assert_eq!(e.event_type, text(&row, "e"));

        let event = data(fixtures::KLINE);
        let row = event["k"].clone();
        let Payload::Kline(e) = payload(fixtures::KLINE) else {
            panic!("not Kline");
        };
        let k = &e.kline;
        assert_eq!(
            [
                k.open,
                k.close,
                k.high,
                k.low,
                k.volume,
                k.quote_volume,
                k.taker_buy_base_volume,
                k.taker_buy_quote_volume,
            ],
            ["o", "c", "h", "l", "v", "q", "V", "Q"].map(|key| dec(&row, key))
        );
        assert_eq!(
            [k.open_time, k.close_time, k.trade_count],
            ["t", "T", "n"].map(|key| int(&row, key))
        );
        assert_eq!(
            [k.first_trade_id, k.last_trade_id],
            ["f", "L"].map(|key| signed(&row, key))
        );
        assert_eq!(k.interval.as_str(), text(&row, "i"));
        assert_eq!(k.is_closed, row["x"].as_bool().unwrap());
        assert_eq!(k.symbol, text(&row, "s"));
        assert_eq!(e.symbol, text(&event, "s"));
        assert_eq!(e.event_type, text(&event, "e"));

        let row = data(fixtures::BOOK_TICKER);
        let Payload::BookTicker(e) = payload(fixtures::BOOK_TICKER) else {
            panic!("not BookTicker");
        };
        assert_eq!(
            [e.bid_price, e.bid_quantity, e.ask_price, e.ask_quantity],
            ["b", "B", "a", "A"].map(|k| dec(&row, k))
        );
        assert_eq!(e.update_id, int(&row, "u"));
        assert_eq!(e.symbol, text(&row, "s"));

        let row = data(fixtures::AVG_PRICE);
        let Payload::AvgPrice(e) = payload(fixtures::AVG_PRICE) else {
            panic!("not AvgPrice");
        };
        assert_eq!(e.price, dec(&row, "w"));
        assert_eq!(
            [e.event_time, e.last_trade_time],
            ["E", "T"].map(|k| int(&row, k))
        );
        assert_eq!(e.interval, text(&row, "i"));
        assert_eq!(e.symbol, text(&row, "s"));
        assert_eq!(e.event_type, text(&row, "e"));
    }

    #[test]
    fn a_partial_depth_frame_takes_its_symbol_from_the_envelope() {
        // The frame carries `lastUpdateId`, `bids` and `asks` and nothing else.
        for (frame, bids) in [
            (fixtures::PARTIAL_DEPTH, 5),
            (fixtures::PARTIAL_DEPTH_100MS, 3),
        ] {
            let row = data(frame);
            assert_eq!(row.as_object().unwrap().len(), 3);
            let update = Update::from_json(frame).unwrap();
            let Payload::PartialDepth(e) = update.payload else {
                panic!("not PartialDepth");
            };
            assert_eq!(e.symbol, "BTCUSDT");
            assert_eq!(e.last_update_id, int(&row, "lastUpdateId"));
            assert_eq!(e.bids.len(), bids);
            assert_eq!(e.asks.len(), bids);
            for (side, key) in [(&e.bids, "bids"), (&e.asks, "asks")] {
                let sent: Vec<(Decimal, Decimal)> = row[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|level| {
                        (
                            level[0].as_str().unwrap().parse().unwrap(),
                            level[1].as_str().unwrap().parse().unwrap(),
                        )
                    })
                    .collect();
                let decoded: Vec<(Decimal, Decimal)> =
                    side.iter().map(|l| (l.price, l.quantity)).collect();
                assert_eq!(decoded, sent, "{key}");
            }
        }

        // Another stream's name gives another symbol: it is the envelope's, not
        // the frame's.
        let ethereum = on_stream(fixtures::PARTIAL_DEPTH, "ethusdt@depth5");
        let Payload::PartialDepth(e) = Update::from_json(&ethereum).unwrap().payload else {
            panic!("not PartialDepth");
        };
        assert_eq!(e.symbol, "ETHUSDT");
        let chinese = on_stream(fixtures::PARTIAL_DEPTH, "币安人生usdt@depth5");
        let Payload::PartialDepth(e) = Update::from_json(&chinese).unwrap().payload else {
            panic!("not PartialDepth");
        };
        assert_eq!(e.symbol, "币安人生USDT");
    }

    #[test]
    fn a_book_ticker_frame_has_no_event_type_and_decodes() {
        assert!(data(fixtures::BOOK_TICKER).get("e").is_none());
        assert!(matches!(
            payload(fixtures::BOOK_TICKER),
            Payload::BookTicker(_)
        ));
    }

    #[test]
    fn the_non_ascii_symbol_keeps_its_characters_in_the_name_and_the_payload() {
        let update = Update::from_json(fixtures::NON_ASCII_MINI_TICKER).unwrap();
        let StreamName::MiniTicker(symbol) = &update.stream else {
            panic!("not MiniTicker: {:?}", update.stream);
        };
        assert!(!symbol.as_str().is_ascii());
        let Payload::MiniTicker(e) = &update.payload else {
            panic!("not MiniTicker");
        };
        assert_eq!(
            e.symbol.to_ascii_lowercase(),
            symbol.as_str().to_ascii_lowercase()
        );
        assert_eq!(
            wire(fixtures::NON_ASCII_MINI_TICKER)["stream"],
            update.stream.to_string()
        );
    }

    #[test]
    fn an_object_whose_event_type_is_not_its_streams_is_kept_whole() {
        // An aggregate trade's data on a trade stream: `e` is `aggTrade`, not
        // `trade`.
        let crossed = on_stream(fixtures::AGG_TRADE, "btcusdt@trade");
        let update = Update::from_json(&crossed).unwrap();
        assert_eq!(update.stream.to_string(), "btcusdt@trade");
        let Payload::Unknown { event_type, raw } = &update.payload else {
            panic!("not Unknown: {:?}", update.payload);
        };
        assert_eq!(event_type, "aggTrade");
        assert_eq!(
            serde_json::from_str::<Value>(raw).unwrap(),
            data(fixtures::AGG_TRADE)
        );

        // A renamed event, and an object with none.
        for (e, expected) in [(json!("aggTradeV2"), "aggTradeV2"), (Value::Null, "")] {
            let mut frame = wire(fixtures::AGG_TRADE);
            match e {
                Value::Null => {
                    frame["data"].as_object_mut().unwrap().remove("e");
                }
                e => frame["data"]["e"] = e,
            }
            let update = Update::from_json(&frame.to_string()).unwrap();
            assert!(
                matches!(&update.payload, Payload::Unknown { event_type, .. } if event_type == expected),
                "{:?}",
                update.payload
            );
            // It serialises back as it came.
            assert_eq!(serde_json::to_value(&update).unwrap(), frame);
        }
    }

    #[test]
    fn a_broken_frame_is_a_frame_error_that_names_its_stream() {
        let mut broken = wire(fixtures::AGG_TRADE);
        broken["data"]["p"] = json!("x");
        let err = Update::from_json(&broken.to_string()).unwrap_err();
        assert!(
            matches!(&err, SpotWsError::Frame { stream, .. } if stream == "btcusdt@aggTrade"),
            "{err:?}"
        );

        // An array stream whose data is an object, and a book ticker with no
        // `u`.
        let mut object = wire(fixtures::ALL_MINI_TICKERS);
        object["data"] = json!({});
        assert!(matches!(
            Update::from_json(&object.to_string()),
            Err(SpotWsError::Frame { .. })
        ));
        let mut thin = wire(fixtures::BOOK_TICKER);
        thin["data"].as_object_mut().unwrap().remove("u");
        assert!(matches!(
            Update::from_json(&thin.to_string()),
            Err(SpotWsError::Frame { .. })
        ));
        let mut empty = wire(fixtures::PARTIAL_DEPTH);
        empty["data"] = json!({});
        assert!(matches!(
            Update::from_json(&empty.to_string()),
            Err(SpotWsError::Frame { .. })
        ));
    }

    #[test]
    fn a_stream_this_crate_does_not_build_is_a_frame_error() {
        for stream in [
            "btcusdt@forceOrder",
            "BTCUSDT@trade",
            "!ticker@arr",
            "btcusdt@depth5@250ms",
        ] {
            let err = Update::from_json(&on_stream(fixtures::TRADE, stream)).unwrap_err();
            assert!(
                matches!(&err, SpotWsError::Frame { stream: s, .. } if s == stream),
                "{stream}: {err:?}"
            );
        }
        assert!(matches!(
            Update::from_json("not json"),
            Err(SpotWsError::Frame { stream, .. }) if stream.is_empty()
        ));
        assert!(matches!(
            Update::from_json(r#"{"result":null,"id":1}"#),
            Err(SpotWsError::Frame { .. })
        ));
    }

    #[test]
    fn an_update_serialises_back_to_its_envelope() {
        // Frames whose every key the types model come back whole. The partial
        // depth's symbol is not serialised: it was never in the frame.
        for frame in [
            fixtures::BOOK_TICKER,
            fixtures::PARTIAL_DEPTH,
            fixtures::PARTIAL_DEPTH_100MS,
            fixtures::MINI_TICKER,
            fixtures::ALL_MINI_TICKERS,
            fixtures::TICKER,
            fixtures::AVG_PRICE,
        ] {
            let update = Update::from_json(frame).unwrap();
            assert_eq!(serde_json::to_value(&update).unwrap(), wire(frame));
        }
    }
}
