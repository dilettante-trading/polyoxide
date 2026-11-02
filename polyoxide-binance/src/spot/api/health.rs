//! Liveness routes: `/api/v3/ping` and `/api/v3/time`.

use std::time::Duration;

use polyoxide_core::{decode_json, ApiError, HttpClient};
use serde::Deserialize;

use crate::spot::{
    error::SpotError,
    request::{route_builder, Routed},
    types::ServerTime,
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
    /// Round-trip time to the host, via `GET /api/v3/ping` (weight 1).
    ///
    /// The latency is that of the attempt that answered, as
    /// [`HttpClient::health`] times it.
    pub async fn ping(&self) -> Result<Duration, SpotError> {
        let route = Route::Ping;
        let pong = self
            .http
            .health::<SpotError>(route.path(), &[route.cost().into()])
            .await?;
        let text = pong.response.text().await.map_err(ApiError::from)?;
        let _: Empty = decode_json(route.path(), &text).map_err(ApiError::from)?;
        Ok(pong.round_trip)
    }

    /// Server time, via `GET /api/v3/time` (weight 1).
    pub fn time(&self) -> GetTime {
        GetTime {
            request: Routed::new(&self.http, Route::Time),
        }
    }
}

route_builder! {
    /// Request builder for `GET /api/v3/time`.
    GetTime => ServerTime
}
