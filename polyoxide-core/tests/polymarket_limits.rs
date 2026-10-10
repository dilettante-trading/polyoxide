//! Polymarket's window-quota tables, held to what Polymarket publishes.
//!
//! The agreement, `documented_*_limits`, table and cooldown tests, moved from
//! `polyoxide-core/src/rate_limit.rs` with their module names. Each asserts
//! through public API only: `RateLimiter::effective_quota`, `rows()`,
//! `acquire`, `begin_cooldown` and `WindowQuotaTable::paced_interval`.

use std::time::Duration;

use polyoxide_core::{polymarket, EffectiveQuota, Matching, RateLimiter, WindowQuotaTable};
use reqwest::Method;

mod quota_arithmetic {
    //! The table sweep of core's `quota_arithmetic`: every bucket of every
    //! table admits no more than it publishes.

    use super::*;

    #[test]
    fn every_configured_bucket_satisfies_the_quota_it_publishes() {
        for (surface, rl) in [
            ("clob", polymarket::clob_limits()),
            ("gamma", polymarket::gamma_limits()),
            ("data", polymarket::data_limits()),
            ("relay", polymarket::relay_limits()),
        ] {
            for row in rl.rows() {
                for bucket in &row.buckets {
                    let EffectiveQuota { count, period, .. } = *bucket;
                    let admitted = bucket.admitted_in_one_window();

                    assert!(
                        admitted <= u128::from(count),
                        "{surface} {} is published as {count}/{period:?} but admits \
                         {admitted} in one window",
                        row.pattern,
                    );
                }
            }
        }
    }
}

mod agreement {
    //! Shared machinery for the per-surface `documented_*_limits` modules.
    //!
    //! Every API surface pins its published table the same way: assert the
    //! *effective quota* a request resolves to, not merely that some entry
    //! exists. Checking only for presence and ordering is why
    //! `/balance-allowance` could once be absent entirely while every test
    //! passed, and why `/closed-positions` could be set to 66x its published
    //! cap without a single failure.

    use super::*;

    /// A quota as published: `count` requests per `period`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Spec {
        pub count: u32,
        pub period: Duration,
    }

    /// The quotas a request resolves to, read through
    /// [`RateLimiter::effective_quota`].
    pub trait ResolveSpecs {
        /// The quotas a request would be held to beyond the general bucket, in
        /// the order they are awaited.
        ///
        /// Empty when nothing matches — meaning the request is governed only by
        /// the general bucket, which is the shape every over-permit bug in this
        /// table has taken.
        fn resolve_specs(&self, path: &str, method: Option<&Method>) -> Vec<Spec>;
    }

    impl ResolveSpecs for RateLimiter {
        fn resolve_specs(&self, path: &str, method: Option<&Method>) -> Vec<Spec> {
            let method = method.expect("every rule asserted here names its method");
            // `effective_quota` lists the general bucket first; slice it off.
            self.effective_quota(method, path)[1..]
                .iter()
                .map(|q| Spec {
                    count: q.count,
                    period: q.period,
                })
                .collect()
        }
    }

    /// One published rule: the request it applies to, and the buckets it must
    /// pass, as `(count, window_secs)` in the order `acquire` awaits them.
    pub type DocumentedRule = (&'static str, Option<Method>, Vec<(u32, u64)>);

    /// Assert every rule resolves to exactly the quota Polymarket publishes.
    ///
    /// `general` is the surface's catch-all allowance. It is only used to make
    /// the failure message name the over-permit factor, since falling through
    /// to the general bucket is the shape every bug in these tables has taken.
    pub fn assert_matches_published(rl: &RateLimiter, rules: Vec<DocumentedRule>, general: u32) {
        for (path, method, expected) in rules {
            let resolved = rl.resolve_specs(path, method.as_ref());
            assert!(
                !resolved.is_empty(),
                "{method:?} {path} matches no endpoint limit — it falls through to the \
                 general {general}/10s bucket, over-permitting by {}x",
                general / expected[0].0.max(1),
            );
            let actual: Vec<(u32, u64)> = resolved
                .iter()
                .map(|s| (s.count, s.period.as_secs()))
                .collect();
            assert_eq!(
                actual, expected,
                "{method:?} {path} resolves to {actual:?}, published limit is {expected:?}"
            );
        }
    }

    /// Assert `path` is governed by nothing but the general bucket.
    ///
    /// Used to pin routes that upstream's table names but the host does not
    /// actually serve, so a dead entry cannot quietly reappear.
    pub fn assert_unconfigured(rl: &RateLimiter, path: &str) {
        assert!(
            rl.resolve_specs(path, Some(&Method::GET)).is_empty(),
            "{path} has an endpoint limit configured, but the host answers 404 there — \
             the entry is dead configuration and the real route is going unlimited"
        );
    }

    /// Assert `path` is paced at runtime by the quota it publishes.
    ///
    /// Matching a spec is not the same as enforcing it; this is the runtime
    /// half of the agreement. Buckets hold a single token, so one request
    /// empties `path`'s bucket and the next has to wait a full replenish
    /// interval — no `count`-sized drain required, and none wanted: draining
    /// `count` under uniform pacing takes a real `period`, which would put a
    /// 10-second sleep in the unit suite for every row asserted.
    ///
    /// Asserting *how long* the wait is, rather than merely that there was
    /// one, is what makes this specific. The delay identifies which bucket the
    /// request came from: `/closed-positions` (150/10s) paces at ~67ms while
    /// the surface's general bucket paces at ~10ms. The upper bound catches
    /// the inverse failure — a path resolving through some tighter rule that
    /// shadows it, which is the shape every ordering bug in these tables has
    /// taken.
    pub async fn assert_paced_by_its_own_quota(
        rl: &RateLimiter,
        path: &str,
        count: u32,
        period: Duration,
    ) {
        let interval = WindowQuotaTable::paced_interval(count, period);

        rl.acquire(path, Some(&Method::GET)).await;

        let start = std::time::Instant::now();
        rl.acquire(path, Some(&Method::GET)).await;
        let waited = start.elapsed();

        assert!(
            waited >= interval.mul_f64(0.8),
            "the 2nd request to {path} returned in {waited:?}; {count}/{period:?} should pace \
             it at {interval:?} and the cap is not being enforced"
        );
        assert!(
            waited <= interval * 3 + Duration::from_millis(25),
            "the 2nd request to {path} waited {waited:?}, far longer than the {interval:?} its \
             published {count}/{period:?} implies — it is resolving through a tighter rule"
        );
    }
}

mod documented_data_limits {
    //! Agreement tests for the Data API's published table.
    //!
    //! Transcribed from <https://docs.polymarket.com/api-reference/rate-limits>
    //! as fetched on 2026-08-05.
    //!
    //! Two rows need interpreting, both verified against the live hosts:
    //!
    //! - The health row is published as `/ok`, but `data-api.polymarket.com/ok`
    //!   answers **404** while `/` answers 200 `{"data":"OK"}`. The `/ok`
    //!   spelling is boilerplate repeated into every surface's table; only
    //!   `clob.polymarket.com` actually serves it. The cap is therefore
    //!   attached to `/`, the route this crate requests and the host answers.
    //! - "User PNL API 200 req/10s" is published as a *host-wide* allowance for
    //!   `user-pnl-api.polymarket.com`. It is modelled as a path rule on
    //!   `/user-pnl` because that is the only route polyoxide calls there and
    //!   the limiter matches on path alone, with no host dimension.

    use super::agreement::*;
    use super::*;

    /// The published table, transcribed by hand. This is the golden vector.
    fn documented() -> Vec<DocumentedRule> {
        vec![
            ("/trades", Some(Method::GET), vec![(200, 10)]),
            ("/positions", Some(Method::GET), vec![(150, 10)]),
            ("/closed-positions", Some(Method::GET), vec![(150, 10)]),
            ("/", Some(Method::GET), vec![(100, 10)]),
            ("/user-pnl", Some(Method::GET), vec![(200, 10)]),
        ]
    }

    #[test]
    fn every_documented_endpoint_resolves_to_its_published_quota() {
        assert_matches_published(&polymarket::data_limits(), documented(), 1_000);
    }

    /// Data API v2 rows. Upstream publishes no v2 figures; each count is the
    /// highest clean ramp rate recorded in `docs/specs/data-v2/OBSERVED.md`.
    fn measured_v2() -> Vec<DocumentedRule> {
        vec![
            ("/v2/positions", Some(Method::GET), vec![(200, 10)]),
            ("/v2/positions/combos", Some(Method::GET), vec![(400, 10)]),
            ("/v2/trades", Some(Method::GET), vec![(200, 10)]),
            ("/v2/activity", Some(Method::GET), vec![(400, 10)]),
            ("/v2/user-pnl", Some(Method::GET), vec![(400, 10)]),
            ("/v2/holders", Some(Method::GET), vec![(400, 10)]),
        ]
    }

    #[test]
    fn every_v2_route_resolves_to_its_measured_quota() {
        assert_matches_published(&polymarket::data_limits(), measured_v2(), 1_000);
    }

    #[test]
    fn the_health_cap_is_attached_to_the_route_the_host_answers_on() {
        // `/ok` is a 404 on data-api. An entry there caps nothing and leaves
        // the real health route — `/` — on the 10x-looser general bucket.
        assert_unconfigured(&polymarket::data_limits(), "/ok");
    }

    #[test]
    fn the_root_health_rule_does_not_swallow_every_other_route() {
        // `/` under prefix matching could plausibly match everything. The
        // segment-boundary rule saves it: `strip_prefix("/")` on `/positions`
        // leaves `positions`, which starts with neither `/` nor `?`.
        let rl = polymarket::data_limits();
        for (path, expected) in [
            ("/positions", 150),
            ("/closed-positions", 150),
            ("/trades", 200),
            ("/", 100),
        ] {
            let specs = rl.resolve_specs(path, Some(&Method::GET));
            assert_eq!(
                specs[0].count, expected,
                "{path} resolved through the wrong rule — the `/` entry is over-matching"
            );
        }
    }

    #[tokio::test]
    async fn the_closed_positions_cap_actually_throttles() {
        // 150/10s paces one request every ~67ms.
        assert_paced_by_its_own_quota(
            &polymarket::data_limits(),
            "/closed-positions",
            150,
            Duration::from_secs(10),
        )
        .await;
    }

    #[tokio::test]
    async fn closed_positions_and_positions_do_not_share_an_allowance() {
        // Upstream publishes 150/10s for each, not 150/10s combined. Emptying
        // one must leave the other untouched — the inverse of the CLOB ledger
        // group, where sharing *is* the published behaviour.
        //
        // The margin here is ~67ms (shared) against ~10ms (separate, and only
        // that much because the request still passes the surface's general
        // 1,000/10s bucket). Both sides of that gap are load-bearing, so the
        // threshold sits between them rather than at zero.
        let rl = polymarket::data_limits();
        rl.acquire("/closed-positions", Some(&Method::GET)).await;

        let start = std::time::Instant::now();
        rl.acquire("/positions", Some(&Method::GET)).await;
        assert!(
            start.elapsed() < Duration::from_millis(25),
            "/positions was throttled by /closed-positions emptying its own bucket"
        );
    }
}

mod documented_gamma_limits {
    //! Agreement tests for the Gamma API's published table.
    //!
    //! Transcribed from <https://docs.polymarket.com/api-reference/rate-limits>
    //! as fetched on 2026-08-05. As with the Data API, the published health row
    //! reads `/ok`, but `gamma-api.polymarket.com/ok` answers 404 — `/status`
    //! is the route that answers 200.

    use super::agreement::*;
    use super::*;

    /// The published table, transcribed by hand. This is the golden vector.
    fn documented() -> Vec<DocumentedRule> {
        vec![
            ("/events", Some(Method::GET), vec![(500, 10)]),
            ("/public-search", Some(Method::GET), vec![(350, 10)]),
            ("/markets", Some(Method::GET), vec![(300, 10)]),
            ("/comments", Some(Method::GET), vec![(200, 10)]),
            ("/tags", Some(Method::GET), vec![(200, 10)]),
            ("/status", Some(Method::GET), vec![(100, 10)]),
        ]
    }

    #[test]
    fn every_documented_endpoint_resolves_to_its_published_quota() {
        assert_matches_published(&polymarket::gamma_limits(), documented(), 4_000);
    }

    #[test]
    fn the_health_cap_is_attached_to_the_route_the_host_answers_on() {
        assert_unconfigured(&polymarket::gamma_limits(), "/ok");
    }

    #[test]
    fn the_markets_plus_events_group_cap_can_never_bind() {
        // Upstream also publishes a 900/10s cap shared by /markets + /events.
        // It is deliberately not modelled because the per-endpoint caps sum to
        // less than it. If either cap is ever raised, this stops being true and
        // the group bucket has to be added — that is what this test watches.
        let rl = polymarket::gamma_limits();
        let markets = rl.resolve_specs("/markets", Some(&Method::GET))[0].count;
        let events = rl.resolve_specs("/events", Some(&Method::GET))[0].count;
        assert!(
            markets + events <= 900,
            "/markets ({markets}) + /events ({events}) now exceeds the published 900/10s \
             group cap, which is no longer unreachable and must be modelled"
        );
    }

    #[tokio::test]
    async fn the_markets_cap_actually_throttles() {
        assert_paced_by_its_own_quota(
            &polymarket::gamma_limits(),
            "/markets",
            300,
            Duration::from_secs(10),
        )
        .await;
    }
}

mod documented_perps_limits {
    //! Agreement tests for the Perps table. Nothing is published, so the
    //! rows are the measured figures in `docs/specs/perps/OBSERVED.md`.

    use super::agreement::*;
    use super::*;

    /// The measured table, transcribed by hand from the OBSERVED.md runs.
    /// This is the golden vector: a second transcription, separate from the
    /// constants inside `perps_default()`, so a typo in either is caught.
    const KLINES: u32 = 30;
    const TRADES: u32 = 10;
    const PORTFOLIO: u32 = 30;
    const BBO: u32 = 50;
    /// Every other `/v1/info/*` route: the lowest measured row.
    const UNSOAKED: u32 = 10;
    /// The client-wide cap the mixed validation run was clean at.
    const GENERAL: u32 = 30;

    fn measured() -> Vec<DocumentedRule> {
        vec![
            ("/v1/info/klines", Some(Method::GET), vec![(KLINES, 10)]),
            ("/v1/info/trades", Some(Method::GET), vec![(TRADES, 10)]),
            (
                "/v1/info/portfolio",
                Some(Method::GET),
                vec![(PORTFOLIO, 10)],
            ),
            ("/v1/info/bbo", Some(Method::GET), vec![(BBO, 10)]),
            (
                "/v1/info/instruments",
                Some(Method::GET),
                vec![(UNSOAKED, 10)],
            ),
            (
                "/v1/info/statistics",
                Some(Method::GET),
                vec![(UNSOAKED, 10)],
            ),
        ]
    }

    #[test]
    fn every_soaked_route_has_its_own_row() {
        assert_matches_published(&polymarket::perps_limits(), measured(), u32::MAX);
    }

    #[test]
    fn an_unsoaked_info_route_is_held_to_the_catch_all_not_the_general_bucket() {
        // The general bucket is 30/10s, so without the catch-all an
        // unmeasured route would be driven three times harder than trades,
        // the tightest route measured.
        let rl = polymarket::perps_limits();
        let specs = rl.resolve_specs("/v1/info/instruments", Some(&Method::GET));
        assert_eq!(
            specs.len(),
            1,
            "instruments matched no row: the /v1/info catch-all is missing or misordered"
        );
        assert_eq!(specs[0].count, UNSOAKED);
        // A soaked route must resolve to its own row, not the catch-all.
        assert_eq!(
            rl.resolve_specs("/v1/info/bbo", Some(&Method::GET))[0].count,
            BBO
        );
        // Outside /v1/info nothing is configured.
        assert_unconfigured(&rl, "/v1/nope");
    }

    #[tokio::test]
    async fn the_general_bucket_caps_the_client_below_the_most_permissive_route() {
        // bbo alone sustains 50/10s, but a mixed run at that cap was
        // throttled: the per-IP budget is partly shared. The general bucket
        // must therefore bind before bbo's own row does. The pacing window
        // accepted here admits a general bucket of roughly 11 to 37, so this
        // proves general < bbo; the exact 30 is pinned by GENERAL against
        // OBSERVED.md, not by timing.
        const _: () = assert!(
            GENERAL < BBO,
            "the general bucket must bind before bbo's row"
        );
        let rl = polymarket::perps_limits();
        assert_paced_by_its_own_quota(&rl, "/v1/info/bbo", GENERAL, Duration::from_secs(10)).await;
    }

    #[tokio::test]
    async fn the_klines_row_actually_paces() {
        let rl = polymarket::perps_limits();
        let count = rl.resolve_specs("/v1/info/klines", Some(&Method::GET))[0].count;
        assert_paced_by_its_own_quota(&rl, "/v1/info/klines", count, Duration::from_secs(10)).await;
    }
}

mod documented_limits {
    //! Table-driven agreement tests against Polymarket's published limits.
    //!
    //! Transcribed from <https://docs.polymarket.com/api-reference/rate-limits>
    //! as fetched on 2026-07-25, re-confirmed 2026-08-05. These assert the
    //! *effective quota* a request resolves to, not merely that some entry
    //! exists — the previous tests only checked that entries were present and
    //! in the right order, which is why `/balance-allowance` could be absent
    //! entirely while every test passed.

    use super::agreement::{assert_paced_by_its_own_quota, DocumentedRule, ResolveSpecs};
    use super::*;

    /// The published table, transcribed by hand. This is the golden vector.
    fn documented() -> Vec<DocumentedRule> {
        vec![
            // ── Account ──
            ("/balance-allowance", Some(Method::GET), vec![(200, 10)]),
            (
                "/balance-allowance/update",
                Some(Method::GET),
                vec![(50, 10)],
            ),
            // ── Trading (dual window) ──
            (
                "/order",
                Some(Method::POST),
                vec![(5_000, 10), (120_000, 600)],
            ),
            (
                "/order",
                Some(Method::DELETE),
                vec![(5_000, 10), (120_000, 600)],
            ),
            (
                "/orders",
                Some(Method::POST),
                vec![(2_000, 10), (21_000, 600)],
            ),
            (
                "/orders",
                Some(Method::DELETE),
                vec![(2_000, 10), (15_000, 600)],
            ),
            (
                "/cancel-all",
                Some(Method::DELETE),
                vec![(250, 10), (6_000, 600)],
            ),
            (
                "/cancel-market-orders",
                Some(Method::DELETE),
                vec![(1_500, 10), (21_000, 600)],
            ),
            // ── Ledger: a cap shared across the group, plus per-endpoint caps ──
            ("/trades", Some(Method::GET), vec![(900, 10)]),
            ("/orders", Some(Method::GET), vec![(900, 10)]),
            ("/order", Some(Method::GET), vec![(900, 10)]),
            (
                "/notifications",
                Some(Method::GET),
                vec![(900, 10), (125, 10)],
            ),
            ("/data/orders", Some(Method::GET), vec![(500, 10)]),
            ("/data/trades", Some(Method::GET), vec![(500, 10)]),
            // ── Market data ──
            ("/book", Some(Method::GET), vec![(1_500, 10)]),
            ("/books", Some(Method::POST), vec![(500, 10)]),
            ("/price", Some(Method::GET), vec![(1_500, 10)]),
            ("/prices", Some(Method::POST), vec![(500, 10)]),
            ("/midpoint", Some(Method::GET), vec![(1_500, 10)]),
            ("/midpoints", Some(Method::POST), vec![(500, 10)]),
            ("/prices-history", Some(Method::GET), vec![(1_000, 10)]),
            ("/tick-size", Some(Method::GET), vec![(200, 10)]),
            // ── Auth & health ──
            ("/auth/api-key", Some(Method::POST), vec![(100, 10)]),
            ("/ok", Some(Method::GET), vec![(100, 10)]),
        ]
    }

    #[test]
    fn every_documented_endpoint_resolves_to_its_published_quota() {
        let rl = polymarket::clob_limits();

        for (path, method, expected) in documented() {
            let resolved = rl.resolve_specs(path, method.as_ref());
            assert!(
                !resolved.is_empty(),
                "{method:?} {path} matches no endpoint limit — it falls through to the \
                 general {}/10s bucket, over-permitting by {}x",
                9_000,
                9_000 / expected[0].0.max(1),
            );
            let actual: Vec<(u32, u64)> = resolved
                .iter()
                .map(|s| (s.count, s.period.as_secs()))
                .collect();
            assert_eq!(
                actual, expected,
                "{method:?} {path} resolves to {actual:?}, published limit is {expected:?}"
            );
        }
    }

    #[test]
    fn batch_endpoints_do_not_inherit_their_singular_sibling() {
        // `/books` must not resolve through the `/book` rule: they are
        // different endpoints with a 3x difference in allowance.
        let rl = polymarket::clob_limits();
        for (batch, singular) in [
            ("/books", "/book"),
            ("/prices", "/price"),
            ("/midpoints", "/midpoint"),
        ] {
            let batch_specs = rl.resolve_specs(batch, Some(&Method::POST));
            let singular_specs = rl.resolve_specs(singular, Some(&Method::GET));
            assert_ne!(
                batch_specs, singular_specs,
                "{batch} is being limited as if it were {singular}"
            );
            assert_eq!(batch_specs[0].count, 500, "{batch} should allow 500/10s");
        }
    }

    #[test]
    fn the_ledger_group_cap_is_one_shared_bucket() {
        // Upstream caps `/trades`, `/orders`, `/notifications` and `/order`
        // at 900/10s *combined*. Modelling that as four independent 900/10s
        // buckets would permit 3,600/10s.
        let rl = polymarket::clob_limits();
        let group: Vec<_> = ["/trades", "/orders", "/order", "/notifications"]
            .iter()
            .map(|p| {
                rl.effective_quota(&Method::GET, p)
                    .get(1)
                    .unwrap_or_else(|| panic!("{p} should match a ledger entry"))
                    .bucket
            })
            .collect();

        for other in &group[1..] {
            assert!(
                group[0] == *other,
                "ledger endpoints must share one bucket, not hold copies"
            );
        }
    }

    #[test]
    fn balance_allowance_update_is_not_shadowed_by_its_parent_path() {
        // `/balance-allowance/update` starts with `/balance-allowance` at a
        // segment boundary, so ordering decides which rule wins. The update
        // route is four times tighter.
        let rl = polymarket::clob_limits();
        let update = rl.resolve_specs("/balance-allowance/update", Some(&Method::GET));
        assert_eq!(
            update[0].count, 50,
            "the tighter /balance-allowance/update rule must be ordered first"
        );
    }

    #[tokio::test]
    async fn a_documented_cap_actually_throttles() {
        // Matching specs is not the same as enforcing them. `/tick-size` is
        // 200/10s, which paces one request every ~50ms.
        assert_paced_by_its_own_quota(
            &polymarket::clob_limits(),
            "/tick-size",
            200,
            Duration::from_secs(10),
        )
        .await;
    }

    #[tokio::test]
    async fn the_ledger_group_allowance_is_consumed_jointly() {
        // The runtime counterpart to the shared-bucket check: consuming the group
        // through one endpoint must leave a *different* group member throttled.
        // The shared 900/10s bucket paces at ~11ms; with four independent
        // buckets /orders would only meet the general 9,000/10s one at ~1.1ms.
        let rl = polymarket::clob_limits();
        rl.acquire("/trades", Some(&Method::GET)).await;

        let start = std::time::Instant::now();
        rl.acquire("/orders", Some(&Method::GET)).await;
        let waited = start.elapsed();

        assert!(
            waited >= Duration::from_millis(5),
            "GET /orders returned in {waited:?} after /trades consumed from the shared 900/10s \
             allowance — the group cap is not actually shared"
        );
    }

    #[test]
    fn post_order_is_not_throttled_by_the_ledger_group() {
        // The ledger group names `/order`, but the trading table allows POST
        // /order 5,000/10s. Both can only hold if the group cap is the ledger
        // *read*. Applying it to POST would make the published burst
        // unreachable.
        let rl = polymarket::clob_limits();
        let specs = rl.resolve_specs("/order", Some(&Method::POST));
        assert_eq!(specs[0].count, 5_000);
        assert!(
            !specs.iter().any(|s| s.count == 900),
            "POST /order must not be caught by the ledger read cap"
        );
    }
}

mod tests {
    use super::agreement::ResolveSpecs;
    use super::*;

    // ── Factory methods ──────────────────────────────────────────

    #[test]
    fn test_clob_default_construction() {
        let rl = polymarket::clob_limits();
        assert_eq!(rl.rows().len(), 27);
        assert!(format!("{:?}", rl).contains("endpoints"));
    }

    #[test]
    fn test_gamma_default_construction() {
        let rl = polymarket::gamma_limits();
        assert_eq!(rl.rows().len(), 6);
    }

    #[test]
    fn test_data_default_construction() {
        let rl = polymarket::data_limits();
        assert_eq!(rl.rows().len(), 11);
    }

    #[test]
    fn test_relay_default_construction() {
        let rl = polymarket::relay_limits();
        assert_eq!(rl.rows().len(), 0);
    }

    #[test]
    fn test_rate_limiter_debug_format() {
        let rl = polymarket::clob_limits();
        let dbg = format!("{:?}", rl);
        assert!(dbg.contains("RateLimiter"), "missing struct name: {dbg}");
        assert!(dbg.contains("endpoints: 27"), "missing count: {dbg}");
    }

    // ── Endpoint matching internals ──────────────────────────────

    #[test]
    fn test_clob_tighter_rules_precede_the_prefixes_that_would_shadow_them() {
        // Ordering is only load-bearing where one pattern is a path-segment
        // prefix of another. Asserting on fixed indices made this test brittle
        // and told us nothing; assert the actual constraint instead.
        let rl = polymarket::clob_limits();
        let rows = rl.rows();
        let index_of = |path: &str| {
            rows.iter()
                .position(|l| l.pattern == path)
                .unwrap_or_else(|| panic!("{path} should be configured"))
        };

        for (specific, general) in [
            ("/balance-allowance/update", "/balance-allowance"),
            ("/data/orders", "/data"),
            ("/data/trades", "/data"),
        ] {
            assert!(
                index_of(specific) < index_of(general),
                "{specific} must be matched before {general} or it can never win"
            );
        }
    }

    // ── acquire() async behavior ─────────────────────────────────

    #[tokio::test]
    async fn test_acquire_single_completes_immediately() {
        let rl = polymarket::clob_limits();
        let start = std::time::Instant::now();
        rl.acquire("/order", Some(&Method::POST)).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_matches_endpoint_by_prefix() {
        let rl = polymarket::clob_limits();
        let start = std::time::Instant::now();
        // /order/123 should match the /order prefix
        rl.acquire("/order/123", Some(&Method::POST)).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_prefix_respects_segment_boundary() {
        let rl = polymarket::clob_limits();
        let limits = rl.rows();

        // Find the /price entry
        let price_idx = limits
            .iter()
            .position(|l| l.pattern == "/price")
            .expect("/price endpoint exists");

        // /prices-history must NOT match /price — it's a different endpoint
        let prices_history_idx = limits
            .iter()
            .position(|l| l.pattern == "/prices-history")
            .expect("/prices-history endpoint exists");

        // /prices-history should have its own entry, ordered before /price
        assert!(
            prices_history_idx < price_idx,
            "/prices-history (idx {prices_history_idx}) should come before /price (idx {price_idx})"
        );
    }

    #[tokio::test]
    async fn test_acquire_method_filtering() {
        let rl = polymarket::clob_limits();
        let start = std::time::Instant::now();
        // GET /order shouldn't match POST or DELETE /order endpoints — falls to default only
        rl.acquire("/order", Some(&Method::GET)).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_no_endpoint_match_uses_default_only() {
        let rl = polymarket::clob_limits();
        let start = std::time::Instant::now();
        rl.acquire("/unknown/path", None).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_acquire_method_none_matches_any_method() {
        let rl = polymarket::gamma_limits();
        let start = std::time::Instant::now();
        // /events has method: None — should match GET, POST, and None
        rl.acquire("/events", Some(&Method::GET)).await;
        rl.acquire("/events", Some(&Method::POST)).await;
        rl.acquire("/events", None).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    // ── Prefix collision tests ──────────────────────────────────

    #[test]
    fn test_clob_price_and_prices_history_are_distinct() {
        let rl = polymarket::clob_limits();
        let limits = rl.rows();

        let price = limits.iter().find(|l| l.pattern == "/price").unwrap();
        let prices_history = limits
            .iter()
            .find(|l| l.pattern == "/prices-history")
            .unwrap();

        // Both should use Prefix mode
        assert_eq!(price.matching, Matching::Prefix);
        assert_eq!(prices_history.matching, Matching::Prefix);

        // Verify "/prices-history" does NOT match the "/price" pattern
        if let Some(rest) = "/prices-history".strip_prefix(price.pattern) {
            assert!(
                !rest.is_empty() && !rest.starts_with('/') && !rest.starts_with('?'),
                "/prices-history must not match /price pattern, rest = '{rest}'"
            );
        }
    }

    #[test]
    fn test_data_positions_and_closed_positions_are_distinct() {
        // This previously asserted `!"/closed-positions".starts_with("/positions")`
        // — a tautology about two string literals that never touched the
        // limiter, and so held even with `/closed-positions` set to 66x its
        // published cap. Ask the limiter instead.
        let rl = polymarket::data_limits();

        let closed = rl.resolve_specs("/closed-positions", Some(&Method::GET));
        let positions = rl.resolve_specs("/positions", Some(&Method::GET));
        assert_eq!(closed, positions, "both are published at 150/10s");

        let bucket_for = |path: &str| {
            rl.effective_quota(&Method::GET, path)
                .get(1)
                .unwrap_or_else(|| panic!("{path} should match a rule"))
                .bucket
        };
        assert!(
            bucket_for("/closed-positions") != bucket_for("/positions"),
            "equal quotas must still be separate buckets — upstream publishes \
             150/10s each, not 150/10s combined"
        );
    }

    #[test]
    fn test_all_clob_endpoints_have_match_mode() {
        let rl = polymarket::clob_limits();
        for limit in &rl.rows() {
            // Every endpoint should have an explicit match mode
            assert!(
                limit.matching == Matching::Prefix || limit.matching == Matching::Exact,
                "endpoint {} has no valid match mode",
                limit.pattern
            );
        }
    }

    // ── Concurrent access tests ─────────────────────────────────

    #[tokio::test]
    async fn concurrent_acquires_are_paced_against_one_shared_allowance() {
        // Concurrency must not multiply the allowance. Ten tasks racing on one
        // limiter have to serialise into ten successive slots, not each take a
        // token of their own — the limiter's state is shared, and this is the
        // assertion that says so.
        //
        // /markets is locally capped at 1,500/10s, pacing at ~6.7ms, so ten
        // acquires occupy ~60ms. The floor is the real assertion; the ceiling
        // only catches a stall.
        const TASKS: u32 = 10;
        let interval = Duration::from_secs(10) / (1_500 - 1);

        let rl = std::sync::Arc::new(polymarket::clob_limits());

        let start = std::time::Instant::now();
        let mut handles = Vec::new();
        for _ in 0..TASKS {
            let rl = rl.clone();
            handles.push(tokio::spawn(async move {
                rl.acquire("/markets", None).await;
            }));
        }
        for handle in handles {
            handle.await.unwrap();
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed >= interval * (TASKS - 1) / 2,
            "{TASKS} concurrent acquires completed in {elapsed:?}; pacing at {interval:?} each \
             they cannot, so concurrent tasks are not sharing one allowance"
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "{TASKS} concurrent acquires took {elapsed:?} — they are stalling, not pacing"
        );
    }

    #[tokio::test]
    async fn test_acquire_concurrent_different_endpoints() {
        // Concurrent tasks hitting different endpoints should not block each other
        let rl = std::sync::Arc::new(polymarket::clob_limits());

        let rl1 = rl.clone();
        let rl2 = rl.clone();
        let rl3 = rl.clone();

        let start = std::time::Instant::now();
        let (r1, r2, r3) = tokio::join!(
            tokio::spawn(async move { rl1.acquire("/markets", None).await }),
            tokio::spawn(async move { rl2.acquire("/auth", None).await }),
            tokio::spawn(async move { rl3.acquire("/order", Some(&Method::POST)).await }),
        );
        r1.unwrap();
        r2.unwrap();
        r3.unwrap();

        assert!(
            start.elapsed() < Duration::from_millis(50),
            "different endpoints should not block: {:?}",
            start.elapsed()
        );
    }

    // ── Dual-window interaction tests ───────────────────────────

    #[test]
    fn test_clob_post_order_has_dual_window() {
        let rl = polymarket::clob_limits();
        let post_order = rl
            .rows()
            .into_iter()
            .find(|l| l.pattern == "/order" && l.method == Some(&Method::POST))
            .expect("POST /order endpoint should exist");

        assert_eq!(
            post_order.buckets.len(),
            2,
            "POST /order should have a burst and a sustained window"
        );
    }

    #[test]
    fn test_clob_delete_order_has_a_sustained_window_too() {
        // This previously asserted the *opposite* — that DELETE /order had only
        // a burst window — and so pinned the omission in place. Upstream
        // publishes 5,000/10s burst plus 120,000/10min sustained.
        let rl = polymarket::clob_limits();
        let delete_order = rl
            .rows()
            .into_iter()
            .find(|l| l.pattern == "/order" && l.method == Some(&Method::DELETE))
            .expect("DELETE /order endpoint should exist");

        assert_eq!(
            delete_order.buckets.len(),
            2,
            "DELETE /order should have both a burst and a sustained window"
        );
    }

    #[tokio::test]
    async fn test_dual_window_both_burst_and_sustained_are_awaited() {
        // POST /order should await both burst and sustained limiters.
        // With high limits, a single acquire should still complete fast.
        let rl = polymarket::clob_limits();
        let start = std::time::Instant::now();
        rl.acquire("/order", Some(&Method::POST)).await;
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "dual window single acquire should be fast: {:?}",
            start.elapsed()
        );
    }
}

mod cooldown_tests {
    //! A 429 is a fact about the host, not about the request that saw it.
    //!
    //! The token buckets above model the *published* quota, which is all a
    //! client can know in advance. When the server disagrees — Cloudflare's
    //! `error code: 1015` arrives as a 429 whatever our buckets believe — that
    //! correction has to reach every request sharing the limiter, or the
    //! siblings already in flight keep feeding a ban that is timed, and so gets
    //! longer the more it is hit.

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn acquire_is_immediate_without_a_cooldown() {
        let rl = polymarket::data_limits();
        let t = tokio::time::Instant::now();
        rl.acquire("/closed-positions", None).await;
        assert!(
            t.elapsed() < Duration::from_millis(1),
            "an untripped limiter must not delay: waited {:?}",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_holds_back_a_path_that_never_saw_the_429() {
        let rl = polymarket::data_limits();
        rl.begin_cooldown(Duration::from_secs(5));

        // /trades has its own bucket, full and untouched. It must wait anyway:
        // the block is on the IP, and every path shares it.
        let t = tokio::time::Instant::now();
        rl.acquire("/trades", None).await;
        assert!(
            t.elapsed() >= Duration::from_secs(5),
            "a sibling path resumed after {:?}, before the cooldown expired",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_requests_all_observe_one_cooldown() {
        let rl = polymarket::data_limits();
        rl.begin_cooldown(Duration::from_secs(3));

        // The shape from the report: several /closed-positions calls in flight
        // at once. One 429 has to stop all of them, not just its own caller.
        let t = tokio::time::Instant::now();
        tokio::join!(
            rl.acquire("/closed-positions", None),
            rl.acquire("/closed-positions", None),
            rl.acquire("/closed-positions", None),
            rl.acquire("/closed-positions", None),
        );
        assert!(
            t.elapsed() >= Duration::from_secs(3),
            "concurrent callers resumed after {:?}",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_shorter_cooldown_never_cuts_a_longer_one_short() {
        let rl = polymarket::data_limits();
        rl.begin_cooldown(Duration::from_secs(10));
        // A sibling's 429 lands next, carrying a smaller delay. Taking the
        // latest value would let the shortest response win the race and
        // release everyone early.
        rl.begin_cooldown(Duration::from_secs(1));

        let t = tokio::time::Instant::now();
        rl.acquire("/positions", None).await;
        assert!(
            t.elapsed() >= Duration::from_secs(10),
            "the longer cooldown was truncated to {:?}",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_extended_mid_wait_is_honoured_in_full() {
        let rl = polymarket::data_limits();
        rl.begin_cooldown(Duration::from_secs(2));

        let extender = {
            let rl = rl.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                rl.begin_cooldown(Duration::from_secs(5));
            })
        };

        let t = tokio::time::Instant::now();
        rl.acquire("/closed-positions", None).await;
        extender.await.unwrap();
        // Extended to 1s + 5s = 6s. Waking at the original 2s deadline and
        // returning would resume straight into the still-active ban.
        assert!(
            t.elapsed() >= Duration::from_secs(6),
            "resumed at {:?}, ignoring the cooldown extension",
            t.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_expired_cooldown_stops_delaying() {
        let rl = polymarket::data_limits();
        rl.begin_cooldown(Duration::from_secs(2));
        rl.acquire("/closed-positions", None).await;

        let t = tokio::time::Instant::now();
        rl.acquire("/closed-positions", None).await;
        assert!(
            t.elapsed() < Duration::from_millis(1),
            "the limiter stayed blocked for {:?} after the cooldown expired",
            t.elapsed()
        );
    }
}
