//! Market data: tickers, premium index, klines, funding history, open
//! interest, aggregate trades and depth.

use polyoxide_core::HttpClient;

use crate::{
    error::BinanceError,
    usdm::{
        request::{route_builder, Routed},
        types::{
            AggTrade, Depth, DepthLimit, FundingRate, Interval, Kline, OpenInterest, PremiumIndex,
            Symbol, Ticker24h,
        },
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

    /// `GET /fapi/v1/ticker/24hr` for one symbol (weight 1).
    pub fn ticker_24h(&self, symbol: &Symbol) -> GetTicker24h {
        GetTicker24h {
            request: self
                .request(Route::Ticker24h { all: false })
                .query("symbol", symbol),
        }
    }

    /// `GET /fapi/v1/ticker/24hr` for every trading symbol (weight 40).
    pub fn tickers_24h(&self) -> GetTickers24h {
        GetTickers24h {
            request: self.request(Route::Ticker24h { all: true }),
        }
    }

    /// `GET /fapi/v1/premiumIndex` for one symbol (weight 1): mark price,
    /// index price and funding.
    pub fn premium_index(&self, symbol: &Symbol) -> GetPremiumIndex {
        GetPremiumIndex {
            request: self
                .request(Route::PremiumIndex { all: false })
                .query("symbol", symbol),
        }
    }

    /// `GET /fapi/v1/premiumIndex` for every symbol (weight 10).
    pub fn premium_indices(&self) -> GetPremiumIndices {
        GetPremiumIndices {
            request: self.request(Route::PremiumIndex { all: true }),
        }
    }

    /// `GET /fapi/v1/klines`: candles, newest last. Weight 5 without a limit,
    /// otherwise by limit (see [`Route::cost`]).
    pub fn klines(&self, symbol: &Symbol, interval: Interval) -> GetKlines {
        GetKlines {
            request: self
                .request(Route::Klines { limit: None })
                .query("symbol", symbol)
                .query("interval", interval),
        }
    }

    /// `GET /fapi/v1/fundingRate`: funding history, oldest first. Paced by
    /// the funding limit, not by weight: one request every 668 ms after the
    /// first. A waiting request holds one of the client's concurrent slots, so
    /// a long backfill can delay the weight routes; give it its own `Usdm`
    /// built with the same `WeightBudget`.
    pub fn funding_rate(&self) -> GetFundingRate {
        GetFundingRate {
            request: self.request(Route::FundingRate),
        }
    }

    /// `GET /fapi/v1/openInterest` (weight 1).
    pub fn open_interest(&self, symbol: &Symbol) -> GetOpenInterest {
        GetOpenInterest {
            request: self.request(Route::OpenInterest).query("symbol", symbol),
        }
    }

    /// `GET /fapi/v1/aggTrades` (weight 20 at any limit). Serves only the last
    /// 48 hours; an older window is refused with code `-4166`.
    pub fn agg_trades(&self, symbol: &Symbol) -> GetAggTrades {
        GetAggTrades {
            request: self.request(Route::AggTrades).query("symbol", symbol),
        }
    }

    /// `GET /fapi/v1/depth`: an order book snapshot. Without a limit it
    /// returns 500 levels for weight 1, the cheapest way to get them.
    pub fn depth(&self, symbol: &Symbol) -> GetDepth {
        GetDepth {
            request: self
                .request(Route::Depth { limit: None })
                .query("symbol", symbol),
        }
    }
}

route_builder! {
    /// Request builder for `GET /fapi/v1/ticker/24hr` for one symbol.
    GetTicker24h => Ticker24h
}

route_builder! {
    /// Request builder for `GET /fapi/v1/ticker/24hr` for every symbol.
    GetTickers24h => Vec<Ticker24h>
}

route_builder! {
    /// Request builder for `GET /fapi/v1/premiumIndex` for one symbol.
    GetPremiumIndex => PremiumIndex
}

route_builder! {
    /// Request builder for `GET /fapi/v1/premiumIndex` for every symbol.
    GetPremiumIndices => Vec<PremiumIndex>
}

route_builder! {
    /// Request builder for `GET /fapi/v1/openInterest`.
    GetOpenInterest => OpenInterest
}

/// Request builder for `GET /fapi/v1/klines`.
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
    }

    /// Candles to return, up to 1500. Sets the weight: up to 100 costs 1, up
    /// to 500 costs 2, up to 1000 costs 5, more costs 10.
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self
            .request
            .route(Route::Klines { limit: Some(limit) })
            .query("limit", limit);
        self
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Kline>, BinanceError> {
        self.request.send().await
    }
}

/// Request builder for `GET /fapi/v1/fundingRate`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetFundingRate {
    request: Routed<Vec<FundingRate>>,
}

impl GetFundingRate {
    polyoxide_core::query_setters! {
        /// Restrict to one symbol. Without it, the most recent `limit` funding
        /// events across all symbols.
        symbol: &Symbol => "symbol",
        /// Earliest funding time, Unix milliseconds.
        start_time: u64 => "startTime",
        /// Latest funding time, Unix milliseconds.
        end_time: u64 => "endTime",
        /// Rows to return, up to 1000.
        limit: u32 => "limit",
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<FundingRate>, BinanceError> {
        self.request.send().await
    }
}

/// Request builder for `GET /fapi/v1/aggTrades`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetAggTrades {
    request: Routed<Vec<AggTrade>>,
}

impl GetAggTrades {
    polyoxide_core::query_setters! {
        /// Start from this aggregate trade id, inclusive.
        from_id: u64 => "fromId",
        /// Earliest trade time, Unix milliseconds, within the last 48 hours.
        start_time: u64 => "startTime",
        /// Latest trade time, Unix milliseconds.
        end_time: u64 => "endTime",
        /// Rows to return, up to 1000. The weight is 20 whatever the limit.
        limit: u32 => "limit",
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<AggTrade>, BinanceError> {
        self.request.send().await
    }
}

/// Request builder for `GET /fapi/v1/depth`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetDepth {
    request: Routed<Depth>,
}

impl GetDepth {
    /// Levels per side. Sets the weight: up to 50 costs 2, 100 costs 5, 500
    /// costs 10, 1000 costs 20.
    pub fn limit(mut self, limit: DepthLimit) -> Self {
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

    /// Execute the request.
    pub async fn send(self) -> Result<Depth, BinanceError> {
        self.request.send().await
    }
}
