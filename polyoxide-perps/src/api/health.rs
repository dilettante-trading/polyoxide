//! Liveness routes: `/v1/info/ping` and `/v1/info/time`.

use std::time::{Duration, Instant};

use polyoxide_core::{ApiError, HttpClient, Request};
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
    /// other route.
    pub async fn ping(&self) -> Result<Duration, PerpsError> {
        let start = Instant::now();
        let ping: Ping =
            Request::<Ping, PerpsError>::new(self.http_client.clone(), "/v1/info/ping")
                .send()
                .await?;
        if ping.status != "ok" {
            return Err(ApiError::Api {
                status: 200,
                message: format!("ping answered status {:?}", ping.status),
            }
            .into());
        }
        Ok(start.elapsed())
    }

    /// Server time, via `GET /v1/info/time`.
    pub fn time(&self) -> Fetch<Time> {
        fetch(&self.http_client, "/v1/info/time")
    }
}
