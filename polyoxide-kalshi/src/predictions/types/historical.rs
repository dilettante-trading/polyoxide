//! The archive: markets that settled before Kalshi's historical cutoff, their candlesticks and
//! their trades, which the live routes no longer serve.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::predictions::key::Ticker;

/// `GET /historical/cutoff` (`GetHistoricalCutoffResponse`): where the archive begins, each time
/// RFC 3339 text as sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalCutoff {
    /// A market that settled before this is served only by the historical routes.
    pub market_settled_ts: String,
    /// A fill before this is served only by `GET /historical/fills`.
    pub trades_created_ts: String,
    /// An order cancelled or executed before this is served only by `GET /historical/orders`.
    pub orders_updated_ts: String,
    /// The backfill horizon of settled positions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_positions_last_updated_ts: Option<String>,
}

/// Which archived markets `historical().markets` lists.
///
/// Kalshi takes one of these at a time and refuses two with a `400`
/// (`docs/specs/kalshi/OBSERVED.md`), so the choice is one value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HistoricalSelection {
    /// Every archived market, multivariate combinations included.
    All,
    /// Every archived market but the multivariate combinations (`mve_filter=exclude`).
    ExcludeMultivariate,
    /// These markets, comma-joined. An empty set, or one past
    /// [`TICKERS_QUERY_BUDGET`](crate::predictions::TICKERS_QUERY_BUDGET), is refused when sent,
    /// with nothing sent.
    Tickers(Vec<Ticker>),
    /// The markets of one event.
    Event(String),
    /// The markets of one series.
    Series(String),
}

/// Offers on one side of an archived market over a candlestick's period
/// (`BidAskDistributionHistorical`), in dollars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalBidAsk {
    /// The price at the start of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub open: Decimal,
    /// The lowest price in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub low: Decimal,
    /// The highest price in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub high: Decimal,
    /// The price at the end of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub close: Decimal,
}

/// Trade prices of an archived market over a candlestick's period (`PriceDistributionHistorical`),
/// in dollars. Kalshi sends every key, and a price it has no value for as `null`, where the live
/// route leaves it out, so each field is required and a `None` is written back as `null`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalPrice {
    /// The first YES trade of the period.
    #[serde(with = "rust_decimal::serde::str_option")]
    pub open: Option<Decimal>,
    /// The lowest YES trade of the period.
    #[serde(with = "rust_decimal::serde::str_option")]
    pub low: Option<Decimal>,
    /// The highest YES trade of the period.
    #[serde(with = "rust_decimal::serde::str_option")]
    pub high: Option<Decimal>,
    /// The last YES trade of the period.
    #[serde(with = "rust_decimal::serde::str_option")]
    pub close: Option<Decimal>,
    /// The mean YES trade price of the period.
    #[serde(with = "rust_decimal::serde::str_option")]
    pub mean: Option<Decimal>,
    /// The last YES trade before the period.
    #[serde(with = "rust_decimal::serde::str_option")]
    pub previous: Option<Decimal>,
}

/// One archived candlestick (`MarketCandlestickHistorical`), named by when its period ends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalCandlestick {
    /// The end of the period, inclusive, in Unix seconds.
    pub end_period_ts: i64,
    /// Offers to buy YES.
    pub yes_bid: HistoricalBidAsk,
    /// Offers to sell YES.
    pub yes_ask: HistoricalBidAsk,
    /// Trades.
    pub price: HistoricalPrice,
    /// Contracts traded in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Contracts bought, without netting, by the end of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest: Decimal,
}

/// `GET /historical/markets/{ticker}/candlesticks` (`GetMarketCandlesticksHistoricalResponse`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalCandlesticksResponse {
    /// The market's ticker.
    pub ticker: String,
    /// Its candlesticks.
    pub candlesticks: Vec<HistoricalCandlestick>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_price_is_null_on_the_wire_and_round_trips_as_null() {
        let text =
            r#"{"open":null,"low":null,"high":null,"close":null,"mean":null,"previous":"0.4200"}"#;
        let price: HistoricalPrice = serde_json::from_str(text).unwrap();
        assert!(price.close.is_none());
        assert_eq!(
            price.previous.map(|p| p.to_string()),
            Some("0.4200".to_owned())
        );
        assert_eq!(serde_json::to_string(&price).unwrap(), text);
    }
}
