# Data API v2 Rate Limits (Phase 3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the provisional Data API v2 rows in `RateLimiter::data_default`, which were borrowed from v1, with limits measured against the live host and then validated at the shipped client's pace.

**Architecture:** A new example, `polyoxide-data/examples/v2_soak/`, sends raw `reqwest` requests so it can see `x-cache` and 429 bodies and can drive rates above any current row. Every probe URL is distinct, because CloudFront answers a repeated URL from its cache without reaching the origin. The harness has two modes. **Ramps** step one route through fixed rates, stop at the first throttle, and print the count to pin. **Validation** paces the same raw requests through `RateLimiter::data_default()`, per route or across all routes in one process, and requires zero 429s. The decision rules are pure, unit-tested functions.

**Tech Stack:** Rust example binary (`tokio`, `reqwest`, `serde_json`, and `polyoxide-core`'s `RateLimiter`); the existing `examples/common` helpers.

**Design:** Component 7 of [`2026-09-14-data-api-v2-design.md`](../specs/2026-09-14-data-api-v2-design.md), the hand-off section of [`2026-09-14-data-api-v2.md`](2026-09-14-data-api-v2.md), and the caching and throttling facts in [`docs/specs/data-v2/OBSERVED.md`](../../specs/data-v2/OBSERVED.md).

---

## What this plan decides, and what it cannot know in advance

Tasks 1–4 build the harness. They contain complete code, which was replayed against a fresh copy of this branch (every edit applied, every gate green, each mutation check caught) and smoke-tested live in all three modes before this plan was written.

Tasks 5–9 **measure**. Their numbers are outputs, so those tasks specify exactly what to run, what each possible outcome means, and the exact code shape to pin a measured number into. The rules turning a run into a number live in `verdict.rs` and are unit-tested in Task 3, not left to judgement:

| Rule | Value | Why |
|------|-------|-----|
| Ramp stages | 10, 15, 20, 30, 40 req/s | Starts below the provisional rows (13.4 and 17.9 req/s sustained) and stops at a ceiling the default client (4 concurrent requests) can actually reach |
| Stage length / cooldown | 60s / 120s | Chosen load budget: six Cloudflare 10s windows per stage |
| Stage verdict | throttled on any 429; invalid on a cache hit, a repeated URL, ≥1% errors, an early end or <90% of target; saturated when p99 > 3× the first stage's | Upstream queues heavy queries for a capacity slot before it answers 429, so latency climbs first |
| Pinned count | highest clean stage rate × 10 per 10s | `quota()` then reserves a tenth, so the client runs below a rate that was itself clean |
| Validation | 120s at the pinned pace, zero 429s; on failure lower the row by a quarter and repeat | A 60s clean stage does not prove 120s is sustainable |
| Mixed validation | all six routes, one process, one limiter | The only run that can detect a per-client allowance shared across routes |

**Load budget.** The upper bound if nothing throttles is about 41,000 ramp requests, 26,000 per-route validation requests and 10,700 mixed, over roughly 1.3 hours plus waits. Ramps stop at their first throttle, so expect much less.

## Conventions

- Run everything from the repository root.
- **Keep build output off `/tmp`.** On this machine it is a 16 GB RAM-backed tmpfs. A target directory there fills RAM and the filesystem; a previous session's workspace build failed that way with `No space left on device`. Leave `CARGO_TARGET_DIR` unset (so the build goes to `target/` on disk), or point it at disk.
- **Nothing else on this IP may call `data-api.polymarket.com` during Tasks 5, 7 and 8.** A Cloudflare `1015` blocks the whole host for the IP, and traffic during the block extends it.
- **Never repeat a ramp stage to "get a better number".** A ramp's result is the first time it ran. Re-runs are allowed only for the invalid outcomes Task 5 lists, and each one is recorded.
- Example unit tests do not run under a plain `cargo test`; each task names its `--example` explicitly.
- Commit messages end with the attribution trailer shown in each commit step.

## File map

| File | Responsibility | Task |
|------|----------------|------|
| `polyoxide-data/examples/common/mod.rs` | Gains the shared `Pacer` and its tests | 1 |
| `polyoxide-data/examples/closed_positions_soak.rs` | Uses `common::Pacer` | 1 |
| `polyoxide-data/examples/v2_soak/probes.rs` | `Route`, `parse_routes`, `ProbeSource` (distinct URLs), `SeenUrls` | 2 |
| `polyoxide-data/examples/v2_soak/verdict.rs` | `classify`, `judge`, `pin`: the measurement rules | 3 |
| `polyoxide-data/examples/v2_soak/main.rs` | CLI, pool bootstrap, stage runner, report | 2–4 |
| `docs/specs/data-v2/OBSERVED.md` | Ramp and validation runs, pinned table | 5, 7, 8 |
| `polyoxide-core/src/rate_limit.rs` | Measured v2 rows and their agreement test | 6–9 |
| `CLAUDE.md`, `docs/specs/data-v2/INDEX.md`, `docs/specs/data/rate-limits.md` | Documentation | 9 |

---

## Tasks

### Task 1: Share `Pacer` between the rate-limit harnesses

**Files:**
- Modify: `polyoxide-data/examples/closed_positions_soak.rs`
- Modify: `polyoxide-data/examples/common/mod.rs`

- [ ] **Step 1: Run the existing harness tests as a baseline**

Run:

```bash
cargo test -p polyoxide-data --example closed_positions_soak --example closed_positions_burst_probe
```

Expected: `closed_positions_soak`: `30 passed` (three of them `pacer_*`); `closed_positions_burst_probe`: `17 passed`.

- [ ] **Step 2: Move `Pacer` out of `closed_positions_soak.rs`**

`v2_soak` needs the same fixed-rate pacer. It moves into `common/mod.rs`, which both existing harnesses already include via `#[path]`, and becomes `pub`.

In `polyoxide-data/examples/closed_positions_soak.rs`, replace:

```rust
// ── Pacing ──────────────────────────────────────────────────────

/// Hands out send slots at a fixed interval, shared by every worker.
///
/// By default the soak lets the client's own limiter set the rate. Asking what
/// rate the *server* tolerates means driving a rate the client would not pick,
/// which means pacing outside it — and only downwards. At or above
/// [`CLIENT_SUSTAINED_RATE`] the client's limiter is the slower of the two and
/// binds first, so the run measures polyoxide instead of Cloudflare. That is
/// the same trap the burst probe avoids by using a fresh client per trial.
struct Pacer {
    interval: Duration,
    /// The earliest unclaimed slot; `None` until the first reservation.
    next: Mutex<Option<Instant>>,
}

impl Pacer {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            next: Mutex::new(None),
        }
    }

    /// Claim the next slot, given the current time.
    fn reserve(&self, now: Instant) -> Instant {
        let mut next = self
            .next
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let slot = next.map_or(now, |claimed| claimed.max(now));
        *next = Some(slot + self.interval);
        slot
    }

    async fn wait(&self) {
        let slot = self.reserve(Instant::now());
        tokio::time::sleep_until(tokio::time::Instant::from_std(slot)).await;
    }
}

```

with:

```rust

```

In `polyoxide-data/examples/closed_positions_soak.rs`, replace:

```rust
    // ── pacer ───────────────────────────────────────────────────

    const TEN_MS: Duration = Duration::from_millis(10);

    #[test]
    fn pacer_hands_out_the_first_slot_immediately() {
        let now = Instant::now();
        assert_eq!(Pacer::new(TEN_MS).reserve(now), now);
    }

    #[test]
    fn pacer_spaces_consecutive_slots_by_the_interval() {
        let pacer = Pacer::new(TEN_MS);
        let now = Instant::now();

        assert_eq!(pacer.reserve(now), now);
        assert_eq!(pacer.reserve(now), now + TEN_MS);
        assert_eq!(pacer.reserve(now), now + 2 * TEN_MS);
    }

    #[test]
    fn pacer_does_not_bank_credit_while_idle() {
        // The whole point of the harness is to hold a rate, and a pacer that
        // carries its cursor forward from an idle period releases the backlog
        // in one burst the moment traffic resumes — the same defect as a token
        // bucket with depth, which is what this run exists to measure the
        // absence of. A slot may never be in the past.
        let pacer = Pacer::new(TEN_MS);
        let start = Instant::now();
        pacer.reserve(start);

        let after_a_long_stall = start + Duration::from_secs(10);
        assert_eq!(
            pacer.reserve(after_a_long_stall),
            after_a_long_stall,
            "the pacer banked credit during the stall and would now burst"
        );
        assert_eq!(
            pacer.reserve(after_a_long_stall),
            after_a_long_stall + TEN_MS,
            "the cursor did not resume from the stall, so the backlog survives it"
        );
    }

```

with:

```rust

```

In `polyoxide-data/examples/closed_positions_soak.rs`, replace:

```rust
use common::{
    install_observer, percentile, ThrottleObserver, DEFAULT_CONCURRENCY, DEFAULT_USER,
    MAX_PAGE_LIMIT,
};
```

with:

```rust
use common::{
    install_observer, percentile, Pacer, ThrottleObserver, DEFAULT_CONCURRENCY, DEFAULT_USER,
    MAX_PAGE_LIMIT,
};
```

- [ ] **Step 3: Add it to `common/mod.rs`, with its tests**

In `polyoxide-data/examples/common/mod.rs`, replace:

```rust
//! Shared throttle detection for the live rate-limit harnesses.
//!
//! Included by both `closed_positions_soak.rs` and
//! `closed_positions_burst_probe.rs` via `#[path]`. Cargo only auto-discovers
```

with:

```rust
//! Shared throttle detection and pacing for the live rate-limit harnesses.
//!
//! Included by `closed_positions_soak.rs`, `closed_positions_burst_probe.rs` and
//! `v2_soak/main.rs` via `#[path]`. Cargo only auto-discovers
```

In `polyoxide-data/examples/common/mod.rs`, replace:

```rust
/// Nearest-rank percentile over an ascending slice.
```

with:

```rust
/// Hands out send slots at a fixed interval, shared by every worker.
///
/// Asking what rate the *server* tolerates means driving a rate the client
/// would not pick, which means pacing outside the client's own limiter. A
/// harness that paces through a polyoxide client can only go below that
/// client's sustained rate, or its limiter binds first and the run measures
/// polyoxide instead of the server; `v2_soak` sends raw requests for exactly
/// that reason.
pub struct Pacer {
    interval: Duration,
    /// The earliest unclaimed slot; `None` until the first reservation.
    next: Mutex<Option<Instant>>,
}

impl Pacer {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            next: Mutex::new(None),
        }
    }

    /// Claim the next slot, given the current time.
    pub fn reserve(&self, now: Instant) -> Instant {
        let mut next = self
            .next
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let slot = next.map_or(now, |claimed| claimed.max(now));
        *next = Some(slot + self.interval);
        slot
    }

    pub async fn wait(&self) {
        let slot = self.reserve(Instant::now());
        tokio::time::sleep_until(tokio::time::Instant::from_std(slot)).await;
    }
}

/// Nearest-rank percentile over an ascending slice.
```

In `polyoxide-data/examples/common/mod.rs`, replace:

```rust
    #[test]
    fn percentile_of_empty_is_zero() {
```

with:

```rust
    // ── pacer ───────────────────────────────────────────────────

    const TEN_MS: Duration = Duration::from_millis(10);

    #[test]
    fn pacer_hands_out_the_first_slot_immediately() {
        let now = Instant::now();
        assert_eq!(Pacer::new(TEN_MS).reserve(now), now);
    }

    #[test]
    fn pacer_spaces_consecutive_slots_by_the_interval() {
        let pacer = Pacer::new(TEN_MS);
        let now = Instant::now();

        assert_eq!(pacer.reserve(now), now);
        assert_eq!(pacer.reserve(now), now + TEN_MS);
        assert_eq!(pacer.reserve(now), now + 2 * TEN_MS);
    }

    #[test]
    fn pacer_does_not_bank_credit_while_idle() {
        // The whole point of the harness is to hold a rate, and a pacer that
        // carries its cursor forward from an idle period releases the backlog
        // in one burst the moment traffic resumes — the same defect as a token
        // bucket with depth, which is what this run exists to measure the
        // absence of. A slot may never be in the past.
        let pacer = Pacer::new(TEN_MS);
        let start = Instant::now();
        pacer.reserve(start);

        let after_a_long_stall = start + Duration::from_secs(10);
        assert_eq!(
            pacer.reserve(after_a_long_stall),
            after_a_long_stall,
            "the pacer banked credit during the stall and would now burst"
        );
        assert_eq!(
            pacer.reserve(after_a_long_stall),
            after_a_long_stall + TEN_MS,
            "the cursor did not resume from the stall, so the backlog survives it"
        );
    }

    #[test]
    fn percentile_of_empty_is_zero() {
```

- [ ] **Step 4: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-data --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-data --example closed_positions_soak --example closed_positions_burst_probe
```

Expected: `closed_positions_soak`: `30 passed`; `closed_positions_burst_probe`: `20 passed`. The three `pacer_*` tests now run from `common::tests`, in both.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-data/examples/closed_positions_soak.rs polyoxide-data/examples/common/mod.rs
git commit -F - <<'EOF'
refactor(data): share the soak harness Pacer via examples/common

v2_soak needs the same fixed-rate pacer as closed_positions_soak. It moves
into examples/common/mod.rs, which both existing harnesses already include,
with its three tests. No behaviour change.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 2: `v2_soak` probe space: distinct URLs per route

**Files:**
- Create: `polyoxide-data/examples/v2_soak/probes.rs`
- Create: `polyoxide-data/examples/v2_soak/main.rs` (placeholder entry point)

- [ ] **Step 1: Create the probe module and a placeholder entry point**

Cargo discovers `examples/v2_soak/main.rs` as the `v2_soak` example, with `probes.rs` as a module beside it. Each route's URLs are a mixed-radix space over live wallets (or markets) and harmless parameter variations, so distinct indices are distinct URLs by construction. The ranges were checked against the live host: `status` takes `OPEN`/`REDEEMABLE`/`CLOSED`, `sort_direction` `ASC` works on activity and combo positions, and `include_pnl=true` caps `limit` at 100.

Create `polyoxide-data/examples/v2_soak/probes.rs`:

```rust
//! The requests a soak sends: one route, and a space of URLs that never
//! repeats within a run.
//!
//! Several v2 routes are cached at CloudFront (`OBSERVED.md`): a repeated URL
//! is answered from the cache, with the same trace id, without reaching the
//! origin. A soak that repeats URLs measures the cache and reports a clean run
//! at any rate. So every probe URL is a distinct point in a mixed-radix space
//! over live wallets or markets and harmless parameter variations, and
//! [`SeenUrls`] refuses a repeat as a backstop.

use std::{
    collections::HashSet,
    fmt,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

/// A v2 route under measurement. Each maps to the path its rate-limit row
/// matches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Positions,
    ComboPositions,
    Trades,
    Activity,
    UserPnl,
    HoldersPnl,
}

impl Route {
    pub const ALL: [Route; 6] = [
        Route::Positions,
        Route::ComboPositions,
        Route::Trades,
        Route::Activity,
        Route::UserPnl,
        Route::HoldersPnl,
    ];

    /// The request path, which is also what `RateLimiter::acquire` matches.
    pub fn path(self) -> &'static str {
        match self {
            Route::Positions => "/v2/positions",
            Route::ComboPositions => "/v2/positions/combos",
            Route::Trades => "/v2/trades",
            Route::Activity => "/v2/activity",
            Route::UserPnl => "/v2/user-pnl",
            Route::HoldersPnl => "/v2/holders",
        }
    }

    /// The name used on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Route::Positions => "positions",
            Route::ComboPositions => "combo-positions",
            Route::Trades => "trades",
            Route::Activity => "activity",
            Route::UserPnl => "user-pnl",
            Route::HoldersPnl => "holders-pnl",
        }
    }

    /// Whether the route is keyed by market rather than by wallet.
    pub fn needs_conditions(self) -> bool {
        matches!(self, Route::HoldersPnl)
    }
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Route {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Route::ALL
            .into_iter()
            .find(|r| r.name() == s)
            .ok_or_else(|| {
                let names: Vec<_> = Route::ALL.iter().map(|r| r.name()).collect();
                format!("unknown route {s:?}; expected one of {}", names.join(", "))
            })
    }
}

/// Parses `all`, or one or more comma-separated route names.
pub fn parse_routes(raw: &str) -> Result<Vec<Route>, String> {
    if raw == "all" {
        return Ok(Route::ALL.to_vec());
    }
    let routes = raw
        .split(',')
        .map(|name| name.trim().parse::<Route>())
        .collect::<Result<Vec<_>, _>>()?;
    let mut seen = HashSet::new();
    if let Some(repeat) = routes.iter().find(|route| !seen.insert(**route)) {
        return Err(format!("{repeat} is listed twice"));
    }
    Ok(routes)
}

/// Live identifiers the probes are built from.
#[derive(Debug, Clone, Default)]
pub struct Pools {
    pub wallets: Vec<String>,
    pub conditions: Vec<String>,
}

const STATUSES: [&str; 3] = ["OPEN", "REDEEMABLE", "CLOSED"];
const DIRECTIONS: [&str; 2] = ["DESC", "ASC"];
const TAKER_ONLY: [&str; 2] = ["true", "false"];
const PNL_INTERVALS: [&str; 7] = ["max", "all", "1m", "1w", "1d", "12h", "6h"];
const PNL_FIDELITIES: [&str; 5] = ["1d", "18h", "12h", "3h", "1h"];

/// Page sizes varied for uniqueness. A narrow band keeps each request's
/// weight roughly constant across the run.
const LIMIT_BAND: std::ops::RangeInclusive<u64> = 50..=100;
/// `include_pnl=true` caps `limit` at 100 upstream.
const HOLDERS_LIMIT_BAND: std::ops::RangeInclusive<u64> = 1..=100;

fn band_len(band: &std::ops::RangeInclusive<u64>) -> u64 {
    band.end() - band.start() + 1
}

/// Splits `index` into digits of the given radices, least significant first.
fn mixed_radix(mut index: u64, radices: &[u64]) -> Vec<u64> {
    radices
        .iter()
        .map(|radix| {
            let digit = index % radix;
            index /= radix;
            digit
        })
        .collect()
}

/// A route's probe space: distinct URLs addressed by an index.
pub struct ProbeSource {
    route: Route,
    pools: Pools,
    offset: u64,
    next: AtomicU64,
}

impl ProbeSource {
    /// `offset` shifts where in the space the run starts, so two runs a few
    /// minutes apart do not replay the same URLs into a warm cache.
    pub fn new(route: Route, pools: Pools, offset: u64) -> Result<Self, String> {
        let keys = if route.needs_conditions() {
            pools.conditions.len()
        } else {
            pools.wallets.len()
        };
        if keys == 0 {
            return Err(format!(
                "{route} needs at least one {}",
                if route.needs_conditions() {
                    "condition"
                } else {
                    "wallet"
                }
            ));
        }
        let capacity = Self::radices_for(route, keys).iter().product::<u64>();
        Ok(Self {
            route,
            pools,
            offset: offset % capacity,
            next: AtomicU64::new(0),
        })
    }

    fn radices_for(route: Route, keys: usize) -> Vec<u64> {
        let keys = keys as u64;
        let limits = band_len(&LIMIT_BAND);
        match route {
            Route::Positions => vec![keys, STATUSES.len() as u64, limits],
            Route::ComboPositions => vec![keys, DIRECTIONS.len() as u64, limits],
            Route::Trades => vec![keys, TAKER_ONLY.len() as u64, limits],
            Route::Activity => vec![keys, DIRECTIONS.len() as u64, limits],
            Route::UserPnl => vec![
                keys,
                PNL_INTERVALS.len() as u64,
                PNL_FIDELITIES.len() as u64,
            ],
            Route::HoldersPnl => vec![keys, band_len(&HOLDERS_LIMIT_BAND)],
        }
    }

    fn radices(&self) -> Vec<u64> {
        let keys = if self.route.needs_conditions() {
            self.pools.conditions.len()
        } else {
            self.pools.wallets.len()
        };
        Self::radices_for(self.route, keys)
    }

    /// How many distinct URLs the space holds.
    pub fn capacity(&self) -> u64 {
        self.radices().iter().product()
    }

    /// The path and query for position `index` in the space. A bijection on
    /// `0..capacity()`, so distinct indices give distinct URLs.
    pub fn url(&self, index: u64) -> String {
        let d = mixed_radix((index + self.offset) % self.capacity(), &self.radices());
        let limit = |band: &std::ops::RangeInclusive<u64>, digit: u64| band.start() + digit;
        let path = self.route.path();
        match self.route {
            Route::Positions => format!(
                "{path}?user={}&status={}&limit={}",
                self.pools.wallets[d[0] as usize],
                STATUSES[d[1] as usize],
                limit(&LIMIT_BAND, d[2])
            ),
            Route::ComboPositions => format!(
                "{path}?user={}&sort_direction={}&limit={}",
                self.pools.wallets[d[0] as usize],
                DIRECTIONS[d[1] as usize],
                limit(&LIMIT_BAND, d[2])
            ),
            Route::Trades => format!(
                "{path}?user={}&taker_only={}&limit={}",
                self.pools.wallets[d[0] as usize],
                TAKER_ONLY[d[1] as usize],
                limit(&LIMIT_BAND, d[2])
            ),
            Route::Activity => format!(
                "{path}?user={}&sort_direction={}&limit={}",
                self.pools.wallets[d[0] as usize],
                DIRECTIONS[d[1] as usize],
                limit(&LIMIT_BAND, d[2])
            ),
            Route::UserPnl => format!(
                "{path}?user={}&interval={}&fidelity={}",
                self.pools.wallets[d[0] as usize],
                PNL_INTERVALS[d[1] as usize],
                PNL_FIDELITIES[d[2] as usize]
            ),
            Route::HoldersPnl => format!(
                "{path}?condition={}&include_pnl=true&limit={}",
                self.pools.conditions[d[0] as usize],
                limit(&HOLDERS_LIMIT_BAND, d[1])
            ),
        }
    }

    /// The next unused URL, or `None` once the space is exhausted.
    pub fn next_url(&self) -> Option<String> {
        let index = self.next.fetch_add(1, Ordering::Relaxed);
        (index < self.capacity()).then(|| self.url(index))
    }
}

/// Every URL a run has sent. A repeat would be answered from the CDN cache.
#[derive(Default)]
pub struct SeenUrls(Mutex<HashSet<String>>);

impl SeenUrls {
    /// Records `url`; `false` if it was already sent.
    pub fn insert(&self, url: &str) -> bool {
        let mut seen = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        seen.insert(url.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pools(wallets: usize, conditions: usize) -> Pools {
        Pools {
            wallets: (0..wallets).map(|i| format!("0xw{i}")).collect(),
            conditions: (0..conditions).map(|i| format!("0xc{i}")).collect(),
        }
    }

    #[test]
    fn every_route_round_trips_through_its_name() {
        for route in Route::ALL {
            assert_eq!(route.name().parse::<Route>(), Ok(route));
        }
        assert!("closed-positions".parse::<Route>().is_err());
    }

    #[test]
    fn routes_parse_from_a_list_or_all() {
        assert_eq!(parse_routes("all"), Ok(Route::ALL.to_vec()));
        assert_eq!(
            parse_routes("trades, user-pnl"),
            Ok(vec![Route::Trades, Route::UserPnl])
        );
        assert!(parse_routes("trades,trades").is_err());
        assert!(parse_routes("trades,nope").is_err());
    }

    #[test]
    fn mixed_radix_digits_are_least_significant_first() {
        assert_eq!(mixed_radix(0, &[3, 2]), vec![0, 0]);
        assert_eq!(mixed_radix(1, &[3, 2]), vec![1, 0]);
        assert_eq!(mixed_radix(3, &[3, 2]), vec![0, 1]);
        assert_eq!(mixed_radix(5, &[3, 2]), vec![2, 1]);
    }

    #[test]
    fn every_url_in_a_space_is_distinct() {
        // The property the whole soak rests on: a repeat reaches the cache.
        for route in Route::ALL {
            let source = ProbeSource::new(route, pools(4, 3), 0).unwrap();
            let urls: HashSet<String> = (0..source.capacity()).map(|i| source.url(i)).collect();
            assert_eq!(
                urls.len() as u64,
                source.capacity(),
                "{route} repeated a URL"
            );
        }
    }

    #[test]
    fn urls_target_the_route_path_and_respect_its_limit_band() {
        let source = ProbeSource::new(Route::HoldersPnl, pools(0, 2), 0).unwrap();
        for i in 0..source.capacity() {
            let url = source.url(i);
            assert!(url.starts_with("/v2/holders?condition=0xc"), "{url}");
            let limit: u64 = url.rsplit("limit=").next().unwrap().parse().unwrap();
            assert!(
                (1..=100).contains(&limit),
                "include_pnl caps limit at 100: {url}"
            );
        }
        let positions = ProbeSource::new(Route::Positions, pools(1, 0), 0).unwrap();
        assert_eq!(
            positions.url(0),
            "/v2/positions?user=0xw0&status=OPEN&limit=50"
        );
    }

    #[test]
    fn the_offset_rotates_the_start_without_leaving_the_space() {
        let plain = ProbeSource::new(Route::UserPnl, pools(2, 0), 0).unwrap();
        let shifted = ProbeSource::new(Route::UserPnl, pools(2, 0), 5).unwrap();
        assert_eq!(shifted.url(0), plain.url(5));
        let wrapped = ProbeSource::new(Route::UserPnl, pools(2, 0), plain.capacity() + 1).unwrap();
        assert_eq!(wrapped.url(0), plain.url(1));
    }

    #[test]
    fn next_url_stops_at_capacity() {
        let source = ProbeSource::new(Route::HoldersPnl, pools(0, 1), 0).unwrap();
        let drawn = std::iter::from_fn(|| source.next_url()).count() as u64;
        assert_eq!(drawn, source.capacity());
        assert_eq!(source.next_url(), None);
    }

    #[test]
    fn an_empty_pool_is_refused() {
        assert!(ProbeSource::new(Route::Positions, pools(0, 5), 0).is_err());
        assert!(ProbeSource::new(Route::HoldersPnl, pools(5, 0), 0).is_err());
    }

    #[test]
    fn seen_urls_refuses_a_repeat() {
        let seen = SeenUrls::default();
        assert!(seen.insert("/v2/trades?user=a"));
        assert!(!seen.insert("/v2/trades?user=a"));
        assert!(seen.insert("/v2/trades?user=b"));
    }
}
```

Create `polyoxide-data/examples/v2_soak/main.rs`:

```rust
//! Measures, then validates, the rate limits for Data API v2 routes.
//!
//! Built up across the Phase 3 plan; the modules land before the entry point.

#[allow(dead_code)] // Used by the entry point, which lands in a later task.
mod probes;

fn main() {}
```

- [ ] **Step 2: Run the tests**

Run:

```bash
cargo test -p polyoxide-data --example v2_soak
```

Expected: `test result: ok. 9 passed`.

- [ ] **Step 3: Prove the uniqueness test can fail**

Reuse the status digit for `limit` on positions, which makes URLs collide, then revert:

Run:

```bash
sed -i '0,/limit(&LIMIT_BAND, d\[2\])/s//limit(\&LIMIT_BAND, d[1])/' polyoxide-data/examples/v2_soak/probes.rs
cargo test -p polyoxide-data --example v2_soak 2>&1 | grep 'repeated a URL'
sed -i 's/limit(&LIMIT_BAND, d\[1\])/limit(\&LIMIT_BAND, d[2])/' polyoxide-data/examples/v2_soak/probes.rs
```

Expected: a panic line containing `positions repeated a URL`.

- [ ] **Step 4: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-data --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-data --example v2_soak
```

Expected: `9 passed`.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-data/examples/v2_soak/main.rs polyoxide-data/examples/v2_soak/probes.rs
git commit -F - <<'EOF'
test(data): v2_soak probe space of distinct URLs per route

Several v2 routes are CloudFront-cached, and a repeated URL is answered
from the cache without reaching the origin, so a soak that repeats URLs
reports a clean run at any rate. Each route's probe URLs are a mixed-radix
space over live wallets or markets and harmless parameters, and a run
refuses any repeat.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 3: `v2_soak` verdicts: responses, stages and the count to pin

**Files:**
- Create: `polyoxide-data/examples/v2_soak/verdict.rs`
- Modify: `polyoxide-data/examples/v2_soak/main.rs` (placeholder entry point)

- [ ] **Step 1: Create the verdict module**

Everything that decides a pinned number is a pure function here, so the decision rules are tested rather than read off a terminal:

- **A 429's layer.** A JSON body with `code: rate_limited` is the origin; `error code: 1015` is Cloudflare's host-wide IP block.
- **A cache hit** (`x-cache: Hit…`) invalidates a stage, because the origin never saw that request.
- **A stage is invalid** if it hit the cache, repeated a URL, ran out of probes, had ≥1% errors, ended early, or drove less than 90% of its target rate.
- **A stage is saturated** if its p99 exceeds 3× the first stage's. Upstream queues heavy queries for a capacity slot before it answers 429, so latency climbs first.
- **The count to pin** is the highest clean stage rate below the first throttled or saturated stage, ×10 per 10 seconds. `quota()` then reserves a tenth of it.

Create `polyoxide-data/examples/v2_soak/verdict.rs`:

```rust
//! What a response, a stage and a whole ramp mean. Pure functions, so the
//! rules that decide a pinned rate limit are unit-tested rather than read off
//! a terminal.

use std::time::Duration;

use crate::common::percentile;

/// Where a 429 came from. The two layers have different windows and different
/// consequences: Cloudflare's `error code: 1015` blocks the whole host for this
/// IP, and traffic during the block prolongs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// The v2 origin's per-client allowance: a JSON body with `code: rate_limited`.
    Origin,
    /// Cloudflare's IP rule: a plain-text `error code: 1015` body.
    Cloudflare,
    /// A 429 in neither shape.
    Unknown,
}

/// One response, as the soak sees it.
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Ok,
    /// Served by CloudFront from cache; the origin never saw the request.
    CacheHit,
    Throttled {
        layer: Layer,
        retry_after: Option<Duration>,
    },
    Error(u16),
}

pub fn classify(
    status: u16,
    x_cache: Option<&str>,
    retry_after: Option<&str>,
    body: &str,
) -> Reply {
    if status == 429 {
        let code = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("code").and_then(|c| c.as_str()).map(str::to_owned));
        let layer = match code.as_deref() {
            Some("rate_limited") => Layer::Origin,
            _ if body.contains("1015") => Layer::Cloudflare,
            _ => Layer::Unknown,
        };
        let retry_after = retry_after
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|secs| secs.is_finite() && *secs >= 0.0)
            .map(Duration::from_secs_f64);
        return Reply::Throttled { layer, retry_after };
    }
    if x_cache.is_some_and(|v| v.to_ascii_lowercase().starts_with("hit")) {
        return Reply::CacheHit;
    }
    if (200..300).contains(&status) {
        Reply::Ok
    } else {
        Reply::Error(status)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// The route's path, so a mixed run can say which route was throttled.
    pub path: &'static str,
    /// From stage start to this response completing.
    pub finished_at: Duration,
    pub latency: Duration,
    pub reply: Reply,
}

/// Why a stage stopped before its planned duration, other than a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abort {
    DuplicateUrl,
    ProbeSpaceExhausted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stage {
    /// Requests per second driven by the harness; `None` when the shipped
    /// limiter set the pace.
    pub target_rps: Option<f64>,
    pub planned: Duration,
    pub elapsed: Duration,
    pub samples: Vec<Sample>,
    pub abort: Option<Abort>,
}

impl Stage {
    /// Requests per second over the planned duration. Every request sent
    /// before the deadline yields one sample, but the last ones complete after
    /// it, so dividing by the elapsed time would understate the rate the
    /// harness actually drove.
    pub fn achieved_rps(&self) -> f64 {
        if self.planned.is_zero() {
            0.0
        } else {
            self.samples.len() as f64 / self.planned.as_secs_f64()
        }
    }

    pub fn latencies(&self) -> Vec<Duration> {
        let mut sorted: Vec<Duration> = self.samples.iter().map(|s| s.latency).collect();
        sorted.sort_unstable();
        sorted
    }

    pub fn p50(&self) -> Duration {
        percentile(&self.latencies(), 50.0)
    }

    pub fn p99(&self) -> Duration {
        percentile(&self.latencies(), 99.0)
    }

    pub fn count(&self, pred: impl Fn(&Reply) -> bool) -> usize {
        self.samples.iter().filter(|s| pred(&s.reply)).count()
    }
}

/// A stage whose p99 latency exceeds this multiple of the first stage's is
/// treated as saturated: upstream queues heavy queries for a capacity slot
/// before it answers 429, so latency climbs first.
pub const SATURATION_FACTOR: u32 = 3;

/// A stage that drove less than this share of its target rate did not measure
/// that rate: the harness, not the server, was the bottleneck.
pub const MIN_ACHIEVED_SHARE: f64 = 0.9;

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Clean,
    Throttled {
        path: &'static str,
        layer: Layer,
        retry_after: Option<Duration>,
        at: Duration,
    },
    Saturated {
        p99: Duration,
        baseline_p99: Duration,
    },
    Invalid(String),
}

/// Judges one stage. `baseline_p99` is the first stage's p99, or `None` for
/// the first stage itself and for a validation run.
pub fn judge(stage: &Stage, baseline_p99: Option<Duration>) -> Verdict {
    if let Some(sample) = stage
        .samples
        .iter()
        .find(|s| matches!(s.reply, Reply::Throttled { .. }))
    {
        if let Reply::Throttled { layer, retry_after } = sample.reply {
            return Verdict::Throttled {
                path: sample.path,
                layer,
                retry_after,
                at: sample.finished_at,
            };
        }
    }
    match stage.abort {
        Some(Abort::DuplicateUrl) => {
            return Verdict::Invalid(
                "a probe URL repeated, so the CDN cache could answer it".into(),
            )
        }
        Some(Abort::ProbeSpaceExhausted) => {
            return Verdict::Invalid(
                "the probe space ran out; bootstrap more wallets or conditions".into(),
            )
        }
        None => {}
    }
    let hits = stage.count(|r| *r == Reply::CacheHit);
    if hits > 0 {
        return Verdict::Invalid(format!("{hits} responses came from the CDN cache"));
    }
    let total = stage.samples.len();
    if total == 0 {
        return Verdict::Invalid("no responses".into());
    }
    let errors = stage.count(|r| matches!(r, Reply::Error(_)));
    if errors * 100 >= total {
        let mut statuses: Vec<u16> = stage
            .samples
            .iter()
            .filter_map(|s| match s.reply {
                Reply::Error(status) => Some(status),
                _ => None,
            })
            .collect();
        statuses.sort_unstable();
        statuses.dedup();
        let statuses: Vec<String> = statuses
            .iter()
            .map(|status| match status {
                0 => "transport".to_owned(),
                status => status.to_string(),
            })
            .collect();
        return Verdict::Invalid(format!(
            "{errors} of {total} requests failed (status {})",
            statuses.join(", ")
        ));
    }
    if stage.elapsed.as_secs_f64() < stage.planned.as_secs_f64() * 0.95 {
        return Verdict::Invalid("the stage ended before its planned duration".into());
    }
    if let Some(target) = stage.target_rps {
        let achieved = stage.achieved_rps();
        if achieved < target * MIN_ACHIEVED_SHARE {
            return Verdict::Invalid(format!(
                "achieved {achieved:.2} of {target} req/s; raise --concurrency"
            ));
        }
    }
    if let Some(baseline_p99) = baseline_p99 {
        let p99 = stage.p99();
        if p99 > baseline_p99 * SATURATION_FACTOR {
            return Verdict::Saturated { p99, baseline_p99 };
        }
    }
    Verdict::Clean
}

/// What a finished ramp says to pin.
#[derive(Debug, Clone, PartialEq)]
pub enum Pin {
    /// Requests per 10 seconds for the route's `simple_limit` row.
    Count(u32),
    /// Not even the lowest stage was clean: re-run with lower stages.
    RetryLower,
    Invalid(String),
}

/// Pins the highest clean rate below the first stage that was throttled or
/// saturated, as a count per 10 seconds. `quota()` then reserves a tenth of
/// that count, so the client runs below a rate that was itself clean. Stages
/// must be ascending; the ramp stops at the first non-clean stage.
pub fn pin(stages: &[(f64, Verdict)]) -> Pin {
    let mut clean = None;
    for (rate, verdict) in stages {
        match verdict {
            Verdict::Clean => clean = Some(*rate),
            Verdict::Throttled { .. } | Verdict::Saturated { .. } => break,
            Verdict::Invalid(why) => return Pin::Invalid(format!("stage at {rate} req/s: {why}")),
        }
    }
    match clean {
        Some(rate) => Pin::Count((rate * 10.0).floor() as u32),
        None if stages.is_empty() => Pin::Invalid("no stages ran".into()),
        None => Pin::RetryLower,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN_429: &str =
        r#"{"error":"slow down","code":"rate_limited","retryable":true,"trace_id":"t-429"}"#;

    fn sample(at_ms: u64, latency_ms: u64, reply: Reply) -> Sample {
        Sample {
            path: "/v2/trades",
            finished_at: Duration::from_millis(at_ms),
            latency: Duration::from_millis(latency_ms),
            reply,
        }
    }

    fn stage(target: Option<f64>, secs: u64, samples: Vec<Sample>) -> Stage {
        Stage {
            target_rps: target,
            planned: Duration::from_secs(secs),
            elapsed: Duration::from_secs(secs),
            samples,
            abort: None,
        }
    }

    fn steady(n: u64, latency_ms: u64) -> Vec<Sample> {
        (0..n)
            .map(|i| sample(i * 100, latency_ms, Reply::Ok))
            .collect()
    }

    // ── classify ────────────────────────────────────────────────

    #[test]
    fn a_json_rate_limited_body_is_the_origin() {
        assert_eq!(
            classify(429, None, Some("7"), ORIGIN_429),
            Reply::Throttled {
                layer: Layer::Origin,
                retry_after: Some(Duration::from_secs(7))
            }
        );
    }

    #[test]
    fn a_1015_page_is_cloudflare_even_with_retry_after_zero() {
        assert_eq!(
            classify(429, None, Some("0"), "error code: 1015"),
            Reply::Throttled {
                layer: Layer::Cloudflare,
                retry_after: Some(Duration::ZERO)
            }
        );
    }

    #[test]
    fn an_unrecognised_429_is_still_a_throttle() {
        assert!(matches!(
            classify(429, None, None, "busy"),
            Reply::Throttled {
                layer: Layer::Unknown,
                retry_after: None
            }
        ));
    }

    #[test]
    fn a_cache_hit_is_not_a_success() {
        assert_eq!(
            classify(200, Some("Hit from cloudfront"), None, "{}"),
            Reply::CacheHit
        );
        assert_eq!(
            classify(200, Some("Miss from cloudfront"), None, "{}"),
            Reply::Ok
        );
        assert_eq!(classify(200, None, None, "{}"), Reply::Ok);
        assert_eq!(
            classify(503, Some("Miss from cloudfront"), None, "{}"),
            Reply::Error(503)
        );
    }

    // ── judge ───────────────────────────────────────────────────

    #[test]
    fn a_steady_stage_at_its_target_is_clean() {
        assert_eq!(
            judge(&stage(Some(10.0), 10, steady(100, 80)), None),
            Verdict::Clean
        );
    }

    #[test]
    fn the_first_429_decides_the_stage() {
        let mut samples = steady(50, 80);
        samples.push(sample(
            5_100,
            80,
            classify(429, None, Some("3"), ORIGIN_429),
        ));
        samples.push(sample(
            5_200,
            80,
            classify(429, None, None, "error code: 1015"),
        ));
        assert_eq!(
            judge(&stage(Some(10.0), 10, samples), None),
            Verdict::Throttled {
                path: "/v2/trades",
                layer: Layer::Origin,
                retry_after: Some(Duration::from_secs(3)),
                at: Duration::from_millis(5_100)
            }
        );
    }

    #[test]
    fn any_cache_hit_invalidates_the_stage() {
        let mut samples = steady(100, 80);
        samples[40].reply = Reply::CacheHit;
        assert!(matches!(
            judge(&stage(Some(10.0), 10, samples), None),
            Verdict::Invalid(_)
        ));
    }

    #[test]
    fn a_duplicate_url_invalidates_the_stage() {
        let mut s = stage(Some(10.0), 10, steady(100, 80));
        s.abort = Some(Abort::DuplicateUrl);
        assert!(matches!(judge(&s, None), Verdict::Invalid(_)));
    }

    #[test]
    fn one_percent_errors_invalidates_the_stage_and_less_does_not() {
        let mut samples = steady(100, 80);
        samples[0].reply = Reply::Error(400);
        assert!(matches!(
            judge(&stage(Some(10.0), 10, samples.clone()), None),
            Verdict::Invalid(_)
        ));
        samples.extend(steady(100, 80));
        assert_eq!(judge(&stage(Some(10.0), 20, samples), None), Verdict::Clean);
    }

    #[test]
    fn a_failed_stage_names_the_statuses_it_saw() {
        let mut samples = steady(10, 80);
        samples[1].reply = Reply::Error(400);
        samples[2].reply = Reply::Error(0);
        samples[3].reply = Reply::Error(400);
        assert_eq!(
            judge(&stage(Some(1.0), 10, samples), None),
            Verdict::Invalid("3 of 10 requests failed (status transport, 400)".into())
        );
    }

    #[test]
    fn a_harness_that_cannot_reach_its_target_measured_nothing() {
        // 80 requests in 10s is 8 req/s against a 10 req/s target.
        assert!(matches!(
            judge(&stage(Some(10.0), 10, steady(80, 80)), None),
            Verdict::Invalid(_)
        ));
        assert_eq!(
            judge(&stage(Some(10.0), 10, steady(90, 80)), None),
            Verdict::Clean
        );
    }

    #[test]
    fn requests_finishing_after_the_deadline_do_not_dilute_the_rate() {
        // Found by the first live smoke run: 8 requests at 1 req/s over an 8s
        // stage, with the last response landing at 8.9s, read as 0.89 req/s.
        let mut s = stage(Some(1.0), 8, steady(8, 250));
        s.elapsed = Duration::from_millis(8_900);
        assert_eq!(judge(&s, None), Verdict::Clean);
    }

    #[test]
    fn a_validation_stage_has_no_target_to_reach() {
        assert_eq!(
            judge(&stage(None, 10, steady(30, 80)), None),
            Verdict::Clean
        );
    }

    #[test]
    fn latency_tripling_over_the_baseline_is_saturation() {
        let baseline = Some(Duration::from_millis(100));
        assert_eq!(
            judge(&stage(Some(10.0), 10, steady(100, 300)), baseline),
            Verdict::Clean
        );
        assert_eq!(
            judge(&stage(Some(10.0), 10, steady(100, 301)), baseline),
            Verdict::Saturated {
                p99: Duration::from_millis(301),
                baseline_p99: Duration::from_millis(100)
            }
        );
    }

    #[test]
    fn a_stage_cut_short_is_invalid() {
        let mut s = stage(Some(10.0), 10, steady(100, 80));
        s.elapsed = Duration::from_secs(5);
        assert!(matches!(judge(&s, None), Verdict::Invalid(_)));
    }

    // ── pin ─────────────────────────────────────────────────────

    fn throttled() -> Verdict {
        Verdict::Throttled {
            path: "/v2/trades",
            layer: Layer::Origin,
            retry_after: None,
            at: Duration::ZERO,
        }
    }

    #[test]
    fn pins_the_highest_clean_stage_below_the_first_throttle() {
        let ramp = [
            (10.0, Verdict::Clean),
            (15.0, Verdict::Clean),
            (20.0, throttled()),
        ];
        assert_eq!(pin(&ramp), Pin::Count(150));
    }

    #[test]
    fn saturation_stops_the_ramp_like_a_throttle() {
        let saturated = Verdict::Saturated {
            p99: Duration::from_secs(4),
            baseline_p99: Duration::from_secs(1),
        };
        let ramp = [(10.0, Verdict::Clean), (15.0, saturated)];
        assert_eq!(pin(&ramp), Pin::Count(100));
    }

    #[test]
    fn a_ramp_clean_to_the_top_pins_the_top() {
        let ramp = [(10.0, Verdict::Clean), (40.0, Verdict::Clean)];
        assert_eq!(pin(&ramp), Pin::Count(400));
    }

    #[test]
    fn fractional_rates_round_down() {
        assert_eq!(pin(&[(13.5, Verdict::Clean)]), Pin::Count(135));
        assert_eq!(pin(&[(2.55, Verdict::Clean)]), Pin::Count(25));
    }

    #[test]
    fn a_throttled_first_stage_asks_for_lower_stages() {
        assert_eq!(pin(&[(10.0, throttled())]), Pin::RetryLower);
    }

    #[test]
    fn an_invalid_stage_invalidates_the_ramp() {
        let ramp = [
            (10.0, Verdict::Clean),
            (15.0, Verdict::Invalid("cache".into())),
        ];
        assert!(matches!(pin(&ramp), Pin::Invalid(_)));
        assert!(matches!(pin(&[]), Pin::Invalid(_)));
    }
}
```

Replace the entire contents of `polyoxide-data/examples/v2_soak/main.rs`:

```rust
//! Measures, then validates, the rate limits for Data API v2 routes.
//!
//! Built up across the Phase 3 plan; the modules land before the entry point.

#[path = "../common/mod.rs"]
mod common;
#[allow(dead_code)] // Used by the entry point, which lands in the next task.
mod probes;
#[allow(dead_code)] // Used by the entry point, which lands in the next task.
mod verdict;

fn main() {}
```

- [ ] **Step 2: Run the tests**

Run:

```bash
cargo test -p polyoxide-data --example v2_soak
```

Expected: `test result: ok. 40 passed` (9 probes, 21 verdict, 10 common).

- [ ] **Step 3: Prove the cache-hit rule can fail**

Run:

```bash
sed -i 's/    if hits > 0 {/    if hits > 1_000_000 {/' polyoxide-data/examples/v2_soak/verdict.rs
cargo test -p polyoxide-data --example v2_soak 2>&1 | grep -E '^test .* FAILED'
sed -i 's/    if hits > 1_000_000 {/    if hits > 0 {/' polyoxide-data/examples/v2_soak/verdict.rs
```

Expected: `test verdict::tests::any_cache_hit_invalidates_the_stage ... FAILED`.

- [ ] **Step 4: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-data --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-data --example v2_soak
```

Expected: `40 passed`.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-data/examples/v2_soak/main.rs polyoxide-data/examples/v2_soak/verdict.rs
git commit -F - <<'EOF'
test(data): v2_soak verdict rules for stages and pinned counts

A 429 is attributed to the origin or to Cloudflare by its body. A stage is
invalid if it hit the CDN cache, repeated a URL, errored, ended early or
could not drive its target rate. It is saturated if p99 triples, since
upstream queues heavy queries before refusing them. The count to pin is
the highest clean rate below the first stage that was not, per 10 seconds.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 4: `v2_soak` entry point: bootstrap, ramps, validation

**Files:**
- Modify: `polyoxide-data/examples/v2_soak/main.rs` (full replacement)

- [ ] **Step 1: Write the entry point**

Ramps send raw `reqwest` requests paced by the shared `Pacer`: a polyoxide client's own limiter would bind above its row, and only a raw response shows `x-cache`. Validation (`--pace client`) paces the same raw requests through `RateLimiter::data_default()`, so it tests the pinned rows and their prefix matching exactly. It also accepts several routes (`--route all`) in one process through one limiter, which is the only way to see an allowance shared across routes.

Replace the entire contents of `polyoxide-data/examples/v2_soak/main.rs`:

````rust
//! Measures, then validates, the rate limits for Data API v2 routes.
//!
//! Upstream publishes no v2 figures, so `RateLimiter::data_default` started
//! with rows borrowed from v1. This harness replaces them with measured ones.
//!
//! ```sh
//! # Ramp: drive fixed rates, stepping up until upstream pushes back.
//! cargo run --release -p polyoxide-data --example v2_soak -- --route positions
//!
//! # Validate: pace by the shipped limiter and require zero 429s.
//! cargo run --release -p polyoxide-data --example v2_soak -- --route positions --pace client
//! ```
//!
//! # Why raw requests
//!
//! Ramps send requests with `reqwest` directly rather than through `DataApi`,
//! for three reasons. A polyoxide client's own limiter would bind before any
//! rate above its row, so it could not measure above a provisional figure. A
//! 429 the client retries away is visible only through `tracing`. And
//! CloudFront caches several v2 routes: only the raw response shows
//! `x-cache: Hit`, which means the origin never saw the request. Validation
//! (`--pace client`) still paces through the shipped `RateLimiter`, so it
//! tests the pinned rows and their prefix matching exactly.
//!
//! # Why every URL is distinct
//!
//! A repeated URL is answered from the cache, so a ramp that repeats URLs
//! reports a clean run at any rate. See `probes.rs`.
//!
//! # Several routes at once
//!
//! Validation accepts `--route all` or a list, and then runs every route from
//! one process through one shared limiter, as a real client would. Upstream
//! describes a per-client allowance; if it is shared across routes, per-route
//! runs cannot see it and only a mixed run can.
//!
//! # Reading the result
//!
//! A ramp stops at the first stage that is throttled, saturated or invalid,
//! and prints the count to pin (the highest clean rate, per 10 seconds). The
//! rules are in `verdict.rs` and unit-tested. Exit code 0 means a count was
//! found, 1 that even the first stage was not clean, 2 that the run was
//! invalid.

use std::{
    collections::HashSet,
    fmt::Write as _,
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use polyoxide_core::RateLimiter;
use reqwest::Method;

#[path = "../common/mod.rs"]
mod common;
mod probes;
mod verdict;

use common::Pacer;
use probes::{parse_routes, Pools, ProbeSource, Route, SeenUrls};
use verdict::{classify, judge, pin, Abort, Layer, Pin, Reply, Sample, Stage, Verdict};

const DEFAULT_BASE_URL: &str = "https://data-api.polymarket.com";
const DEFAULT_STAGES: [f64; 5] = [10.0, 15.0, 20.0, 30.0, 40.0];

/// The highest rate a ramp may drive. Above it the default client (four
/// concurrent requests) cannot reach the rate at typical latencies anyway, so
/// a figure above it would be unmeasured territory nobody can use.
const CEILING_RPS: f64 = 40.0;

/// Pages of the bare trade feed read to find wallets and markets.
const MAX_BOOTSTRAP_PAGES: usize = 20;

// ── Configuration ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Ramp { stages: Vec<f64> },
    Client,
}

#[derive(Debug, Clone, PartialEq)]
struct Config {
    routes: Vec<Route>,
    mode: Mode,
    stage_secs: u64,
    cooldown_secs: u64,
    concurrency: usize,
    wallets: usize,
    conditions: usize,
    base_url: String,
}

const USAGE: &str = "\
Measure or validate the rate limits for Data API v2 routes.

Usage: v2_soak --route <routes> [options]

  --route <routes>        positions | combo-positions | trades | activity |
                          user-pnl | holders-pnl. A ramp takes exactly one;
                          --pace client also takes a comma list or `all`
  --stages <r1,r2,...>    Ramp rates in req/s, ascending, at most 40
                          (default: 10,15,20,30,40)
  --pace client           Validate instead: pace by RateLimiter::data_default
                          and require zero 429s
  --stage-secs <n>        Seconds per stage (default: 60 ramp, 120 validation)
  --cooldown-secs <n>     Idle seconds between ramp stages (default: 120)
  --concurrency <n>       In-flight requests per route (default: 16 ramp,
                          4 validation)
  --wallets <n>           Live wallets to build probes from (default: 500)
  --conditions <n>        Live markets to build probes from (default: 300)
  --base-url <url>        Override the Data API host
  -h, --help              Show this message";

fn parse_stages(raw: &str) -> Result<Vec<f64>, String> {
    let stages: Vec<f64> = raw
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<f64>()
                .map_err(|_| format!("bad stage rate: {s:?}"))
        })
        .collect::<Result<_, _>>()?;
    if stages.is_empty() {
        return Err("--stages needs at least one rate".into());
    }
    for rate in &stages {
        if !rate.is_finite() || *rate <= 0.0 {
            return Err(format!("stage rate {rate} must be positive"));
        }
        if *rate > CEILING_RPS {
            return Err(format!(
                "stage rate {rate} is above the {CEILING_RPS} req/s ceiling"
            ));
        }
    }
    if stages.windows(2).any(|w| w[1] <= w[0]) {
        return Err("--stages must be strictly ascending".into());
    }
    Ok(stages)
}

impl Config {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Option<Self>, String> {
        let mut routes = None;
        let mut stages = None;
        let mut client = false;
        let mut stage_secs = None;
        let mut cooldown_secs = 120;
        let mut concurrency = None;
        let mut wallets = 500;
        let mut conditions = 300;
        let mut base_url = DEFAULT_BASE_URL.to_owned();

        let mut args = args.peekable();
        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| format!("{flag} requires a value"))
            };
            let number = |raw: String| raw.parse::<u64>().map_err(|_| format!("bad {flag}: {raw}"));
            match flag.as_str() {
                "-h" | "--help" => return Ok(None),
                "--route" => routes = Some(parse_routes(&value()?)?),
                "--stages" => stages = Some(parse_stages(&value()?)?),
                "--pace" => match value()?.as_str() {
                    "client" => client = true,
                    other => return Err(format!("--pace takes only `client`, got {other:?}")),
                },
                "--stage-secs" => stage_secs = Some(number(value()?)?),
                "--cooldown-secs" => cooldown_secs = number(value()?)?,
                "--concurrency" => concurrency = Some(number(value()?)? as usize),
                "--wallets" => wallets = number(value()?)? as usize,
                "--conditions" => conditions = number(value()?)? as usize,
                "--base-url" => base_url = value()?.trim_end_matches('/').to_owned(),
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        let routes = routes.ok_or("--route is required")?;
        if !client && routes.len() != 1 {
            return Err(
                "a ramp measures one route at a time; list several only with --pace client".into(),
            );
        }
        if client && stages.is_some() {
            return Err("--pace client and --stages are mutually exclusive".into());
        }
        let mode = if client {
            Mode::Client
        } else {
            Mode::Ramp {
                stages: stages.unwrap_or_else(|| DEFAULT_STAGES.to_vec()),
            }
        };
        let stage_secs = stage_secs.unwrap_or(if client { 120 } else { 60 });
        let concurrency = concurrency.unwrap_or(if client { 4 } else { 16 });
        if stage_secs == 0 || concurrency == 0 {
            return Err("--stage-secs and --concurrency must be greater than zero".into());
        }
        if wallets == 0 || conditions == 0 {
            return Err("--wallets and --conditions must be greater than zero".into());
        }

        Ok(Some(Config {
            routes,
            mode,
            stage_secs,
            cooldown_secs,
            concurrency,
            wallets,
            conditions,
            base_url,
        }))
    }
}

// ── Bootstrap ───────────────────────────────────────────────────

/// Adds each item not already present, preserving first-seen order.
fn push_distinct(into: &mut Vec<String>, seen: &mut HashSet<String>, item: &str, cap: usize) {
    if into.len() < cap && seen.insert(item.to_owned()) {
        into.push(item.to_owned());
    }
}

/// Collects live wallets and markets from the bare trade feed, one page per
/// second, before any measurement starts.
async fn bootstrap(http: &reqwest::Client, config: &Config) -> Result<Pools, String> {
    let mut pools = Pools::default();
    let (mut seen_wallets, mut seen_conditions) = (HashSet::new(), HashSet::new());
    let mut cursor: Option<String> = None;

    for page in 0..MAX_BOOTSTRAP_PAGES {
        if page > 0 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let mut request = http
            .get(format!("{}/v2/trades", config.base_url))
            .query(&[("limit", "1000")]);
        if let Some(cursor) = &cursor {
            request = request.query(&[("cursor", cursor.as_str())]);
        }
        let response = request
            .send()
            .await
            .map_err(|e| format!("bootstrap: {e}"))?;
        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("bootstrap: {status}: {e}"))?;
        if !status.is_success() {
            return Err(format!("bootstrap: {status}: {body}"));
        }

        for row in body["data"].as_array().into_iter().flatten() {
            if let Some(wallet) = row["proxy_wallet"].as_str() {
                push_distinct(
                    &mut pools.wallets,
                    &mut seen_wallets,
                    wallet,
                    config.wallets,
                );
            }
            if let Some(condition) = row["condition_id"].as_str() {
                push_distinct(
                    &mut pools.conditions,
                    &mut seen_conditions,
                    condition,
                    config.conditions,
                );
            }
        }
        if pools.wallets.len() >= config.wallets && pools.conditions.len() >= config.conditions {
            break;
        }
        match body["pagination"]["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }
    Ok(pools)
}

// ── Running a stage ─────────────────────────────────────────────

#[derive(Clone)]
enum Pace {
    Fixed(Arc<Pacer>),
    Client(RateLimiter),
}

/// One route and its probe space. A mixed validation runs several lanes.
struct Lane {
    route: Route,
    source: Arc<ProbeSource>,
}

struct Run {
    http: reqwest::Client,
    base_url: Arc<str>,
    lanes: Vec<Lane>,
    seen: Arc<SeenUrls>,
    /// In-flight requests per lane.
    concurrency: usize,
}

async fn send_probe(http: &reqwest::Client, url: &str) -> Reply {
    let response = match http.get(url).send().await {
        Ok(response) => response,
        Err(_) => return Reply::Error(0),
    };
    let status = response.status().as_u16();
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let (x_cache, retry_after) = (header("x-cache"), header("retry-after"));
    let body = response.text().await.unwrap_or_default();
    classify(status, x_cache.as_deref(), retry_after.as_deref(), &body)
}

async fn run_stage(run: &Run, pace: Pace, target_rps: Option<f64>, planned: Duration) -> Stage {
    let start = Instant::now();
    let deadline = start + planned;
    let stop = Arc::new(AtomicBool::new(false));
    let abort: Arc<Mutex<Option<Abort>>> = Arc::new(Mutex::new(None));
    let samples: Arc<Mutex<Vec<Sample>>> = Arc::new(Mutex::new(Vec::new()));

    let mut workers = tokio::task::JoinSet::new();
    let assignments = run
        .lanes
        .iter()
        .flat_map(|lane| std::iter::repeat_n(lane, run.concurrency));
    for lane in assignments {
        let (http, base_url, route) = (run.http.clone(), Arc::clone(&run.base_url), lane.route);
        let (source, seen) = (Arc::clone(&lane.source), Arc::clone(&run.seen));
        let (pace, stop, abort, samples) =
            (pace.clone(), stop.clone(), abort.clone(), samples.clone());

        workers.spawn(async move {
            let halt = |why: Abort| {
                abort
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .get_or_insert(why);
                stop.store(true, Ordering::Relaxed);
            };
            loop {
                if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    break;
                }
                match &pace {
                    Pace::Fixed(pacer) => pacer.wait().await,
                    Pace::Client(limiter) => {
                        limiter.acquire(route.path(), Some(&Method::GET)).await
                    }
                }
                if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    break;
                }
                let Some(url) = source.next_url() else {
                    halt(Abort::ProbeSpaceExhausted);
                    break;
                };
                if !seen.insert(&url) {
                    halt(Abort::DuplicateUrl);
                    break;
                }

                let sent = Instant::now();
                let reply = send_probe(&http, &format!("{base_url}{url}")).await;
                let finished = Instant::now();
                let ends_stage = matches!(reply, Reply::Throttled { .. } | Reply::CacheHit);
                samples
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(Sample {
                        path: route.path(),
                        finished_at: finished.duration_since(start),
                        latency: finished.duration_since(sent),
                        reply,
                    });
                if ends_stage {
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
            }
        });
    }
    while workers.join_next().await.is_some() {}

    let mut samples = std::mem::take(&mut *samples.lock().unwrap_or_else(|p| p.into_inner()));
    samples.sort_by_key(|s| s.finished_at);
    let abort = *abort.lock().unwrap_or_else(|p| p.into_inner());
    Stage {
        target_rps,
        planned,
        elapsed: start.elapsed(),
        samples,
        abort,
    }
}

// ── Reporting ───────────────────────────────────────────────────

fn millis(d: Duration) -> String {
    format!("{:.0}", d.as_secs_f64() * 1000.0)
}

fn layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::Origin => "origin",
        Layer::Cloudflare => "cloudflare 1015",
        Layer::Unknown => "unrecognised 429",
    }
}

fn verdict_label(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Clean => "clean".into(),
        Verdict::Throttled {
            path,
            layer,
            retry_after,
            at,
        } => format!(
            "**throttled** on {path} ({}, retry-after {}, at {:.1}s)",
            layer_name(*layer),
            retry_after.map_or("none".into(), |d| format!("{}s", d.as_secs_f64())),
            at.as_secs_f64()
        ),
        Verdict::Saturated { p99, baseline_p99 } => format!(
            "**saturated** (p99 {}ms vs baseline {}ms)",
            millis(*p99),
            millis(*baseline_p99)
        ),
        Verdict::Invalid(why) => format!("**invalid**: {why}"),
    }
}

const TABLE_HEADER: &str = "| stage | requests | achieved req/s | p50 ms | p99 ms | 429s | cache hits | errors | verdict |\n|-------|----------|----------------|--------|--------|------|------------|--------|---------|";

fn stage_row(label: &str, stage: &Stage, verdict: &Verdict) -> String {
    let mut row = String::new();
    let _ = write!(
        row,
        "| {label} | {} | {:.2} | {} | {} | {} | {} | {} | {} |",
        stage.samples.len(),
        stage.achieved_rps(),
        millis(stage.p50()),
        millis(stage.p99()),
        stage.count(|r| matches!(r, Reply::Throttled { .. })),
        stage.count(|r| *r == Reply::CacheHit),
        stage.count(|r| matches!(r, Reply::Error(_))),
        verdict_label(verdict)
    );
    row
}

/// What to wait before touching the host again after a throttle.
fn cooldown_advice(verdict: &Verdict) -> Option<String> {
    match verdict {
        Verdict::Throttled {
            layer: Layer::Cloudflare,
            ..
        } => Some(
            "Cloudflare's 1015 blocks every data-api path for this IP, and traffic during the \
             block extends it. Wait at least 10 minutes before any further run."
                .into(),
        ),
        Verdict::Throttled { retry_after, .. } => {
            let wait = retry_after.unwrap_or_default().max(Duration::from_secs(60));
            Some(format!(
                "Wait at least {}s before the next run.",
                wait.as_secs()
            ))
        }
        _ => None,
    }
}

fn pin_report(route: Route, decision: &Pin) -> String {
    match decision {
        Pin::Count(count) => format!(
            "Pin for {path}: {count} per 10s\n\n    simple_limit(\"{path}\", None, {count}, ten_sec),",
            path = route.path()
        ),
        Pin::RetryLower => format!(
            "No clean stage for {}. Re-run with lower stages, e.g. `--stages 3,5,7`.",
            route.path()
        ),
        Pin::Invalid(why) => format!("The ramp is invalid, so nothing may be pinned: {why}"),
    }
}

// ── Entry point ─────────────────────────────────────────────────

fn start_offset() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default()
}

#[tokio::main]
async fn main() -> ExitCode {
    let config = match Config::from_args(std::env::args().skip(1)) {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    let http = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("polyoxide-v2-soak")
        .build()
    {
        Ok(http) => http,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    println!("bootstrapping wallets and markets from the trade feed …");
    let pools = match bootstrap(&http, &config).await {
        Ok(pools) => pools,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(2);
        }
    };
    println!(
        "{} wallets, {} markets",
        pools.wallets.len(),
        pools.conditions.len()
    );
    let mut lanes = Vec::new();
    for route in &config.routes {
        match ProbeSource::new(*route, pools.clone(), start_offset()) {
            Ok(source) => {
                println!(
                    "  {}: {} distinct probe URLs",
                    route.path(),
                    source.capacity()
                );
                lanes.push(Lane {
                    route: *route,
                    source: Arc::new(source),
                });
            }
            Err(message) => {
                eprintln!("error: {message}");
                return ExitCode::from(2);
            }
        }
    }
    let paths: Vec<&str> = config.routes.iter().map(|r| r.path()).collect();
    let paths = paths.join(", ");

    let run = Run {
        http,
        base_url: Arc::from(config.base_url.as_str()),
        lanes,
        seen: Arc::new(SeenUrls::default()),
        concurrency: config.concurrency,
    };
    let planned = Duration::from_secs(config.stage_secs);

    match &config.mode {
        Mode::Client => {
            println!(
                "\nvalidating {paths} for {}s at the shipped limiter's pace, concurrency {} per route …\n",
                config.stage_secs,
                config.concurrency
            );
            let stage = run_stage(
                &run,
                Pace::Client(RateLimiter::data_default()),
                None,
                planned,
            )
            .await;
            let verdict = judge(&stage, None);
            println!("{TABLE_HEADER}\n{}", stage_row("client", &stage, &verdict));
            if let Some(advice) = cooldown_advice(&verdict) {
                println!("\n{advice}");
            }
            if verdict == Verdict::Clean {
                println!("\nPASS");
                ExitCode::SUCCESS
            } else {
                println!("\nFAIL");
                ExitCode::from(1)
            }
        }
        Mode::Ramp { stages } => {
            println!(
                "\nramping {paths} through {stages:?} req/s, {}s per stage, {}s cooldown, concurrency {}\n",
                config.stage_secs,
                config.cooldown_secs,
                config.concurrency
            );
            println!("{TABLE_HEADER}");
            let mut results = Vec::new();
            let mut baseline_p99 = None;
            for (i, rate) in stages.iter().enumerate() {
                if i > 0 {
                    tokio::time::sleep(Duration::from_secs(config.cooldown_secs)).await;
                }
                let pacer = Arc::new(Pacer::new(Duration::from_secs_f64(1.0 / rate)));
                let stage = run_stage(&run, Pace::Fixed(pacer), Some(*rate), planned).await;
                let verdict = judge(&stage, baseline_p99);
                if i == 0 {
                    baseline_p99 = Some(stage.p99());
                }
                println!("{}", stage_row(&format!("{rate} req/s"), &stage, &verdict));
                let advice = cooldown_advice(&verdict);
                let clean = verdict == Verdict::Clean;
                results.push((*rate, verdict));
                if !clean {
                    if let Some(advice) = advice {
                        println!("\n{advice}");
                    }
                    break;
                }
            }
            let decision = pin(&results);
            println!("\n{}", pin_report(config.routes[0], &decision));
            match decision {
                Pin::Count(_) => ExitCode::SUCCESS,
                Pin::RetryLower => ExitCode::from(1),
                Pin::Invalid(_) => ExitCode::from(2),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Config>, String> {
        Config::from_args(args.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn a_ramp_defaults_to_the_documented_stages() {
        let config = parse(&["--route", "positions"]).unwrap().unwrap();
        assert_eq!(config.routes, [Route::Positions]);
        assert_eq!(
            config.mode,
            Mode::Ramp {
                stages: DEFAULT_STAGES.to_vec()
            }
        );
        assert_eq!(
            (config.stage_secs, config.cooldown_secs, config.concurrency),
            (60, 120, 16)
        );
    }

    #[test]
    fn validation_defaults_to_the_shipped_client_shape() {
        let config = parse(&["--route", "trades", "--pace", "client"])
            .unwrap()
            .unwrap();
        assert_eq!(config.mode, Mode::Client);
        assert_eq!((config.stage_secs, config.concurrency), (120, 4));
    }

    #[test]
    fn stages_must_be_ascending_positive_and_under_the_ceiling() {
        assert_eq!(parse_stages("3, 5,7.5"), Ok(vec![3.0, 5.0, 7.5]));
        assert!(parse_stages("10,10").is_err());
        assert!(parse_stages("20,10").is_err());
        assert!(parse_stages("0").is_err());
        assert!(parse_stages("nan").is_err());
        assert!(parse_stages("41").is_err());
        assert!(parse_stages("").is_err());
    }

    #[test]
    fn a_route_is_required_and_pace_conflicts_with_stages() {
        assert!(parse(&[]).is_err());
        assert!(parse(&["--route", "closed-positions"]).is_err());
        assert!(parse(&["--route", "trades", "--pace", "client", "--stages", "5"]).is_err());
        assert!(parse(&["--route", "trades", "--pace", "fast"]).is_err());
    }

    #[test]
    fn only_validation_takes_several_routes() {
        assert!(parse(&["--route", "all"]).is_err());
        assert!(parse(&["--route", "trades,activity"]).is_err());
        let config = parse(&["--route", "all", "--pace", "client"])
            .unwrap()
            .unwrap();
        assert_eq!(config.routes, Route::ALL);
    }

    #[test]
    fn zero_values_are_refused() {
        assert!(parse(&["--route", "trades", "--concurrency", "0"]).is_err());
        assert!(parse(&["--route", "trades", "--stage-secs", "0"]).is_err());
        assert!(parse(&["--route", "trades", "--wallets", "0"]).is_err());
    }

    #[test]
    fn help_short_circuits() {
        assert_eq!(parse(&["--help"]), Ok(None));
    }

    #[test]
    fn push_distinct_keeps_first_seen_order_up_to_the_cap() {
        let (mut into, mut seen) = (Vec::new(), HashSet::new());
        for item in ["a", "b", "a", "c", "d"] {
            push_distinct(&mut into, &mut seen, item, 3);
        }
        assert_eq!(into, ["a", "b", "c"]);
    }

    #[test]
    fn cloudflare_advice_names_the_host_wide_block() {
        let verdict = Verdict::Throttled {
            path: "/v2/trades",
            layer: Layer::Cloudflare,
            retry_after: Some(Duration::ZERO),
            at: Duration::ZERO,
        };
        assert!(cooldown_advice(&verdict).unwrap().contains("10 minutes"));
        let origin = Verdict::Throttled {
            path: "/v2/trades",
            layer: Layer::Origin,
            retry_after: Some(Duration::from_secs(90)),
            at: Duration::ZERO,
        };
        assert_eq!(
            cooldown_advice(&origin).unwrap(),
            "Wait at least 90s before the next run."
        );
        assert_eq!(cooldown_advice(&Verdict::Clean), None);
    }

    #[test]
    fn the_pin_report_prints_the_row_to_paste() {
        let report = pin_report(Route::Positions, &Pin::Count(150));
        assert!(
            report.contains("simple_limit(\"/v2/positions\", None, 150, ten_sec),"),
            "{report}"
        );
    }
}
````

- [ ] **Step 2: Run the tests**

Run:

```bash
cargo test -p polyoxide-data --example v2_soak
```

Expected: `test result: ok. 50 passed`.

- [ ] **Step 3: Smoke-test all three modes against the live host, gently**

About 250 requests in total, all below any provisional row. These prove the harness works end to end: bootstrap, distinct URLs, pacing, cache detection and the report. They measure nothing.

Run:

```bash
cargo run -p polyoxide-data --example v2_soak -- --route user-pnl --stages 1,2 --stage-secs 8 --cooldown-secs 3 --concurrency 2 --wallets 20 --conditions 20
```

Expected: two `clean` rows at `1.00` and `2.00` achieved req/s, then `Pin for /v2/user-pnl: 20 per 10s`. Exit 0.

Run:

```bash
cargo run -p polyoxide-data --example v2_soak -- --route holders-pnl --pace client --stage-secs 5 --concurrency 1 --wallets 20 --conditions 20
```

Expected: one `client` row, `0` cache hits although `/v2/holders` is cached for 120s, then `PASS`.

Run:

```bash
cargo run -p polyoxide-data --example v2_soak -- --route all --pace client --stage-secs 5 --concurrency 1 --wallets 20 --conditions 20
```

Expected: six probe spaces listed, one `client` row with `0` 429s, `0` cache hits, `0` errors, then `PASS`.

- [ ] **Step 4: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-data --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-data --example v2_soak
```

Expected: `50 passed`.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-data/examples/v2_soak/main.rs
git commit -F - <<'EOF'
feat(data): v2_soak harness for measuring Data API v2 rate limits

Ramps drive fixed rates with raw requests over distinct URLs and stop at
the first 429, then print the count to pin. Validation paces the same
requests through RateLimiter::data_default, per route or across all routes
in one process, and requires zero 429s. Smoke-tested live in all three
modes.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 5: Ramp each route and record the results

**Files:**
- Modify: `docs/specs/data-v2/OBSERVED.md`

- [ ] **Step 1: Pre-flight**

Measurement puts real load on Polymarket's host. Under the budget chosen for this plan (60s stages, 120s cooldowns) the ramps send at most about 41,000 requests across all six routes if nothing throttles, and far fewer in practice because each ramp stops at its first throttle.

- **Nothing else on this machine's IP may use `data-api.polymarket.com` while this runs.** A Cloudflare `1015` blocks every data-api path for the IP, and traffic during the block extends it.
- **Build output must not be under `/tmp`.** It is a RAM-backed tmpfs on this machine. Check that `echo $CARGO_TARGET_DIR` is empty or on disk.
- Build the release binary once, so ramps don't compete with a compile:

Run:

```bash
cargo build --release -p polyoxide-data --example v2_soak
```

Expected: finishes; the binary is at `target/release/examples/v2_soak`.

Add the section the results go in:

Append to the end of `docs/specs/data-v2/OBSERVED.md`:

```markdown

## Measured rate limits

Upstream publishes no v2 figures. Each route below was ramped with
`polyoxide-data/examples/v2_soak` (raw requests, every URL distinct so the CDN
cannot answer, abort on the first 429) and then validated at the shipped
limiter's pace. `RateLimiter::data_default` pins the counts in the table at the
end of this section.

### Ramps

<!-- For each route: the date, the exact command, then the harness's table and
its "Pin for" line pasted verbatim. Re-runs (lower stages, more concurrency)
get their own entry below the first, with the reason. -->

### Validation

<!-- One row per `--pace client` run: command, result table row, PASS/FAIL,
and what was changed before the next run if it failed. -->

### Pinned

| Route | Per 10s | Ramp stopped by |
|-------|---------|-----------------|
```

- [ ] **Step 2: Ramp the six routes, one at a time**

Run each command, wait for it to finish, and paste its output into `OBSERVED.md` under **Ramps** (see the next step) before starting the next one. Each takes 5 to 13 minutes. Wait at least 5 minutes between routes, and follow any wait the harness prints after a throttle; after a Cloudflare `1015` that means at least 10 minutes.

Run:

```bash
target/release/examples/v2_soak --route positions
```

Run:

```bash
target/release/examples/v2_soak --route combo-positions
```

Run:

```bash
target/release/examples/v2_soak --route trades
```

Run:

```bash
target/release/examples/v2_soak --route activity
```

Run:

```bash
target/release/examples/v2_soak --route user-pnl
```

Run:

```bash
target/release/examples/v2_soak --route holders-pnl
```

- [ ] **Step 3: Handle each outcome**

The harness ends each ramp with one of these lines. Act on it before moving to the next route.

| Last line | Exit | Action |
|-----------|------|--------|
| `Pin for <path>: N per 10s` | 0 | Record `N` in **Pinned**, with the verdict of the stage that stopped the ramp (or `clean to 40 req/s`). |
| `No clean stage for <path>` | 1 | Wait as advised, then re-run with `--stages 3,5,7`. If that is also not clean, record `20` (2 req/s) as the pin and write down in `OBSERVED.md` that the route throttles below 3 req/s. |
| `The ramp is invalid … achieved X of Y req/s; raise --concurrency` | 2 | Re-run the route with `--concurrency 32`. |
| `The ramp is invalid … the probe space ran out` | 2 | Re-run with `--wallets 1000 --conditions 600`. |
| `The ramp is invalid … N of M requests failed (status …)` | 2 | A `400` means a probe parameter the server now rejects: stop and fix `probes.rs` against the error body (`curl` one URL from the route's shape). Transport or `5xx` errors: wait 10 minutes and re-run once; if it repeats, stop and ask. |
| `The ramp is invalid … responses came from the CDN cache` | 2 | **Stop.** Distinct URLs are being served from cache, so the CDN's cache key is not the full URL. Nothing from this harness can be trusted until that is understood. Record it in `OBSERVED.md` and ask. |
| `The ramp is invalid … a probe URL repeated` | 2 | A harness bug (`SeenUrls` caught a collision the probe space should rule out). Stop and fix. |

A `**throttled** … (unrecognised 429 …)` stage is still a throttle, and the ramp's pin stands. Note the unrecognised body in `OBSERVED.md`.

- [ ] **Step 4: Record the ramps**

Under **Ramps**, add one entry per run in this shape. The table and pin line are the harness output, pasted verbatim:

````markdown
#### `/v2/positions` (2026-09-15)

`target/release/examples/v2_soak --route positions`

| stage | requests | achieved req/s | p50 ms | p99 ms | 429s | cache hits | errors | verdict |
|-------|----------|----------------|--------|--------|------|------------|--------|---------|
| 10 req/s | 600 | 10.00 | 210 | 640 | 0 | 0 | 0 | clean |
| 15 req/s | … |

Pin for /v2/positions: 150 per 10s
````

(The numbers above are only an example of the shape.) Then fill **Pinned** with one row per route.

- [ ] **Step 5: Commit**

```bash
git add docs/specs/data-v2/OBSERVED.md
git commit -F - <<'EOF'
docs(specs): record Data API v2 rate-limit ramps

Each route ramped with v2_soak over distinct URLs, stopping at the first
throttle. OBSERVED.md carries the runs verbatim and the count each one
supports.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 6: Pin the measured rows

**Files:**
- Modify: `polyoxide-core/src/rate_limit.rs`

- [ ] **Step 1: Write the failing test from the Pinned table**

In `polyoxide-core/src/rate_limit.rs`, inside `mod documented_data_limits`, replace `provisional_v2` and its test with the measured version. Each count comes from the **Pinned** table in `OBSERVED.md`. `/v2/positions/combos` always appears in the test: with its own count if its ramp pinned a different figure from `/v2/positions`, otherwise with the positions count, which proves the prefix covers it.

Replace:

```rust
    /// Data API v2 rows. Not published upstream: each borrows the v1 quota of
    /// the route it replaces, pending measurement.
    fn provisional_v2() -> Vec<DocumentedRule> {
        vec![
            ("/v2/positions", Some(Method::GET), vec![(150, 10)]),
            ("/v2/positions/combos", Some(Method::GET), vec![(150, 10)]),
            ("/v2/trades", Some(Method::GET), vec![(200, 10)]),
            ("/v2/user-pnl", Some(Method::GET), vec![(200, 10)]),
        ]
    }

    #[test]
    fn every_v2_route_resolves_to_its_provisional_quota() {
        assert_matches_published(&RateLimiter::data_default(), provisional_v2(), 1_000);
    }
```

with this, substituting the six counts:

```rust
    /// Data API v2 rows. Upstream publishes no v2 figures; each count is the
    /// highest clean ramp rate recorded in `docs/specs/data-v2/OBSERVED.md`.
    fn measured_v2() -> Vec<DocumentedRule> {
        vec![
            ("/v2/positions", Some(Method::GET), vec![(POSITIONS, 10)]),
            ("/v2/positions/combos", Some(Method::GET), vec![(COMBO_POSITIONS, 10)]),
            ("/v2/trades", Some(Method::GET), vec![(TRADES, 10)]),
            ("/v2/activity", Some(Method::GET), vec![(ACTIVITY, 10)]),
            ("/v2/user-pnl", Some(Method::GET), vec![(USER_PNL, 10)]),
            ("/v2/holders", Some(Method::GET), vec![(HOLDERS, 10)]),
        ]
    }

    #[test]
    fn every_v2_route_resolves_to_its_measured_quota() {
        assert_matches_published(&RateLimiter::data_default(), measured_v2(), 1_000);
    }
```

The capitalised names are not constants to define. Type the numbers in: for example, if **Pinned** says `/v2/trades` is `300`, the row reads `("/v2/trades", Some(Method::GET), vec![(300, 10)]),`.

Run:

```bash
cargo test -p polyoxide-core --lib every_v2_route
```

Expected: `every_v2_route_resolves_to_its_measured_quota` fails. Either `/v2/activity` or `/v2/holders` falls through to the general bucket, or a borrowed count differs from the measured one.

- [ ] **Step 2: Replace the provisional rows**

In `RateLimiter::data_default`, replace the provisional block:

```rust
                    // Data API v2. Upstream publishes no v2 figures, so each row
                    // borrows the v1 quota of the route(s) it replaces until it is
                    // measured: `/v2/positions` folds `/positions` and
                    // `/closed-positions` together, and by prefix it also covers
                    // `/v2/positions/combos`.
                    simple_limit("/v2/positions", None, 150, ten_sec),
                    simple_limit("/v2/trades", None, 200, ten_sec),
                    simple_limit("/v2/user-pnl", None, 200, ten_sec),
```

with the measured rows, using the same numbers as the test:

```rust
                    // Data API v2. Upstream publishes no v2 figures; each count is the
                    // highest clean rate from the ramps in
                    // docs/specs/data-v2/OBSERVED.md, and `quota()` reserves a tenth.
                    // `/v2/positions/combos` must precede `/v2/positions`: prefix
                    // matching takes the first row that matches.
                    simple_limit("/v2/positions/combos", None, COMBO_POSITIONS, ten_sec),
                    simple_limit("/v2/positions", None, POSITIONS, ten_sec),
                    simple_limit("/v2/trades", None, TRADES, ten_sec),
                    simple_limit("/v2/activity", None, ACTIVITY, ten_sec),
                    simple_limit("/v2/user-pnl", None, USER_PNL, ten_sec),
                    simple_limit("/v2/holders", None, HOLDERS, ten_sec),
```

If `COMBO_POSITIONS` equals `POSITIONS`, delete the combos row and its two comment lines about ordering: the `/v2/positions` prefix already covers it, and the test still asserts that.

Then update the row count in `test_data_default_construction`: 5 v1 rows plus the v2 rows you kept (`11` with a combos row, `10` without).

- [ ] **Step 3: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output.

Run:

```bash
cargo clippy -p polyoxide-core --all-targets --all-features -- -D warnings
```

Expected: no warnings.

Run:

```bash
cargo test -p polyoxide-core --lib rate_limit
```

Expected: every `rate_limit` test passes, including `every_v2_route_resolves_to_its_measured_quota` and `test_data_default_construction`.

Run:

```bash
cargo build --release -p polyoxide-data --example v2_soak
```

Expected: rebuilds against the new rows. Validation in the next task uses this binary.

- [ ] **Step 4: Commit**

```bash
git add polyoxide-core/src/rate_limit.rs
git commit -F - <<'EOF'
feat(core): pin measured Data API v2 rate limits

The provisional rows borrowed from v1 are replaced by the highest clean
rate from each route's v2_soak ramp (docs/specs/data-v2/OBSERVED.md), and
/v2/activity and /v2/holders get rows of their own.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 7: Validate each route at the shipped limiter's pace

**Files:**
- Modify: `polyoxide-core/src/rate_limit.rs` (only if a validation fails)
- Modify: `docs/specs/data-v2/OBSERVED.md`

- [ ] **Step 1: Validate the six routes, one at a time**

Each run lasts 120 seconds at the pinned row's own pace. Wait 5 minutes between runs, and paste each result row under **Validation** in `OBSERVED.md` with its command.

Run:

```bash
target/release/examples/v2_soak --route positions --pace client
```

Run:

```bash
target/release/examples/v2_soak --route combo-positions --pace client
```

Run:

```bash
target/release/examples/v2_soak --route trades --pace client
```

Run:

```bash
target/release/examples/v2_soak --route activity --pace client
```

Run:

```bash
target/release/examples/v2_soak --route user-pnl --pace client
```

Run:

```bash
target/release/examples/v2_soak --route holders-pnl --pace client
```

- [ ] **Step 2: If a route fails, lower its row and validate it again**

A `FAIL` means the row paces faster than the server sustains for 120s, even though the ramp stage was clean for 60s. For that route:

1. Wait as the harness advises.
2. Lower its count by a quarter, rounding down: `new = floor(old * 0.75)`. Change both the row in `data_default` and the matching `measured_v2` entry.
3. Run `cargo test -p polyoxide-core --lib rate_limit` and then the release build again.
4. Re-run that route's validation.

Repeat until it passes. Record every attempt, and update **Pinned** to the final count with the note `lowered after validation`.

- [ ] **Step 3: Commit**

```bash
git add docs/specs/data-v2/OBSERVED.md polyoxide-core/src/rate_limit.rs
git commit -F - <<'EOF'
test(data): validate the pinned Data API v2 rows per route

Each route ran for 120s through RateLimiter::data_default with zero 429s.
OBSERVED.md records every run, including any row lowered after a failure.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 8: Validate all routes together, and handle a shared allowance

**Files:**
- Modify: `polyoxide-core/src/rate_limit.rs` (only if the mixed run fails)
- Modify: `docs/specs/data-v2/OBSERVED.md`

- [ ] **Step 1: Run the mixed validation**

One process, six routes, one limiter, four requests in flight per route. Per-route validation cannot detect a per-client allowance shared across routes; this can. The aggregate is capped by the general 1,000/10s bucket (~89 req/s sustained), which is what a single real client could do.

Run:

```bash
target/release/examples/v2_soak --route all --pace client
```

Expected: `PASS`, or a `FAIL` naming the path and layer of the first 429.

- [ ] **Step 2: On PASS**

Record the result row under **Validation**, with the line `No allowance shared across routes was observed at the general bucket's pace.` Skip the next step.

- [ ] **Step 3: On FAIL with an origin 429: add a shared v2 bucket**

Every route passed on its own, so the origin's allowance is shared across routes. Model it the way the CLOB ledger group is modelled: one `Bucket` whose clones every v2 row draws from, plus a catch-all `/v2` row so unmeasured v2 routes draw from it too.

1. Start the shared count at `SHARED = floor(A * 10 * 0.75)`, where `A` is the `achieved req/s` of the failed mixed run.
2. In `data_default`, declare the bucket next to `ten_sec`:

```rust
        // Data API v2's per-client allowance is shared across routes: every route
        // passed validation alone and failed together (OBSERVED.md). Every v2 row
        // draws from this bucket as well as its own, as CLOB's ledger group does.
        let v2_client = Bucket::new(SHARED, ten_sec);
```

3. Replace each measured `simple_limit("/v2/…", None, COUNT, ten_sec)` row with its two-bucket form, and add the catch-all as the last v2 row:

```rust
                    endpoint_limit("/v2/positions", None, vec![v2_client.clone(), Bucket::new(POSITIONS, ten_sec)]),
                    // … one per measured route, same order as before …
                    endpoint_limit("/v2", None, vec![v2_client]),
```

4. In `measured_v2`, give every row both buckets in the order they are awaited, `vec![(SHARED, 10), (COUNT, 10)]`, and add an unmeasured route for the catch-all: `("/v2/leaderboard", Some(Method::GET), vec![(SHARED, 10)])`. Update `test_data_default_construction` for the extra row.
5. Run `cargo test -p polyoxide-core --lib rate_limit`, rebuild the release binary, wait as the harness advised, and re-run the mixed validation.
6. If it fails again, `SHARED = floor(SHARED * 0.75)` and repeat from step 5.

Record each attempt, and add a **Shared allowance** line to **Pinned** with the final `SHARED`.

- [ ] **Step 4: On FAIL with a Cloudflare 1015**

**Stop and ask.** The shipped general bucket (1,000/10s, reserved to ~89 req/s) is meant to keep a single client under Cloudflare's host-wide rule. A `1015` here would mean that published general figure does not hold for v2 traffic, and that changes a v1 assumption, not just a v2 row. Wait at least 10 minutes, and record the run first.

- [ ] **Step 5: Commit**

```bash
git add docs/specs/data-v2/OBSERVED.md polyoxide-core/src/rate_limit.rs
git commit -F - <<'EOF'
test(data): validate the Data API v2 rows across all routes at once

Six routes through one RateLimiter in one process, as a real client would
run them. The result, and any shared bucket it required, is in
docs/specs/data-v2/OBSERVED.md.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```


### Task 9: Document the measured limits

**Files:**
- Modify: `polyoxide-core/src/rate_limit.rs` (doc comment)
- Modify: `CLAUDE.md`
- Modify: `docs/specs/data-v2/INDEX.md`
- Modify: `docs/specs/data/rate-limits.md`

- [ ] **Step 1: Update the limiter's doc comment**

In the `/// Data API rate limits.` doc comment on `data_default`, add after the `` - `/` (health): 100/10s `` bullet:

```rust
    /// - Data API v2: measured per route, since upstream publishes no figures;
    ///   see `docs/specs/data-v2/OBSERVED.md`
```

- [ ] **Step 2: Update CLAUDE.md**

Replace the sentence:

```markdown
The v2 rows in `RateLimiter::data_default` are borrowed
from v1 and are provisional until measured; several v2 routes are CDN-cached, so a soak
that repeats a URL measures CloudFront rather than the origin.
```

with:

```markdown
The v2 rows in `RateLimiter::data_default` were measured with
`polyoxide-data/examples/v2_soak`, which sends raw requests over distinct URLs: several v2
routes are CDN-cached, and a repeated URL is answered by CloudFront without reaching the
origin, so a soak that repeats URLs reports a clean run at any rate. The runs are in
`docs/specs/data-v2/OBSERVED.md`.
```

If Task 8 added a shared bucket, append: ``The origin's allowance is shared across v2 routes, so every v2 row also draws from one `v2_client` bucket, as CLOB's ledger group does.``

- [ ] **Step 3: Point the spec indexes at the measurements**

In `docs/specs/data-v2/INDEX.md`, replace `` No figures are published, and `data/rate-limits.md` covers
  only v1 routes. `` with `` No figures are published, and `data/rate-limits.md` covers
  only v1 routes. Measured figures: [OBSERVED.md](OBSERVED.md#measured-rate-limits). ``

In `docs/specs/data/rate-limits.md`, add after the `Source:` line:

```markdown

These are v1 routes. Data API v2 publishes no limits; the measured `/v2` rows are in
[../data-v2/OBSERVED.md](../data-v2/OBSERVED.md#measured-rate-limits).
```

- [ ] **Step 4: Run the final gate**

From CLAUDE.md, with build output kept off `/tmp`:

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output.

Run:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: no warnings.

Run:

```bash
cargo test --all-features --workspace
```

Expected: every test passes.

Run:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
```

Expected: no warnings.

Run:

```bash
(cd .github/scripts && uv run pytest tests/ -q)
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md docs/specs/data-v2/INDEX.md docs/specs/data/rate-limits.md polyoxide-core/src/rate_limit.rs
git commit -F - <<'EOF'
docs: document the measured Data API v2 rate limits

CLAUDE.md, the v2 and v1 rate-limit pages and the limiter's doc comment
now point at the v2_soak runs in docs/specs/data-v2/OBSERVED.md instead of
calling the v2 rows provisional.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```

---

## After this plan

1. **Release.** Phases 0–3 are complete. Follow the release workflow: fetch `origin`, run `git rev-list --left-right --count origin/main...HEAD` before choosing the version (a 0.x minor, since `DataApiError` became `#[non_exhaustive]`), regenerate `CHANGELOG.md` with `git-cliff --unreleased --tag vX.Y.Z --prepend CHANGELOG.md`, and keep the release commit last.
2. **Phase 4 (Python)** and **Phase 5 (CLI)**, as separate loom sessions with plans written against the released API.

## Spec coverage

| Design Component 7 requirement | Where |
|--------------------------------|-------|
| `v2_soak.rs` reusing `examples/common` | Tasks 1–4 (`examples/v2_soak/`, with `Pacer` moved into `common`) |
| `--route positions\|trades\|activity\|user-pnl\|holders-pnl` | Task 2 (plus `combo-positions`, which the provisional prefix row covered without measurement) |
| `--rate`, or the shipped limiter when omitted | Task 4: `--stages` for fixed rates, `--pace client` for the shipped limiter |
| Cache busting: no repeated URL, and a hard stop otherwise | Task 2 (distinct probe space, `SeenUrls`), Task 3 (any cache hit invalidates a stage) |
| Start provisional, step up, stop at the first 429, pin the highest clean rate | Tasks 3 and 5–6 |
| Record runs in `OBSERVED.md` | Tasks 5, 7, 8 |
| Unmeasured routes stay on the general 1,000/10s bucket | Task 6 pins only the six measured routes; Task 8's shared bucket, if needed, also covers the rest |
