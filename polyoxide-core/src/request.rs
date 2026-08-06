use std::marker::PhantomData;

use reqwest::Response;
use serde::de::DeserializeOwned;

use crate::client::{retry_after_header, HttpClient};
use crate::ApiError;

/// Query parameter builder
pub trait QueryBuilder: Sized {
    /// Append a query parameter in place. Implementor hook for the builder methods below.
    fn add_query(&mut self, key: String, value: String);

    /// Append a query parameter and return `self` for chaining.
    fn query(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.add_query(key.into(), value.to_string());
        self
    }

    /// Add optional query parameter (only if Some)
    fn query_opt(mut self, key: impl Into<String>, value: Option<impl ToString>) -> Self {
        if let Some(v) = value {
            self.add_query(key.into(), v.to_string());
        }
        self
    }

    /// Add multiple query parameters with the same key
    fn query_many<I, V>(self, key: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: ToString,
    {
        let key = key.into();
        let mut result = self;
        for value in values {
            result.add_query(key.clone(), value.to_string());
        }
        result
    }

    /// Add multiple optional query parameters with the same key
    fn query_many_opt<I, V>(self, key: impl Into<String>, values: Option<I>) -> Self
    where
        I: IntoIterator<Item = V>,
        V: ToString,
    {
        if let Some(values) = values {
            self.query_many(key, values)
        } else {
            self
        }
    }
}

/// Trait for error types that can be created from API responses
pub trait RequestError: From<ApiError> + std::fmt::Debug {
    /// Create error from HTTP response
    fn from_response(response: Response) -> impl std::future::Future<Output = Self> + Send;
}

/// Generic request builder for simple GET-only APIs (Gamma, Data)
pub struct Request<T, E> {
    pub(crate) http_client: HttpClient,
    pub(crate) path: String,
    pub(crate) query: Vec<(String, String)>,
    pub(crate) _marker: PhantomData<(T, E)>,
}

impl<T, E> Request<T, E> {
    /// Create a new request
    pub fn new(http_client: HttpClient, path: impl Into<String>) -> Self {
        Self {
            http_client,
            path: path.into(),
            query: Vec::new(),
            _marker: PhantomData,
        }
    }
}

impl<T, E> QueryBuilder for Request<T, E> {
    fn add_query(&mut self, key: String, value: String) {
        self.query.push((key, value));
    }
}

impl<T: DeserializeOwned, E: RequestError> Request<T, E> {
    /// Execute the request and deserialize response
    pub async fn send(self) -> Result<T, E> {
        let response = self.send_raw().await?;

        // Get text for debugging
        let text = response
            .text()
            .await
            .map_err(|e| E::from(ApiError::from(e)))?;

        // Deserialize and provide better error context
        serde_json::from_str(&text).map_err(|e| {
            tracing::error!("Deserialization failed: {}", e);
            tracing::error!("Failed to deserialize: {}", crate::truncate_for_log(&text));
            E::from(ApiError::from(e))
        })
    }

    /// Execute the request and return raw response
    pub async fn send_raw(self) -> Result<Response, E> {
        let url = self
            .http_client
            .base_url
            .join(&self.path)
            .map_err(|e| E::from(ApiError::from(e)))?;

        let http_client = self.http_client;
        let query = self.query;
        let path = self.path;
        let mut attempt = 0u32;

        loop {
            let _permit = http_client.acquire_concurrency().await;
            http_client.acquire_rate_limit(&path, None).await;

            let mut request = http_client.client.get(url.clone());

            if !query.is_empty() {
                request = request.query(&query);
            }

            let response = request
                .send()
                .await
                .map_err(|e| E::from(ApiError::from(e)))?;
            let status = response.status();
            let retry_after = retry_after_header(&response);

            // Before `should_retry`, and unconditionally: a 429 has to become
            // backpressure for every request on this limiter even when *this*
            // request is out of attempts and about to give up.
            http_client.note_rate_limited(status, retry_after.as_deref());

            if let Some(backoff) = http_client.should_retry(status, attempt, retry_after.as_deref())
            {
                attempt += 1;
                tracing::warn!(
                    "Retriable status {} on {}, retry {} after {}ms",
                    status,
                    path,
                    attempt,
                    backoff.as_millis()
                );
                drop(_permit);
                tokio::time::sleep(backoff).await;
                continue;
            }

            tracing::debug!("Response status: {}", status);

            if !status.is_success() {
                let error = E::from_response(response).await;
                tracing::error!("Request failed: {:?}", error);
                return Err(error);
            }

            return Ok(response);
        }
    }
}

/// Type marker for deserializable responses
pub struct TypedRequest<T> {
    pub(crate) _marker: PhantomData<T>,
}

impl<T> TypedRequest<T> {
    /// Create a new typed request marker.
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T> Default for TypedRequest<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HttpClientBuilder;

    // ── QueryBuilder via Request<T, E> ──────────────────────────

    /// Helper to build a Request and extract its query pairs for assertions.
    fn make_request() -> Request<(), ApiError> {
        let http = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        Request::new(http, "/test")
    }

    #[test]
    fn test_query_adds_key_value() {
        let req = make_request().query("limit", 10);
        assert_eq!(req.query, vec![("limit".into(), "10".into())]);
    }

    #[test]
    fn test_query_chaining_preserves_order() {
        let req = make_request()
            .query("limit", 10)
            .query("offset", "abc")
            .query("active", true);
        assert_eq!(
            req.query,
            vec![
                ("limit".into(), "10".into()),
                ("offset".into(), "abc".into()),
                ("active".into(), "true".into()),
            ]
        );
    }

    #[test]
    fn test_query_opt_some_adds_parameter() {
        let req = make_request().query_opt("tag", Some("politics"));
        assert_eq!(req.query, vec![("tag".into(), "politics".into())]);
    }

    #[test]
    fn test_query_opt_none_skips_parameter() {
        let req = make_request().query_opt("tag", None::<&str>);
        assert!(req.query.is_empty());
    }

    #[test]
    fn test_query_opt_interleaved_with_query() {
        let req = make_request()
            .query("limit", 25)
            .query_opt("cursor", None::<String>)
            .query("active", true)
            .query_opt("slug", Some("will-x-happen"));

        assert_eq!(
            req.query,
            vec![
                ("limit".into(), "25".into()),
                ("active".into(), "true".into()),
                ("slug".into(), "will-x-happen".into()),
            ]
        );
    }

    #[test]
    fn test_query_many_adds_repeated_key() {
        let req = make_request().query_many("id", vec!["abc", "def", "ghi"]);
        assert_eq!(
            req.query,
            vec![
                ("id".into(), "abc".into()),
                ("id".into(), "def".into()),
                ("id".into(), "ghi".into()),
            ]
        );
    }

    #[test]
    fn test_query_many_empty_iterator() {
        let req = make_request().query_many("id", Vec::<String>::new());
        assert!(req.query.is_empty());
    }

    #[test]
    fn test_query_many_opt_some_adds_values() {
        let ids = vec![1u64, 2, 3];
        let req = make_request().query_many_opt("id", Some(ids));
        assert_eq!(
            req.query,
            vec![
                ("id".into(), "1".into()),
                ("id".into(), "2".into()),
                ("id".into(), "3".into()),
            ]
        );
    }

    #[test]
    fn test_query_many_opt_none_skips() {
        let req = make_request().query_many_opt("id", None::<Vec<String>>);
        assert!(req.query.is_empty());
    }

    #[test]
    fn test_query_duplicate_keys_allowed() {
        let req = make_request()
            .query("sort", "price")
            .query("sort", "volume");
        assert_eq!(
            req.query,
            vec![
                ("sort".into(), "price".into()),
                ("sort".into(), "volume".into()),
            ]
        );
    }

    // ── Request::new ────────────────────────────────────────────

    #[test]
    fn test_request_new_stores_path() {
        let req = make_request();
        assert_eq!(req.path, "/test");
        assert!(req.query.is_empty());
    }

    #[test]
    fn test_request_new_with_string_path() {
        let http = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        let req: Request<(), ApiError> = Request::new(http, String::from("/events"));
        assert_eq!(req.path, "/events");
    }

    // ── TypedRequest ────────────────────────────────────────────

    #[test]
    fn test_typed_request_new_and_default() {
        let _t1: TypedRequest<String> = TypedRequest::new();
        let _t2: TypedRequest<String> = TypedRequest::default();
        // Both should compile and create distinct instances — no state to verify
    }
}
