//! The archive: markets that settled before Kalshi's historical cutoff, their candlesticks and
//! their trades, which the live routes answer as if they did not exist.

use std::sync::Arc;

use crate::predictions::{
    key::Ticker,
    request::{routes, Routed},
    types::{
        HistoricalCandlesticksResponse, HistoricalCutoff, HistoricalSelection, MarketResponse,
        MarketsPage, MveFilter, PeriodInterval, TradesPage,
    },
    Inner, PredictionsError,
};

/// Historical namespace.
#[derive(Debug, Clone)]
pub struct HistoricalApi {
    pub(crate) inner: Arc<Inner>,
}

impl HistoricalApi {
    /// `GET /trade-api/v2/historical/cutoff`: where the archive begins. A market that settled
    /// before `market_settled_ts` is served only by this namespace's routes. The venue traits read
    /// archived markets without it, from what the live routes answer.
    pub fn cutoff(&self) -> GetHistoricalCutoff {
        GetHistoricalCutoff {
            request: Routed::new(&self.inner, routes::HISTORICAL_CUTOFF, &[]),
        }
    }

    /// `GET /trade-api/v2/historical/markets`: one page of archived markets, chosen by one
    /// selection, since Kalshi refuses two.
    pub fn markets(&self, selection: HistoricalSelection) -> ListHistoricalMarkets {
        let request = Routed::new(&self.inner, routes::HISTORICAL_MARKETS, &[]);
        let request = match &selection {
            HistoricalSelection::All => request,
            HistoricalSelection::ExcludeMultivariate => {
                request.query("mve_filter", MveFilter::Exclude)
            }
            HistoricalSelection::Tickers(tickers) => {
                let texts: Vec<&str> = tickers.iter().map(Ticker::as_str).collect();
                request.tickers_joined("tickers", &texts)
            }
            HistoricalSelection::Event(event_ticker) => request.query("event_ticker", event_ticker),
            HistoricalSelection::Series(series_ticker) => {
                request.query("series_ticker", series_ticker)
            }
        };
        ListHistoricalMarkets { request }
    }

    /// `GET /trade-api/v2/historical/markets/{ticker}`: one archived market. A market not in the
    /// archive is a `404`, a live one included.
    pub fn market(&self, ticker: &Ticker) -> GetHistoricalMarket {
        GetHistoricalMarket {
            request: Routed::new(&self.inner, routes::HISTORICAL_MARKET, &[ticker.as_str()]),
        }
    }

    /// `GET /trade-api/v2/historical/markets/{ticker}/candlesticks`: an archived market's
    /// candlesticks of one width that end between `start_ts` and `end_ts`, both inclusive and in
    /// Unix seconds. An unknown ticker is a `404`.
    pub fn candlesticks(
        &self,
        ticker: &Ticker,
        period: PeriodInterval,
        start_ts: i64,
        end_ts: i64,
    ) -> GetHistoricalCandlesticks {
        GetHistoricalCandlesticks {
            request: Routed::new(
                &self.inner,
                routes::HISTORICAL_CANDLESTICKS,
                &[ticker.as_str()],
            )
            .query("start_ts", start_ts)
            .query("end_ts", end_ts)
            .query("period_interval", period),
        }
    }

    /// `GET /trade-api/v2/historical/trades`: archived trades across markets, newest first.
    pub fn trades(&self) -> ListHistoricalTrades {
        ListHistoricalTrades {
            request: Routed::new(&self.inner, routes::HISTORICAL_TRADES, &[]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/historical/cutoff`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetHistoricalCutoff {
    pub(crate) request: Routed<HistoricalCutoff>,
}

impl GetHistoricalCutoff {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<HistoricalCutoff, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/historical/markets`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListHistoricalMarkets {
    pub(crate) request: Routed<MarketsPage>,
}

impl ListHistoricalMarkets {
    polyoxide_core::query_setters! {
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

/// Request builder for `GET /trade-api/v2/historical/markets/{ticker}`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetHistoricalMarket {
    pub(crate) request: Routed<MarketResponse>,
}

impl GetHistoricalMarket {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<MarketResponse, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/historical/markets/{ticker}/candlesticks`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetHistoricalCandlesticks {
    pub(crate) request: Routed<HistoricalCandlesticksResponse>,
}

impl GetHistoricalCandlesticks {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<HistoricalCandlesticksResponse, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/historical/trades`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListHistoricalTrades {
    pub(crate) request: Routed<TradesPage>,
}

impl ListHistoricalTrades {
    polyoxide_core::query_setters! {
        /// Only the trades of this market.
        ticker: &Ticker => "ticker",
        /// Only trades at or after this Unix time, in seconds.
        min_ts: i64 => "min_ts",
        /// Only trades at or before this Unix time, in seconds.
        max_ts: i64 => "max_ts",
        /// Trades per page, up to 1000; the host defaults to 100.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
        /// Only block trades, or only the others; every trade when unset.
        is_block_trade: bool => "is_block_trade",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<TradesPage, PredictionsError> {
        self.request.send().await
    }
}
