//! Market data: depth, trades, aggregate trades, klines, the average price and
//! the three tickers.

use polyoxide_core::HttpClient;

use crate::spot::{
    error::SpotError,
    request::{route_builder, Routed},
    types::{
        AggTrade, AvgPrice, BookTicker, Depth, Interval, Kline, PriceTicker, Symbol, Ticker24h,
        Trade,
    },
    weight::{Cost, Route},
};

/// Market namespace.
#[derive(Debug, Clone)]
pub struct MarketApi {
    pub(crate) http: HttpClient,
}

impl MarketApi {
    fn request<T>(&self, route: Route) -> Routed<T> {
        Routed::new(&self.http, route)
    }

    /// `GET /api/v3/depth`: an order book snapshot. Without a limit it returns
    /// 100 levels for weight 5; see [`GetDepth::limit`] for the rest.
    pub fn depth(&self, symbol: &Symbol) -> GetDepth {
        GetDepth {
            request: self
                .request(Route::Depth { limit: None })
                .query("symbol", symbol),
        }
    }

    /// `GET /api/v3/trades`: recent trades, oldest first (weight 25 at any
    /// limit).
    pub fn trades(&self, symbol: &Symbol) -> GetTrades {
        GetTrades {
            request: self.request(Route::Trades).query("symbol", symbol),
        }
    }

    /// `GET /api/v3/aggTrades`: aggregate trades, oldest first (weight 4 at
    /// any limit).
    pub fn agg_trades(&self, symbol: &Symbol) -> GetAggTrades {
        GetAggTrades {
            request: self.request(Route::AggTrades).query("symbol", symbol),
        }
    }

    /// `GET /api/v3/klines`: candles, oldest first (weight 2 at any limit).
    pub fn klines(&self, symbol: &Symbol, interval: Interval) -> GetKlines {
        GetKlines {
            request: self
                .request(Route::Klines)
                .query("symbol", symbol)
                .query("interval", interval),
        }
    }

    /// `GET /api/v3/avgPrice`: the current average price (weight 2).
    pub fn avg_price(&self, symbol: &Symbol) -> GetAvgPrice {
        GetAvgPrice {
            request: self.request(Route::AvgPrice).query("symbol", symbol),
        }
    }

    /// `GET /api/v3/ticker/24hr` for one symbol (weight 2).
    pub fn ticker_24h(&self, symbol: &Symbol) -> GetTicker24h {
        GetTicker24h {
            request: self
                .request(Route::Ticker24h { all: false })
                .query("symbol", symbol),
        }
    }

    /// `GET /api/v3/ticker/24hr` for every symbol (weight 80).
    pub fn tickers_24h(&self) -> GetTickers24h {
        GetTickers24h {
            request: self.request(Route::Ticker24h { all: true }),
        }
    }

    /// `GET /api/v3/ticker/price` for one symbol (weight 2).
    pub fn price(&self, symbol: &Symbol) -> GetPrice {
        GetPrice {
            request: self
                .request(Route::Price { all: false })
                .query("symbol", symbol),
        }
    }

    /// `GET /api/v3/ticker/price` for every symbol (weight 4).
    pub fn prices(&self) -> GetPrices {
        GetPrices {
            request: self.request(Route::Price { all: true }),
        }
    }

    /// `GET /api/v3/ticker/bookTicker` for one symbol (weight 2).
    pub fn book_ticker(&self, symbol: &Symbol) -> GetBookTicker {
        GetBookTicker {
            request: self
                .request(Route::BookTicker { all: false })
                .query("symbol", symbol),
        }
    }

    /// `GET /api/v3/ticker/bookTicker` for every symbol (weight 4).
    pub fn book_tickers(&self) -> GetBookTickers {
        GetBookTickers {
            request: self.request(Route::BookTicker { all: true }),
        }
    }
}

route_builder! {
    /// Request builder for `GET /api/v3/avgPrice`.
    GetAvgPrice => AvgPrice
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/24hr` for one symbol.
    GetTicker24h => Ticker24h
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/24hr` for every symbol.
    GetTickers24h => Vec<Ticker24h>
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/price` for one symbol.
    GetPrice => PriceTicker
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/price` for every symbol.
    GetPrices => Vec<PriceTicker>
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/bookTicker` for one symbol.
    GetBookTicker => BookTicker
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/bookTicker` for every symbol.
    GetBookTickers => Vec<BookTicker>
}

/// Request builder for `GET /api/v3/depth`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetDepth {
    request: Routed<Depth>,
}

impl GetDepth {
    /// Levels per side, sent as given. Any value is accepted, and a limit past
    /// 5000 returns 5000 levels. Sets the weight: up to 100 costs 5, up to 500
    /// costs 25, up to 1000 costs 50, more costs 250.
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self
            .request
            .route(Route::Depth { limit: Some(limit) })
            .query("limit", limit);
        self
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Waits for the budget, sends, and decodes the response.
    ///
    /// A `429` holds every request on the budget, then this request is
    /// retried on the client's schedule; with no retry left and no
    /// `Retry-After`, the hold runs to the next UTC minute. A `418` holds
    /// every request for its `Retry-After`, or 2 minutes, and is not retried.
    ///
    /// The wait has no deadline of its own: a request can wait out a minute's
    /// spent budget or a ban, which Binance documents at up to 3 days. Wrap
    /// the call in `tokio::time::timeout` where a caller needs a deadline.
    pub async fn send(self) -> Result<Depth, SpotError> {
        self.request.send().await
    }
}

/// Request builder for `GET /api/v3/trades`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetTrades {
    request: Routed<Vec<Trade>>,
}

impl GetTrades {
    polyoxide_core::query_setters! {
        /// Rows to return, 500 by default and up to 1000; the venue truncates
        /// past 1000. The weight is 25 whatever the limit.
        limit: u32 => "limit",
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Waits for the budget, sends, and decodes the response.
    ///
    /// A `429` holds every request on the budget, then this request is
    /// retried on the client's schedule; with no retry left and no
    /// `Retry-After`, the hold runs to the next UTC minute. A `418` holds
    /// every request for its `Retry-After`, or 2 minutes, and is not retried.
    ///
    /// The wait has no deadline of its own: a request can wait out a minute's
    /// spent budget or a ban, which Binance documents at up to 3 days. Wrap
    /// the call in `tokio::time::timeout` where a caller needs a deadline.
    pub async fn send(self) -> Result<Vec<Trade>, SpotError> {
        self.request.send().await
    }
}

/// Request builder for `GET /api/v3/aggTrades`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetAggTrades {
    request: Routed<Vec<AggTrade>>,
}

impl GetAggTrades {
    polyoxide_core::query_setters! {
        /// Start from this aggregate trade id, inclusive.
        from_id: u64 => "fromId",
        /// Earliest trade time, Unix milliseconds.
        start_time: u64 => "startTime",
        /// Latest trade time, Unix milliseconds.
        end_time: u64 => "endTime",
        /// Rows to return, 500 by default and up to 1000; the venue truncates
        /// past 1000. The weight is 4 whatever the limit.
        limit: u32 => "limit",
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Waits for the budget, sends, and decodes the response.
    ///
    /// A `429` holds every request on the budget, then this request is
    /// retried on the client's schedule; with no retry left and no
    /// `Retry-After`, the hold runs to the next UTC minute. A `418` holds
    /// every request for its `Retry-After`, or 2 minutes, and is not retried.
    ///
    /// The wait has no deadline of its own: a request can wait out a minute's
    /// spent budget or a ban, which Binance documents at up to 3 days. Wrap
    /// the call in `tokio::time::timeout` where a caller needs a deadline.
    pub async fn send(self) -> Result<Vec<AggTrade>, SpotError> {
        self.request.send().await
    }
}

/// Request builder for `GET /api/v3/klines`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetKlines {
    request: Routed<Vec<Kline>>,
}

impl GetKlines {
    polyoxide_core::query_setters! {
        /// Earliest candle start, Unix milliseconds.
        start_time: u64 => "startTime",
        /// Latest candle start, Unix milliseconds.
        end_time: u64 => "endTime",
        /// Candles to return, 500 by default and up to 1000; the venue
        /// truncates past 1000. The weight is 2 whatever the limit.
        limit: u32 => "limit",
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Waits for the budget, sends, and decodes the response.
    ///
    /// A `429` holds every request on the budget, then this request is
    /// retried on the client's schedule; with no retry left and no
    /// `Retry-After`, the hold runs to the next UTC minute. A `418` holds
    /// every request for its `Retry-After`, or 2 minutes, and is not retried.
    ///
    /// The wait has no deadline of its own: a request can wait out a minute's
    /// spent budget or a ban, which Binance documents at up to 3 days. Wrap
    /// the call in `tokio::time::timeout` where a caller needs a deadline.
    pub async fn send(self) -> Result<Vec<Kline>, SpotError> {
        self.request.send().await
    }
}
