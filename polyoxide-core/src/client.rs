use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use url::Url;

use reqwest::header::RETRY_AFTER;

use crate::error::ApiError;
use crate::rate_limit::{RateLimiter, RetryConfig};

/// Extract the `Retry-After` header value as a string, if present and valid UTF-8.
pub fn retry_after_header(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get(RETRY_AFTER)?
        .to_str()
        .ok()
        .map(String::from)
}

/// Default request timeout in milliseconds
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// Default connection pool size per host
pub const DEFAULT_POOL_SIZE: usize = 10;

/// Shared HTTP client with base URL, optional rate limiter, and retry config.
///
/// This is the common structure used by all API clients to hold
/// the configured reqwest client, base URL, and rate-limiting state.
#[derive(Debug, Clone)]
pub struct HttpClient {
    /// The underlying reqwest HTTP client
    pub client: reqwest::Client,
    /// Base URL for API requests
    pub base_url: Url,
    rate_limiter: Option<RateLimiter>,
    retry_config: RetryConfig,
    concurrency_limiter: Option<Arc<Semaphore>>,
}

impl HttpClient {
    /// Await rate limiter for the given endpoint path + method.
    pub async fn acquire_rate_limit(&self, path: &str, method: Option<&reqwest::Method>) {
        if let Some(rl) = &self.rate_limiter {
            rl.acquire(path, method).await;
        }
    }

    /// Acquire a concurrency permit, if a limiter is configured.
    ///
    /// The returned permit **must** be held until the HTTP response has been
    /// received. Dropping the permit releases the concurrency slot.
    /// Returns `None` when no concurrency limit is set.
    pub async fn acquire_concurrency(&self) -> Option<OwnedSemaphorePermit> {
        let sem = self.concurrency_limiter.as_ref()?;
        Some(
            sem.clone()
                .acquire_owned()
                .await
                .expect("concurrency semaphore is never closed"),
        )
    }

    /// Check if a 429 response should be retried; returns backoff duration if yes.
    ///
    /// When `retry_after` is `Some`, the server-provided delay is used instead of
    /// the client-computed exponential backoff (clamped to `max_backoff_ms`).
    pub fn should_retry(
        &self,
        status: StatusCode,
        attempt: u32,
        retry_after: Option<&str>,
    ) -> Option<Duration> {
        if status == StatusCode::TOO_MANY_REQUESTS && attempt < self.retry_config.max_retries {
            if let Some(delay) = retry_after.and_then(|v| v.parse::<f64>().ok()) {
                let ms = (delay * 1000.0) as u64;
                Some(Duration::from_millis(
                    ms.min(self.retry_config.max_backoff_ms),
                ))
            } else {
                Some(self.retry_config.backoff(attempt))
            }
        } else {
            None
        }
    }

    /// GET a URL and return the raw response body as bytes.
    ///
    /// Use this for endpoints that return non-JSON payloads (e.g. `application/zip`
    /// downloads). Applies the same rate-limiting, concurrency gating, and 429
    /// retry behavior as the JSON-oriented [`Request`](crate::Request) helper.
    ///
    /// Non-2xx responses are mapped to [`ApiError`] via
    /// [`ApiError::from_response`].
    ///
    /// # Errors
    ///
    /// Returns [`ApiError`] on URL-join failure, network errors, or non-2xx
    /// responses.
    pub async fn get_bytes(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<Vec<u8>, ApiError> {
        let url = self.base_url.join(path)?;
        let mut attempt = 0u32;

        loop {
            let _permit = self.acquire_concurrency().await;
            self.acquire_rate_limit(path, None).await;

            let mut request = self.client.get(url.clone());
            if !query.is_empty() {
                request = request.query(query);
            }

            let response = request.send().await?;
            let status = response.status();
            let retry_after = retry_after_header(&response);

            if let Some(backoff) = self.should_retry(status, attempt, retry_after.as_deref()) {
                attempt += 1;
                tracing::warn!(
                    "Rate limited (429) on {}, retry {} after {}ms",
                    path,
                    attempt,
                    backoff.as_millis()
                );
                drop(_permit);
                tokio::time::sleep(backoff).await;
                continue;
            }

            if !status.is_success() {
                return Err(ApiError::from_response(response).await);
            }

            let bytes = response.bytes().await?;
            return Ok(bytes.to_vec());
        }
    }
}

/// Builder for configuring HTTP clients.
///
/// Provides a consistent way to configure HTTP clients across all API crates
/// with sensible defaults.
///
/// # Example
///
/// ```
/// use polyoxide_core::HttpClientBuilder;
///
/// let client = HttpClientBuilder::new("https://api.example.com")
///     .timeout_ms(60_000)
///     .pool_size(20)
///     .build()
///     .unwrap();
/// ```
pub struct HttpClientBuilder {
    base_url: String,
    timeout_ms: u64,
    pool_size: usize,
    rate_limiter: Option<RateLimiter>,
    retry_config: RetryConfig,
    max_concurrent: Option<usize>,
}

impl HttpClientBuilder {
    /// Create a new HTTP client builder with the given base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            rate_limiter: None,
            retry_config: RetryConfig::default(),
            max_concurrent: None,
        }
    }

    /// Set request timeout in milliseconds.
    ///
    /// Default: 30,000ms (30 seconds)
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = timeout;
        self
    }

    /// Set connection pool size per host.
    ///
    /// Default: 10 connections
    pub fn pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }

    /// Set a rate limiter for this client.
    pub fn with_rate_limiter(mut self, limiter: RateLimiter) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Set retry configuration for 429 responses.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Set the maximum number of concurrent in-flight HTTP requests.
    ///
    /// Prevents Cloudflare 1015 rate-limit errors caused by request bursts
    /// when many callers share the same client concurrently.
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }

    /// Build the HTTP client.
    pub fn build(self) -> Result<HttpClient, ApiError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .pool_max_idle_per_host(self.pool_size)
            .build()?;

        let base_url = Url::parse(&self.base_url)?;

        Ok(HttpClient {
            client,
            base_url,
            rate_limiter: self.rate_limiter,
            retry_config: self.retry_config,
            concurrency_limiter: self.max_concurrent.map(|n| Arc::new(Semaphore::new(n))),
        })
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            rate_limiter: None,
            retry_config: RetryConfig::default(),
            max_concurrent: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── should_retry() ───────────────────────────────────────────

    #[test]
    fn test_should_retry_429_under_max() {
        let client = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        // Default max_retries=3, so attempts 0 and 2 should retry
        assert!(client
            .should_retry(StatusCode::TOO_MANY_REQUESTS, 0, None)
            .is_some());
        assert!(client
            .should_retry(StatusCode::TOO_MANY_REQUESTS, 2, None)
            .is_some());
    }

    #[test]
    fn test_should_retry_429_at_max() {
        let client = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        // attempt == max_retries → no retry
        assert!(client
            .should_retry(StatusCode::TOO_MANY_REQUESTS, 3, None)
            .is_none());
    }

    #[test]
    fn test_should_retry_non_429_returns_none() {
        let client = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        for status in [
            StatusCode::OK,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_REQUEST,
            StatusCode::FORBIDDEN,
        ] {
            assert!(
                client.should_retry(status, 0, None).is_none(),
                "expected None for {status}"
            );
        }
    }

    #[test]
    fn test_should_retry_custom_config() {
        let client = HttpClientBuilder::new("https://example.com")
            .with_retry_config(RetryConfig {
                max_retries: 1,
                ..RetryConfig::default()
            })
            .build()
            .unwrap();
        assert!(client
            .should_retry(StatusCode::TOO_MANY_REQUESTS, 0, None)
            .is_some());
        assert!(client
            .should_retry(StatusCode::TOO_MANY_REQUESTS, 1, None)
            .is_none());
    }

    #[test]
    fn test_should_retry_uses_retry_after_header() {
        let client = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        let d = client
            .should_retry(StatusCode::TOO_MANY_REQUESTS, 0, Some("2"))
            .unwrap();
        assert_eq!(d, Duration::from_millis(2000));
    }

    #[test]
    fn test_should_retry_retry_after_fractional_seconds() {
        let client = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        let d = client
            .should_retry(StatusCode::TOO_MANY_REQUESTS, 0, Some("0.5"))
            .unwrap();
        assert_eq!(d, Duration::from_millis(500));
    }

    #[test]
    fn test_should_retry_retry_after_clamped_to_max_backoff() {
        let client = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        // Default max_backoff_ms = 10_000; header says 60s
        let d = client
            .should_retry(StatusCode::TOO_MANY_REQUESTS, 0, Some("60"))
            .unwrap();
        assert_eq!(d, Duration::from_millis(10_000));
    }

    #[test]
    fn test_should_retry_retry_after_invalid_falls_back() {
        let client = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        // Non-numeric Retry-After (HTTP-date format) falls back to computed backoff
        let d = client
            .should_retry(
                StatusCode::TOO_MANY_REQUESTS,
                0,
                Some("Wed, 21 Oct 2025 07:28:00 GMT"),
            )
            .unwrap();
        // Should be in the jitter range for attempt 0: [375, 625]ms
        let ms = d.as_millis() as u64;
        assert!(
            (375..=625).contains(&ms),
            "expected fallback backoff in [375, 625], got {ms}"
        );
    }

    // ── Builder wiring ───────────────────────────────────────────

    #[tokio::test]
    async fn test_builder_with_rate_limiter() {
        let client = HttpClientBuilder::new("https://example.com")
            .with_rate_limiter(RateLimiter::clob_default())
            .build()
            .unwrap();
        let start = std::time::Instant::now();
        client
            .acquire_rate_limit("/order", Some(&reqwest::Method::POST))
            .await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_builder_without_rate_limiter() {
        let client = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        let start = std::time::Instant::now();
        client
            .acquire_rate_limit("/order", Some(&reqwest::Method::POST))
            .await;
        assert!(start.elapsed() < Duration::from_millis(10));
    }

    // ── Concurrency limiter ─────────────────────────────────────

    #[tokio::test]
    async fn test_acquire_concurrency_none_when_not_configured() {
        let client = HttpClientBuilder::new("https://example.com")
            .build()
            .unwrap();
        assert!(client.acquire_concurrency().await.is_none());
    }

    #[tokio::test]
    async fn test_acquire_concurrency_returns_permit() {
        let client = HttpClientBuilder::new("https://example.com")
            .with_max_concurrent(2)
            .build()
            .unwrap();
        let permit = client.acquire_concurrency().await;
        assert!(permit.is_some());
    }

    #[tokio::test]
    async fn test_concurrency_shared_across_clones() {
        let client = HttpClientBuilder::new("https://example.com")
            .with_max_concurrent(1)
            .build()
            .unwrap();
        let clone = client.clone();

        // Hold the only permit from the original
        let _permit = client.acquire_concurrency().await.unwrap();

        // Clone should block because concurrency=1 and permit is held
        let result =
            tokio::time::timeout(Duration::from_millis(50), clone.acquire_concurrency()).await;
        assert!(result.is_err(), "clone should block when permit is held");
    }

    #[tokio::test]
    async fn test_concurrency_limits_parallel_tasks() {
        let client = HttpClientBuilder::new("https://example.com")
            .with_max_concurrent(2)
            .build()
            .unwrap();

        let start = std::time::Instant::now();
        let mut handles = Vec::new();
        for _ in 0..4 {
            let c = client.clone();
            handles.push(tokio::spawn(async move {
                let _permit = c.acquire_concurrency().await;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        // 4 tasks, concurrency 2, 50ms each => ~100ms minimum
        assert!(
            start.elapsed() >= Duration::from_millis(90),
            "expected ~100ms, got {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn test_builder_with_max_concurrent() {
        let client = HttpClientBuilder::new("https://example.com")
            .with_max_concurrent(5)
            .build()
            .unwrap();
        // Should be able to acquire 5 permits
        let mut permits = Vec::new();
        for _ in 0..5 {
            permits.push(client.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|p| p.is_some()));

        // 6th should block
        let result =
            tokio::time::timeout(Duration::from_millis(50), client.acquire_concurrency()).await;
        assert!(result.is_err());
    }

    // ── get_bytes() ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_bytes_returns_body_verbatim() {
        let mut server = mockito::Server::new_async().await;
        // Intentionally non-UTF-8 bytes to prove we're not assuming text.
        let body: Vec<u8> = vec![0x50, 0x4B, 0x03, 0x04, 0x00, 0xFF, 0xFE, 0x42];
        let mock = server
            .mock("GET", "/v1/accounting/snapshot")
            .match_query(mockito::Matcher::UrlEncoded(
                "user".into(),
                "0xabc".into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/zip")
            .with_body(body.clone())
            .create_async()
            .await;

        let client = HttpClientBuilder::new(server.url()).build().unwrap();
        let out = client
            .get_bytes(
                "/v1/accounting/snapshot",
                &[("user".to_string(), "0xabc".to_string())],
            )
            .await
            .unwrap();
        assert_eq!(out, body);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_bytes_maps_non_2xx_to_api_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/does-not-exist")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "not found"}"#)
            .create_async()
            .await;

        let client = HttpClientBuilder::new(server.url()).build().unwrap();
        let err = client.get_bytes("/does-not-exist", &[]).await.unwrap_err();
        match err {
            ApiError::Api { status, message } => {
                assert_eq!(status, 404);
                assert_eq!(message, "not found");
            }
            other => panic!("expected ApiError::Api, got {other:?}"),
        }
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_bytes_no_query_params() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/raw")
            .with_status(200)
            .with_body(&b"hello"[..])
            .create_async()
            .await;

        let client = HttpClientBuilder::new(server.url()).build().unwrap();
        let out = client.get_bytes("/raw", &[]).await.unwrap();
        assert_eq!(out, b"hello");
        mock.assert_async().await;
    }
}
