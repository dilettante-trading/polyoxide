use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{error::GammaError, types::Comment};

/// Comments namespace for comment-related operations
#[derive(Clone)]
pub struct Comments {
    pub(crate) http_client: HttpClient,
}

impl Comments {
    /// List comments with optional filtering
    pub fn list(&self) -> ListComments {
        ListComments {
            request: Request::new(self.http_client.clone(), "/comments"),
        }
    }

    /// Get the comment thread containing `id` (`GET /comments/{id}`).
    ///
    /// Despite the name, upstream returns the **whole thread** — the root
    /// comment and every reply — not just the comment identified by `id`.
    /// Confirmed by probe on 2026-08-19: requesting `3218542` returned six
    /// comments with the requested one third in the list. Callers wanting the
    /// single comment must search the result:
    ///
    /// ```no_run
    /// # async fn f(gamma: &polyoxide_gamma::Gamma) -> Result<(), polyoxide_gamma::GammaError> {
    /// let thread = gamma.comments().get("3218542").send().await?;
    /// let this = thread.iter().find(|c| c.id == "3218542");
    /// # Ok(())
    /// # }
    /// ```
    pub fn get(&self, id: impl Into<String>) -> Request<Vec<Comment>, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/comments/{}", urlencoding::encode(&id.into())),
        )
    }

    /// Get comments by user address
    pub fn by_user(&self, address: impl Into<String>) -> Request<Vec<Comment>, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!(
                "/comments/user_address/{}",
                urlencoding::encode(&address.into())
            ),
        )
    }
}

/// Request builder for listing comments
pub struct ListComments {
    request: Request<Vec<Comment>, GammaError>,
}

impl ListComments {
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

    /// Filter by parent entity type (Event, Series, market)
    pub fn parent_entity_type(mut self, entity_type: impl Into<String>) -> Self {
        self.request = self.request.query("parent_entity_type", entity_type.into());
        self
    }

    /// Filter by parent entity ID
    pub fn parent_entity_id(mut self, id: i64) -> Self {
        self.request = self.request.query("parent_entity_id", id);
        self
    }

    /// Include position data in response
    pub fn get_positions(mut self, include: bool) -> Self {
        self.request = self.request.query("get_positions", include);
        self
    }

    /// Restrict results to position holders only
    pub fn holders_only(mut self, holders_only: bool) -> Self {
        self.request = self.request.query("holders_only", holders_only);
        self
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Comment>, GammaError> {
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
    fn test_get_comment_accepts_str_and_string() {
        let _req1 = gamma().comments().get("c-123");
        let _req2 = gamma().comments().get(String::from("c-123"));
    }

    #[test]
    fn test_by_user_accepts_str_and_string() {
        let _req1 = gamma().comments().by_user("0xabc");
        let _req2 = gamma().comments().by_user(String::from("0xabc"));
    }

    #[test]
    fn test_list_comments_full_chain() {
        let _req = gamma()
            .comments()
            .list()
            .limit(10)
            .offset(0)
            .order("createdAt")
            .ascending(false)
            .parent_entity_type("Event")
            .parent_entity_id(42)
            .get_positions(true)
            .holders_only(false);
    }
}
