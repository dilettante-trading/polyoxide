//! The transport settings every client builder holds.

use std::sync::Arc;

use polyoxide_rt::Runtime;

use crate::client::{HttpClientBuilder, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS};
use crate::rate_limit::RetryConfig;

/// The transport settings a client builder holds: the base URL, the timeout,
/// the connection pool, the retry schedule, the concurrency budget, the
/// runtime and the transport.
///
/// A venue's builder holds one, generates its setters with
/// [`client_config_setters!`](crate::client_config_setters), and builds its
/// [`HttpClient`](crate::HttpClient) from [`http_builder`](Self::http_builder),
/// adding the throttle and the retry policy it installs itself, the throttle
/// on the runtime [`runtime_or_default`](Self::runtime_or_default) names. The
/// config holds no throttle, policy or `gzip` setting, since those belong to
/// the venue.
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
    /// The transport every attempt is sent on; `None` keeps the target's
    /// default, which is built from the timeout, the pool size and the
    /// client's `gzip` (`ReqwestTransport` on the native targets).
    pub http_transport: Option<std::sync::Arc<dyn crate::transport::HttpTransport>>,
    /// The runtime every wait, clock read and spawn of the client goes
    /// through; `None` takes [`polyoxide_rt::default_runtime`].
    pub runtime: Option<Arc<dyn Runtime>>,
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
            http_transport: None,
            runtime: None,
            default_max_concurrent,
        }
    }

    /// An [`HttpClientBuilder`] for the base URL, with the timeout, the pool
    /// size, the concurrency (the set value, else the client's default), the
    /// runtime and the retry config if they are set. It sets no throttle,
    /// policy or `gzip`.
    pub fn http_builder(&self) -> HttpClientBuilder {
        let builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_max_concurrent(self.max_concurrent.unwrap_or(self.default_max_concurrent));
        let builder = match &self.runtime {
            Some(runtime) => builder.runtime(Arc::clone(runtime)),
            None => builder,
        };
        let builder = match &self.http_transport {
            Some(transport) => builder.http_transport(std::sync::Arc::clone(transport)),
            None => builder,
        };
        match &self.retry_config {
            Some(config) => builder.with_retry_config(config.clone()),
            None => builder,
        }
    }

    /// The runtime this config names, else the target's default
    /// ([`polyoxide_rt::default_runtime`]).
    ///
    /// A venue's `build` calls it once and gives the result both to
    /// [`HttpClientBuilder::runtime`](crate::HttpClientBuilder::runtime) and
    /// to the throttle it installs, so the client and its throttle keep time
    /// on one runtime.
    pub fn runtime_or_default(&self) -> Arc<dyn Runtime> {
        self.runtime
            .clone()
            .unwrap_or_else(polyoxide_rt::default_runtime)
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
            matches!(&err, ApiError::Transport(e) if e.kind() == crate::TransportKind::Timeout),
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

    #[test]
    fn the_runtime_setter_names_the_configs_runtime() {
        let runtime: Arc<dyn Runtime> = Arc::new(polyoxide_rt::TokioRuntime);
        let knobs = Knobs {
            config: ClientConfig::new("https://example.com", 4),
        }
        .runtime(Arc::clone(&runtime));

        assert!(Arc::ptr_eq(
            knobs.config.runtime.as_ref().unwrap(),
            &runtime
        ));
    }

    #[test]
    fn the_core_defaults_are_thirty_seconds_and_ten_idle() {
        let config = ClientConfig::new("https://x", 4);
        assert_eq!(config.timeout_ms, 30_000);
        assert_eq!(config.pool_size, 10);
        assert!(config.retry_config.is_none());
    }

    /// A transport that fails every attempt, so a request that reaches it
    /// proves the setter's value is the one the client sends on.
    #[derive(Debug)]
    struct Refusing;

    impl crate::HttpTransport for Refusing {
        fn send(
            &self,
            _request: crate::HttpRequest,
        ) -> polyoxide_rt::BoxFuture<Result<crate::Response, crate::TransportError>> {
            Box::pin(async {
                Err(crate::TransportError::new(
                    crate::TransportKind::Other,
                    "refused by the test",
                ))
            })
        }
    }

    #[tokio::test]
    async fn an_injected_transport_is_the_one_the_client_sends_on() {
        let knobs = Knobs {
            config: ClientConfig::new("https://example.com", 4),
        }
        .http_transport(std::sync::Arc::new(Refusing));
        assert!(knobs.config.http_transport.is_some());

        let http = knobs.config.http_builder().build().unwrap();
        let err = http.get_bytes("/", &[]).await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "Transport error: other: refused by the test"
        );
    }
}
