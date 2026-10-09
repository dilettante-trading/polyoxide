//! Liveness routes: `/fapi/v1/ping` and `/fapi/v1/time`.

use std::time::Duration;

use polyoxide_core::{decode_json, ApiError, HttpClient};
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
    /// The latency is that of the attempt that answered, as
    /// [`HttpClient::health`] times it.
    pub async fn ping(&self) -> Result<Duration, BinanceError> {
        let route = Route::Ping;
        let pong = self
            .http
            .health::<BinanceError>(route.path(), &[route.cost().into()])
            .await?;
        let text = pong.response.text().await.map_err(ApiError::from)?;
        let _: Empty = decode_json(route.path(), &text).map_err(ApiError::from)?;
        Ok(pong.round_trip)
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
