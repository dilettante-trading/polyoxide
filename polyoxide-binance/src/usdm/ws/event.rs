//! Stream payloads, and the update that carries one.
//!
//! The socket uses one-letter keys and different fields from REST, so each
//! stream has its own payload type, with long field names over the wire's keys.
//! Symbols are `String`, as sent, in the listing's case.

use std::fmt;

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::usdm::{
    types::{Interval, Level},
    ws::{error::UsdmWsError, stream::StreamName},
};

/// Which futures market a symbol belongs to, from a payload's `st`.
///
/// Binance documents it as "(After CM migration) Symbol type: 1 = UM, 2 = CM".
/// COIN-M symbols do arrive on this host: 30 of 745 rows of one
/// `!markPrice@arr@1s` frame on 2026-10-07 carried `2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SymbolType {
    /// `1`: USDⓈ-M.
    Um,
    /// `2`: COIN-M. These rows arrive on the USDⓈ-M socket too. Their
    /// quantities and volumes count contracts, and a ticker's `quote_volume`
    /// is base-asset volume, so they do not sum with USDⓈ-M rows.
    Cm,
    /// A value this version does not know, kept as sent.
    Other(u64),
}

impl Serialize for SymbolType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(match self {
            Self::Um => 1,
            Self::Cm => 2,
            Self::Other(raw) => *raw,
        })
    }
}

impl<'de> Deserialize<'de> for SymbolType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match u64::deserialize(deserializer)? {
            1 => Self::Um,
            2 => Self::Cm,
            raw => Self::Other(raw),
        })
    }
}

/// `<s>@ticker` and each row of `!ticker@arr`: rolling 24-hour statistics.
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
    /// The pair, as listed.
    #[serde(rename = "ps")]
    pub pair: String,
    /// Last price less the open price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price_change: Decimal,
    /// The change as a percentage.
    #[serde(rename = "P", with = "rust_decimal::serde::str")]
    pub price_change_percent: Decimal,
    /// Volume-weighted average price.
    #[serde(rename = "w", with = "rust_decimal::serde::str")]
    pub weighted_avg_price: Decimal,
    /// Last price.
    #[serde(rename = "c", with = "rust_decimal::serde::str")]
    pub last_price: Decimal,
    /// Last quantity.
    #[serde(rename = "Q", with = "rust_decimal::serde::str")]
    pub last_quantity: Decimal,
    /// Price 24 hours ago.
    #[serde(rename = "o", with = "rust_decimal::serde::str")]
    pub open_price: Decimal,
    /// Highest price.
    #[serde(rename = "h", with = "rust_decimal::serde::str")]
    pub high_price: Decimal,
    /// Lowest price.
    #[serde(rename = "l", with = "rust_decimal::serde::str")]
    pub low_price: Decimal,
    /// Base-asset volume; contracts on a COIN-M row.
    #[serde(rename = "v", with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Quote-asset volume; base-asset volume on a COIN-M row.
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
    /// USDⓈ-M or COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: SymbolType,
}

/// `<s>@markPrice@1s` and each row of `!markPrice@arr@1s`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MarkPriceEvent {
    /// `markPriceUpdate`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Mark price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub mark_price: Decimal,
    /// Mark price moving average.
    #[serde(rename = "ap", with = "rust_decimal::serde::str")]
    pub mark_price_moving_average: Decimal,
    /// Estimated settle price, meaningful only in the hour before a settlement.
    #[serde(rename = "P", with = "rust_decimal::serde::str")]
    pub estimated_settle_price: Decimal,
    /// Index price.
    #[serde(rename = "i", with = "rust_decimal::serde::str")]
    pub index_price: Decimal,
    /// Funding rate. `0` where no funding is scheduled.
    #[serde(rename = "r", with = "rust_decimal::serde::str")]
    pub funding_rate: Decimal,
    /// Next funding time; `0` means none is scheduled, as on 51 of 745 rows on
    /// 2026-10-07 (delisted contracts).
    #[serde(rename = "T")]
    pub next_funding_time: u64,
    /// USDⓈ-M or COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: SymbolType,
}

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
    /// Aggregate trade id.
    #[serde(rename = "a")]
    pub id: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Quantity without the trades involving RPI orders.
    #[serde(rename = "nq", with = "rust_decimal::serde::str")]
    pub normal_quantity: Decimal,
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
    /// USDⓈ-M or COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: SymbolType,
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
    /// First trade id.
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    /// Last trade id.
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

/// `<s>@depth<levels>@<speed>`: the top of the book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PartialDepthEvent {
    /// `depthUpdate`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// When the book last changed.
    #[serde(rename = "T")]
    pub transaction_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The pair, as listed.
    #[serde(rename = "ps")]
    pub pair: String,
    /// First book update id in the event.
    #[serde(rename = "U")]
    pub first_update_id: u64,
    /// Last book update id in the event.
    #[serde(rename = "u")]
    pub final_update_id: u64,
    /// Last book update id of the previous event.
    #[serde(rename = "pu")]
    pub previous_final_update_id: u64,
    /// Bids, best first.
    #[serde(rename = "b")]
    pub bids: Vec<Level>,
    /// Asks, best first.
    #[serde(rename = "a")]
    pub asks: Vec<Level>,
    /// USDⓈ-M or COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: SymbolType,
}

/// `<s>@bookTicker`: the best bid and ask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BookTickerEvent {
    /// `bookTicker`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// Book update id.
    #[serde(rename = "u")]
    pub update_id: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The pair, as listed.
    #[serde(rename = "ps")]
    pub pair: String,
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
    /// When the book last changed.
    #[serde(rename = "T")]
    pub transaction_time: u64,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// USDⓈ-M or COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: SymbolType,
}

/// What a stream carries, by stream kind.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Payload {
    /// `!ticker@arr`: only the symbols that changed. A row's `e` is not
    /// checked, and one row that does not decode fails the whole frame.
    Tickers(Vec<TickerEvent>),
    /// `!markPrice@arr@1s`. A row's `e` is not checked, and one row that does
    /// not decode fails the whole frame.
    MarkPrices(Vec<MarkPriceEvent>),
    /// `<s>@aggTrade`.
    AggTrade(AggTradeEvent),
    /// `<s>@kline_<interval>`.
    Kline(KlineEvent),
    /// `<s>@markPrice@1s`.
    MarkPrice(MarkPriceEvent),
    /// `<s>@ticker`.
    Ticker(TickerEvent),
    /// `<s>@depth<levels>@<speed>`.
    PartialDepth(PartialDepthEvent),
    /// `<s>@bookTicker`.
    BookTicker(BookTickerEvent),
    /// A single-symbol stream's object whose event type (`e`) is not the one
    /// its stream carries, kept whole rather than failing the frame. The live
    /// suite fails on one, since it means Binance renamed an event.
    Unknown {
        /// The payload's `e`, or empty when it has none.
        event_type: String,
        /// The payload as sent.
        raw: String,
    },
}

impl Serialize for Payload {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Tickers(rows) => rows.serialize(serializer),
            Self::MarkPrices(rows) => rows.serialize(serializer),
            Self::AggTrade(event) => event.serialize(serializer),
            Self::Kline(event) => event.serialize(serializer),
            Self::MarkPrice(event) => event.serialize(serializer),
            Self::Ticker(event) => event.serialize(serializer),
            Self::PartialDepth(event) => event.serialize(serializer),
            Self::BookTicker(event) => event.serialize(serializer),
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
    /// Fails with [`UsdmWsError::Frame`] when the text is not an envelope, its
    /// stream is not one this crate builds, or its payload does not decode as
    /// the stream's type.
    pub fn from_json(text: &str) -> Result<Self, UsdmWsError> {
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

fn frame_error(stream: &str, raw: &str, reason: impl fmt::Display) -> UsdmWsError {
    UsdmWsError::Frame {
        stream: stream.to_owned(),
        raw: raw.to_owned(),
        reason: reason.to_string(),
    }
}

fn decode(stream: &StreamName, data: Value) -> Result<Payload, serde_json::Error> {
    let expected = match stream {
        StreamName::AllTickers => return serde_json::from_value(data).map(Payload::Tickers),
        StreamName::AllMarkPrices => return serde_json::from_value(data).map(Payload::MarkPrices),
        StreamName::AggTrade(_) => "aggTrade",
        StreamName::Kline(..) => "kline",
        StreamName::MarkPrice(_) => "markPriceUpdate",
        StreamName::Ticker(_) => "24hrTicker",
        StreamName::PartialDepth(..) => "depthUpdate",
        StreamName::BookTicker(_) => "bookTicker",
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
        StreamName::Kline(..) => Payload::Kline(serde_json::from_value(data)?),
        StreamName::MarkPrice(_) => Payload::MarkPrice(serde_json::from_value(data)?),
        StreamName::Ticker(_) => Payload::Ticker(serde_json::from_value(data)?),
        StreamName::PartialDepth(..) => Payload::PartialDepth(serde_json::from_value(data)?),
        StreamName::BookTicker(_) => Payload::BookTicker(serde_json::from_value(data)?),
        StreamName::AllTickers | StreamName::AllMarkPrices => unreachable!("returned above"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usdm::ws::fixtures;

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
    fn the_mark_price_array_keeps_every_field_prader_reads() {
        let update = Update::from_json(fixtures::ALL_MARK_PRICES).unwrap();
        assert_eq!(update.stream, StreamName::AllMarkPrices);
        let Payload::MarkPrices(rows) = update.payload else {
            panic!("not MarkPrices");
        };
        assert_eq!(rows.len(), 2);
        assert!(rows[0].mark_price > Decimal::ZERO);
        assert!(rows[0].next_funding_time > 0);
        assert_eq!(rows[0].symbol_type, SymbolType::Um);
    }

    /// The round trip proves the key set, not which field a key lands in: two
    /// fields of one type swapped would pass it. Each field is compared here
    /// with its own key in the raw frame, as Binance's pages define the keys,
    /// so this holds for any capture. Where two keys carry equal values on
    /// every frame (`p` and `ap`), a swap of those two stays invisible.
    #[test]
    fn each_field_holds_the_value_of_its_own_key() {
        use serde_json::Value;
        fn data(frame: &str) -> Value {
            serde_json::from_str::<Value>(frame).unwrap()["data"].clone()
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
        fn payload(frame: &str) -> Payload {
            Update::from_json(frame).unwrap().payload
        }
        fn ticker(e: &TickerEvent, row: &Value) {
            assert_eq!(
                [
                    e.price_change,
                    e.price_change_percent,
                    e.weighted_avg_price,
                    e.last_price,
                    e.last_quantity,
                    e.open_price,
                    e.high_price,
                    e.low_price,
                    e.volume,
                    e.quote_volume,
                ],
                ["p", "P", "w", "c", "Q", "o", "h", "l", "v", "q"].map(|k| dec(row, k)),
                "{}",
                e.symbol
            );
            assert_eq!(
                [e.event_time, e.open_time, e.close_time, e.trade_count],
                ["E", "O", "C", "n"].map(|k| int(row, k)),
                "{}",
                e.symbol
            );
        }
        fn mark(e: &MarkPriceEvent, row: &Value) {
            assert_eq!(
                [
                    e.mark_price,
                    e.mark_price_moving_average,
                    e.estimated_settle_price,
                    e.index_price,
                    e.funding_rate,
                ],
                ["p", "ap", "P", "i", "r"].map(|k| dec(row, k)),
                "{}",
                e.symbol
            );
            assert_eq!(
                [e.event_time, e.next_funding_time],
                ["E", "T"].map(|k| int(row, k)),
                "{}",
                e.symbol
            );
        }

        let Payload::Tickers(rows) = payload(fixtures::ALL_TICKERS) else {
            panic!("not Tickers");
        };
        for (e, row) in rows
            .iter()
            .zip(data(fixtures::ALL_TICKERS).as_array().unwrap())
        {
            ticker(e, row);
        }
        let Payload::Ticker(e) = payload(fixtures::TICKER) else {
            panic!("not Ticker");
        };
        ticker(&e, &data(fixtures::TICKER));
        let Payload::MarkPrices(rows) = payload(fixtures::ALL_MARK_PRICES) else {
            panic!("not MarkPrices");
        };
        for (e, row) in rows
            .iter()
            .zip(data(fixtures::ALL_MARK_PRICES).as_array().unwrap())
        {
            mark(e, row);
        }
        let Payload::MarkPrice(e) = payload(fixtures::MARK_PRICE) else {
            panic!("not MarkPrice");
        };
        mark(&e, &data(fixtures::MARK_PRICE));

        let row = data(fixtures::AGG_TRADE);
        let Payload::AggTrade(e) = payload(fixtures::AGG_TRADE) else {
            panic!("not AggTrade");
        };
        assert_eq!(
            [e.price, e.quantity, e.normal_quantity],
            ["p", "q", "nq"].map(|k| dec(&row, k))
        );
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

        let row = data(fixtures::KLINE)["k"].clone();
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

        let row = data(fixtures::PARTIAL_DEPTH);
        let Payload::PartialDepth(e) = payload(fixtures::PARTIAL_DEPTH) else {
            panic!("not PartialDepth");
        };
        assert_eq!(
            [
                e.event_time,
                e.transaction_time,
                e.first_update_id,
                e.final_update_id,
                e.previous_final_update_id,
            ],
            ["E", "T", "U", "u", "pu"].map(|k| int(&row, k))
        );
        for (side, key) in [(&e.bids, "b"), (&e.asks, "a")] {
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

        let row = data(fixtures::BOOK_TICKER);
        let Payload::BookTicker(e) = payload(fixtures::BOOK_TICKER) else {
            panic!("not BookTicker");
        };
        assert_eq!(
            [e.bid_price, e.bid_quantity, e.ask_price, e.ask_quantity],
            ["b", "B", "a", "A"].map(|k| dec(&row, k))
        );
        assert_eq!(
            [e.update_id, e.transaction_time, e.event_time],
            ["u", "T", "E"].map(|k| int(&row, k))
        );
    }

    #[test]
    fn a_coin_m_row_and_an_unknown_symbol_type_both_decode() {
        assert_eq!(
            serde_json::from_str::<SymbolType>("2").unwrap(),
            SymbolType::Cm
        );
        assert_eq!(
            serde_json::from_str::<SymbolType>("7").unwrap(),
            SymbolType::Other(7)
        );
        assert_eq!(serde_json::to_string(&SymbolType::Other(7)).unwrap(), "7");
    }

    #[test]
    fn a_renamed_event_is_kept_whole_and_a_broken_one_is_a_frame_error() {
        let renamed = r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTradeV2","x":1}}"#;
        let update = Update::from_json(renamed).unwrap();
        assert!(matches!(
            &update.payload,
            Payload::Unknown { event_type, .. } if event_type == "aggTradeV2"
        ));

        let broken = r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTrade","p":"x"}}"#;
        let err = Update::from_json(broken).unwrap_err();
        assert!(
            matches!(&err, UsdmWsError::Frame { stream, .. } if stream == "btcusdt@aggTrade"),
            "{err:?}"
        );

        let unbuilt = r#"{"stream":"btcusdt@forceOrder","data":{}}"#;
        assert!(matches!(
            Update::from_json(unbuilt),
            Err(UsdmWsError::Frame { .. })
        ));
        assert!(matches!(
            Update::from_json("not json"),
            Err(UsdmWsError::Frame { .. })
        ));
    }

    #[test]
    fn an_update_serialises_back_to_its_envelope() {
        let update = Update::from_json(fixtures::BOOK_TICKER).unwrap();
        let again: Value = serde_json::to_value(&update).unwrap();
        let wire: Value = serde_json::from_str(fixtures::BOOK_TICKER).unwrap();
        assert_eq!(again, wire);
    }
}
