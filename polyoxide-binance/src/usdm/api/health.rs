//! Liveness routes: `/fapi/v1/ping` and `/fapi/v1/time`.

use std::time::{Duration, Instant};

use polyoxide_core::HttpClient;
use serde::Deserialize;

use crate::{
    error::BinanceError,
    usdm::{
        request::{route_builder, Routed},
        types::ServerTime,
    },
    weight::Route,
};

/// Health namespace.
#[derive(Debug, Clone)]
pub struct Health {
    pub(crate) http: HttpClient,
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
        Routed::<Empty>::new(&self.http, Route::Ping).send().await?;
        Ok(start.elapsed())
    }

    /// Server time, via `GET /fapi/v1/time` (weight 1).
    pub fn time(&self) -> GetTime {
        GetTime {
            request: Routed::new(&self.http, Route::Time),
        }
    }
}

route_builder! {
    /// Request builder for `GET /fapi/v1/time`.
    GetTime => ServerTime
}
