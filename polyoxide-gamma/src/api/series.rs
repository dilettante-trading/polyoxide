use polyoxide_core::{HttpClient, Request};

use crate::{
    error::GammaError,
    types::{CountResponse, SeriesData, SeriesSummary},
};

/// Series namespace for series-related operations
#[derive(Clone)]
pub struct Series {
    pub(crate) http_client: HttpClient,
}

impl Series {
    /// List series with optional filtering
    pub fn list(&self) -> ListSeries {
        ListSeries {
            request: Request::new(self.http_client.clone(), "/series"),
        }
    }

    /// Get a series by ID
    pub fn get(&self, id: impl Into<String>) -> GetSeries {
        GetSeries {
            request: Request::new(
                self.http_client.clone(),
                format!("/series/{}", urlencoding::encode(&id.into())),
            ),
        }
    }

    /// Get series summary by ID (`GET /series-summary/{id}`).
    pub fn get_summary(&self, id: impl Into<String>) -> Request<SeriesSummary, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/series-summary/{}", urlencoding::encode(&id.into())),
        )
    }

    /// Get series summary by slug (`GET /series-summary/slug/{slug}`).
    pub fn get_summary_by_slug(
        &self,
        slug: impl Into<String>,
    ) -> Request<SeriesSummary, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/series-summary/slug/{}", urlencoding::encode(&slug.into())),
        )
    }

    /// Get comment count for a series (`GET /series/{id}/comments/count`).
    pub fn comment_count(&self, id: impl Into<String>) -> Request<CountResponse, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/series/{}/comments/count", urlencoding::encode(&id.into())),
        )
    }
}

/// Request builder for getting a single series
pub struct GetSeries {
    request: Request<SeriesData, GammaError>,
}

impl GetSeries {
    polyoxide_core::query_setters! {
        /// Include chat data in response
        include_chat: bool => "include_chat",
    }

    /// Execute the request
    pub async fn send(self) -> Result<SeriesData, GammaError> {
        self.request.send().await
    }
}

/// Request builder for listing series
pub struct ListSeries {
    request: Request<Vec<SeriesData>, GammaError>,
}

impl ListSeries {
    polyoxide_core::query_setters! {
        /// Limit the number of results
        limit: u32 => "limit",
        /// Offset the results
        offset: u32 => "offset",
        /// Sort in ascending order
        ascending: bool => "ascending",
        /// Filter by closed status
        closed: bool => "closed",
        /// Filter by slugs
        ///
        /// Safe batch size: ≤ 150 per request. URL length is capped at ~8 KB
        /// upstream; slug entries vary so pick a cap based on your longest slug.
        slug: many impl IntoIterator<Item = impl ToString> => "slug",
        /// Filter by category IDs
        ///
        /// Safe batch size: ≤ 100 per request. URLs over ~8 KB are rejected
        /// upstream with `414 URI Too Long`.
        categories_ids: many impl IntoIterator<Item = impl ToString> => "categories_ids",
        /// Filter by category labels
        ///
        /// Safe batch size: ≤ 150 per request. URL length is capped at ~8 KB
        /// upstream; label entries vary so pick a cap based on your longest label.
        categories_labels: many impl IntoIterator<Item = impl ToString> => "categories_labels",
        /// Include chat data in response
        include_chat: bool => "include_chat",
        /// Filter by recurrence pattern
        recurrence: impl Into<String> => "recurrence",
        /// Comma-separated list of JSON field names to order by.
        order: impl Into<String> => "order",
        /// Omit the nested `events` relation from each series.
        exclude_events: bool => "exclude_events",
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<SeriesData>, GammaError> {
        self.request.send().await
    }
}

#[cfg(test)]
mod tests {
    use crate::Gamma;

    fn gamma() -> Gamma {
        Gamma::new().unwrap()
    }

    #[test]
    fn test_list_series_full_chain() {
        let _req = gamma()
            .series()
            .list()
            .limit(10)
            .offset(0)
            .ascending(true)
            .closed(false)
            .slug(vec!["nfl-2025"])
            .categories_ids(vec!["1", "2"])
            .categories_labels(vec!["Sports"])
            .include_chat(true)
            .recurrence("weekly");
    }

    #[test]
    fn test_get_series_with_include_chat() {
        let _req = gamma().series().get("s-123").include_chat(true);
    }

    #[test]
    fn test_get_summary_accepts_str_and_string() {
        let _r1 = gamma().series().get_summary("s-1");
        let _r2 = gamma().series().get_summary(String::from("s-1"));
    }

    #[test]
    fn test_get_summary_by_slug_accepts_str_and_string() {
        let _r1 = gamma().series().get_summary_by_slug("nfl-2025");
        let _r2 = gamma()
            .series()
            .get_summary_by_slug(String::from("nfl-2025"));
    }

    #[test]
    fn test_comment_count_accepts_str_and_string() {
        let _r1 = gamma().series().comment_count("s-1");
        let _r2 = gamma().series().comment_count(String::from("s-1"));
    }
}
