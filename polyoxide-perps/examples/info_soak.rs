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
//! as saturated rather than clean. The rules are
//! `polyoxide_test_support::soak::verdict::strict`, unit-tested there.

use std::{
    process::ExitCode,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use polyoxide_perps::{types::InstrumentId, Perps};
use polyoxide_test_support::soak::{
    self,
    observe::install_observer,
    verdict::strict::{classify, judge, pin, summarize, Reply, Verdict},
    Pacer,
};

const DEFAULT_BASE_URL: &str = "https://api.perpetuals.polymarket.com";
const DEFAULT_STAGES: [f64; 5] = [5.0, 10.0, 15.0, 20.0, 30.0];
const CEILING_RPS: f64 = 40.0;

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
    soak::parse_routes(raw, &Route::ALL, Route::name)
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
        // The workspace enables reqwest's `gzip` feature for polyoxide-binance;
        // keep this soak's requests as they were measured.
        .gzip(false)
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
    let observer = install_observer(Instant::now());
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
    let throttles = observer.throttles_by_path();
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
    soak::parse_stages(raw, CEILING_RPS)
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
    fn stages_must_ascend_and_stay_under_the_ceiling() {
        assert!(parse_stages("10,5").is_err());
        assert!(parse_stages("10,50").is_err());
        assert_eq!(parse_stages("5,10").unwrap(), vec![5.0, 10.0]);
    }
}
