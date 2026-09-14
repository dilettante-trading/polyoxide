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
use probes::{is_market_condition_id, parse_routes, Pools, ProbeSource, Route, SeenUrls};
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
            if let Some(condition) = row["condition_id"]
                .as_str()
                .filter(|id| is_market_condition_id(id))
            {
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
