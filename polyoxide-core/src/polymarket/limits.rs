//! Polymarket's five window-quota tables, one per API surface.
//!
//! Each is pinned row for row by the `documented_*_limits` agreement tests,
//! which assert the effective quota a request resolves to.

use std::time::Duration;

use reqwest::Method;

use crate::rate_limit::{Matching, RateLimiter, WindowQuotaTable};

const TEN_SECONDS: Duration = Duration::from_secs(10);
const TEN_MINUTES: Duration = Duration::from_secs(600);

/// A dual-window row: a burst window plus a sustained window, each a bucket
/// of the row's own.
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
pub fn clob_limits() -> RateLimiter {
    let get = Some(Method::GET);
    let mut table = WindowQuotaTable::new(9_000, TEN_SECONDS);

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
pub fn gamma_limits() -> RateLimiter {
    let mut table = WindowQuotaTable::new(4_000, TEN_SECONDS);
    table
        .prefix("/comments", None, 200, TEN_SECONDS)
        .prefix("/tags", None, 200, TEN_SECONDS)
        .prefix("/markets", None, 300, TEN_SECONDS)
        .prefix("/public-search", None, 350, TEN_SECONDS)
        .prefix("/events", None, 500, TEN_SECONDS)
        .prefix("/status", None, 100, TEN_SECONDS);
    table.build()
}

/// Data API rate limits.
///
/// - General: 1,000/10s
/// - /trades: 200/10s
/// - /positions and /closed-positions: 150/10s
/// - `/` (health): 100/10s
/// - Data API v2: measured per route, since upstream publishes no figures;
///   see `docs/specs/data-v2/OBSERVED.md`
///
/// The published table spells the health row `/ok`, but that path answers
/// **404** on `data-api.polymarket.com` — `/` answers 200 `{"data":"OK"}`,
/// and is the route this crate requests. `/ok` is boilerplate repeated into
/// every surface's table; only the CLOB host serves it.
///
/// Matching `/` is safe despite entries being prefix-matched: the
/// segment-boundary rule means `strip_prefix("/")` on `/positions` leaves
/// `positions`, which starts with neither `/` nor `?`, so the entry matches
/// only the bare root and the root with a query string.
///
/// This limiter is shared with the two sibling hosts, so it also carries
/// their rules:
///
/// - `/user-pnl`: 200/10s, published as the *host-wide* allowance for
///   `user-pnl-api.polymarket.com`. Modelled per-path because it is the
///   only route polyoxide calls there and matching has no host dimension.
/// - `lb-api.polymarket.com` (`/volume`, `/profit`) has no published limit,
///   so those fall to the general bucket.
pub fn data_limits() -> RateLimiter {
    let mut table = WindowQuotaTable::new(1_000, TEN_SECONDS);
    table
        .prefix("/closed-positions", None, 150, TEN_SECONDS)
        .prefix("/positions", None, 150, TEN_SECONDS)
        .prefix("/trades", None, 200, TEN_SECONDS)
        .prefix("/user-pnl", None, 200, TEN_SECONDS)
        // Data API v2. Upstream publishes no v2 figures; each count is the
        // highest clean rate from the ramps in docs/specs/data-v2/OBSERVED.md,
        // and the table reserves a tenth. `/v2/positions/combos` must precede
        // `/v2/positions`: prefix matching takes the first row that matches.
        .prefix("/v2/positions/combos", None, 400, TEN_SECONDS)
        .prefix("/v2/positions", None, 200, TEN_SECONDS)
        .prefix("/v2/trades", None, 200, TEN_SECONDS)
        .prefix("/v2/activity", None, 400, TEN_SECONDS)
        .prefix("/v2/user-pnl", None, 400, TEN_SECONDS)
        .prefix("/v2/holders", None, 400, TEN_SECONDS)
        .prefix("/", None, 100, TEN_SECONDS);
    table.build()
}

/// Relay API rate limits.
///
/// - 25 requests per 1 minute (single limiter, no endpoint-specific limits)
pub fn relay_limits() -> RateLimiter {
    WindowQuotaTable::new(25, Duration::from_secs(60)).build()
}

/// Perps API rate limits.
///
/// Upstream publishes no figure for the public `/v1/info/*` routes, only
/// that a per-IP token bucket exists. Each count is the highest clean
/// ramp stage measured by `polyoxide-perps/examples/info_soak.rs` on
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
pub fn perps_limits() -> RateLimiter {
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
    table
        .prefix("/v1/info/klines", None, PIN_KLINES, TEN_SECONDS)
        .prefix("/v1/info/trades", None, PIN_TRADES, TEN_SECONDS)
        .prefix("/v1/info/portfolio", None, PIN_PORTFOLIO, TEN_SECONDS)
        .prefix("/v1/info/bbo", None, PIN_BBO, TEN_SECONDS)
        .prefix("/v1/info", None, lowest, TEN_SECONDS);
    table.build()
}
