use std::sync::Arc;
use std::time::Duration;

use reqwest::Method;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use url::Url;

use reqwest::header::RETRY_AFTER;

use crate::error::ApiError;
use crate::hooks::{
    DefaultRetryPolicy, DynRetryPolicy, DynThrottle, NoThrottle, RequestParts, RetryPolicy,
    Throttle,
};
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

/// Shared HTTP client with base URL, throttle, retry policy and retry config.
///
/// This is the common structure used by all API clients to hold the
/// configured reqwest client, base URL, and the hooks its send loop,
/// [`send`](Self::send), runs on every request.
#[derive(Clone)]
pub struct HttpClient {
    /// The underlying reqwest HTTP client
    pub client: reqwest::Client,
    /// Base URL for API requests
    pub base_url: Url,
    pub(crate) throttle: Arc<DynThrottle<'static>>,
    pub(crate) policy: Arc<DynRetryPolicy<'static>>,
    pub(crate) retry_config: RetryConfig,
    concurrency_limiter: Option<Arc<Semaphore>>,
}

/// Written by hand: the throttle and the policy are trait objects, which
/// print nothing useful.
impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient")
            .field("client", &self.client)
            .field("base_url", &self.base_url)
            .field("retry_config", &self.retry_config)
            .field("concurrency_limiter", &self.concurrency_limiter)
            .finish_non_exhaustive()
    }
}

impl HttpClient {
    /// Clone this client, pointed at a different base URL.
    ///
    /// The underlying reqwest client (and so its connection pool), throttle
    /// (and so its hold), retry policy, retry config, and concurrency limiter
    /// are all shared with the original. Use this to reach a sibling API host
    /// that should share the same transport configuration and request budget —
    /// several Polymarket APIs live on their own subdomains but are consumed by
    /// one client, and a 429 on one holds them all.
    ///
    /// Sharing the concurrency limiter is deliberate: the limit exists to keep
    /// Cloudflare from seeing a burst from this process, and that is a
    /// per-process concern rather than a per-host one.
    ///
    /// # Path prefixes are ignored
    ///
    /// Request paths are absolute (`/user-pnl`), and resolving an absolute
    /// path against a base replaces the base's path entirely. So a prefix in
    /// `base_url` is **silently dropped**, with or without a trailing slash:
    ///
    /// ```text
    /// http://host          + /user-pnl -> http://host/user-pnl
    /// http://host/proxy    + /user-pnl -> http://host/user-pnl   (prefix gone)
    /// http://host/proxy/   + /user-pnl -> http://host/user-pnl   (prefix gone)
    /// ```
    ///
    /// Point this at a scheme, host, and port — not at a sub-path. Fronting
    /// the API with a path-prefixed reverse proxy is not supported.
    pub fn with_base_url(&self, base_url: &str) -> Result<Self, ApiError> {
        Ok(Self {
            base_url: Url::parse(base_url)?,
            ..self.clone()
        })
    }

    /// Acquire a concurrency permit, if a limiter is configured.
    ///
    /// The returned permit **must** be held until the HTTP response has been
    /// received. Dropping the permit releases the concurrency slot.
    /// Returns `None` when no concurrency limit is set.
    ///
    /// [`send`](Self::send) takes one for each attempt. Transitional for any
    /// other caller: no crate calls it since Stories 3.4 to 3.6 moved the
    /// hand-written loops onto [`send`](Self::send), but tests observe the
    /// permit through it. It is removed from the public API once they can
    /// observe it another way, and `docs/s1-removals.md` names [`send`](Self::send).
    pub async fn acquire_concurrency(&self) -> Option<OwnedSemaphorePermit> {
        let sem = self.concurrency_limiter.as_ref()?;
        Some(
            sem.clone()
                .acquire_owned()
                .await
                .expect("concurrency semaphore is never closed"),
        )
    }

    /// GET a URL and return the raw response body as bytes.
    ///
    /// Use this for endpoints that return non-JSON payloads (e.g. `application/zip`
    /// downloads). It runs on [`send`](Self::send), so it is throttled, gated,
    /// retried and held as every other request is.
    ///
    /// A non-2xx response is [`ApiError::Response`].
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
        let mut parts = RequestParts::new(Method::GET, path);
        parts.query = query.to_vec();
        let response = self.send(parts, &[], None).await?;

        // Only a policy that is `Done` with a failed response gets here.
        if !response.status().is_success() {
            return Err(ApiError::from_response(response).await);
        }

        let bytes = response.bytes().await?;
        Ok(bytes.to_vec())
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
    throttle: Option<Arc<DynThrottle<'static>>>,
    policy: Option<Arc<DynRetryPolicy<'static>>>,
    retry_config: RetryConfig,
    max_concurrent: Option<usize>,
    gzip: Option<bool>,
}

impl HttpClientBuilder {
    /// Create a new HTTP client builder with the given base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            throttle: None,
            policy: None,
            retry_config: RetryConfig::default(),
            max_concurrent: None,
            gzip: None,
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

    /// Set a rate limiter for this client: [`with_throttle`](Self::with_throttle)
    /// with a [`RateLimiter`].
    pub fn with_rate_limiter(self, limiter: RateLimiter) -> Self {
        self.with_throttle(limiter)
    }

    /// Set the throttle every request on this client goes through, shared
    /// with every [`with_base_url`](HttpClient::with_base_url) sibling.
    ///
    /// Default: [`NoThrottle`], which charges and holds
    /// nothing.
    pub fn with_throttle(mut self, throttle: impl Throttle + 'static) -> Self {
        self.throttle = Some(DynThrottle::new_arc(throttle));
        self
    }

    /// Set the policy that decides what follows each response.
    ///
    /// Default: [`DefaultRetryPolicy`], which
    /// retries a 429 and nothing else.
    pub fn with_retry_policy(mut self, policy: impl RetryPolicy + 'static) -> Self {
        self.policy = Some(DynRetryPolicy::new_arc(policy));
        self
    }

    /// Set the retry schedule: how many retries, and the backoff between them.
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

    /// Ask for gzip-compressed responses and decode them, or pin that off.
    ///
    /// Unset by default, which leaves reqwest's own default. This crate enables
    /// reqwest's `gzip` feature, so an unset client asks for gzip and decodes
    /// the body transparently, as every client did for a consumer whose
    /// workspace enabled the feature before this setting existed. 0.37.0 set
    /// it off unless asked, which took compression away from those consumers.
    pub fn gzip(mut self, enabled: bool) -> Self {
        self.gzip = Some(enabled);
        self
    }

    /// Build the HTTP client.
    pub fn build(self) -> Result<HttpClient, ApiError> {
        let mut builder = reqwest::Client::builder();
        if let Some(enabled) = self.gzip {
            builder = builder.gzip(enabled);
        }
        let client = builder
            .timeout(Duration::from_millis(self.timeout_ms))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .pool_max_idle_per_host(self.pool_size)
            .build()?;

        let base_url = Url::parse(&self.base_url)?;

        Ok(HttpClient {
            client,
            base_url,
            throttle: self
                .throttle
                .unwrap_or_else(|| DynThrottle::new_arc(NoThrottle)),
            policy: self
                .policy
                .unwrap_or_else(|| DynRetryPolicy::new_arc(DefaultRetryPolicy)),
            retry_config: self.retry_config,
            concurrency_limiter: self.max_concurrent.map(|n| Arc::new(Semaphore::new(n))),
        })
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    // ── Polymarket's retry decision and the retry delay ──────────
    //
    // These tests drove `HttpClient::should_retry` until the hand-written
    // loops that called it moved onto the send loop. They keep their names
    // and their answers, read from the policy the loop asks and from
    // `RetryConfig::retry_delay`, the loop's own floor.

    /// Whether Polymarket's policy retries `status` on `attempt` under
    /// `config`. `decide` itself fails a request with no retry left, so this
    /// is the send loop's answer.
    fn retries(config: &RetryConfig, status: StatusCode, attempt: u32) -> bool {
        use crate::hooks::{Outcome, ResponseMeta};

        let headers = reqwest::header::HeaderMap::new();
        let response = ResponseMeta {
            status,
            headers: &headers,
        };
        matches!(
            crate::polymarket::PolymarketRetryPolicy
                .decide(&response, &config.attempt_info(attempt), config)
                .outcome,
            Outcome::Retry(_)
        )
    }

    #[test]
    fn test_should_retry_429_under_max() {
        let config = RetryConfig::default();
        // Default max_retries=3, so attempts 0 and 2 should retry
        assert!(retries(&config, StatusCode::TOO_MANY_REQUESTS, 0));
        assert!(retries(&config, StatusCode::TOO_MANY_REQUESTS, 2));
    }

    #[test]
    fn test_should_retry_429_at_max() {
        let config = RetryConfig::default();
        // attempt == max_retries → no retry
        assert!(!retries(&config, StatusCode::TOO_MANY_REQUESTS, 3));
    }

    #[test]
    fn test_should_retry_425_under_max() {
        let config = RetryConfig::default();
        // 425 Too Early — Polymarket's matching engine restarting.
        assert!(retries(&config, StatusCode::TOO_EARLY, 0));
        assert!(retries(&config, StatusCode::TOO_EARLY, 2));
    }

    #[test]
    fn test_should_retry_425_at_max() {
        let config = RetryConfig::default();
        assert!(!retries(&config, StatusCode::TOO_EARLY, 3));
    }

    #[test]
    fn test_should_retry_ignores_other_statuses() {
        let config = RetryConfig::default();
        for status in [
            StatusCode::OK,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::BAD_REQUEST,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            // 503 is deliberately excluded even though post-only mode sends it
            // with a Retry-After: the documented wait is ~79s, far too long to
            // block inside a request, and it rejects orders wholesale.
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert!(!retries(&config, status, 0), "{status} was retried");
        }
    }

    #[test]
    fn test_should_retry_5xx_not_retried_despite_being_is_retriable() {
        // The class describes the error; this loop resends writes, so it stays narrower.
        let config = RetryConfig::default();
        assert!(!retries(&config, StatusCode::INTERNAL_SERVER_ERROR, 0));
        let err = ApiError::from(crate::error::ErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            Default::default(),
            "",
        ));
        assert!(polyoxide_venue::Classify::is_retriable(&err));
    }

    #[test]
    fn test_should_retry_custom_config() {
        let config = RetryConfig {
            max_retries: 1,
            ..RetryConfig::default()
        };
        assert!(retries(&config, StatusCode::TOO_MANY_REQUESTS, 0));
        assert!(!retries(&config, StatusCode::TOO_MANY_REQUESTS, 1));
    }

    #[test]
    fn test_should_retry_uses_retry_after_header() {
        let config = RetryConfig::default();
        let d = config.retry_delay(0, Some("2"));
        assert_eq!(d, Duration::from_millis(2000));
    }

    #[test]
    fn test_should_retry_retry_after_fractional_seconds() {
        let config = RetryConfig::default();
        // 1.5s, not the 0.5s this once used: a server-supplied delay is only
        // honoured when it exceeds the client's own backoff, and attempt 0's
        // jitter range is [375, 625]ms — straddling it made the assertion
        // depend on the roll. The point here is that fractions parse.
        let d = config.retry_delay(0, Some("1.5"));
        assert_eq!(d, Duration::from_millis(1500));
    }

    #[test]
    fn retry_after_below_our_own_backoff_does_not_shorten_the_wait() {
        let config = RetryConfig::default();
        // Cloudflare answers a tripped rate limit with 429 + `error code: 1015`
        // and a Retry-After that floors to zero. Taking it verbatim collapsed
        // the sleep to nothing: the observed failure was three "retry after 0ms"
        // attempts inside 65ms, which deepens a 1015 ban rather than waiting it
        // out. A server asking us to wait *longer* is honoured; one asking us to
        // wait less than our own policy is not.
        for header in ["0", "0.0", "-1", "-30", "0.0001"] {
            let d = config.retry_delay(0, Some(header));
            assert!(
                d >= Duration::from_millis(375),
                "Retry-After: {header:?} produced a {d:?} sleep; the floor is the \
                 client's own attempt-0 backoff, >=375ms after jitter"
            );
        }
    }

    #[test]
    fn retry_after_zero_still_backs_off_exponentially_across_attempts() {
        let config = RetryConfig::default();
        // Flooring at a flat minimum would still let a 1015 ban be hammered at a
        // fixed cadence. The floor has to be the *attempt's* backoff, so a
        // degenerate header still yields 500ms, 1s, 2s.
        for (attempt, min_ms) in [(0u32, 375u64), (1, 750), (2, 1_500)] {
            let d = config.retry_delay(attempt, Some("0"));
            assert!(
                d >= Duration::from_millis(min_ms),
                "attempt {attempt} with Retry-After: 0 slept {d:?}, expected >={min_ms}ms"
            );
        }
    }

    #[test]
    fn test_should_retry_retry_after_clamped_to_max_backoff() {
        let config = RetryConfig::default();
        // Default max_backoff_ms = 10_000; header says 60s
        let d = config.retry_delay(0, Some("60"));
        assert_eq!(d, Duration::from_millis(10_000));
    }

    #[test]
    fn test_should_retry_retry_after_invalid_falls_back() {
        let config = RetryConfig::default();
        // Non-numeric Retry-After (HTTP-date format) falls back to computed backoff
        let d = config.retry_delay(0, Some("Wed, 21 Oct 2025 07:28:00 GMT"));
        // Should be in the jitter range for attempt 0: [375, 625]ms
        let ms = d.as_millis() as u64;
        assert!(
            (375..=625).contains(&ms),
            "expected fallback backoff in [375, 625], got {ms}"
        );
    }

    // ── Builder wiring ───────────────────────────────────────────

    /// Notes when it signs: just after the throttle let the attempt go.
    #[derive(Default)]
    struct SignedAt(std::sync::Mutex<Option<std::time::Instant>>);

    impl crate::hooks::Authenticator for SignedAt {
        async fn sign(&self, _parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
            *self.0.lock().unwrap() = Some(std::time::Instant::now());
            Ok(())
        }
    }

    /// How long `client` takes to let a `POST /order` through to its signing,
    /// against a mock server.
    async fn time_to_sign(builder: HttpClientBuilder, server: &mockito::ServerGuard) -> Duration {
        let client = builder.build().unwrap();
        let client = client.with_base_url(&server.url()).unwrap();
        let signed = SignedAt::default();
        let start = std::time::Instant::now();
        client
            .send(
                RequestParts::new(Method::POST, "/order"),
                &[],
                Some(crate::hooks::DynAuthenticator::from_ref(&signed)),
            )
            .await
            .unwrap();
        let at = signed.0.lock().unwrap().expect("the request was signed");
        at - start
    }

    #[tokio::test]
    async fn test_builder_with_rate_limiter() {
        let mut server = mockito::Server::new_async().await;
        let _order = server.mock("POST", "/order").create_async().await;
        let builder = HttpClientBuilder::new("https://example.com")
            .with_rate_limiter(crate::polymarket::clob_limits());
        let elapsed = time_to_sign(builder, &server).await;
        assert!(elapsed < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_builder_without_rate_limiter() {
        let mut server = mockito::Server::new_async().await;
        let _order = server.mock("POST", "/order").create_async().await;
        let builder = HttpClientBuilder::new("https://example.com");
        let elapsed = time_to_sign(builder, &server).await;
        assert!(elapsed < Duration::from_millis(10));
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
            .match_query(mockito::Matcher::UrlEncoded("user".into(), "0xabc".into()))
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
            ApiError::Response(response) => {
                assert_eq!(response.status, StatusCode::NOT_FOUND);
                assert_eq!(response.message, "not found");
            }
            other => panic!("expected ApiError::Response, got {other:?}"),
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

    // ── gzip ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn gzip_follows_reqwest_unless_set() {
        // Unset, the client leaves reqwest's default, which asks for gzip with
        // the feature on. 0.37.0 forced it off, so a consumer whose workspace
        // had the feature on lost compression on every polyoxide client.
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/plain")
            .match_header("accept-encoding", mockito::Matcher::Regex("gzip".into()))
            .with_body("ok")
            .create_async()
            .await;

        let client = HttpClientBuilder::new(server.url()).build().unwrap();
        assert_eq!(client.get_bytes("/plain", &[]).await.unwrap(), b"ok");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn gzip_false_asks_for_no_compression() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/plain")
            .match_header("accept-encoding", mockito::Matcher::Missing)
            .with_body("ok")
            .create_async()
            .await;

        let client = HttpClientBuilder::new(server.url())
            .gzip(false)
            .build()
            .unwrap();
        assert_eq!(client.get_bytes("/plain", &[]).await.unwrap(), b"ok");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn gzip_asks_for_and_decodes_a_compressed_body() {
        // `{"serverTime":1}` compressed by python3's `gzip.compress(.., mtime=0)`.
        const GZIPPED: &[u8] = &[
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xab, 0x56, 0x2a, 0x4e,
            0x2d, 0x2a, 0x4b, 0x2d, 0x0a, 0xc9, 0xcc, 0x4d, 0x55, 0xb2, 0x32, 0xac, 0x05, 0x00,
            0xe2, 0x1d, 0x3e, 0x1a, 0x10, 0x00, 0x00, 0x00,
        ];
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/time")
            .match_header("accept-encoding", mockito::Matcher::Regex("gzip".into()))
            .with_header("content-encoding", "gzip")
            .with_body(GZIPPED)
            .create_async()
            .await;

        let client = HttpClientBuilder::new(server.url())
            .gzip(true)
            .build()
            .unwrap();
        let body = client.get_bytes("/time", &[]).await.unwrap();
        assert_eq!(body, br#"{"serverTime":1}"#);
        mock.assert_async().await;
    }

    #[test]
    fn with_base_url_retargets_and_shares_transport() {
        let client = HttpClientBuilder::new("https://data-api.polymarket.com")
            .with_max_concurrent(4)
            .build()
            .unwrap();
        let sibling = client
            .with_base_url("https://user-pnl-api.polymarket.com")
            .unwrap();

        assert_eq!(
            sibling.base_url.host_str(),
            Some("user-pnl-api.polymarket.com")
        );
        // The original is untouched — this returns a clone, not a mutation.
        assert_eq!(client.base_url.host_str(), Some("data-api.polymarket.com"));
        // Both must draw on the same concurrency budget, since the limit exists
        // to stop this process bursting rather than to pace any one host.
        assert!(sibling.concurrency_limiter.is_some());
    }

    #[test]
    fn with_base_url_rejects_a_malformed_url() {
        let client = HttpClientBuilder::new("https://data-api.polymarket.com")
            .build()
            .unwrap();
        assert!(client.with_base_url("not-a-url").is_err());
    }

    #[test]
    fn base_url_path_prefixes_are_dropped() {
        // Documented footgun, pinned so it cannot change silently: request
        // paths are absolute, so they replace the base path entirely. If this
        // test ever fails, the doc comment on with_base_url needs updating too.
        let client = HttpClientBuilder::new("http://localhost:8080/proxy")
            .build()
            .unwrap();
        assert_eq!(
            client.base_url.join("/user-pnl").unwrap().as_str(),
            "http://localhost:8080/user-pnl",
            "a path prefix in the base URL is not preserved"
        );

        let with_slash = client
            .with_base_url("http://localhost:8080/proxy/")
            .unwrap();
        assert_eq!(
            with_slash.base_url.join("/user-pnl").unwrap().as_str(),
            "http://localhost:8080/user-pnl",
            "a trailing slash does not preserve the prefix either"
        );
    }
}
