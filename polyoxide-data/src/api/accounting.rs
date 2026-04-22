use polyoxide_core::HttpClient;

use crate::error::DataApiError;

/// Accounting namespace — `GET /v1/accounting/snapshot`.
///
/// Downloads a ZIP archive containing `positions.csv` and `equity.csv` for a
/// given user. The response is returned as raw bytes so callers can hand the
/// archive off to their own zip/CSV tooling.
#[derive(Clone)]
pub struct AccountingApi {
    pub(crate) http_client: HttpClient,
}

impl AccountingApi {
    /// Download an accounting snapshot ZIP for `user_address`.
    ///
    /// The returned `Vec<u8>` is the `application/zip` body verbatim; this
    /// method does not parse or validate the archive contents.
    pub async fn snapshot(&self, user_address: impl Into<String>) -> Result<Vec<u8>, DataApiError> {
        let query = [("user".to_string(), user_address.into())];
        self.http_client
            .get_bytes("/v1/accounting/snapshot", &query)
            .await
            .map_err(DataApiError::from)
    }
}
