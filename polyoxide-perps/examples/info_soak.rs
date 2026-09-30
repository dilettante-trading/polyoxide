//! Measures, then validates, the rate limit for public Perps routes.
//!
//! Upstream publishes no figure for `/v1/info/*`. This harness ramps a fixed
//! rate against one route, stepping up until the host pushes back, and
//! prints the count to pin (the highest clean rate, per 10 seconds).
//!
//! ```sh
//! # Ramp one route.
//! cargo run --release -p polyoxide-perps --example info_soak -- --route klines
//!
//! # Validate: pace every route by the shipped limiter and require zero 429s.
//! cargo run --release -p polyoxide-perps --example info_soak -- --route all --pace client
//! ```
//!
//! Every URL in a ramp is distinct, because CloudFront fronts the host and a
//! repeated URL is answered from cache (`docs/specs/perps/OBSERVED.md`). A
//! stage whose origin-served share falls below `MIN_ORIGIN_SHARE` is reported
//! as saturated rather than clean.

use std::{
    collections::BTreeMap,
    process::ExitCode,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use polyoxide_perps::{types::InstrumentId, Perps};
use tracing::field::{Field, Visit};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

const DEFAULT_BASE_URL: &str = "https://api.perpetuals.polymarket.com";
const DEFAULT_STAGES: [f64; 5] = [5.0, 10.0, 15.0, 20.0, 30.0];
const CEILING_RPS: f64 = 40.0;
/// Below this share of origin-served replies a stage measured the CDN, not
/// the host.
const MIN_ORIGIN_SHARE: f64 = 0.9;

// ── Routes ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Klines,
    Trades,
    Portfolio,
    Bbo,
}

impl Route {
    const ALL: [Route; 4] = [Route::Klines, Route::Trades, Route::Portfolio, Route::Bbo];

    fn name(self) -> &'static str {
        match self {
            Route::Klines => "klines",
            Route::Trades => "trades",
            Route::Portfolio => "portfolio",
            Route::Bbo => "bbo",
        }
    }

    fn path(self) -> &'static str {
        match self {
            Route::Klines => "/v1/info/klines",
            Route::Trades => "/v1/info/trades",
            Route::Portfolio => "/v1/info/portfolio",
            Route::Bbo => "/v1/info/bbo",
        }
    }
}

fn parse_routes(raw: &str) -> Result<Vec<Route>, String> {
    if raw == "all" {
        return Ok(Route::ALL.to_vec());
    }
    raw.split(',')
        .map(|s| match s.trim() {
            "klines" => Ok(Route::Klines),
            "trades" => Ok(Route::Trades),
            "portfolio" => Ok(Route::Portfolio),
            "bbo" => Ok(Route::Bbo),
            other => Err(format!("unknown route {other:?}")),
        })
        .collect()
}

/// Distinct URLs for a route, drawn from live inputs.
struct Probes {
    instruments: Vec<InstrumentId>,
    addresses: Vec<String>,
    counter: AtomicU64,
}

impl Probes {
    fn next(&self, route: Route, base_url: &str) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let iid = self.instruments[(n as usize) % self.instruments.len()];
        let query = match route {
            // Each request asks for a different window, so the URL never repeats.
            Route::Klines => format!(
                "instrument_id={iid}&interval=1m&start_timestamp={}",
                1_700_000_000_000u64 + n * 60_000
            ),
            Route::Trades => format!(
                "instrument_id={iid}&start_timestamp={}",
                1_700_000_000_000u64 + n * 1_000
            ),
            Route::Portfolio => format!(
                "address={}",
                self.addresses[(n as usize) % self.addresses.len()]
            ),
            Route::Bbo => format!("instrument_id={iid}"),
        };
        format!("{base_url}{}?{query}", route.path())
    }
}

/// Distinct inputs for the probes. The leaderboard walk is only made when a
/// portfolio run needs addresses; the other routes vary timestamps or
/// instruments.
async fn load_probes(perps: &Perps, addresses: usize, need_addresses: bool) -> Probes {
    let instruments = perps
        .exchange()
        .instruments()
        .send()
        .await
        .expect("instruments")
        .into_iter()
        .map(|i| i.instrument_id)
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    let mut offset = 0u64;
    while need_addresses && found.len() < addresses {
        let page = perps
            .public()
            .leaderboard()
            .window(polyoxide_perps::types::LeaderboardWindow::Month)
            .limit(100)
            .offset(offset)
            .send()
            .await
            .expect("leaderboard");
        if page.entries.is_empty() {
            break;
        }
        offset += page.entries.len() as u64;
        found.extend(page.entries.into_iter().map(|e| e.account));
    }
    assert!(
        !instruments.is_empty(),
        "no instruments listed: nothing to probe"
    );
    assert!(
        !need_addresses || !found.is_empty(),
        "leaderboard is empty: no addresses for portfolio probes"
    );
    Probes {
        instruments,
        addresses: found,
        counter: AtomicU64::new(0),
    }
}

// ── Verdicts (pure, unit-tested) ────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Reply {
    Ok,
    CacheHit,
    Throttled {
        code: String,
        retry_after: Option<u64>,
    },
    Error(u16),
}

fn classify(status: u16, x_cache: Option<&str>, retry_after: Option<&str>, body: &str) -> Reply {
    if status == 429 {
        let code = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("error").and_then(|c| c.as_str()).map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned());
        return Reply::Throttled {
            code,
            retry_after: retry_after.and_then(|v| v.trim().parse().ok()),
        };
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
enum Verdict {
    Clean,
    Throttled {
        after: Duration,
        code: String,
    },
    Saturated {
        origin_share: f64,
    },
    Invalid {
        errors: usize,
    },
    /// The harness did not reach its own target rate, so a clean result
    /// would be about a lower rate than the one it is labelled with.
    UnderDriven {
        achieved: f64,
    },
}

/// Share of the target rate a stage must actually achieve for a clean
/// verdict to mean anything.
const MIN_ACHIEVED_SHARE: f64 = 0.9;

fn judge(replies: &[(Duration, Reply)], rate: f64, secs: u64) -> Verdict {
    if let Some((at, Reply::Throttled { code, .. })) = replies
        .iter()
        .find(|(_, r)| matches!(r, Reply::Throttled { .. }))
    {
        return Verdict::Throttled {
            after: *at,
            code: code.clone(),
        };
    }
    let errors = replies
        .iter()
        .filter(|(_, r)| matches!(r, Reply::Error(_)))
        .count();
    if errors > 0 {
        return Verdict::Invalid { errors };
    }
    let achieved = replies.len() as f64 / secs as f64;
    if achieved < MIN_ACHIEVED_SHARE * rate {
        return Verdict::UnderDriven { achieved };
    }
    let origin = replies.iter().filter(|(_, r)| *r == Reply::Ok).count();
    let share = if replies.is_empty() {
        0.0
    } else {
        origin as f64 / replies.len() as f64
    };
    if share < MIN_ORIGIN_SHARE {
        return Verdict::Saturated {
            origin_share: share,
        };
    }
    Verdict::Clean
}

/// One line of reply counts for a stage, plus the first throttle's
/// `Retry-After`, which `OBSERVED.md` records alongside the 429 body.
fn summarize(replies: &[(Duration, Reply)]) -> String {
    let count = |f: &dyn Fn(&Reply) -> bool| replies.iter().filter(|(_, r)| f(r)).count();
    let ok = count(&|r| *r == Reply::Ok);
    let cached = count(&|r| *r == Reply::CacheHit);
    let throttled = count(&|r| matches!(r, Reply::Throttled { .. }));
    let errors = count(&|r| matches!(r, Reply::Error(_)));
    let retry_after = replies.iter().find_map(|(_, r)| match r {
        Reply::Throttled { retry_after, .. } => *retry_after,
        _ => None,
    });
    let last = replies.last().map_or(Duration::ZERO, |(at, _)| *at);
    format!(
        "{ok} origin, {cached} cached, {throttled} throttled, {errors} errors; \
         last reply at {last:.1?}; first Retry-After {retry_after:?}"
    )
}

/// The count to pin for a 10-second window: the highest clean stage rate.
fn pin(stages: &[(f64, Verdict)]) -> Option<u32> {
    stages
        .iter()
        .take_while(|(_, v)| *v == Verdict::Clean)
        .last()
        .map(|(rate, _)| (rate * 10.0).floor() as u32)
}

// ── Pacing ──────────────────────────────────────────────────────

struct Pacer {
    interval: Duration,
    next: Mutex<Option<Instant>>,
}

impl Pacer {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            next: Mutex::new(None),
        }
    }

    /// The next slot, never in the past: an idle pacer must not bank credit
    /// and release it as a burst.
    fn reserve(&self, now: Instant) -> Instant {
        let mut next = self.next.lock().unwrap();
        let slot = next.map_or(now, |claimed| claimed.max(now));
        *next = Some(slot + self.interval);
        slot
    }

    async fn wait(&self) {
        let slot = self.reserve(Instant::now());
        tokio::time::sleep_until(tokio::time::Instant::from_std(slot)).await;
    }
}

// ── Throttle observer for validation mode ───────────────────────

#[derive(Default)]
struct MessageVisitor(Option<String>);

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

/// Retried-away 429s per request path, read off the retry loop's WARN
/// message (`Retriable status 429 Too Many Requests on /v1/info/trades, …`),
/// so a mixed run says which route was refused.
struct ThrottleLayer(Arc<Mutex<BTreeMap<String, u64>>>);

fn throttled_path(message: &str) -> Option<&str> {
    let rest = message.split(" on ").nth(1)?;
    Some(rest.split(',').next()?.trim())
}

impl<S: tracing::Subscriber> Layer<S> for ThrottleLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let meta = event.metadata();
        if !meta.target().starts_with("polyoxide_core") || *meta.level() != tracing::Level::WARN {
            return;
        }
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        if let Some(message) = visitor.0.filter(|m| m.contains("Retriable status 429")) {
            let path = throttled_path(&message).unwrap_or("?").to_owned();
            *self.0.lock().unwrap().entry(path).or_insert(0) += 1;
        }
    }
}

// ── Drivers ─────────────────────────────────────────────────────

async fn ramp_stage(
    client: &reqwest::Client,
    probes: &Probes,
    route: Route,
    base_url: &str,
    rate: f64,
    secs: u64,
    concurrency: usize,
) -> Vec<(Duration, Reply)> {
    let pacer = Arc::new(Pacer::new(Duration::from_secs_f64(1.0 / rate)));
    let replies = Arc::new(Mutex::new(Vec::new()));
    let start = Instant::now();
    let deadline = start + Duration::from_secs(secs);
    let mut workers = Vec::new();
    for _ in 0..concurrency {
        let client = client.clone();
        let pacer = Arc::clone(&pacer);
        let replies = Arc::clone(&replies);
        let urls: Vec<String> = (0..(rate * secs as f64 / concurrency as f64).ceil() as u64 + 1)
            .map(|_| probes.next(route, base_url))
            .collect();
        workers.push(tokio::spawn(async move {
            for url in urls {
                if Instant::now() >= deadline {
                    break;
                }
                pacer.wait().await;
                let response = client.get(&url).send().await;
                let reply = match response {
                    Ok(r) => {
                        let status = r.status().as_u16();
                        let x_cache = r
                            .headers()
                            .get("x-cache")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned);
                        let retry_after = r
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned);
                        let body = r.text().await.unwrap_or_default();
                        classify(status, x_cache.as_deref(), retry_after.as_deref(), &body)
                    }
                    Err(_) => Reply::Error(0),
                };
                let stop = matches!(reply, Reply::Throttled { .. });
                replies.lock().unwrap().push((start.elapsed(), reply));
                if stop {
                    break;
                }
            }
        }));
    }
    for w in workers {
        let _ = w.await;
    }
    let mut out = replies.lock().unwrap().clone();
    out.sort_by_key(|(at, _)| *at);
    out
}

async fn run_ramp(cfg: &Config, route: Route) -> ExitCode {
    let perps = Perps::builder().base_url(&cfg.base_url).build().unwrap();
    let probes = load_probes(&perps, cfg.addresses, route == Route::Portfolio).await;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    let mut stages = Vec::new();
    for &rate in &cfg.stages {
        eprintln!(
            "== {} at {rate:.1} req/s for {}s",
            route.name(),
            cfg.stage_secs
        );
        let replies = ramp_stage(
            &client,
            &probes,
            route,
            &cfg.base_url,
            rate,
            cfg.stage_secs,
            cfg.concurrency,
        )
        .await;
        let verdict = judge(&replies, rate, cfg.stage_secs);
        eprintln!("   {} replies, verdict {verdict:?}", replies.len());
        eprintln!("   {}", summarize(&replies));
        let stop = verdict != Verdict::Clean;
        stages.push((rate, verdict));
        if stop {
            break;
        }
        eprintln!("   cooling down {}s", cfg.cooldown_secs);
        tokio::time::sleep(Duration::from_secs(cfg.cooldown_secs)).await;
    }
    match pin(&stages) {
        Some(count) => {
            println!("{}: pin {count} per 10s ({stages:?})", route.name());
            ExitCode::SUCCESS
        }
        None => {
            println!(
                "{}: even the first stage was not clean ({stages:?})",
                route.name()
            );
            ExitCode::from(1)
        }
    }
}

async fn run_validation(cfg: &Config) -> ExitCode {
    let throttles = Arc::new(Mutex::new(BTreeMap::new()));
    tracing_subscriber::registry()
        .with(ThrottleLayer(Arc::clone(&throttles)))
        .init();
    // The shipped client, untouched: its own limiter and its own default
    // concurrency cap, which bounds in-flight requests for the whole run.
    let perps = Perps::builder().base_url(&cfg.base_url).build().unwrap();
    let needs_addresses = cfg.routes.contains(&Route::Portfolio);
    let probes = Arc::new(load_probes(&perps, cfg.addresses, needs_addresses).await);
    let deadline = Instant::now() + Duration::from_secs(cfg.stage_secs);
    let sent = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let mut workers = Vec::new();
    for route in &cfg.routes {
        for _ in 0..cfg.concurrency {
            let perps = perps.clone();
            let probes = Arc::clone(&probes);
            let sent = Arc::clone(&sent);
            let failed = Arc::clone(&failed);
            let route = *route;
            workers.push(tokio::spawn(async move {
                while Instant::now() < deadline {
                    let n = probes.counter.fetch_add(1, Ordering::Relaxed);
                    let iid = probes.instruments[(n as usize) % probes.instruments.len()];
                    let result = match route {
                        Route::Klines => perps
                            .market()
                            .klines(
                                iid,
                                polyoxide_perps::types::Interval::M1,
                                1_700_000_000_000 + n * 60_000,
                            )
                            .send()
                            .await
                            .map(|_| ()),
                        Route::Trades => perps
                            .market()
                            .trades(iid)
                            .start(1_700_000_000_000 + n * 1_000)
                            .send()
                            .await
                            .map(|_| ()),
                        Route::Portfolio => perps
                            .public()
                            .portfolio(&probes.addresses[(n as usize) % probes.addresses.len()])
                            .send()
                            .await
                            .map(|_| ()),
                        Route::Bbo => perps
                            .market()
                            .bbo()
                            .instrument_id(iid)
                            .send()
                            .await
                            .map(|_| ()),
                    };
                    sent.fetch_add(1, Ordering::Relaxed);
                    if let Err(e) = result {
                        failed.fetch_add(1, Ordering::Relaxed);
                        eprintln!("{}: {e}", route.name());
                    }
                }
            }));
        }
    }
    for w in workers {
        let _ = w.await;
    }
    let sent = sent.load(Ordering::Relaxed);
    let failed = failed.load(Ordering::Relaxed);
    let throttles = throttles.lock().unwrap();
    let throttled: u64 = throttles.values().sum();
    // `sent` counts calls: a call the retry loop re-sends is several
    // requests on the wire but one here; the throttle counts are per request.
    println!(
        "validation: {sent} calls over {}s, {throttled} throttled, {failed} failed",
        cfg.stage_secs
    );
    for (path, count) in throttles.iter() {
        println!("  {path}: {count} throttled");
    }
    if throttled == 0 && failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

// ── Configuration ───────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Config {
    routes: Vec<Route>,
    stages: Vec<f64>,
    client_paced: bool,
    stage_secs: u64,
    cooldown_secs: u64,
    concurrency: usize,
    addresses: usize,
    base_url: String,
}

const USAGE: &str = "\
Measure or validate the rate limit for public Perps routes.

Usage: info_soak --route <routes> [options]

  --route <routes>        klines | trades | portfolio | bbo. A ramp takes one;
                          --pace client takes a comma list or `all`
  --stages <r1,r2,...>    Ramp rates in req/s, ascending, at most 40
                          (default: 5,10,15,20,30)
  --pace client           Validate instead: pace by the shipped limiter and
                          require zero 429s
  --stage-secs <n>        Seconds per stage (default: 60 ramp, 120 validation)
  --cooldown-secs <n>     Idle seconds between ramp stages (default: 120)
  --concurrency <n>       Ramp: workers per route (default 8). Validation: tasks
                          per route (default 4); in-flight requests are capped
                          by the client's own default of 4 overall
  --addresses <n>         Leaderboard addresses to draw portfolio probes from (default: 500)
  --base-url <url>        Override the host
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
    if stages.is_empty()
        || stages
            .iter()
            .any(|r| !r.is_finite() || *r <= 0.0 || *r > CEILING_RPS)
    {
        return Err(format!("stages must be positive and at most {CEILING_RPS}"));
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
        let mut client_paced = false;
        let mut stage_secs = None;
        let mut cooldown_secs = 120;
        let mut concurrency = None;
        let mut addresses = 500;
        let mut base_url = DEFAULT_BASE_URL.to_owned();
        let mut args = args.peekable();
        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| format!("{flag} requires a value"))
            };
            match flag.as_str() {
                "-h" | "--help" => return Ok(None),
                "--route" => routes = Some(parse_routes(&value()?)?),
                "--stages" => stages = Some(parse_stages(&value()?)?),
                "--pace" => match value()?.as_str() {
                    "client" => client_paced = true,
                    other => return Err(format!("--pace takes only `client`, got {other:?}")),
                },
                "--stage-secs" => {
                    stage_secs = Some(value()?.parse().map_err(|_| "bad --stage-secs")?)
                }
                "--cooldown-secs" => {
                    cooldown_secs = value()?.parse().map_err(|_| "bad --cooldown-secs")?
                }
                "--concurrency" => {
                    concurrency = Some(value()?.parse().map_err(|_| "bad --concurrency")?)
                }
                "--addresses" => addresses = value()?.parse().map_err(|_| "bad --addresses")?,
                "--base-url" => base_url = value()?.trim_end_matches('/').to_owned(),
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        let routes = routes.ok_or("--route is required")?;
        if concurrency == Some(0) || stage_secs == Some(0) {
            return Err("--concurrency and --stage-secs must be at least 1".into());
        }
        if !client_paced && routes.len() != 1 {
            return Err(
                "a ramp measures one route at a time; list several only with --pace client".into(),
            );
        }
        Ok(Some(Self {
            routes,
            stages: stages.unwrap_or_else(|| DEFAULT_STAGES.to_vec()),
            client_paced,
            stage_secs: stage_secs.unwrap_or(if client_paced { 120 } else { 60 }),
            cooldown_secs,
            concurrency: concurrency.unwrap_or(if client_paced { 4 } else { 8 }),
            addresses,
            base_url,
        }))
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cfg = match Config::from_args(std::env::args().skip(1)) {
        Ok(Some(cfg)) => cfg,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if cfg.client_paced {
        run_validation(&cfg).await
    } else {
        run_ramp(&cfg, cfg.routes[0]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(at: u64) -> (Duration, Reply) {
        (Duration::from_secs(at), Reply::Ok)
    }

    #[test]
    fn a_429_is_throttled_with_its_identifier_and_retry_after() {
        assert_eq!(
            classify(
                429,
                None,
                Some("2"),
                r#"{"status":"err","error":"ip_rate_limited"}"#
            ),
            Reply::Throttled {
                code: "ip_rate_limited".into(),
                retry_after: Some(2)
            }
        );
    }

    #[test]
    fn a_cache_hit_is_not_an_origin_reply() {
        assert_eq!(
            classify(200, Some("Hit from cloudfront"), None, "[]"),
            Reply::CacheHit
        );
        assert_eq!(
            classify(200, Some("Miss from cloudfront"), None, "[]"),
            Reply::Ok
        );
    }

    #[test]
    fn a_stage_with_any_throttle_is_throttled_at_its_first() {
        let replies = vec![
            ok(1),
            (
                Duration::from_secs(7),
                Reply::Throttled {
                    code: "ip_rate_limited".into(),
                    retry_after: None,
                },
            ),
            ok(9),
        ];
        assert_eq!(
            judge(&replies, 1.0, 10),
            Verdict::Throttled {
                after: Duration::from_secs(7),
                code: "ip_rate_limited".into()
            }
        );
    }

    #[test]
    fn a_stage_mostly_served_from_cache_is_saturated_not_clean() {
        let replies: Vec<_> = (0..10)
            .map(|i| {
                if i < 5 {
                    ok(i)
                } else {
                    (Duration::from_secs(i), Reply::CacheHit)
                }
            })
            .collect();
        assert_eq!(
            judge(&replies, 1.0, 10),
            Verdict::Saturated { origin_share: 0.5 }
        );
    }

    #[test]
    fn a_stage_that_did_not_reach_its_rate_is_under_driven_not_clean() {
        // 5 replies in 10 s at a 1 req/s target is half the rate: a clean
        // verdict here would pin a number the host was never asked for.
        let replies: Vec<_> = (0..5).map(ok).collect();
        assert_eq!(
            judge(&replies, 1.0, 10),
            Verdict::UnderDriven { achieved: 0.5 }
        );
        let replies: Vec<_> = (0..10).map(ok).collect();
        assert_eq!(judge(&replies, 1.0, 10), Verdict::Clean);
    }

    #[test]
    fn pin_is_the_last_clean_stage_before_the_first_unclean_one() {
        let stages = vec![
            (5.0, Verdict::Clean),
            (10.0, Verdict::Clean),
            (
                15.0,
                Verdict::Throttled {
                    after: Duration::from_secs(3),
                    code: "x".into(),
                },
            ),
            (20.0, Verdict::Clean),
        ];
        assert_eq!(pin(&stages), Some(100));
        assert_eq!(pin(&[(5.0, Verdict::Invalid { errors: 1 })]), None);
    }

    #[test]
    fn pacer_never_hands_out_a_slot_in_the_past() {
        let pacer = Pacer::new(Duration::from_millis(10));
        let start = Instant::now();
        pacer.reserve(start);
        let later = start + Duration::from_secs(10);
        assert_eq!(pacer.reserve(later), later);
        assert_eq!(pacer.reserve(later), later + Duration::from_millis(10));
    }

    #[test]
    fn a_ramp_takes_exactly_one_route() {
        let err = Config::from_args(["--route", "klines,trades"].map(String::from).into_iter())
            .unwrap_err();
        assert!(err.contains("one route at a time"));
        let cfg = Config::from_args(
            ["--route", "all", "--pace", "client"]
                .map(String::from)
                .into_iter(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(cfg.routes.len(), 4);
        assert_eq!(cfg.stage_secs, 120);
    }

    #[test]
    fn zero_concurrency_or_stage_seconds_is_refused() {
        // A validation with no workers reports "0 throttled" and exits 0: a
        // vacuous pass.
        let err = Config::from_args(
            ["--route", "all", "--pace", "client", "--concurrency", "0"]
                .map(String::from)
                .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("at least 1"));
    }

    #[test]
    fn the_throttled_path_is_read_off_the_retry_loop_message() {
        assert_eq!(
            throttled_path(
                "Retriable status 429 Too Many Requests on /v1/info/trades, retry 1 after 500ms"
            ),
            Some("/v1/info/trades")
        );
        assert_eq!(throttled_path("no path here"), None);
    }

    #[test]
    fn stages_must_ascend_and_stay_under_the_ceiling() {
        assert!(parse_stages("10,5").is_err());
        assert!(parse_stages("10,50").is_err());
        assert_eq!(parse_stages("5,10").unwrap(), vec![5.0, 10.0]);
    }
}
