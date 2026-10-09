//! The transport settings every client builder holds.

use crate::client::{HttpClientBuilder, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS};
use crate::rate_limit::RetryConfig;

/// The transport settings a client builder holds: the base URL, the timeout,
/// the connection pool, the retry schedule and the concurrency budget.
///
/// A venue's builder holds one, generates its five setters with
/// [`client_config_setters!`](crate::client_config_setters), and builds its
/// [`HttpClient`](crate::HttpClient) from [`http_builder`](Self::http_builder),
/// adding the throttle and the retry policy it installs itself. The config
/// holds no throttle, policy or `gzip` setting, since those belong to the
/// venue.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The API's base URL.
    pub base_url: String,
    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Idle connections kept per host.
    pub pool_size: usize,
    /// The retry schedule; `None` keeps [`RetryConfig::default`].
    pub retry_config: Option<RetryConfig>,
    /// In-flight requests allowed; `None` keeps the client's default.
    pub max_concurrent: Option<usize>,
    default_max_concurrent: usize,
}

impl ClientConfig {
    /// A config for `base_url` with core's default timeout and pool size, no
    /// retry config of its own, and `default_max_concurrent` in-flight
    /// requests unless [`max_concurrent`](Self::max_concurrent) is set.
    pub fn new(base_url: impl Into<String>, default_max_concurrent: usize) -> Self {
        Self {
            base_url: base_url.into(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            retry_config: None,
            max_concurrent: None,
            default_max_concurrent,
        }
    }

    /// An [`HttpClientBuilder`] for the base URL, with the timeout, the pool
    /// size, the concurrency (the set value, else the client's default) and
    /// the retry config if one is set. It sets no throttle, policy or `gzip`.
    pub fn http_builder(&self) -> HttpClientBuilder {
        let builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_max_concurrent(self.max_concurrent.unwrap_or(self.default_max_concurrent));
        match &self.retry_config {
            Some(config) => builder.with_retry_config(config.clone()),
            None => builder,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::ApiError;

    #[test]
    fn a_new_config_holds_the_core_defaults() {
        let config = ClientConfig::new("https://example.com", 4);
        assert_eq!(config.base_url, "https://example.com");
        assert_eq!(config.timeout_ms, DEFAULT_TIMEOUT_MS);
        assert_eq!(config.pool_size, DEFAULT_POOL_SIZE);
        assert!(config.retry_config.is_none());
        assert_eq!(config.max_concurrent, None);
        assert_eq!(config.default_max_concurrent, 4);
    }

    /// The base URL, retry config and concurrency are read off the built
    /// client, and the timeout is seen to fire. reqwest exposes no pool size,
    /// so that knob is covered only by `client_config_setters_set_each_knob`.
    #[tokio::test]
    async fn http_builder_applies_every_knob_that_is_set() {
        // Accepted by the kernel, never answered.
        let silent = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut config = ClientConfig::new(format!("http://{}", silent.local_addr().unwrap()), 4);
        config.timeout_ms = 200;
        config.pool_size = 3;
        config.retry_config = Some(RetryConfig {
            max_retries: 7,
            initial_backoff_ms: 11,
            max_backoff_ms: 13,
        });
        config.max_concurrent = Some(2);

        let http = config.http_builder().build().unwrap();
        assert_eq!(http.base_url.as_str(), config.base_url.clone() + "/");
        assert_eq!(http.retry_config.max_retries, 7);
        assert_eq!(http.retry_config.initial_backoff_ms, 11);
        assert_eq!(http.retry_config.max_backoff_ms, 13);

        let first = http.acquire_concurrency().await;
        let second = http.acquire_concurrency().await;
        assert!(first.is_some() && second.is_some());
        let third =
            tokio::time::timeout(Duration::from_millis(50), http.acquire_concurrency()).await;
        assert!(
            third.is_err(),
            "a third permit was granted under a limit of 2"
        );
        drop((first, second));

        let started = std::time::Instant::now();
        let err = http.get_bytes("/", &[]).await.unwrap_err();
        assert!(
            matches!(&err, ApiError::Network(e) if e.is_timeout()),
            "expected a timeout, got {err:?}"
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn an_unset_concurrency_takes_the_clients_default() {
        let http = ClientConfig::new("https://example.com", 3)
            .http_builder()
            .build()
            .unwrap();
        let mut permits = Vec::new();
        for _ in 0..3 {
            permits.push(http.acquire_concurrency().await);
        }
        assert!(permits.iter().all(Option::is_some));
        let fourth =
            tokio::time::timeout(Duration::from_millis(50), http.acquire_concurrency()).await;
        assert!(
            fourth.is_err(),
            "a fourth permit was granted under a default of 3"
        );
        assert_eq!(
            http.retry_config.max_retries,
            RetryConfig::default().max_retries
        );
    }

    /// Core enables reqwest's `gzip` feature, so a client that leaves `gzip`
    /// unset asks for it.
    #[tokio::test]
    async fn the_config_leaves_gzip_unset() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .match_header("accept-encoding", mockito::Matcher::Regex("gzip".into()))
            .with_status(200)
            .create_async()
            .await;

        let http = ClientConfig::new(server.url(), 4)
            .http_builder()
            .build()
            .unwrap();
        http.get_bytes("/", &[]).await.unwrap();
        mock.assert_async().await;
    }

    struct Knobs {
        config: ClientConfig,
    }

    impl Knobs {
        crate::client_config_setters!(config);
    }

    #[test]
    fn client_config_setters_set_each_knob() {
        let knobs = Knobs {
            config: ClientConfig::new("https://example.com", 4),
        }
        .base_url("https://other.example.com")
        .timeout_ms(1_234)
        .pool_size(5)
        .with_retry_config(RetryConfig {
            max_retries: 9,
            ..RetryConfig::default()
        })
        .max_concurrent(6);

        assert_eq!(knobs.config.base_url, "https://other.example.com");
        assert_eq!(knobs.config.timeout_ms, 1_234);
        assert_eq!(knobs.config.pool_size, 5);
        assert_eq!(knobs.config.retry_config.unwrap().max_retries, 9);
        assert_eq!(knobs.config.max_concurrent, Some(6));
        assert_eq!(knobs.config.default_max_concurrent, 4);
    }
}
