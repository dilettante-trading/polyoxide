//! Series: the templates Kalshi's recurring events follow, with their categories and tags.

use std::sync::Arc;

use crate::predictions::{
    request::{routes, Routed},
    types::{SeriesList, SeriesResponse},
    Inner, PredictionsError,
};

/// Series namespace.
#[derive(Debug, Clone)]
pub struct SeriesApi {
    pub(crate) inner: Arc<Inner>,
}

impl SeriesApi {
    /// `GET /trade-api/v2/series`: every series the filters admit, in one body with no cursor.
    ///
    /// Unfiltered it is every series Kalshi lists, 15,336 of them in 19 MB on 2026-10-22, so a
    /// caller filters by `category` or `min_updated_ts` where it can.
    pub fn list(&self) -> ListSeries {
        ListSeries {
            request: Routed::new(&self.inner, routes::SERIES_LIST, &[]),
        }
    }

    /// `GET /trade-api/v2/series/{series_ticker}`: one series.
    pub fn get(&self, series_ticker: &str) -> GetSeries {
        GetSeries {
            request: Routed::new(&self.inner, routes::SERIES, &[series_ticker]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/series`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListSeries {
    pub(crate) request: Routed<SeriesList>,
}

impl ListSeries {
    polyoxide_core::query_setters! {
        /// Only series listed under this discovery category, matched exactly and case-sensitively
        /// against any of a series' `categories`.
        category: impl Into<String> => "category",
        /// Only series with these tags.
        tags: impl Into<String> => "tags",
        /// Include each series' `product_metadata`.
        include_product_metadata: bool => "include_product_metadata",
        /// Include each series' traded volume, `volume_fp`.
        include_volume: bool => "include_volume",
        /// Only series whose metadata changed after this Unix time, in seconds.
        min_updated_ts: i64 => "min_updated_ts",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<SeriesList, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/series/{series_ticker}`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetSeries {
    pub(crate) request: Routed<SeriesResponse>,
}

impl GetSeries {
    polyoxide_core::query_setters! {
        /// Include the series' traded volume, `volume_fp`.
        include_volume: bool => "include_volume",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<SeriesResponse, PredictionsError> {
        self.request.send().await
    }
}
