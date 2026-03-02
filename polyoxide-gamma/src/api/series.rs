use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{error::GammaError, types::SeriesData};

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
}

/// Request builder for getting a single series
pub struct GetSeries {
    request: Request<SeriesData, GammaError>,
}

impl GetSeries {
    /// Include chat data in response
    pub fn include_chat(mut self, include: bool) -> Self {
        self.request = self.request.query("include_chat", include);
        self
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
    /// Limit the number of results
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Offset the results
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Sort in ascending order
    pub fn ascending(mut self, ascending: bool) -> Self {
        self.request = self.request.query("ascending", ascending);
        self
    }

    /// Filter by closed status
    pub fn closed(mut self, closed: bool) -> Self {
        self.request = self.request.query("closed", closed);
        self
    }

    /// Filter by slugs
    pub fn slug(mut self, slugs: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("slug", slugs);
        self
    }

    /// Filter by category IDs
    pub fn categories_ids(mut self, ids: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("categories_ids", ids);
        self
    }

    /// Filter by category labels
    pub fn categories_labels(mut self, labels: impl IntoIterator<Item = impl ToString>) -> Self {
        self.request = self.request.query_many("categories_labels", labels);
        self
    }

    /// Include chat data in response
    pub fn include_chat(mut self, include: bool) -> Self {
        self.request = self.request.query("include_chat", include);
        self
    }

    /// Filter by recurrence pattern
    pub fn recurrence(mut self, recurrence: impl Into<String>) -> Self {
        self.request = self.request.query("recurrence", recurrence.into());
        self
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
}
