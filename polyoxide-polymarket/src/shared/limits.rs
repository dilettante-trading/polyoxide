//! Polymarket's five window-quota tables, one per API surface.
//!
//! Each is pinned row for row by the `documented_*_limits` agreement tests,
//! which assert the effective quota a request resolves to.

use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "clob")]
use polyoxide_core::http::Method;
#[cfg(any(feature = "clob", feature = "data"))]
use polyoxide_core::rate_limit::Matching;
use polyoxide_core::rate_limit::{RateLimiter, WindowQuotaTable};
use polyoxide_rt::Runtime;

#[cfg(any(
    feature = "clob",
    feature = "gamma",
    feature = "data",
    feature = "perps"
))]
const TEN_SECONDS: Duration = Duration::from_secs(10);
#[cfg(feature = "clob")]
const TEN_MINUTES: Duration = Duration::from_secs(600);

/// A dual-window row: a burst window plus a sustained window, each a bucket
/// of the row's own.
#[cfg(feature = "clob")]
fn dual(
    table: &mut WindowQuotaTable,
    pattern: &'static str,
    method: Method,
    burst: (u32, Duration),
    sustained: (u32, Duration),
) {
    let buckets = [
        table.bucket(burst.0, burst.1),
        table.bucket(sustained.0, sustained.1),
    ];
    table.row(Matching::Prefix, pattern, Some(method), &buckets);
}

/// CLOB API rate limits.
///
/// Transcribed from <https://docs.polymarket.com/api-reference/rate-limits>
/// as fetched on 2026-07-25, and pinned by the `documented_limits` tests.
///
/// Two things about the published tables need interpreting:
///
/// - The **ledger group cap** (900/10s across `/trades`, `/orders`,
///   `/notifications` and `/order`) is genuinely shared, so those rows name
///   one shared bucket rather than four of their own.
/// - That group names `/order` and `/orders`, which also appear in the
///   trading table at 5,000 and 2,000 per 10s. Both tables can only hold
///   simultaneously if the group cap governs the ledger *reads*; a 900/10s
///   cap on all methods would make the published trading burst
///   unreachable. The group is therefore scoped to `GET`.
///
/// Ordering matters wherever one pattern is a path-segment prefix of
/// another: `/balance-allowance/update` must precede `/balance-allowance`,
/// and the specific `/data/*` routes must precede the `/data` catch-all.
#[cfg(feature = "clob")]
pub fn clob_limits() -> RateLimiter {
    clob_limits_on(polyoxide_rt::default_runtime())
}

/// [`clob_limits`], keeping time on `runtime`: the table's waits for a bucket
/// and for its hold sleep on it, and so do the signer buckets of a
/// [`ClobThrottle`](crate::shared::ClobThrottle) composed over its hold.
#[cfg(feature = "clob")]
pub fn clob_limits_on(runtime: Arc<dyn Runtime>) -> RateLimiter {
    let get = Some(Method::GET);
    let mut table = WindowQuotaTable::new(9_000, TEN_SECONDS);
    table.with_runtime(runtime);

    // Shared across the ledger read endpoints — one bucket, four patterns.
    let ledger_group = table.bucket(900, TEN_SECONDS);

    // ── Account. The tighter /update route must come first: it matches the
    // /balance-allowance prefix at a boundary.
    table
        .prefix("/balance-allowance/update", None, 50, TEN_SECONDS)
        .prefix("/balance-allowance", None, 200, TEN_SECONDS);
    // ── Trading (dual window: burst + sustained).
    for (pattern, method, burst, sustained) in [
        ("/order", Method::POST, 5_000, 120_000),
        ("/order", Method::DELETE, 5_000, 120_000),
        ("/orders", Method::POST, 2_000, 21_000),
        ("/orders", Method::DELETE, 2_000, 15_000),
        ("/cancel-all", Method::DELETE, 250, 6_000),
        ("/cancel-market-orders", Method::DELETE, 1_500, 21_000),
    ] {
        dual(
            &mut table,
            pattern,
            method,
            (burst, TEN_SECONDS),
            (sustained, TEN_MINUTES),
        );
    }
    // ── Ledger reads, sharing one 900/10s bucket. /notifications additionally
    // carries its own 125/10s cap.
    let notifications = table.bucket(125, TEN_SECONDS);
    table
        .row(
            Matching::Prefix,
            "/notifications",
            None,
            &[ledger_group, notifications],
        )
        .row(Matching::Prefix, "/trades", get.clone(), &[ledger_group])
        .row(Matching::Prefix, "/orders", get.clone(), &[ledger_group])
        .row(Matching::Prefix, "/order", get, &[ledger_group]);
    table
        // Specific /data routes before the catch-all. The previous pattern
        // here was "/data/", which the segment-boundary rule can never match —
        // it was dead configuration.
        .prefix("/data/orders", None, 500, TEN_SECONDS)
        .prefix("/data/trades", None, 500, TEN_SECONDS)
        .prefix("/data", None, 500, TEN_SECONDS)
        // ── Auth (matches /auth/derive-api-key etc.)
        .prefix("/auth", None, 100, TEN_SECONDS)
        // ── Market data. The batch forms are 3x tighter than their singular
        // siblings and do not match them: the boundary rule means "/books"
        // never resolves through "/book".
        .prefix("/prices-history", None, 1_000, TEN_SECONDS)
        .prefix("/book", None, 1_500, TEN_SECONDS)
        .prefix("/books", None, 500, TEN_SECONDS)
        .prefix("/price", None, 1_500, TEN_SECONDS)
        .prefix("/prices", None, 500, TEN_SECONDS)
        .prefix("/midpoint", None, 1_500, TEN_SECONDS)
        .prefix("/midpoints", None, 500, TEN_SECONDS)
        .prefix("/tick-size", None, 200, TEN_SECONDS)
        // ── Health.
        .prefix("/ok", None, 100, TEN_SECONDS)
        // ── Not in the published table. These are local, deliberately
        // conservative caps kept from earlier revisions; they only ever permit
        // less than the general bucket would. Listed last so no documented
        // rule is shadowed by them.
        .prefix("/markets", None, 1_500, TEN_SECONDS)
        .prefix("/neg-risk", None, 1_500, TEN_SECONDS);
    table.build()
}

/// Gamma API rate limits.
///
/// - General: 4,000/10s
/// - /events: 500/10s
/// - /markets: 300/10s
/// - /public-search: 350/10s
/// - /comments: 200/10s
/// - /tags: 200/10s
/// - `/status` (health): 100/10s
///
/// Upstream also lists a 900/10s cap shared by `/markets` + `/events`.
/// It is not modelled because it can never bind: the per-endpoint caps of
/// 300 and 500 sum to 800, which is already below it.
///
/// The published table spells the health row `/ok`, but that path answers
/// **404** on `gamma-api.polymarket.com` — `/status` is the route that
/// answers 200, and the one `Gamma::health().ping()` requests. `/ok` is
/// boilerplate repeated into every surface's table; only the CLOB host
/// serves it.
#[cfg(feature = "gamma")]
pub fn gamma_limits() -> RateLimiter {
    gamma_limits_on(polyoxide_rt::default_runtime())
}

/// [`gamma_limits`], keeping time on `runtime`.
#[cfg(feature = "gamma")]
pub fn gamma_limits_on(runtime: Arc<dyn Runtime>) -> RateLimiter {
    let mut table = WindowQuotaTable::new(4_000, TEN_SECONDS);
    table.with_runtime(runtime);
    table
        .prefix("/comments", None, 200, TEN_SECONDS)
        .prefix("/tags", None, 200, TEN_SECONDS)
        .prefix("/markets", None, 300, TEN_SECONDS)
        .prefix("/public-search", None, 350, TEN_SECONDS)
        .prefix("/events", None, 500, TEN_SECONDS)
        .prefix("/status", None, 100, TEN_SECONDS);
    table.build()
}

/// Data API rate limits, as Polymarket publishes them for v2 (it retired v1 on
/// 2026-11-02): <https://docs.polymarket.com/api-reference/rate-limits>.
///
/// - General (every `/v2` route): 800/10s
/// - `/v2/positions` and `/v2/positions/combos`: 200/10s, one allowance for both
/// - `/v2/activity` and `/v2/activity/combos`: 200/10s, one allowance for both
/// - `/v2/prices-history`: 200/10s
/// - `/v2/status`: 100/10s
///
/// The table prints each pair as one row, as it prints the CLOB's ledger reads,
/// which share one bucket, so each pair shares one here.
///
/// Three routes keep a measured rate instead (`docs/specs/data-v2/OBSERVED.md`):
/// `/v2/trades`, published at 300/10s, drew 429s on 2026-11-04 at the 27 req/s
/// a tenth under that allows, so it keeps the 200/10s measured clean before;
/// `/v2/user-pnl` and `/v2/holders`, which the table does not list, keep 400/10s.
#[cfg(feature = "data")]
pub fn data_limits() -> RateLimiter {
    data_limits_on(polyoxide_rt::default_runtime())
}

/// [`data_limits`], keeping time on `runtime`.
#[cfg(feature = "data")]
pub fn data_limits_on(runtime: Arc<dyn Runtime>) -> RateLimiter {
    let mut table = WindowQuotaTable::new(800, TEN_SECONDS);
    table.with_runtime(runtime);
    let positions = table.bucket(200, TEN_SECONDS);
    let activity = table.bucket(200, TEN_SECONDS);
    table
        .row(Matching::Prefix, "/v2/positions/combos", None, &[positions])
        .row(Matching::Prefix, "/v2/positions", None, &[positions])
        .row(Matching::Prefix, "/v2/activity/combos", None, &[activity])
        .row(Matching::Prefix, "/v2/activity", None, &[activity])
        .prefix("/v2/prices-history", None, 200, TEN_SECONDS)
        .prefix("/v2/status", None, 100, TEN_SECONDS)
        // Measured, `docs/specs/data-v2/OBSERVED.md`: published at 300/10s.
        .prefix("/v2/trades", None, 200, TEN_SECONDS)
        // Not in the published table: measured.
        .prefix("/v2/user-pnl", None, 400, TEN_SECONDS)
        .prefix("/v2/holders", None, 400, TEN_SECONDS);
    table.build()
}

/// Relay API rate limits.
///
/// - 25 requests per 1 minute (single limiter, no endpoint-specific limits)
#[cfg(feature = "relay")]
pub fn relay_limits() -> RateLimiter {
    relay_limits_on(polyoxide_rt::default_runtime())
}

/// [`relay_limits`], keeping time on `runtime`.
#[cfg(feature = "relay")]
pub fn relay_limits_on(runtime: Arc<dyn Runtime>) -> RateLimiter {
    let mut table = WindowQuotaTable::new(25, Duration::from_secs(60));
    table.with_runtime(runtime);
    table.build()
}

/// Perps API rate limits.
///
/// Upstream publishes no figure for the public `/v1/info/*` routes, only
/// that a per-IP token bucket exists. Each count is the highest clean
/// ramp stage measured by `polyoxide-polymarket/examples/info_soak.rs` on
/// 2026-09-30 and recorded in `docs/specs/perps/OBSERVED.md`; the table
/// reserves a tenth. The four routes throttle at different rates (a 429
/// on one arrived while the others kept being served), so each has its
/// own row. The 17 routes that were not soaked share a `/v1/info`
/// catch-all at the lowest measured rate, so an unmeasured route cannot
/// be driven harder than any measured one. The general bucket is a
/// client-wide cap measured separately: the budget is partly shared
/// across routes, so a mixed run capped at the most permissive route's
/// 50 per 10 s was throttled (11 of 487 requests over 120 s) while the
/// same run capped at `PERPS_GENERAL` was clean. See the validation runs
/// in OBSERVED.md.
///
/// With the general bucket at 30, bbo's own row never binds and the
/// klines and portfolio rows coincide with the general bucket; only the
/// trades row and the catch-all restrict further. The rows are kept at
/// their measured values so a later change to the general bucket does
/// not silently loosen a route.
///
/// Row order matters: prefix matching takes the first match, so the
/// catch-all must come last.
#[cfg(feature = "perps")]
pub fn perps_limits() -> RateLimiter {
    perps_limits_on(polyoxide_rt::default_runtime())
}

/// [`perps_limits`], keeping time on `runtime`.
#[cfg(feature = "perps")]
pub fn perps_limits_on(runtime: Arc<dyn Runtime>) -> RateLimiter {
    const PIN_KLINES: u32 = 30;
    const PIN_TRADES: u32 = 10;
    const PIN_PORTFOLIO: u32 = 30;
    const PIN_BBO: u32 = 50;
    /// Client-wide cap, per 10 s, from the mixed-route validation runs.
    const PERPS_GENERAL: u32 = 30;
    let lowest = [PIN_KLINES, PIN_TRADES, PIN_PORTFOLIO, PIN_BBO]
        .into_iter()
        .min()
        .expect("four rows");
    let mut table = WindowQuotaTable::new(PERPS_GENERAL, TEN_SECONDS);
    table.with_runtime(runtime);
    table
        .prefix("/v1/info/klines", None, PIN_KLINES, TEN_SECONDS)
        .prefix("/v1/info/trades", None, PIN_TRADES, TEN_SECONDS)
        .prefix("/v1/info/portfolio", None, PIN_PORTFOLIO, TEN_SECONDS)
        .prefix("/v1/info/bbo", None, PIN_BBO, TEN_SECONDS)
        .prefix("/v1/info", None, lowest, TEN_SECONDS);
    table.build()
}

#[cfg(all(test, feature = "clob"))]
mod tests {
    use std::time::Duration;

    use polyoxide_core::http::Method;
    use polyoxide_core::{ApiError, HttpClientBuilder, RequestParts};

    /// Notes when it signs: just after the throttle let the attempt go.
    #[derive(Default)]
    struct SignedAt(std::sync::Mutex<Option<std::time::Instant>>);

    impl polyoxide_core::hooks::Authenticator for SignedAt {
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
                Some(polyoxide_core::hooks::DynAuthenticator::from_ref(&signed)),
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
            .with_rate_limiter(crate::shared::clob_limits());
        let elapsed = time_to_sign(builder, &server).await;
        assert!(elapsed < Duration::from_millis(50));
    }
}

#[cfg(test)]
mod runtime_tests {
    //! Each table, and the CLOB's throttle, keeps time on the runtime it is
    //! built on.

    use polyoxide_rt::TokioRuntime;

    use super::*;

    fn injected() -> Arc<dyn Runtime> {
        Arc::new(TokioRuntime)
    }

    /// A runtime over tokio's that records its sleeps.
    #[cfg(feature = "clob")]
    #[derive(Debug, Default)]
    struct Counting {
        inner: TokioRuntime,
        sleeps: std::sync::Mutex<Vec<Duration>>,
    }

    #[cfg(feature = "clob")]
    impl Runtime for Counting {
        fn spawn(&self, task: polyoxide_rt::BoxFuture<()>) {
            self.inner.spawn(task);
        }

        fn sleep(&self, duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            self.sleeps.lock().unwrap().push(duration);
            self.inner.sleep(duration)
        }

        fn yield_now(&self) -> polyoxide_rt::BoxFuture<()> {
            Box::pin(tokio::task::yield_now())
        }

        fn now(&self) -> polyoxide_rt::Instant {
            self.inner.now()
        }
    }

    #[cfg(feature = "clob")]
    #[test]
    fn the_clob_table_keeps_time_on_its_runtime() {
        let runtime = injected();
        let limiter = clob_limits_on(Arc::clone(&runtime));
        assert!(Arc::ptr_eq(limiter.hold().runtime(), &runtime));
    }

    #[cfg(feature = "gamma")]
    #[test]
    fn the_gamma_table_keeps_time_on_its_runtime() {
        let runtime = injected();
        let limiter = gamma_limits_on(Arc::clone(&runtime));
        assert!(Arc::ptr_eq(limiter.hold().runtime(), &runtime));
    }

    #[cfg(feature = "data")]
    #[test]
    fn the_data_table_keeps_time_on_its_runtime() {
        let runtime = injected();
        let limiter = data_limits_on(Arc::clone(&runtime));
        assert!(Arc::ptr_eq(limiter.hold().runtime(), &runtime));
    }

    #[cfg(feature = "relay")]
    #[test]
    fn the_relay_table_keeps_time_on_its_runtime() {
        let runtime = injected();
        let limiter = relay_limits_on(Arc::clone(&runtime));
        assert!(Arc::ptr_eq(limiter.hold().runtime(), &runtime));
    }

    #[cfg(feature = "perps")]
    #[test]
    fn the_perps_table_keeps_time_on_its_runtime() {
        let runtime = injected();
        let limiter = perps_limits_on(Arc::clone(&runtime));
        assert!(Arc::ptr_eq(limiter.hold().runtime(), &runtime));
    }

    #[cfg(feature = "clob")]
    #[tokio::test]
    async fn a_hold_on_the_clob_throttle_is_waited_out_on_its_runtime() {
        use polyoxide_core::hooks::{RequestMeta, Throttle};

        let counting = Arc::new(Counting::default());
        let throttle = crate::shared::clob_throttle_on(counting.clone());
        // The IP table and both signer buckets share one hold, on one runtime.
        assert!(Arc::ptr_eq(
            throttle.signer().hold().runtime(),
            throttle.table().hold().runtime()
        ));

        throttle.hold(Duration::from_millis(60));
        let method = polyoxide_core::http::Method::GET;
        throttle
            .acquire(&RequestMeta {
                method: &method,
                path: "/ok",
                query: &[],
                costs: &[],
            })
            .await
            .unwrap();

        let slept = counting.sleeps.lock().unwrap().clone();
        assert!(
            slept
                .iter()
                .any(|d| *d > Duration::ZERO && *d <= Duration::from_millis(60)),
            "the hold was not waited out on the runtime: {slept:?}"
        );
    }
}
