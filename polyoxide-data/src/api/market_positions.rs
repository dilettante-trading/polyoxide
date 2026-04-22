use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::DataApiError,
    types::{MarketPositionSortBy, MarketPositionStatus, MetaMarketPositionV1, SortDirection},
};

/// Market positions namespace — `GET /v1/market-positions`.
///
/// Returns positions for a specific market across all users (or a single user
/// via [`ListMarketPositions::user`]).
#[derive(Clone)]
pub struct MarketPositionsApi {
    pub(crate) http_client: HttpClient,
}

impl MarketPositionsApi {
    /// List positions for a market by condition ID.
    ///
    /// `market` is required by the upstream API.
    pub fn list(&self, market: impl Into<String>) -> ListMarketPositions {
        let mut request = Request::new(self.http_client.clone(), "/v1/market-positions");
        request = request.query("market", market.into());
        ListMarketPositions { request }
    }
}

/// Request builder for listing positions in a market.
pub struct ListMarketPositions {
    request: Request<Vec<MetaMarketPositionV1>, DataApiError>,
}

impl ListMarketPositions {
    /// Filter to a single user by proxy wallet address.
    pub fn user(mut self, user_address: impl Into<String>) -> Self {
        self.request = self.request.query("user", user_address.into());
        self
    }

    /// Filter positions by status (default: `ALL`).
    pub fn status(mut self, status: MarketPositionStatus) -> Self {
        self.request = self.request.query("status", status);
        self
    }

    /// Sort positions by field (default: `TOTAL_PNL`).
    pub fn sort_by(mut self, sort_by: MarketPositionSortBy) -> Self {
        self.request = self.request.query("sortBy", sort_by);
        self
    }

    /// Set sort direction (default: `DESC`).
    pub fn sort_direction(mut self, direction: SortDirection) -> Self {
        self.request = self.request.query("sortDirection", direction);
        self
    }

    /// Maximum number of positions per outcome token (0-500, default: 50).
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Pagination offset per outcome token (0-10000, default: 0).
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<MetaMarketPositionV1>, DataApiError> {
        self.request.send().await
    }
}
