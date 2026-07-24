use polyoxide_core::{HttpClient, QueryBuilder, Request};

use crate::{
    error::DataApiError,
    types::{OtherSize, RevisionPayload},
};

/// Misc namespace for endpoints that don't belong to a larger group.
#[derive(Clone)]
pub struct MiscApi {
    pub(crate) http_client: HttpClient,
}

impl MiscApi {
    /// Get the "Other" size for an augmented neg-risk event and user
    /// (`GET /other`).
    ///
    /// `event_id` is the Gamma event ID of the augmented neg-risk event.
    pub fn other_size(
        &self,
        event_id: u64,
        user_address: impl Into<String>,
    ) -> Request<Vec<OtherSize>, DataApiError> {
        Request::new(self.http_client.clone(), "/other")
            .query("id", event_id)
            .query("user", user_address.into())
    }

    /// Get moderated revisions for a question (`GET /revisions`).
    ///
    /// `question_id` is a 32-byte hash (`0x` + 64 hex).
    pub fn revisions(&self, question_id: impl Into<String>) -> ListRevisions {
        ListRevisions {
            request: Request::new(self.http_client.clone(), "/revisions")
                .query("questionID", question_id.into()),
        }
    }
}

/// Request builder for listing moderated question revisions.
pub struct ListRevisions {
    request: Request<Vec<RevisionPayload>, DataApiError>,
}

impl ListRevisions {
    /// Set maximum number of revisions returned (0-500, default: 100).
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<RevisionPayload>, DataApiError> {
        self.request.send().await
    }
}
