use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::GammaError,
    types::{Event, Tag},
};

/// Tags namespace for tag-related operations
#[derive(Clone)]
pub struct Tags {
    pub(crate) http_client: HttpClient,
}

impl Tags {
    /// List tags with optional filtering
    pub fn list(&self) -> ListTags {
        ListTags {
            request: Request::new(self.http_client.clone(), "/tags"),
        }
    }

    /// Get a tag by ID
    pub fn get(&self, id: impl Into<String>) -> Request<Tag, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/tags/{}", urlencoding::encode(&id.into())),
        )
    }

    /// Get a tag by slug
    pub fn get_by_slug(&self, slug: impl Into<String>) -> Request<Tag, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/tags/slug/{}", urlencoding::encode(&slug.into())),
        )
    }

    /// Get related tags by tag ID
    pub fn get_related(&self, id: impl Into<String>) -> RelatedTags {
        RelatedTags {
            request: Request::new(
                self.http_client.clone(),
                format!("/tags/{}/related-tags", urlencoding::encode(&id.into())),
            ),
        }
    }

    /// Get related tags by tag slug
    pub fn get_related_by_slug(&self, slug: impl Into<String>) -> RelatedTags {
        RelatedTags {
            request: Request::new(
                self.http_client.clone(),
                format!(
                    "/tags/slug/{}/related-tags",
                    urlencoding::encode(&slug.into())
                ),
            ),
        }
    }

    /// Get detailed related tags by tag ID (includes events)
    pub fn get_related_detailed(&self, id: impl Into<String>) -> Request<Vec<Event>, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!(
                "/tags/{}/related-tags/tags",
                urlencoding::encode(&id.into())
            ),
        )
    }

    /// Get detailed related tags by tag slug (includes events)
    pub fn get_related_detailed_by_slug(
        &self,
        slug: impl Into<String>,
    ) -> Request<Vec<Event>, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!(
                "/tags/slug/{}/related-tags/tags",
                urlencoding::encode(&slug.into())
            ),
        )
    }
}

/// Request builder for related tags with optional filters
pub struct RelatedTags {
    request: Request<Vec<Tag>, GammaError>,
}

impl RelatedTags {
    /// Omit tags with no events
    pub fn omit_empty(mut self, omit: bool) -> Self {
        self.request = self.request.query("omit_empty", omit);
        self
    }

    /// Filter by tag status
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.request = self.request.query("status", status.into());
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Tag>, GammaError> {
        self.request.send().await
    }
}

/// Request builder for listing tags
pub struct ListTags {
    request: Request<Vec<Tag>, GammaError>,
}

impl ListTags {
    /// Set maximum number of results (minimum: 0)
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Set pagination offset (minimum: 0)
    pub fn offset(mut self, offset: u32) -> Self {
        self.request = self.request.query("offset", offset);
        self
    }

    /// Set order fields (comma-separated list)
    pub fn order(mut self, order: impl Into<String>) -> Self {
        self.request = self.request.query("order", order.into());
        self
    }

    /// Set sort direction
    pub fn ascending(mut self, ascending: bool) -> Self {
        self.request = self.request.query("ascending", ascending);
        self
    }

    /// Include template data in response
    pub fn include_template(mut self, include: bool) -> Self {
        self.request = self.request.query("include_template", include);
        self
    }

    /// Filter by carousel status
    pub fn is_carousel(mut self, is_carousel: bool) -> Self {
        self.request = self.request.query("is_carousel", is_carousel);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Tag>, GammaError> {
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
    fn test_get_related_with_filters() {
        let _req = gamma()
            .tags()
            .get_related("42")
            .omit_empty(true)
            .status("active");
    }

    #[test]
    fn test_get_related_by_slug_with_filters() {
        let _req = gamma()
            .tags()
            .get_related_by_slug("politics")
            .omit_empty(false)
            .status("closed");
    }

    #[test]
    fn test_get_related_detailed() {
        let _req1 = gamma().tags().get_related_detailed("42");
        let _req2 = gamma().tags().get_related_detailed(String::from("42"));
    }

    #[test]
    fn test_get_related_detailed_by_slug() {
        let _req1 = gamma().tags().get_related_detailed_by_slug("politics");
        let _req2 = gamma()
            .tags()
            .get_related_detailed_by_slug(String::from("politics"));
    }
}
