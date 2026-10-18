//! Markets: the listing, one market, its order book and the public trades.

use std::sync::Arc;

use crate::predictions::{
    key::Ticker,
    request::{routes, Routed},
    types::{
        MarketResponse, MarketStatusFilter, MarketsPage, MveFilter, OrderbookResponse, TradesPage,
    },
    Inner, PredictionsError,
};

/// Markets namespace.
#[derive(Debug, Clone)]
pub struct MarketsApi {
    pub(crate) inner: Arc<Inner>,
}

impl MarketsApi {
    /// `GET /trade-api/v2/markets`: one page of markets.
    ///
    /// The host lists multivariate combinations first, thousands of them (9,996 of the first
    /// 10,000 open rows on 2026-10-18), so a walk for a single market sends
    /// `mve_filter(MveFilter::Exclude)`.
    pub fn list(&self) -> ListMarkets {
        ListMarkets {
            request: Routed::new(&self.inner, routes::MARKETS, &[]),
        }
    }

    /// `GET /trade-api/v2/markets/{ticker}`: a ticker the host does not list is a `404`.
    pub fn get(&self, ticker: &Ticker) -> GetMarket {
        GetMarket {
            request: Routed::new(&self.inner, routes::MARKET, &[ticker.as_str()]),
        }
    }

    /// `GET /trade-api/v2/markets/{ticker}/orderbook`: the YES and NO bids, each side worst
    /// first. A ticker the host does not list answers `200` with both sides empty.
    pub fn orderbook(&self, ticker: &Ticker) -> GetOrderbook {
        GetOrderbook {
            request: Routed::new(&self.inner, routes::ORDERBOOK, &[ticker.as_str()]),
        }
    }

    /// `GET /trade-api/v2/markets/trades`: public trades across markets, newest first.
    pub fn trades(&self) -> ListTrades {
        ListTrades {
            request: Routed::new(&self.inner, routes::TRADES, &[]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/markets`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListMarkets {
    pub(crate) request: Routed<MarketsPage>,
}

impl ListMarkets {
    polyoxide_core::query_setters! {
        /// Only markets in this lifecycle state.
        status: MarketStatusFilter => "status",
        /// Only the markets of this event.
        event_ticker: impl Into<String> => "event_ticker",
        /// Only the markets of this series.
        series_ticker: impl Into<String> => "series_ticker",
        /// Only the multivariate combinations, or everything but them.
        mve_filter: MveFilter => "mve_filter",
        /// Markets per page, up to 1000; the host defaults to 100.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<MarketsPage, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/markets/{ticker}`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetMarket {
    pub(crate) request: Routed<MarketResponse>,
}

impl GetMarket {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<MarketResponse, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/markets/{ticker}/orderbook`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetOrderbook {
    pub(crate) request: Routed<OrderbookResponse>,
}

impl GetOrderbook {
    polyoxide_core::query_setters! {
        /// Levels kept on each side, the best ones; 0 keeps every level.
        depth: u32 => "depth",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<OrderbookResponse, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/markets/trades`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListTrades {
    pub(crate) request: Routed<TradesPage>,
}

impl ListTrades {
    polyoxide_core::query_setters! {
        /// Only the trades of this market.
        ticker: &Ticker => "ticker",
        /// Trades per page, up to 1000; the host defaults to 100.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
        /// Only trades at or after this Unix time, in seconds.
        min_ts: i64 => "min_ts",
        /// Only trades at or before this Unix time, in seconds.
        max_ts: i64 => "max_ts",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<TradesPage, PredictionsError> {
        self.request.send().await
    }
}
