//! Liveness routes: `/v1/info/ping` and `/v1/info/time`.

use std::time::Duration;

use polyoxide_core::{decode_json, ApiError, ErrorResponse, HttpClient};
use serde::{Deserialize, Serialize};

use crate::{
    api::{fetch, Fetch},
    error::PerpsError,
};

/// Health namespace.
#[derive(Clone)]
pub struct Health {
    pub(crate) http_client: HttpClient,
}

#[derive(Deserialize)]
struct Ping {
    status: String,
}

/// `GET /v1/info/time`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Time {
    /// Server clock, Unix milliseconds.
    pub time: u64,
}

impl Health {
    /// Round-trip time to the host, via `GET /v1/info/ping`.
    ///
    /// Goes through the same rate limiter and concurrency budget as every
    /// other route. The latency is that of the attempt that answered, as
    /// [`HttpClient::health`](polyoxide_core::HttpClient::health) times it.
    pub async fn ping(&self) -> Result<Duration, PerpsError> {
        const PATH: &str = "/v1/info/ping";
        let pong = self.http_client.health::<PerpsError>(PATH, &[]).await?;
        let status = pong.response.status();
        let headers = pong.response.headers().clone();
        let text = pong.response.text().await.map_err(ApiError::from)?;
        let ping: Ping = decode_json(PATH, &text).map_err(ApiError::from)?;
        if ping.status != "ok" {
            // A 2xx whose body says the host is not ok, classed `Decode`.
            return Err(ApiError::from(ErrorResponse::new(status, headers, text)).into());
        }
        Ok(pong.round_trip)
    }

    /// Server time, via `GET /v1/info/time`.
    pub fn time(&self) -> Fetch<Time> {
        fetch(&self.http_client, "/v1/info/time")
    }
}
