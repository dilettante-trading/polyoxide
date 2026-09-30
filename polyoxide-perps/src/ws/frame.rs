//! Wire shapes. Requests go out, responses come back correlated by `id`, and
//! push frames arrive labelled by channel. Payload structs use the terse
//! keys the socket sends (`bp`, `aq`, `oi`); `event.rs` is the public face.

// The envelope types are `pub(crate)` and only reached from `client`, which
// lands in a later task. Remove this once `client` exists.
#![allow(dead_code)]

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::types::{InstrumentId, Kline, Level, Side};

/// An outbound request.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Request {
    pub id: u64,
    pub req: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op: Option<Op>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Op {
    #[serde(rename = "type")]
    pub kind: &'static str,
}

impl Request {
    pub fn subscribe(id: u64, channels: &[String]) -> Self {
        Self {
            id,
            req: "sub",
            chs: Some(channels.to_vec()),
            op: None,
        }
    }

    pub fn unsubscribe(id: u64, channels: &[String]) -> Self {
        Self {
            id,
            req: "unsub",
            chs: Some(channels.to_vec()),
            op: None,
        }
    }

    pub fn ping(id: u64) -> Self {
        Self {
            id,
            req: "post",
            chs: None,
            op: Some(Op { kind: "ping" }),
        }
    }
}

/// One entry of a subscribe/unsubscribe response, or the body of a pong.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Status {
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub ts: Option<u64>,
    #[serde(default)]
    pub sq: Option<u64>,
}

impl Status {
    pub fn is_ok(&self) -> bool {
        self.status == "ok"
    }
}

/// A response to a request, correlated by `id`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Response {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub ts: Option<u64>,
    pub data: serde_json::Value,
}

impl Response {
    /// The per-channel statuses of a `sub`/`unsub` response.
    pub fn statuses(&self) -> Result<Vec<Status>, serde_json::Error> {
        serde_json::from_value(self.data.clone())
    }

    /// The body of a ping response.
    pub fn pong(&self) -> Result<Status, serde_json::Error> {
        serde_json::from_value(self.data.clone())
    }
}

/// A push frame. `data` stays raw until the channel says how to read it.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Push {
    pub ch: String,
    pub ts: u64,
    #[serde(default)]
    pub ets: Option<u64>,
    pub sq: u64,
    pub data: serde_json::Value,
}

/// Anything the socket sends as text.
#[derive(Debug)]
pub(crate) enum Incoming {
    Response(Response),
    Push(Push),
}

impl Incoming {
    /// Classify by shape: a push has `ch`, a response has `data` without it.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        if value.get("ch").is_some() {
            serde_json::from_value(value).map(Incoming::Push)
        } else if value.get("data").is_some() {
            serde_json::from_value(value).map(Incoming::Response)
        } else {
            Err(serde::de::Error::custom(
                "neither a push frame (ch) nor a response (data)",
            ))
        }
    }
}

/// `bbo::N` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BboData {
    /// Instrument id.
    pub iid: InstrumentId,
    /// Best bid price.
    #[serde(with = "rust_decimal::serde::str")]
    pub bp: Decimal,
    /// Best bid quantity.
    #[serde(with = "rust_decimal::serde::str")]
    pub bq: Decimal,
    /// Best ask price.
    #[serde(with = "rust_decimal::serde::str")]
    pub ap: Decimal,
    /// Best ask quantity.
    #[serde(with = "rust_decimal::serde::str")]
    pub aq: Decimal,
}

/// `book::N` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookData {
    /// Bid levels, best first.
    pub b: Vec<Level>,
    /// Ask levels, best first.
    pub a: Vec<Level>,
}

/// One trade of a `trades::N` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeData {
    /// Trade id.
    pub tid: u64,
    /// Instrument id.
    pub iid: InstrumentId,
    /// Taker side.
    pub side: Side,
    /// Price.
    #[serde(with = "rust_decimal::serde::str")]
    pub p: Decimal,
    /// Quantity in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub qty: Decimal,
    /// Settlement trade. On the wire, not in the schema.
    #[serde(default)]
    pub settlement: Option<bool>,
    /// Trade time, Unix ms.
    pub ts: u64,
    /// Transaction hash.
    pub hash: String,
}

/// `tickers::N` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickerData {
    /// Instrument id.
    pub iid: InstrumentId,
    /// Index price.
    #[serde(with = "rust_decimal::serde::str")]
    pub idx: Decimal,
    /// Mark price.
    #[serde(with = "rust_decimal::serde::str")]
    pub mark: Decimal,
    /// Last traded price.
    #[serde(with = "rust_decimal::serde::str")]
    pub last: Decimal,
    /// Mid price.
    #[serde(with = "rust_decimal::serde::str")]
    pub mid: Decimal,
    /// Open interest in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub oi: Decimal,
    /// Funding rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub fr: Decimal,
    /// Next funding time, Unix ms.
    pub nxf: u64,
}

/// `statistics::N` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatisticsData {
    /// Instrument id.
    pub iid: InstrumentId,
    /// 24-hour volume in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub vol: Decimal,
    /// Price 24 hours ago.
    #[serde(with = "rust_decimal::serde::str")]
    pub open: Decimal,
    /// Hourly candles for the last 24 hours.
    pub klines: Vec<Kline>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn a_subscribe_request_serialises_as_the_documented_envelope() {
        let req = Request::subscribe(1, &["bbo::1".to_owned(), "book::1::50".to_owned()]);
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            r#"{"id":1,"req":"sub","chs":["bbo::1","book::1::50"]}"#
        );
        let req = Request::unsubscribe(4, &["bbo::1".to_owned()]);
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            r#"{"id":4,"req":"unsub","chs":["bbo::1"]}"#
        );
        let req = Request::ping(2);
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            r#"{"id":2,"req":"post","op":{"type":"ping"}}"#
        );
    }

    #[test]
    fn a_subscribe_response_lists_one_status_per_channel() {
        let raw = r#"{"id":3,"data":[{"status":"ok"},{"status":"err","error":"invalid channel"},{"status":"err","error":"invalid channel"}]}"#;
        let resp: Response = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.id, Some(3));
        let statuses = resp.statuses().unwrap();
        assert_eq!(statuses.len(), 3);
        assert!(statuses[0].is_ok());
        assert_eq!(statuses[1].error.as_deref(), Some("invalid channel"));
    }

    #[test]
    fn a_ping_response_carries_its_sequence() {
        let raw = r#"{"id":2,"ts":1790776558969,"data":{"status":"ok","ts":1790776558969,"sq":59023554562}}"#;
        let resp: Response = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.id, Some(2));
        let pong = resp.pong().unwrap();
        assert!(pong.is_ok());
        assert_eq!(pong.sq, Some(59023554562));
    }

    #[test]
    fn push_frames_decode_with_their_terse_keys() {
        let bbo: Push = serde_json::from_str(r#"{"ch":"bbo::1","ts":1790776558972,"ets":1790776558970,"sq":59023554618,"data":{"iid":1,"bp":"7702.6","bq":"0.31605","ap":"7703.9","aq":"0.96696"}}"#).unwrap();
        assert_eq!(bbo.ch, "bbo::1");
        assert_eq!(bbo.ets, Some(1790776558970));
        let bbo: BboData = serde_json::from_value(bbo.data).unwrap();
        assert_eq!(bbo.ap, Decimal::new(77039, 1));

        let book: Push = serde_json::from_str(r#"{"ch":"book::1","ts":1790776559001,"ets":0,"sq":59023555830,"data":{"a":[["7703.8","0.71393"]],"b":[["7702.6","0.31605"]]}}"#).unwrap();
        assert_eq!(book.ets, Some(0));
        let book: BookData = serde_json::from_value(book.data).unwrap();
        assert_eq!(book.b[0].quantity, Decimal::new(31605, 5));

        let trades: Push = serde_json::from_str(r#"{"ch":"trades::1","ts":1790776576797,"ets":1790776576796,"sq":59024034707,"data":[{"tid":5880140173696939,"iid":1,"side":"short","p":"7701.5","qty":"0.59079","settlement":false,"ts":1790776576796,"hash":"0x"}]}"#).unwrap();
        let trades: Vec<TradeData> = serde_json::from_value(trades.data).unwrap();
        assert_eq!(trades[0].tid, 5880140173696939);
        assert_eq!(trades[0].settlement, Some(false));

        let klines: Push = serde_json::from_str(r#"{"ch":"klines::1::1m","ts":1790776562551,"ets":1790776561708,"sq":59023593930,"data":[]}"#).unwrap();
        let klines: Vec<crate::types::Kline> = serde_json::from_value(klines.data).unwrap();
        assert!(klines.is_empty());

        let ticker: Push = serde_json::from_str(r#"{"ch":"tickers::1","ts":1790776559001,"ets":1790776559000,"sq":59023555830,"data":{"iid":1,"idx":"7703.5","mark":"7703.8","last":"7698.8","mid":"7703.2","oi":"1871.02808","fr":"0.00000625","nxf":1790776800000}}"#).unwrap();
        let ticker: TickerData = serde_json::from_value(ticker.data).unwrap();
        assert_eq!(ticker.nxf, 1790776800000);

        let stats: Push = serde_json::from_str(r#"{"ch":"statistics::1","ts":1790776560207,"ets":1790775778465,"sq":59023593930,"data":{"iid":1,"vol":"555348.007315","open":"7678.8","klines":[[1790686800000,"7678.8","7678.8","7678.8","7678.8","0.20379",2]]}}"#).unwrap();
        let stats: StatisticsData = serde_json::from_value(stats.data).unwrap();
        assert_eq!(stats.klines[0].trades, 2);
    }

    #[test]
    fn a_frame_is_told_apart_by_shape() {
        assert!(matches!(
            Incoming::parse(r#"{"id":1,"data":[]}"#).unwrap(),
            Incoming::Response(_)
        ));
        assert!(matches!(
            Incoming::parse(r#"{"ch":"bbo::1","ts":1,"sq":1,"data":{}}"#).unwrap(),
            Incoming::Push(_)
        ));
        assert!(Incoming::parse("not json").is_err());
        assert!(Incoming::parse(r#"{"hello":"world"}"#).is_err());
    }
}
