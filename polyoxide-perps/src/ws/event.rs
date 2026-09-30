//! What a consumer receives: a typed [`Update`] per push frame, wrapped in
//! [`Frame`] by the bare tier and in [`Event`] by the supervised tier.

use crate::{
    types::Kline,
    ws::{
        channel::Channel,
        error::PerpsWsError,
        frame::{BboData, BookData, Push, StatisticsData, TickerData, TradeData},
    },
};

/// The typed body of a push frame.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Payload {
    /// `bbo::N`.
    Bbo(BboData),
    /// `book::N` or `book::N::50`.
    Book(BookData),
    /// `trades::N`; may be empty.
    Trades(Vec<TradeData>),
    /// `klines::N::<interval>`; may be empty while a candle is open.
    Klines(Vec<Kline>),
    /// `tickers::N` (also what a `tickers::all` subscription delivers).
    Ticker(TickerData),
    /// `statistics::N` (also what a `statistics::all` subscription delivers).
    Statistics(StatisticsData),
}

/// One push frame, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Update {
    /// The frame's channel label. For a `::all` subscription this names the
    /// instrument the frame is about, never `all`.
    pub channel: Channel,
    /// Server send time, Unix ms.
    pub ts: u64,
    /// Event time, Unix ms; `None` when the server sent `0` (unattested) or
    /// omitted it.
    pub ets: Option<u64>,
    /// Server sequence stamp. Shared by every frame of one server batch and
    /// non-decreasing per channel; not contiguous, so gaps cannot be counted.
    pub sq: u64,
    /// The body.
    pub payload: Payload,
}

/// What the bare tier yields.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Frame {
    /// A decoded push frame.
    Update(Update),
    /// A push frame on a channel this crate does not model (a private
    /// channel, or one added upstream). The stream continues.
    Unknown {
        /// The `ch` label.
        channel: String,
        /// The frame text.
        raw: String,
    },
}

impl Frame {
    pub(crate) fn from_push(push: Push, raw: &str) -> Result<Self, PerpsWsError> {
        let Ok(channel) = push.ch.parse::<Channel>() else {
            return Ok(Frame::Unknown {
                channel: push.ch,
                raw: raw.to_owned(),
            });
        };
        let decode = |source: serde_json::Error| PerpsWsError::Frame {
            channel: push.ch.clone(),
            raw: raw.to_owned(),
            source,
        };
        let data = push.data.clone();
        let payload = match channel {
            Channel::Bbo(_) => Payload::Bbo(serde_json::from_value(data).map_err(decode)?),
            Channel::Book(..) => Payload::Book(serde_json::from_value(data).map_err(decode)?),
            Channel::Trades(_) => Payload::Trades(serde_json::from_value(data).map_err(decode)?),
            Channel::Klines(..) => Payload::Klines(serde_json::from_value(data).map_err(decode)?),
            Channel::Tickers(_) => Payload::Ticker(serde_json::from_value(data).map_err(decode)?),
            Channel::Statistics(_) => {
                Payload::Statistics(serde_json::from_value(data).map_err(decode)?)
            }
        };
        Ok(Frame::Update(Update {
            channel,
            ts: push.ts,
            ets: push.ets.filter(|e| *e != 0),
            sq: push.sq,
            payload,
        }))
    }
}

/// What the supervised tier yields: every [`Frame`] plus lifecycle events.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    /// A decoded push frame.
    Update(Update),
    /// A push frame on an unmodelled channel.
    Unknown {
        /// The `ch` label.
        channel: String,
        /// The frame text.
        raw: String,
    },
    /// The connection was re-established and every subscription replayed.
    /// Book consumers must discard state here.
    Reconnected,
    /// A frame's `sq` was lower than the previous one on the same channel,
    /// so frames were reordered or replayed. Book consumers should resync.
    SequenceRegressed {
        /// The channel label.
        channel: Channel,
        /// The previous `sq`.
        previous: u64,
        /// The `sq` that arrived.
        got: u64,
    },
}

impl From<Frame> for Event {
    fn from(frame: Frame) -> Self {
        match frame {
            Frame::Update(u) => Event::Update(u),
            Frame::Unknown { channel, raw } => Event::Unknown { channel, raw },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{types::InstrumentId, ws::channel::StreamDepth};

    fn push(text: &str) -> Push {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn a_push_becomes_a_typed_update_and_a_zero_ets_is_unattested() {
        let frame = Frame::from_push(
            push(r#"{"ch":"book::1","ts":5,"ets":0,"sq":9,"data":{"a":[],"b":[["1","2"]]}}"#),
            "",
        )
        .unwrap();
        let Frame::Update(update) = frame else {
            panic!("expected an update")
        };
        assert_eq!(
            update.channel,
            Channel::Book(InstrumentId(1), StreamDepth::Twenty)
        );
        assert_eq!(update.ets, None);
        assert_eq!(update.sq, 9);
        assert!(matches!(update.payload, Payload::Book(ref b) if b.bids.len() == 1));
    }

    #[test]
    fn a_missing_ets_is_unattested_too() {
        let frame =
            Frame::from_push(push(r#"{"ch":"trades::1","ts":5,"sq":9,"data":[]}"#), "").unwrap();
        let Frame::Update(update) = frame else {
            panic!("expected an update")
        };
        assert_eq!(update.ets, None);
    }

    #[test]
    fn an_unknown_channel_is_surfaced_not_fatal() {
        let frame = Frame::from_push(
            push(r#"{"ch":"fills","ts":5,"sq":9,"data":{}}"#),
            r#"{"ch":"fills"}"#,
        )
        .unwrap();
        assert!(matches!(frame, Frame::Unknown { ref channel, .. } if channel == "fills"));
    }

    #[test]
    fn a_known_channel_with_a_bad_payload_is_a_frame_error() {
        let err = Frame::from_push(
            push(r#"{"ch":"bbo::1","ts":5,"sq":9,"data":{"iid":1}}"#),
            "raw",
        )
        .unwrap_err();
        assert!(matches!(err, PerpsWsError::Frame { ref channel, .. } if channel == "bbo::1"));
    }

    #[test]
    fn every_channel_kind_maps_to_its_payload() {
        let cases = [
            (
                r#"{"ch":"bbo::1","ts":1,"sq":1,"data":{"iid":1,"bp":"1","bq":"1","ap":"2","aq":"1"}}"#,
                "bbo",
            ),
            (r#"{"ch":"trades::1","ts":1,"sq":1,"data":[]}"#, "trades"),
            (
                r#"{"ch":"klines::1::1m","ts":1,"sq":1,"data":[[1,"1","1","1","1","1",1]]}"#,
                "klines",
            ),
            (
                r#"{"ch":"tickers::1","ts":1,"sq":1,"data":{"iid":1,"idx":"1","mark":"1","last":"1","mid":"1","oi":"1","fr":"0","nxf":1}}"#,
                "ticker",
            ),
            (
                r#"{"ch":"statistics::1","ts":1,"sq":1,"data":{"iid":1,"vol":"1","open":"1","klines":[]}}"#,
                "statistics",
            ),
        ];
        for (text, expected) in cases {
            let Frame::Update(u) = Frame::from_push(push(text), text).unwrap() else {
                panic!("{expected}")
            };
            let got = match u.payload {
                Payload::Bbo(_) => "bbo",
                Payload::Book(_) => "book",
                Payload::Trades(_) => "trades",
                Payload::Klines(_) => "klines",
                Payload::Ticker(_) => "ticker",
                Payload::Statistics(_) => "statistics",
            };
            assert_eq!(got, expected);
        }
    }
}
