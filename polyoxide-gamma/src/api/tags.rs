use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::GammaError,
    types::{RelatedTag, Tag},
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

    /// Get the tag-relationship rows for a tag, by tag ID.
    ///
    /// Returns [`RelatedTag`] edges, not [`Tag`] values — see
    /// [`get_related_detailed`](Self::get_related_detailed) for the tags themselves.
    pub fn get_related(&self, id: impl Into<String>) -> RelatedTags {
        RelatedTags {
            request: Request::new(
                self.http_client.clone(),
                format!("/tags/{}/related-tags", urlencoding::encode(&id.into())),
            ),
        }
    }

    /// Get the tag-relationship rows for a tag, by tag slug.
    ///
    /// Returns [`RelatedTag`] edges, not [`Tag`] values — see
    /// [`get_related_detailed_by_slug`](Self::get_related_detailed_by_slug) for
    /// the tags themselves.
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
    pub fn get_related_detailed(&self, id: impl Into<String>) -> Request<Vec<Tag>, GammaError> {
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
    ) -> Request<Vec<Tag>, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!(
                "/tags/slug/{}/related-tags/tags",
                urlencoding::encode(&slug.into())
            ),
        )
    }
}

/// Request builder for tag-relationship rows with optional filters
pub struct RelatedTags {
    request: Request<Vec<RelatedTag>, GammaError>,
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
    pub async fn send(self) -> Result<Vec<RelatedTag>, GammaError> {
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
    use super::*;
    use crate::Gamma;

    fn gamma() -> Gamma {
        Gamma::new().unwrap()
    }

    /// Golden vector captured live on 2026-07-25 from
    /// `GET https://gamma-api.polymarket.com/tags/slug/politics/related-tags`.
    /// The by-ID route (`/tags/2/related-tags`) returns byte-identical rows.
    const LIVE_RELATED_TAGS: &str = r#"[
        {"id":"36304","tagID":2,"relatedTagID":126,"rank":1},
        {"id":"36305","tagID":2,"relatedTagID":104776,"rank":2},
        {"id":"36306","tagID":2,"relatedTagID":102289,"rank":3}
    ]"#;

    #[test]
    fn related_tags_deserialize_as_relationship_rows() {
        let rows: Vec<RelatedTag> = serde_json::from_str(LIVE_RELATED_TAGS).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].id, "36304");
        assert_eq!(rows[0].tag_id, Some(2));
        assert_eq!(rows[0].related_tag_id, Some(126));
        assert_eq!(rows[0].rank, Some(1));
        assert_eq!(rows[2].related_tag_id, Some(102289));
    }

    #[test]
    fn a_null_numeric_field_does_not_sink_the_whole_response() {
        // The gamma OpenAPI mirror marks tagID, relatedTagID and rank
        // `nullable: true`. Every row observed live had all three populated,
        // but observation cannot establish requiredness — and because serde
        // fails the entire `Vec<RelatedTag>`, one null row would cost the
        // caller every other row in the response, not just that one.
        const WITH_NULLS: &str = r#"[
            {"id":"1","tagID":2,"relatedTagID":126,"rank":1},
            {"id":"2","tagID":null,"relatedTagID":null,"rank":null}
        ]"#;

        let rows: Vec<RelatedTag> = serde_json::from_str(WITH_NULLS)
            .expect("a nullable field the spec permits must not fail the response");
        assert_eq!(rows.len(), 2, "the populated row must survive alongside it");
        assert_eq!(rows[0].related_tag_id, Some(126));
        assert_eq!(rows[1].tag_id, None);
        assert_eq!(rows[1].related_tag_id, None);
        assert_eq!(rows[1].rank, None);
    }

    #[test]
    fn a_missing_numeric_field_is_also_tolerated() {
        // `nullable` and "absent" are different on the wire; accept both.
        let rows: Vec<RelatedTag> = serde_json::from_str(r#"[{"id":"3"}]"#)
            .expect("an omitted optional field must not fail the response");
        assert_eq!(rows[0].id, "3");
        assert_eq!(rows[0].tag_id, None);
    }

    #[test]
    fn related_tags_payload_is_not_a_tag_payload() {
        // Pins *why* this was retyped: the venue's relationship rows carry no
        // slug or label, so the previous `Vec<Tag>` typing could never parse a
        // non-empty response. Without this the regression is invisible — every
        // tag with zero relations returns `[]`, which parses fine either way.
        assert!(
            serde_json::from_str::<Vec<crate::types::Tag>>(LIVE_RELATED_TAGS).is_err(),
            "relationship rows must not be mistaken for Tag objects"
        );
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
