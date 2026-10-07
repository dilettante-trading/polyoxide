//! Liveness routes: `/fapi/v1/ping` and `/fapi/v1/time`.

use std::time::{Duration, Instant};

use polyoxide_core::HttpClient;
use serde::Deserialize;

use crate::{
    error::BinanceError,
    usdm::{request::WeightedRequest, types::ServerTime},
    weight::{Route, WeightBudget},
};

/// Health namespace.
#[derive(Debug, Clone)]
pub struct Health {
    pub(crate) http: HttpClient,
    pub(crate) budget: WeightBudget,
}

/// `ping`'s body, `{}`.
#[derive(Deserialize)]
struct Empty {}

impl Health {
    /// Round-trip time to the host, via `GET /fapi/v1/ping` (weight 1).
    ///
    /// Includes any wait for the budget, as every route's latency does.
    pub async fn ping(&self) -> Result<Duration, BinanceError> {
        let start = Instant::now();
        WeightedRequest::<Empty>::new(&self.http, &self.budget, Route::Ping)
            .send()
            .await?;
        Ok(start.elapsed())
    }

    /// Server time, via `GET /fapi/v1/time` (weight 1).
    pub fn time(&self) -> WeightedRequest<ServerTime> {
        WeightedRequest::new(&self.http, &self.budget, Route::Time)
    }
}
