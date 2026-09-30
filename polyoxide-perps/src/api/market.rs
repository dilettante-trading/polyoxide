//! Market data keyed by instrument: tickers, statistics, klines, mark history,
//! BBO, book, index, trades, funding, and exchange-wide statistics.

use polyoxide_core::{HttpClient, QueryBuilder, Request};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    api::{setter, Fetch},
    error::PerpsError,
    types::{BookDepth, InstrumentId, Interval, Kline, Level, MarkPoint, Side},
};

/// Market namespace.
#[derive(Clone)]
pub struct MarketApi {
    pub(crate) http_client: HttpClient,
}

impl MarketApi {
    /// `GET /v1/info/tickers`: mark, index, last and mid prices per instrument.
    pub fn tickers(&self) -> ListTickers {
        ListTickers {
            request: Request::new(self.http_client.clone(), "/v1/info/tickers"),
        }
    }

    /// `GET /v1/info/statistics`: 24-hour volume, open and hourly klines.
    pub fn statistics(&self) -> ListStatistics {
        ListStatistics {
            request: Request::new(self.http_client.clone(), "/v1/info/statistics"),
        }
    }

    /// `GET /v1/info/exchange-stats`: exchange-wide volume, open interest and
    /// fees over `[start, end]` (Unix ms).
    pub fn exchange_stats(
        &self,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> Fetch<ExchangeStatistics> {
        Fetch {
            request: Request::new(self.http_client.clone(), "/v1/info/exchange-stats")
                .query("start_timestamp", start_timestamp)
                .query("end_timestamp", end_timestamp),
        }
    }

    /// `GET /v1/info/klines`: candles from `start_timestamp` (Unix ms).
    pub fn klines(
        &self,
        instrument_id: InstrumentId,
        interval: Interval,
        start_timestamp: u64,
    ) -> GetKlines {
        GetKlines {
            request: Request::new(self.http_client.clone(), "/v1/info/klines")
                .query("instrument_id", instrument_id)
                .query("interval", interval)
                .query("start_timestamp", start_timestamp),
        }
    }

    /// `GET /v1/info/mark-history`: mark-price samples from `start_timestamp`.
    pub fn mark_history(
        &self,
        instrument_id: InstrumentId,
        interval: Interval,
        start_timestamp: u64,
    ) -> GetMarkHistory {
        GetMarkHistory {
            request: Request::new(self.http_client.clone(), "/v1/info/mark-history")
                .query("instrument_id", instrument_id)
                .query("interval", interval)
                .query("start_timestamp", start_timestamp),
        }
    }

    /// `GET /v1/info/bbo`: best bid and offer per instrument.
    pub fn bbo(&self) -> ListBbo {
        ListBbo {
            request: Request::new(self.http_client.clone(), "/v1/info/bbo"),
        }
    }

    /// `GET /v1/info/book`: the order book for one instrument.
    ///
    /// An instrument the host does not know answers 200 with empty sides,
    /// not 404 (`docs/specs/perps/OBSERVED.md`).
    pub fn book(&self, instrument_id: InstrumentId) -> GetBook {
        GetBook {
            request: Request::new(self.http_client.clone(), "/v1/info/book")
                .query("instrument_id", instrument_id),
        }
    }

    /// `GET /v1/info/index`: the index price and its constituents for a base
    /// asset name such as `BTC`.
    pub fn index(&self, asset: impl Into<String>) -> Fetch<Index> {
        Fetch {
            request: Request::new(self.http_client.clone(), "/v1/info/index")
                .query("asset", asset.into()),
        }
    }

    /// `GET /v1/info/trades`: recent public trades.
    pub fn trades(&self, instrument_id: InstrumentId) -> ListTrades {
        ListTrades {
            request: Request::new(self.http_client.clone(), "/v1/info/trades")
                .query("instrument_id", instrument_id),
        }
    }

    /// `GET /v1/info/funding`: historical funding rates.
    pub fn funding(&self, instrument_id: InstrumentId) -> GetFunding {
        GetFunding {
            request: Request::new(self.http_client.clone(), "/v1/info/funding")
                .query("instrument_id", instrument_id),
        }
    }
}

/// Request builder for `GET /v1/info/tickers`.
pub struct ListTickers {
    request: Request<Vec<Ticker>, PerpsError>,
}

impl ListTickers {
    setter! {
        /// Restrict to one instrument.
        instrument_id: InstrumentId => "instrument_id"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Ticker>, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/statistics`.
pub struct ListStatistics {
    request: Request<Vec<Statistic>, PerpsError>,
}

impl ListStatistics {
    setter! {
        /// Restrict to one instrument.
        instrument_id: InstrumentId => "instrument_id"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Statistic>, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/klines`.
pub struct GetKlines {
    request: Request<Klines, PerpsError>,
}

impl GetKlines {
    setter! {
        /// End of the range, Unix ms. Defaults to now.
        end: u64 => "end_timestamp"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Klines, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/mark-history`.
pub struct GetMarkHistory {
    request: Request<MarkHistory, PerpsError>,
}

impl GetMarkHistory {
    setter! {
        /// End of the range, Unix ms. Defaults to now.
        end: u64 => "end_timestamp"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<MarkHistory, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/bbo`.
pub struct ListBbo {
    request: Request<Vec<Bbo>, PerpsError>,
}

impl ListBbo {
    setter! {
        /// Restrict to one instrument.
        instrument_id: InstrumentId => "instrument_id"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Bbo>, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/book`.
pub struct GetBook {
    request: Request<Book, PerpsError>,
}

impl GetBook {
    setter! {
        /// Levels per side. The server default is 100.
        depth: BookDepth => "depth"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Book, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/trades`.
pub struct ListTrades {
    request: Request<Trades, PerpsError>,
}

impl ListTrades {
    setter! {
        /// Start of the range, Unix ms.
        start: u64 => "start_timestamp"
    }
    setter! {
        /// End of the range, Unix ms.
        end: u64 => "end_timestamp"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Trades, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/funding`.
pub struct GetFunding {
    request: Request<FundingHistory, PerpsError>,
}

impl GetFunding {
    setter! {
        /// Start of the range, Unix ms.
        start: u64 => "start_timestamp"
    }
    setter! {
        /// End of the range, Unix ms.
        end: u64 => "end_timestamp"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<FundingHistory, PerpsError> {
        self.request.send().await
    }
}

/// One instrument's ticker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Ticker {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Symbol.
    pub symbol: String,
    /// Index price.
    #[serde(with = "rust_decimal::serde::str")]
    pub index_price: Decimal,
    /// Mark price.
    #[serde(with = "rust_decimal::serde::str")]
    pub mark_price: Decimal,
    /// Last traded price.
    #[serde(with = "rust_decimal::serde::str")]
    pub last_price: Decimal,
    /// Mid price.
    #[serde(with = "rust_decimal::serde::str")]
    pub mid_price: Decimal,
    /// Open interest in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest: Decimal,
    /// Current funding rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_rate: Decimal,
    /// Next funding time, Unix ms.
    pub next_funding: u64,
    /// Sample time, Unix ms.
    pub timestamp: u64,
}

/// One instrument's 24-hour statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Statistic {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Symbol.
    pub symbol: String,
    /// 24-hour volume in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Price 24 hours ago.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_price: Decimal,
    /// Hourly candles for the last 24 hours.
    pub klines: Vec<Kline>,
}

/// `GET /v1/info/exchange-stats`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExchangeStatistics {
    /// Range start, Unix ms.
    pub start_timestamp: u64,
    /// Range end, Unix ms.
    pub end_timestamp: u64,
    /// Volume over the range.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Open interest at `open_interest_timestamp`. Null when no sample exists
    /// in the range.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub open_interest: Option<Decimal>,
    /// When `open_interest` was sampled, Unix ms. Null when no sample exists
    /// in the range.
    pub open_interest_timestamp: Option<u64>,
    /// Fees collected over the range.
    #[serde(with = "rust_decimal::serde::str")]
    pub fees: Decimal,
}

/// `GET /v1/info/klines`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Klines {
    /// Candles, at most 1000.
    pub data: Vec<Kline>,
    /// Whether more candles exist past the last one returned.
    pub more: bool,
}

/// `GET /v1/info/mark-history`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MarkHistory {
    /// Samples, at most 1000.
    pub data: Vec<MarkPoint>,
    /// Whether more samples exist past the last one returned.
    pub more: bool,
}

/// Best bid and offer for one instrument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Bbo {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Best bid price.
    #[serde(with = "rust_decimal::serde::str")]
    pub bid_price: Decimal,
    /// Quantity at the best bid.
    #[serde(with = "rust_decimal::serde::str")]
    pub bid_quantity: Decimal,
    /// Best ask price.
    #[serde(with = "rust_decimal::serde::str")]
    pub ask_price: Decimal,
    /// Quantity at the best ask.
    #[serde(with = "rust_decimal::serde::str")]
    pub ask_quantity: Decimal,
    /// Sample time, Unix ms.
    pub timestamp: u64,
}

/// `GET /v1/info/book`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Book {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Bid levels, best first.
    pub bids: Vec<Level>,
    /// Ask levels, best first.
    pub asks: Vec<Level>,
    /// Snapshot time, Unix ms.
    pub timestamp: u64,
    /// Book sequence number.
    pub sequence: u64,
}

/// `GET /v1/info/index`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Index {
    /// Base asset name.
    pub asset: String,
    /// Index price.
    #[serde(with = "rust_decimal::serde::str")]
    pub index_price: Decimal,
    /// Sources the index is computed from. Empty for some assets.
    pub constituents: Vec<IndexConstituent>,
    /// Sample time, Unix ms.
    pub ts: u64,
}

/// One source of an index price.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IndexConstituent {
    /// Venue.
    pub source: String,
    /// Symbol at the venue.
    pub symbol: String,
    /// Weight in the index.
    #[serde(with = "rust_decimal::serde::str")]
    pub weight: Decimal,
    /// Price at the venue.
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
}

/// `GET /v1/info/trades`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Trades {
    /// Trades, newest first.
    pub data: Vec<Trade>,
    /// Whether more trades exist past the last one returned.
    pub more: bool,
}

/// One public trade.
///
/// `settlement` is on the wire and not in the published schema
/// (`docs/specs/perps/OBSERVED.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Trade {
    /// Trade id.
    pub trade_id: u64,
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Taker side.
    pub side: Side,
    /// Price.
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Whether this was a settlement trade. Undocumented.
    pub settlement: Option<bool>,
    /// Trade time, Unix ms.
    pub timestamp: u64,
    /// Transaction hash.
    pub hash: String,
}

/// `GET /v1/info/funding`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FundingHistory {
    /// Funding rates, newest first.
    pub data: Vec<FundingRate>,
    /// Whether more rates exist past the last one returned.
    pub more: bool,
}

/// One funding settlement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FundingRate {
    /// Rate applied.
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_rate: Decimal,
    /// Settlement time, Unix ms.
    pub timestamp: u64,
}
