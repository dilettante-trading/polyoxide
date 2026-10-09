use polyoxide_core::{HttpClient, Request};

use crate::{
    error::GammaError,
    types::{Comment, ParentEntityType},
};

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
    polyoxide_core::query_setters! {
        /// Bound the number of top-level comments, not the number of rows
        /// returned: replies come along with their parents and are not counted
        /// against `limit`. Measured 2026-08-19 (`docs/specs/gamma/OBSERVED.md`):
        /// `limit=2` returned 8 rows, `limit=64` returned 160. Callers sizing a
        /// buffer from `limit` will under-allocate.
        limit: u32 => "limit",
        /// Set pagination offset (minimum: 0)
        offset: u32 => "offset",
        /// Set order fields (comma-separated list)
        order: impl Into<String> => "order",
        /// Set sort direction
        ascending: bool => "ascending",
        /// Filter by parent entity type.
        ///
        /// [`ParentEntityType::Unknown`] is not a filter the server understands,
        /// so passing it sends no parameter at all — mirroring
        /// `ListActivity::activity_type` in `polyoxide-data`.
        parent_entity_type(entity_type: ParentEntityType) => "parent_entity_type"
            if entity_type != ParentEntityType::Unknown,
        /// Filter by parent entity ID
        parent_entity_id: i64 => "parent_entity_id",
        /// Include position data in response
        get_positions: bool => "get_positions",
        /// Restrict results to position holders only
        holders_only: bool => "holders_only",
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Comment>, GammaError> {
        self.request.send().await
    }
}

#[cfg(test)]
mod tests {
    use crate::{types::ParentEntityType, Gamma};

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
            .parent_entity_type(ParentEntityType::Event)
            .parent_entity_id(42)
            .get_positions(true)
            .holders_only(false);
    }
}
