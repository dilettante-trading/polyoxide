use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::DataApiError,
    types::{RankingEntry, RankingWindow},
};

/// Rankings namespace — `GET /volume` and `GET /profit` on
/// `lb-api.polymarket.com`.
///
/// This is a **different service** from
/// [`DataApi::leaderboard`](crate::DataApi::leaderboard), which calls
/// `/v1/leaderboard` on the main Data API host and returns a different shape.
/// This one ranks traders by all-time or windowed volume and profit.
///
/// # Stability
///
/// This host is **not covered by any published Polymarket OpenAPI spec**. It
/// was verified live, but carries no documented compatibility guarantee —
/// treat it as more likely to change without notice than the rest of this
/// crate. The base URL is configurable via
/// [`DataApiBuilder::rankings_base_url`](crate::DataApiBuilder::rankings_base_url).
#[derive(Clone)]
pub struct RankingsApi {
    pub(crate) http_client: HttpClient,
}

impl RankingsApi {
    /// Rank traders by traded volume (`GET /volume`).
    pub fn volume(&self) -> RankingRequest {
        RankingRequest {
            request: Request::new(self.http_client.clone(), "/volume"),
        }
    }

    /// Rank traders by realized profit (`GET /profit`).
    pub fn profit(&self) -> RankingRequest {
        RankingRequest {
            request: Request::new(self.http_client.clone(), "/profit"),
        }
    }
}

/// Request builder for a rankings query.
pub struct RankingRequest {
    request: Request<Vec<RankingEntry>, DataApiError>,
}

impl RankingRequest {
    /// Set the ranking window. An unrecognized value is rejected upstream with
    /// `{"error": "invalid request"}`, which is why this is an enum.
    pub fn window(mut self, window: RankingWindow) -> Self {
        self.request = self.request.query("window", window);
        self
    }

    /// Limit the number of ranked entries returned.
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<RankingEntry>, DataApiError> {
        self.request.send().await
    }
}
