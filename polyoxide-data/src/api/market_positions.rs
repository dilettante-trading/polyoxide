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
    polyoxide_core::query_setters! {
        /// Filter to a single user by proxy wallet address.
        user: impl Into<String> => "user",
        /// Filter positions by status (default: `ALL`).
        status: MarketPositionStatus => "status",
        /// Sort positions by field (default: `TOTAL_PNL`).
        sort_by: MarketPositionSortBy => "sortBy",
        /// Set sort direction (default: `DESC`).
        sort_direction: SortDirection => "sortDirection",
        /// Maximum number of positions per outcome token (0-500, default: 50).
        limit: u32 => "limit",
        /// Pagination offset per outcome token (0-10000, default: 0).
        offset: u32 => "offset",
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<MetaMarketPositionV1>, DataApiError> {
        self.request.send().await
    }
}
