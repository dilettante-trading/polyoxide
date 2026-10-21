//! # Predictions
//!
//! Client for Kalshi's Predictions exchange (`external-api.kalshi.com`), behind the `predictions`
//! feature, which is on by default. What the module offers is in its namespaces.
//!
//! Every request runs on core's send loop and costs tokens from a Read throttle. A client with no
//! key has one bucket of 600 tokens refilling 180 a second, a tenth under Basic's 200 a second,
//! the highest rate Kalshi's unsigned reads were measured to serve clean. A client with a key
//! sizes the key's bucket from the account's own limits when it first sends (until then it holds
//! Basic's sizing), and sends its public reads from a second bucket sized as an unsigned
//! client's, because a signed public read does not draw on the key's. The hosts are [`PRODUCTION`] and [`DEMO`], and keys do not cross between
//! them.
//!
//! With the `predictions-ws` feature the module also has `ws`, the socket: it streams the order
//! book (a snapshot, then deltas, in YES pricing), the `ticker` and the `trade` channels. Its
//! handshake needs `Credentials` even for public data, which the REST reads do not.

pub mod api;
pub mod auth;
pub mod error;
pub mod key;
pub mod request;
mod throttle;
pub mod types;
pub mod venue;
#[cfg(feature = "predictions-ws")]
pub mod ws;

pub use auth::{Credentials, CredentialsError};
pub use error::{PredictionsError, VenueError};
pub use key::key;

use std::fmt;
use std::sync::Arc;

use polyoxide_core::{ApiError, ClientConfig, DynAuthenticator, HttpClient};
use tokio::sync::OnceCell;

use throttle::{PredictionsThrottle, RouteCosts};

/// Production REST host: a scheme and host, with no path.
pub const PRODUCTION: &str = "https://external-api.kalshi.com";

/// Demo REST host. Keys do not cross environments.
pub const DEMO: &str = "https://external-api.demo.kalshi.co";

/// In-flight requests the client allows by default.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;

/// Client for Kalshi's Predictions exchange.
///
/// A clone shares the client's connection pool, throttle and confirmation. A client built with no
/// credentials sends unsigned requests, which is all the public routes need.
///
/// ```no_run
/// use polyoxide_kalshi::predictions::Predictions;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let kalshi = Predictions::new()?;
/// let status = kalshi.exchange().status().send().await?;
/// println!("trading is active: {}", status.trading_active);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Predictions {
    pub(crate) inner: Arc<Inner>,
}

impl Predictions {
    /// A client for the production host, with default settings and no credentials.
    pub fn new() -> Result<Self, PredictionsError> {
        Self::builder().build()
    }

    /// Start configuring a client.
    pub fn builder() -> PredictionsBuilder {
        PredictionsBuilder::new()
    }

    polyoxide_core::namespaces! { inner;
        /// Liveness: a ping of the exchange-status route.
        health: api::Health,
        /// The exchange's status.
        exchange: api::ExchangeApi,
        /// Markets, their order books and their public trades.
        markets: api::MarketsApi,
        /// Events and their markets.
        events: api::EventsApi,
        /// The account's limits and the routes' token costs.
        account: api::AccountApi,
    }
}

/// Prints no key material.
impl fmt::Debug for Predictions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Predictions")
            .field("inner", &self.inner)
            .finish()
    }
}

/// What a client and every clone and namespace of it share.
pub(crate) struct Inner {
    pub(crate) http: HttpClient,
    pub(crate) throttle: PredictionsThrottle,
    pub(crate) costs: RouteCosts,
    pub(crate) confirmation: OnceCell<()>,
    pub(crate) auth: Option<Arc<DynAuthenticator<'static>>>,
}

/// Prints whether a key is set, never the key.
impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inner")
            .field("http", &self.http)
            .field("read", self.throttle.bucket())
            .field("public", self.throttle.public_bucket())
            .field("split", &self.throttle.splits_public_reads())
            .field("costs", &self.costs)
            .field("confirmed", &self.confirmation.initialized())
            .field("keyed", &self.auth.is_some())
            .finish()
    }
}

/// Builder for [`Predictions`].
///
/// It allows [`DEFAULT_MAX_CONCURRENT`] in-flight requests unless told otherwise.
#[derive(Clone)]
pub struct PredictionsBuilder {
    config: ClientConfig,
    pub(crate) auth: Option<Arc<DynAuthenticator<'static>>>,
}

impl Default for PredictionsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Prints whether a key is set, never the key.
impl fmt::Debug for PredictionsBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PredictionsBuilder")
            .field("config", &self.config)
            .field("keyed", &self.auth.is_some())
            .finish()
    }
}

impl PredictionsBuilder {
    /// A builder for the production host, with no credentials.
    pub fn new() -> Self {
        Self {
            config: ClientConfig::new(PRODUCTION, DEFAULT_MAX_CONCURRENT),
            auth: None,
        }
    }

    polyoxide_core::client_config_setters!(config);

    /// Build the client.
    ///
    /// The base URL is a scheme and host: every route's path is written from the API root, and the
    /// signed path is the request's path as written, so a base URL with a path would be silently
    /// dropped and is refused instead.
    ///
    /// The client's throttle is its Read layer, and its retry policy is core's default, which
    /// retries a `429` and holds every request, and fails every other status.
    ///
    /// # Errors
    ///
    /// [`ApiError::Url`] when the base URL does not parse, and [`ApiError::Validation`] when it
    /// has a path other than `/`.
    pub fn build(self) -> Result<Predictions, PredictionsError> {
        let base = &self.config.base_url;
        let parsed = url::Url::parse(base).map_err(ApiError::from)?;
        if parsed.path() != "/" {
            return Err(PredictionsError::Api(ApiError::Validation(format!(
                "the base URL must be a scheme and host with no path: {base}"
            ))));
        }
        let throttle = PredictionsThrottle::new();
        let http = self
            .config
            .http_builder()
            .with_throttle(throttle.clone())
            .build()?;
        Ok(Predictions {
            inner: Arc::new(Inner {
                http,
                throttle,
                costs: RouteCosts::default(),
                confirmation: OnceCell::new(),
                auth: self.auth,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_client_targets_production_and_the_demo_host_is_a_builder_away() {
        let production = Predictions::new().unwrap();
        assert_eq!(
            production.inner.http.base_url.as_str(),
            "https://external-api.kalshi.com/"
        );
        let demo = Predictions::builder().base_url(DEMO).build().unwrap();
        assert_eq!(
            demo.inner.http.base_url.as_str(),
            "https://external-api.demo.kalshi.co/"
        );
    }

    #[test]
    fn a_base_url_is_a_scheme_and_host() {
        for url in [
            "http://127.0.0.1:8080",
            "http://127.0.0.1:8080/",
            "https://external-api.kalshi.com",
        ] {
            assert!(
                Predictions::builder().base_url(url).build().is_ok(),
                "{url}"
            );
        }
        for url in [
            "http://127.0.0.1:8080/proxy",
            "http://127.0.0.1:8080/proxy/",
            "https://external-api.kalshi.com/trade-api/v2",
        ] {
            let err = Predictions::builder().base_url(url).build().unwrap_err();
            let message = match err {
                PredictionsError::Api(ApiError::Validation(message)) => message,
                other => panic!("{url}: {other:?}"),
            };
            assert_eq!(
                message,
                format!("the base URL must be a scheme and host with no path: {url}")
            );
        }
    }

    #[test]
    fn a_base_url_that_is_not_a_url_is_a_url_error() {
        let err = Predictions::builder()
            .base_url("not a url")
            .build()
            .unwrap_err();
        assert!(matches!(err, PredictionsError::Api(ApiError::Url(_))));
    }

    #[tokio::test]
    async fn the_default_concurrency_limit_is_4() {
        let kalshi = Predictions::new().unwrap();
        let mut permits = Vec::new();
        for _ in 0..DEFAULT_MAX_CONCURRENT {
            permits.push(kalshi.inner.http.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|permit| permit.is_some()));

        let fifth = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            kalshi.inner.http.acquire_concurrency(),
        )
        .await;
        assert!(
            fifth.is_err(),
            "a 5th permit should wait at the default of 4"
        );
    }

    #[test]
    fn a_clone_and_its_namespaces_share_one_state() {
        let kalshi = Predictions::new().unwrap();
        let clone = kalshi.clone();
        assert!(Arc::ptr_eq(&kalshi.inner, &clone.inner));
        assert!(Arc::ptr_eq(&kalshi.inner, &kalshi.health().inner));
        assert!(Arc::ptr_eq(&kalshi.inner, &kalshi.exchange().inner));
        let other = Predictions::new().unwrap();
        assert!(!Arc::ptr_eq(&kalshi.inner, &other.inner));
    }

    #[test]
    fn a_new_client_has_the_provisional_basic_read_bucket_and_no_key() {
        let kalshi = Predictions::new().unwrap();
        assert_eq!(
            kalshi.inner.throttle.bucket().capacity(),
            throttle::BASIC_READ_CAPACITY
        );
        assert_eq!(kalshi.inner.throttle.bucket().layer(), request::READ);
        assert!(kalshi.inner.auth.is_none());
        assert!(!kalshi.inner.confirmation.initialized());
        let text = format!("{kalshi:?}");
        assert!(text.contains("keyed: false"), "{text}");
        assert!(text.contains("provisional: true"), "{text}");
    }

    #[test]
    fn the_client_and_its_futures_are_send() {
        fn send<T: Send>(_: T) {}
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Predictions>();
        send_sync::<PredictionsBuilder>();
        let kalshi = Predictions::new().unwrap();
        send(kalshi.exchange().status().send());
        send(kalshi.health().ping());
    }

    #[test]
    fn the_printed_client_shows_both_read_buckets_and_the_split() {
        let kalshi = Predictions::new().unwrap();
        let text = format!("{kalshi:?}");
        // The key's bucket at Basic's refill and the public one at the unsigned client's.
        let read = text
            .split("public:")
            .next()
            .expect("split always yields one");
        assert!(read.contains("read: CapacityBucket"), "{text}");
        assert!(read.contains("refill_per_sec: 200,"), "{text}");
        let public = text
            .split("public:")
            .nth(1)
            .unwrap_or_else(|| panic!("the public bucket is not printed: {text}"));
        assert!(public.contains("CapacityBucket"), "{text}");
        assert!(public.contains("refill_per_sec: 180,"), "{text}");
        assert!(text.contains("split: false"), "{text}");
        assert!(text.contains("keyed: false"), "{text}");
    }
}
