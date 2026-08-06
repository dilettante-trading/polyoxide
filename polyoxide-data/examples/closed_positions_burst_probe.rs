//! Brackets the real burst ceiling for `GET /closed-positions`.
//!
//! `closed_positions_soak` established that 150 requests fired in ~0.75s trips
//! Cloudflare's `error code: 1015`, and that a single request does not.
//! Everything between was unmeasured, which leaves any change to
//! `RateLimiter::data_default`'s `allow_burst(150)` a guess. This binary
//! searches that interval.
//!
//! ```sh
//! cargo run --release -p polyoxide-data --example closed_positions_burst_probe
//! ```
//!
//! # What it found
//!
//! Every one-shot burst up to 140 was clean, and a separate one-shot of
//! exactly 150 in 0.7s was clean too — while the sustained soak tripped after
//! 152 completions. Upstream's published `150 / 10s` is therefore accurate in
//! both count and window; the client's fault is arithmetic. `allow_burst(150)`
//! on a 150-per-10s bucket starts full *and* refills at 15/s, so the first
//! 10-second window permits 150 + 150 = 300 requests, twice the quota. Later
//! windows are correct, because by then the bucket is empty.
//!
//! # One trial
//!
//! A trial fires N requests as fast as its concurrency allows, from a **fresh
//! client**. That freshness is load-bearing: a reused client carries a drained
//! bucket from the previous trial, so its own limiter would pace the requests
//! and the run would measure polyoxide instead of Cloudflare. It is also why
//! N is capped at the burst allowance — above it the client starts pacing, and
//! the trial silently stops being a burst.
//!
//! A trial aborts the instant upstream pushes back, and records how many
//! requests had completed at that moment. That count is the most direct
//! evidence available: it is an upper bound on the ceiling from a single
//! trial, independent of where the search happens to be.
//!
//! # Between trials
//!
//! Trials are separated by a gap longer than the published 10s window, because
//! a trial that lands inside the previous one's window measures their sum.
//! After a throttled trial the gap is longer still — traffic during a 1015
//! prolongs it. Results are checked for monotonicity afterwards: a small burst
//! failing after a larger one passed means the gap was too short for the
//! server's real window, and the bracket cannot be trusted.

use std::{
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use polyoxide_data::DataApi;

#[path = "common/mod.rs"]
mod common;
use common::{
    install_observer, percentile, ThrottleObserver, DEFAULT_CONCURRENCY, DEFAULT_USER,
    MAX_PAGE_LIMIT,
};

/// `RateLimiter::data_default` calls `allow_burst(150)`. Trials above this
/// would be paced by the client's own limiter rather than fired as a burst.
const CLIENT_BURST_ALLOWANCE: u32 = 150;

#[derive(Debug, Clone)]
struct Config {
    low: u32,
    high: u32,
    tolerance: u32,
    concurrency: usize,
    user: String,
    limit: u32,
    base_url: Option<String>,
    gap: Duration,
    recovery: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Known clean and known throttled respectively, from the soak.
            low: 1,
            high: CLIENT_BURST_ALLOWANCE,
            tolerance: 10,
            concurrency: DEFAULT_CONCURRENCY,
            user: DEFAULT_USER.to_owned(),
            limit: MAX_PAGE_LIMIT,
            base_url: None,
            // 3x the published 10s window.
            gap: Duration::from_secs(30),
            recovery: Duration::from_secs(90),
        }
    }
}

const USAGE: &str = "\
Brackets the burst ceiling for GET /closed-positions by binary search.

Usage: closed_positions_burst_probe [options]

  --low <n>            Burst size assumed clean (default: 1)
  --high <n>           Burst size assumed throttled (default: 150)
  --tolerance <n>      Stop when the bracket is this wide (default: 10)
  --concurrency <n>    In-flight requests per trial (default: 4)
  --user <address>     Address to query (default: the repo's probe address)
  --limit <n>          Results per request, 0-50 (default: 50)
  --base-url <url>     Override the Data API host
  --gap <secs>         Wait between trials (default: 30)
  --recovery <secs>    Wait after a throttled trial (default: 90)
  -h, --help           Show this message

Exits 0 if a trustworthy bracket was found, 1 otherwise.";

impl Config {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Option<Self>, String> {
        let mut config = Config::default();
        let mut args = args.peekable();

        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| format!("{flag} requires a value"))
            };
            let number = |raw: String| -> Result<u64, String> {
                raw.parse().map_err(|_| format!("bad {flag}: {raw}"))
            };

            match flag.as_str() {
                "-h" | "--help" => return Ok(None),
                "--low" => config.low = number(value()?)? as u32,
                "--high" => config.high = number(value()?)? as u32,
                "--tolerance" => config.tolerance = number(value()?)? as u32,
                "--concurrency" => config.concurrency = number(value()?)? as usize,
                "--user" => config.user = value()?,
                "--limit" => config.limit = number(value()?)? as u32,
                "--base-url" => config.base_url = Some(value()?),
                "--gap" => config.gap = Duration::from_secs(number(value()?)?),
                "--recovery" => config.recovery = Duration::from_secs(number(value()?)?),
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        config.validate()?;
        Ok(Some(config))
    }

    fn validate(&self) -> Result<(), String> {
        if self.concurrency == 0 {
            return Err("--concurrency must be greater than zero".into());
        }
        if self.limit > MAX_PAGE_LIMIT {
            return Err(format!(
                "--limit above {MAX_PAGE_LIMIT} is rejected by the server"
            ));
        }
        if self.low == 0 {
            return Err("--low must be at least 1".into());
        }
        if self.high <= self.low {
            return Err("--high must exceed --low".into());
        }
        // Above the client's own burst allowance the limiter paces the
        // requests, so the trial stops being a burst and measures polyoxide
        // rather than upstream.
        if self.high > CLIENT_BURST_ALLOWANCE {
            return Err(format!(
                "--high above {CLIENT_BURST_ALLOWANCE} would be paced by the client's own limiter, \
                 so the trial would no longer be a burst"
            ));
        }
        if self.tolerance == 0 {
            return Err("--tolerance must be greater than zero".into());
        }
        Ok(())
    }

    fn build_client(&self) -> Result<DataApi, String> {
        let mut builder = DataApi::builder().max_concurrent(self.concurrency);
        if let Some(url) = &self.base_url {
            builder = builder.base_url(url.clone());
        }
        builder.build().map_err(|e| e.to_string())
    }
}

// ── Trials ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Trial {
    burst: u32,
    completed: u32,
    failed: u32,
    throttles: u64,
    /// Requests that had completed when the first 429 was seen. An upper
    /// bound on the ceiling, from this trial alone.
    completed_at_trip: Option<u32>,
    elapsed: Duration,
    p50: Duration,
}

impl Trial {
    fn throttled(&self) -> bool {
        self.throttles > 0
    }
}

/// Fires `burst` requests from a fresh client and reports what upstream did.
async fn run_trial(config: &Config, burst: u32, observer: &Arc<ThrottleObserver>) -> Trial {
    // Each trial gets its own verdict, not a running total across the ramp.
    observer.reset();
    let client = match config.build_client() {
        Ok(client) => client,
        Err(e) => {
            eprintln!("  could not build client: {e}");
            return Trial {
                burst,
                completed: 0,
                failed: 0,
                throttles: 0,
                completed_at_trip: None,
                elapsed: Duration::ZERO,
                p50: Duration::ZERO,
            };
        }
    };

    let start = Instant::now();
    let issued = Arc::new(AtomicU64::new(0));
    let completed = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let trip_at = Arc::new(AtomicU64::new(u64::MAX));
    let abort = Arc::new(AtomicBool::new(false));
    let latencies: Arc<Mutex<Vec<Duration>>> = Arc::new(Mutex::new(Vec::new()));

    let mut workers = tokio::task::JoinSet::new();
    for _ in 0..config.concurrency {
        let client = client.clone();
        let observer = Arc::clone(observer);
        let issued = Arc::clone(&issued);
        let completed = Arc::clone(&completed);
        let failed = Arc::clone(&failed);
        let trip_at = Arc::clone(&trip_at);
        let abort = Arc::clone(&abort);
        let latencies = Arc::clone(&latencies);
        let user = config.user.clone();
        let limit = config.limit;

        workers.spawn(async move {
            while !abort.load(Ordering::Relaxed)
                && issued.fetch_add(1, Ordering::Relaxed) < u64::from(burst)
            {
                let sent = Instant::now();
                let result = client
                    .user(&user)
                    .closed_positions()
                    .limit(limit)
                    .send()
                    .await;
                let latency = sent.elapsed();

                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                if result.is_err() {
                    failed.fetch_add(1, Ordering::Relaxed);
                }
                if let Ok(mut latencies) = latencies.lock() {
                    latencies.push(latency);
                }

                if observer.throttled() {
                    // First worker to notice pins the completion count; the
                    // rest leave it alone.
                    let _ = trip_at.compare_exchange(
                        u64::MAX,
                        done,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    );
                    abort.store(true, Ordering::Relaxed);
                    break;
                }
            }
        });
    }
    while workers.join_next().await.is_some() {}

    let mut latencies = match latencies.lock() {
        Ok(mut collected) => std::mem::take(&mut *collected),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    };
    latencies.sort_unstable();

    Trial {
        burst,
        completed: completed.load(Ordering::Relaxed) as u32,
        failed: failed.load(Ordering::Relaxed) as u32,
        throttles: observer.throttle_count(),
        completed_at_trip: match trip_at.load(Ordering::Relaxed) {
            u64::MAX => None,
            n => Some(n as u32),
        },
        elapsed: start.elapsed(),
        p50: percentile(&latencies, 50.0),
    }
}

async fn wait(label: &str, duration: Duration) {
    if duration.is_zero() {
        return;
    }
    println!("  {label} {}s …", duration.as_secs());
    tokio::time::sleep(duration).await;
}

// ── Search ──────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct Bracket {
    largest_clean: Option<u32>,
    smallest_throttled: Option<u32>,
}

impl Bracket {
    fn observe(&mut self, trial: &Trial) {
        if trial.throttled() {
            self.smallest_throttled = Some(match self.smallest_throttled {
                Some(current) => current.min(trial.burst),
                None => trial.burst,
            });
        } else {
            self.largest_clean = Some(match self.largest_clean {
                Some(current) => current.max(trial.burst),
                None => trial.burst,
            });
        }
    }

    /// A clean burst at or above a throttled one means the trials were not
    /// independent — the gap was shorter than the server's real window, so
    /// each trial counted some of its predecessor's requests.
    fn is_consistent(&self) -> bool {
        match (self.largest_clean, self.smallest_throttled) {
            (Some(clean), Some(throttled)) => clean < throttled,
            _ => true,
        }
    }
}

async fn search(config: &Config, observer: &Arc<ThrottleObserver>) -> (Vec<Trial>, Bracket) {
    let mut low = config.low;
    let mut high = config.high;
    let mut trials = Vec::new();
    let mut bracket = Bracket::default();
    let mut first = true;

    while high - low > config.tolerance {
        let burst = low + (high - low) / 2;

        if !first {
            let throttled_last = trials.last().is_some_and(Trial::throttled);
            let (label, delay) = if throttled_last {
                ("recovering", config.recovery)
            } else {
                ("waiting", config.gap)
            };
            wait(label, delay).await;
        }
        first = false;

        println!("  trial: burst of {burst} …");
        let trial = run_trial(config, burst, observer).await;
        println!(
            "         {} completed in {:.2}s, p50 {:.0}ms, {} throttles{}",
            trial.completed,
            trial.elapsed.as_secs_f64(),
            trial.p50.as_secs_f64() * 1000.0,
            trial.throttles,
            match trial.completed_at_trip {
                Some(at) => format!(" (first 429 after {at} completions)"),
                None => String::new(),
            }
        );

        bracket.observe(&trial);
        if trial.throttled() {
            high = burst;
        } else {
            low = burst;
        }
        trials.push(trial);
    }

    (trials, bracket)
}

// ── Reporting ───────────────────────────────────────────────────

fn report(config: &Config, trials: &[Trial], bracket: &Bracket) {
    println!("\n══ /closed-positions burst ceiling ═════════════════════");
    println!(
        "  target        {}",
        config
            .base_url
            .as_deref()
            .unwrap_or("data-api.polymarket.com")
    );
    println!(
        "  searched      [{}, {}] at concurrency {}, tolerance {}",
        config.low, config.high, config.concurrency, config.tolerance
    );

    println!("\n── Trials ──────────────────────────────────────────────");
    println!("  burst  completed  failed  throttles  trip@  verdict");
    for trial in trials {
        println!(
            "  {:>5}  {:>9}  {:>6}  {:>9}  {:>5}  {}",
            trial.burst,
            trial.completed,
            trial.failed,
            trial.throttles,
            match trial.completed_at_trip {
                Some(at) => at.to_string(),
                None => "—".to_owned(),
            },
            if trial.throttled() {
                "THROTTLED"
            } else {
                "clean"
            }
        );
    }

    println!("\n── Bracket ─────────────────────────────────────────────");
    match (bracket.largest_clean, bracket.smallest_throttled) {
        (Some(clean), Some(throttled)) => {
            println!("  largest clean burst      {clean}");
            println!("  smallest throttled burst {throttled}");
            println!("  ceiling lies in          ({clean}, {throttled}]");
        }
        (Some(clean), None) => {
            println!("  no burst up to {clean} was throttled — ceiling is above the search range");
        }
        (None, Some(throttled)) => {
            println!("  every trial throttled, down to {throttled} — ceiling is below the range");
        }
        (None, None) => println!("  no trials ran"),
    }

    // The tightest single-trial evidence: a trip after k completions bounds
    // the ceiling at k regardless of where the search landed.
    if let Some(tightest) = trials.iter().filter_map(|t| t.completed_at_trip).min() {
        println!("  tightest trip observed   after {tightest} completions");
    }

    println!("\n  client currently allows a burst of {CLIENT_BURST_ALLOWANCE}");

    if !bracket.is_consistent() {
        println!(
            "\n  INCONSISTENT: a clean burst at or above a throttled one means the\n  \
             gap between trials was shorter than the server's real window, so\n  \
             trials were not independent. Re-run with a larger --gap."
        );
    }
}

// ── Entry point ─────────────────────────────────────────────────

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
            return ExitCode::FAILURE;
        }
    };

    let observer = install_observer(Instant::now());
    println!(
        "bracketing the burst ceiling in [{}, {}] …",
        config.low, config.high
    );

    let (trials, bracket) = search(&config, &observer).await;
    report(&config, &trials, &bracket);

    let trustworthy = bracket.is_consistent()
        && bracket.largest_clean.is_some()
        && bracket.smallest_throttled.is_some();
    println!();
    if trustworthy {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial(burst: u32, throttles: u64) -> Trial {
        Trial {
            burst,
            completed: burst,
            failed: 0,
            throttles,
            completed_at_trip: None,
            elapsed: Duration::ZERO,
            p50: Duration::ZERO,
        }
    }

    // ── Bracket ─────────────────────────────────────────────────

    #[test]
    fn bracket_narrows_from_both_sides() {
        let mut bracket = Bracket::default();
        bracket.observe(&trial(75, 0));
        bracket.observe(&trial(112, 3));
        bracket.observe(&trial(93, 0));
        bracket.observe(&trial(103, 2));

        assert_eq!(bracket.largest_clean, Some(93));
        assert_eq!(bracket.smallest_throttled, Some(103));
        assert!(bracket.is_consistent());
    }

    #[test]
    fn bracket_flags_a_clean_run_above_a_throttled_one() {
        // Trials contaminating each other looks exactly like this, and it
        // must not be reported as a valid bracket.
        let mut bracket = Bracket::default();
        bracket.observe(&trial(60, 4));
        bracket.observe(&trial(90, 0));

        assert!(!bracket.is_consistent());
    }

    #[test]
    fn bracket_with_one_sided_evidence_is_not_contradictory() {
        let mut bracket = Bracket::default();
        bracket.observe(&trial(50, 0));
        assert!(bracket.is_consistent());
        assert_eq!(bracket.smallest_throttled, None);
    }

    // ── Config ──────────────────────────────────────────────────

    fn parse(args: &[&str]) -> Result<Option<Config>, String> {
        Config::from_args(args.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn defaults_search_the_interval_the_soak_left_open() {
        let config = parse(&[]).unwrap().unwrap();
        assert_eq!(config.low, 1);
        assert_eq!(config.high, CLIENT_BURST_ALLOWANCE);
        assert!(
            config.gap > Duration::from_secs(10),
            "gap must exceed the published window"
        );
    }

    #[test]
    fn high_above_the_client_burst_allowance_is_rejected() {
        // Beyond it the client's own limiter paces the trial, so it would
        // measure polyoxide rather than Cloudflare.
        let err = parse(&["--high", "200"]).unwrap_err();
        assert!(err.contains("paced by the client"), "got: {err}");
    }

    #[test]
    fn inverted_range_is_rejected() {
        assert!(parse(&["--low", "100", "--high", "50"]).is_err());
    }

    #[test]
    fn zero_tolerance_is_rejected() {
        // Would binary search forever at ±1.
        assert!(parse(&["--tolerance", "0"]).is_err());
    }

    #[test]
    fn limit_above_the_server_ceiling_is_rejected_locally() {
        assert!(parse(&["--limit", "51"]).is_err());
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert!(parse(&["--burst"]).is_err());
    }

    #[test]
    fn help_short_circuits() {
        assert!(parse(&["--help"]).unwrap().is_none());
    }
}
