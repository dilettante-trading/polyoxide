//! Sustained-rate soak test against `GET /closed-positions`.
//!
//! Drives the real `DataApi` client at the rate its own limiter permits and
//! reports whether upstream throttled us. The pass condition is **zero
//! observed 429s**: the client's bucket for this path is modelled at 150
//! requests per 10s (`RateLimiter::data_default`), and this run asks whether
//! that model survives contact with Cloudflare.
//!
//! ```sh
//! cargo run -p polyoxide-data --example closed_positions_soak -- \
//!     --duration 180 --concurrency 4 --user 0xabc...
//! ```
//!
//! Exits 0 when no throttling was seen, 1 otherwise.
//!
//! # Why it aborts on the first 429
//!
//! Cloudflare's `error code: 1015` is an IP-scoped block on the whole host,
//! and traffic *during* the block prolongs it (see
//! `docs/specs/data/rate-limits.md`). Continuing to sample after the first
//! throttle would both corrupt the measurement and extend the ban, so the
//! run stops immediately and reports the single data point it has. A failing
//! run tells you *when* the throttle arrived, not how many followed.
//!
//! # How throttling is detected
//!
//! A 429 the client retries away is invisible to the caller, so detection
//! runs through a counting `tracing` subscriber. See `examples/common/mod.rs`.

use std::{
    fmt::Write as _,
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

const DEFAULT_DURATION_SECS: u64 = 180;

/// The rate the client's own limiter sustains on this path.
///
/// `/closed-positions` is published at 150/10s, but `quota()` reserves a tenth
/// and paces the remainder: `10s / 134`, so 13.4 req/s, and never a burst above
/// it. A `--rate` at or beyond this can never bind — the client would be the
/// slower of the two pacers and the run would report polyoxide's behaviour
/// under the guise of the server's.
///
/// Keep this in step with `RESERVED_FRACTION` in `polyoxide-core`. If the
/// reserve changes and this does not, the guard silently stops guarding.
const CLIENT_SUSTAINED_RATE: f64 = 13.4;

/// Backstop against a misconfigured run: far above the ~15 req/s the limiter
/// should sustain, but low enough to bound a runaway.
const MAX_REQUESTS_PER_SEC: u64 = 100;

// ── Configuration ───────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Config {
    duration: Duration,
    concurrency: usize,
    user: String,
    limit: u32,
    base_url: Option<String>,
    max_requests: u64,
    /// Interval between sends when driving a fixed rate; `None` lets the
    /// client's own limiter set the pace.
    rate: Option<Duration>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(DEFAULT_DURATION_SECS),
            concurrency: DEFAULT_CONCURRENCY,
            user: DEFAULT_USER.to_owned(),
            limit: MAX_PAGE_LIMIT,
            base_url: None,
            max_requests: DEFAULT_DURATION_SECS * MAX_REQUESTS_PER_SEC,
            rate: None,
        }
    }
}

const USAGE: &str = "\
Sustained-rate soak against GET /closed-positions.

Usage: closed_positions_soak [options]

  --duration <secs>     How long to soak (default: 180)
  --concurrency <n>     In-flight requests (default: 4, the client's own default)
  --user <address>      Address to query (default: the repo's probe address)
  --limit <n>           Results per request, 0-50 (default: 50)
  --base-url <url>      Override the Data API host
  --max-requests <n>    Hard cap on total requests (default: duration * 100)
  --rate <req/s>        Drive a fixed rate instead of the client's own pace.
                        Must be below the client's sustained 14.9 req/s or its
                        limiter binds first and the run measures polyoxide.
  -h, --help            Show this message

Exits 0 if no throttling was observed, 1 otherwise.";

impl Config {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Option<Self>, String> {
        let mut config = Config::default();
        let mut duration_set = false;
        let mut max_requests_set = false;
        let mut args = args.peekable();

        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| format!("{flag} requires a value"))
            };

            match flag.as_str() {
                "-h" | "--help" => return Ok(None),
                "--duration" => {
                    let raw = value()?;
                    let secs: u64 = raw.parse().map_err(|_| format!("bad --duration: {raw}"))?;
                    if secs == 0 {
                        return Err("--duration must be greater than zero".into());
                    }
                    config.duration = Duration::from_secs(secs);
                    duration_set = true;
                }
                "--concurrency" => {
                    let raw = value()?;
                    config.concurrency = raw
                        .parse()
                        .map_err(|_| format!("bad --concurrency: {raw}"))?;
                    if config.concurrency == 0 {
                        return Err("--concurrency must be greater than zero".into());
                    }
                }
                "--user" => config.user = value()?,
                "--limit" => {
                    let raw = value()?;
                    config.limit = raw.parse().map_err(|_| format!("bad --limit: {raw}"))?;
                    if config.limit > 50 {
                        return Err("--limit above 50 is rejected by the server".into());
                    }
                }
                "--base-url" => config.base_url = Some(value()?),
                "--rate" => {
                    let raw = value()?;
                    let per_sec: f64 = raw.parse().map_err(|_| format!("bad --rate: {raw}"))?;
                    // `is_finite` is load-bearing: "nan" and "inf" both parse,
                    // and a bare `<= 0.0` lets NaN through to become a
                    // nonsensical interval.
                    if !per_sec.is_finite() || per_sec <= 0.0 {
                        return Err("--rate must be a positive, finite number".into());
                    }
                    if per_sec >= CLIENT_SUSTAINED_RATE {
                        return Err(format!(
                            "--rate {per_sec} is at or above the client's own limiter \
                             ({CLIENT_SUSTAINED_RATE} req/s on this path), which would bind \
                             first — the run would measure polyoxide, not the server"
                        ));
                    }
                    config.rate = Some(Duration::from_secs_f64(1.0 / per_sec));
                }
                "--max-requests" => {
                    let raw = value()?;
                    config.max_requests = raw
                        .parse()
                        .map_err(|_| format!("bad --max-requests: {raw}"))?;
                    max_requests_set = true;
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        // Keep the backstop proportional to a duration the caller chose,
        // unless they pinned it themselves.
        if duration_set && !max_requests_set {
            config.max_requests = config.duration.as_secs() * MAX_REQUESTS_PER_SEC;
        }

        Ok(Some(config))
    }
}

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

// ── Sampling ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Sample {
    /// Elapsed time from run start to this request completing.
    finished_at: Duration,
    latency: Duration,
    error: Option<String>,
}

impl Sample {
    fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

/// Why the soak stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopReason {
    DurationElapsed,
    Throttled,
    RequestCapReached,
}

impl StopReason {
    fn as_str(self) -> &'static str {
        match self {
            StopReason::DurationElapsed => "duration elapsed",
            StopReason::Throttled => "ABORTED — upstream throttled us",
            StopReason::RequestCapReached => "request cap reached",
        }
    }
}

async fn soak(
    config: &Config,
    client: DataApi,
    observer: Arc<ThrottleObserver>,
    start: Instant,
) -> (Vec<Sample>, StopReason, Duration) {
    let deadline = start + config.duration;
    let abort = Arc::new(AtomicBool::new(false));
    let issued = Arc::new(AtomicU64::new(0));
    let samples: Arc<Mutex<Vec<Sample>>> = Arc::new(Mutex::new(Vec::new()));
    let cap_hit = Arc::new(AtomicBool::new(false));
    let pacer = config.rate.map(|interval| Arc::new(Pacer::new(interval)));

    let mut workers = tokio::task::JoinSet::new();
    for _ in 0..config.concurrency {
        let client = client.clone();
        let pacer = pacer.clone();
        let observer = Arc::clone(&observer);
        let abort = Arc::clone(&abort);
        let issued = Arc::clone(&issued);
        let cap_hit = Arc::clone(&cap_hit);
        let samples = Arc::clone(&samples);
        let user = config.user.clone();
        let limit = config.limit;
        let max_requests = config.max_requests;

        workers.spawn(async move {
            loop {
                if abort.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    break;
                }
                if issued.fetch_add(1, Ordering::Relaxed) >= max_requests {
                    cap_hit.store(true, Ordering::Relaxed);
                    break;
                }

                // Hold the caller's rate, if one was asked for. Claiming the
                // slot before re-checking the deadline would leave the fleet
                // sleeping past the end of the run.
                if let Some(pacer) = &pacer {
                    pacer.wait().await;
                    if abort.load(Ordering::Relaxed) || Instant::now() >= deadline {
                        break;
                    }
                }

                let sent = Instant::now();
                let result = client
                    .user(&user)
                    .closed_positions()
                    .limit(limit)
                    .send()
                    .await;
                let finished = Instant::now();

                let sample = Sample {
                    finished_at: finished.duration_since(start),
                    latency: finished.duration_since(sent),
                    error: result.err().map(|e| e.to_string()),
                };
                if let Ok(mut samples) = samples.lock() {
                    samples.push(sample);
                }

                // Stop the whole fleet the moment upstream pushes back:
                // requests issued during a 1015 extend it.
                if observer.throttled() {
                    abort.store(true, Ordering::Relaxed);
                    break;
                }
            }
        });
    }

    // Progress ticker, so a three-minute run isn't a silent one.
    let ticker = {
        let observer = Arc::clone(&observer);
        let abort = Arc::clone(&abort);
        let samples = Arc::clone(&samples);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if abort.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    break;
                }
                let done = samples.lock().map(|s| s.len()).unwrap_or(0);
                let elapsed = start.elapsed().as_secs_f64();
                let rate = if elapsed > 0.0 {
                    done as f64 / elapsed
                } else {
                    0.0
                };
                println!(
                    "  [{elapsed:6.1}s] {done:6} requests  {rate:6.2} req/s  throttles: {}",
                    observer.throttle_count()
                );
            }
        })
    };

    while workers.join_next().await.is_some() {}
    ticker.abort();

    let stop = if observer.throttled() {
        StopReason::Throttled
    } else if cap_hit.load(Ordering::Relaxed) {
        StopReason::RequestCapReached
    } else {
        StopReason::DurationElapsed
    };

    (drain(&samples), stop, start.elapsed())
}

/// Takes every recorded sample out of the shared buffer, ascending by
/// completion time.
///
/// Deliberately locks rather than `Arc::try_unwrap`: the progress ticker holds
/// its own `Arc` clone and `JoinHandle::abort` does not guarantee the task has
/// been dropped by the time we get here, so `try_unwrap` fails intermittently.
/// Recovering from that with `unwrap_or_default` reported a throttled run as
/// `requests 0` — the measurement silently replaced by its own absence.
fn drain(samples: &Mutex<Vec<Sample>>) -> Vec<Sample> {
    let mut drained = match samples.lock() {
        Ok(mut collected) => std::mem::take(&mut *collected),
        // A poisoned mutex means a worker panicked mid-push. The samples
        // already written are still valid and are the only record of what
        // happened, so recover them rather than discarding the run.
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    };
    drained.sort_by_key(|s| s.finished_at);
    drained
}

// ── Reporting ───────────────────────────────────────────────────

/// Completions bucketed into one-second bins by completion time.
///
/// This is the measurement that matters: the client's buckets start full, so
/// the first seconds run several times faster than the sustained rate, and an
/// average over the whole run hides exactly that.
fn per_second_histogram(samples: &[Sample]) -> Vec<usize> {
    let Some(last) = samples.iter().map(|s| s.finished_at.as_secs()).max() else {
        return Vec::new();
    };
    let mut buckets = vec![0usize; last as usize + 1];
    for sample in samples {
        buckets[sample.finished_at.as_secs() as usize] += 1;
    }
    buckets
}

fn render_histogram(buckets: &[usize], bar_width: usize) -> String {
    let peak = buckets.iter().copied().max().unwrap_or(0);
    let mut out = String::new();
    for (second, count) in buckets.iter().enumerate() {
        let filled = (count * bar_width).checked_div(peak).unwrap_or(0);
        let _ = writeln!(
            out,
            "  {second:>4}s {count:>5}  {}",
            "█".repeat(filled.max(usize::from(*count > 0)))
        );
    }
    out
}

fn report(
    config: &Config,
    samples: &[Sample],
    observer: &ThrottleObserver,
    stop: StopReason,
    // Measured from the clock, not from the last sample: an aborted run can
    // end with every request still in flight and no samples at all.
    wall: Duration,
) {
    let total = samples.len();
    let ok = samples.iter().filter(|s| s.is_ok()).count();
    let mut latencies: Vec<Duration> = samples.iter().map(|s| s.latency).collect();
    latencies.sort_unstable();

    println!("\n══ closed-positions soak ═══════════════════════════════");
    println!(
        "  target        {}",
        config
            .base_url
            .as_deref()
            .unwrap_or("data-api.polymarket.com")
    );
    println!("  user          {}", config.user);
    println!(
        "  requested     {}s at concurrency {}, limit={}",
        config.duration.as_secs(),
        config.concurrency,
        config.limit
    );
    println!(
        "  paced at      {}",
        match config.rate {
            Some(interval) => format!(
                "{:.2} req/s ({:.0}% of the published 150/10s), driven by the harness",
                1.0 / interval.as_secs_f64(),
                100.0 / interval.as_secs_f64() / 15.0,
            ),
            None => "the client's own limiter".to_owned(),
        }
    );
    println!("  stopped       {}", stop.as_str());

    println!("\n── Throughput ──────────────────────────────────────────");
    println!("  requests      {total} ({ok} ok, {} failed)", total - ok);
    println!("  wall time     {:.1}s", wall.as_secs_f64());
    if wall > Duration::ZERO {
        println!(
            "  achieved      {:.2} req/s",
            total as f64 / wall.as_secs_f64()
        );
    }
    println!("  modelled      15.00 req/s sustained (150 per 10s), burst of 150");

    println!("\n── Latency ─────────────────────────────────────────────");
    for (label, p) in [("p50", 50.0), ("p90", 90.0), ("p99", 99.0)] {
        println!(
            "  {label}           {:>7.1}ms",
            percentile(&latencies, p).as_secs_f64() * 1000.0
        );
    }
    println!(
        "  max           {:>7.1}ms",
        latencies
            .last()
            .copied()
            .unwrap_or(Duration::ZERO)
            .as_secs_f64()
            * 1000.0
    );

    let buckets = per_second_histogram(samples);
    if !buckets.is_empty() {
        println!("\n── Completions per second ──────────────────────────────");
        print!("{}", render_histogram(&buckets, 50));
    }

    let errors = distinct_errors(samples);
    if !errors.is_empty() {
        println!("── Errors ──────────────────────────────────────────────");
        for (message, count) in errors {
            println!("  {count:>5}x {message}");
        }
    }

    println!("\n── Throttling ──────────────────────────────────────────");
    let throttles = observer.throttle_count();
    println!("  429s observed {throttles}");
    println!("  other warns   {}", observer.other_warning_count());
    if let Some(at) = observer.first_throttle_at() {
        println!("  first at      {:.2}s into the run", at.as_secs_f64());
        let first_window = at < Duration::from_secs(10);
        println!(
            "  reading       {}",
            if first_window {
                "first window — a full-start bucket that also refills permits 2x the quota"
            } else {
                "after the first window — the sustained 150/10s rate itself over-permits"
            }
        );
    }
    for line in observer.warn_samples() {
        println!("    {line}");
    }

    println!(
        "\n══ {} ═══════════════════════════════════════════════\n",
        if throttles == 0 { "PASS" } else { "FAIL" }
    );
}

fn distinct_errors(samples: &[Sample]) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for message in samples.iter().filter_map(|s| s.error.as_ref()) {
        match counts.iter_mut().find(|(m, _)| m == message) {
            Some((_, count)) => *count += 1,
            None => counts.push((message.clone(), 1)),
        }
    }
    counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    counts
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

    let start = Instant::now();
    let observer = install_observer(start);

    let mut builder = DataApi::builder().max_concurrent(config.concurrency);
    if let Some(url) = &config.base_url {
        builder = builder.base_url(url.clone());
    }
    let client = match builder.build() {
        Ok(client) => client,
        Err(e) => {
            eprintln!("error: could not build client: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!(
        "soaking /closed-positions for {}s at concurrency {} …",
        config.duration.as_secs(),
        config.concurrency
    );

    let (samples, stop, wall) = soak(&config, client, Arc::clone(&observer), start).await;
    report(&config, &samples, &observer, stop, wall);

    if observer.throttled() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_at(secs: f64, latency_ms: u64) -> Sample {
        Sample {
            finished_at: Duration::from_secs_f64(secs),
            latency: Duration::from_millis(latency_ms),
            error: None,
        }
    }

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

    // ── histogram ───────────────────────────────────────────────

    #[test]
    fn histogram_of_empty_has_no_buckets() {
        assert!(per_second_histogram(&[]).is_empty());
    }

    #[test]
    fn histogram_bins_by_completion_second_and_fills_idle_seconds() {
        let samples = vec![
            sample_at(0.1, 80),
            sample_at(0.9, 80),
            sample_at(2.5, 80), // nothing completed during second 1
        ];
        assert_eq!(per_second_histogram(&samples), vec![2, 0, 1]);
    }

    #[test]
    fn histogram_shows_the_burst_cliff() {
        // The shape the soak exists to surface: a full bucket draining fast,
        // then the limiter clamping to the sustained rate.
        let mut samples = Vec::new();
        for i in 0..50 {
            samples.push(sample_at(f64::from(i) / 50.0, 80));
        }
        for i in 0..15 {
            samples.push(sample_at(5.0 + f64::from(i) / 15.0, 80));
        }
        let buckets = per_second_histogram(&samples);
        assert_eq!(buckets[0], 50, "burst phase drains the full bucket");
        assert_eq!(
            buckets[5], 15,
            "sustained phase settles at the modelled rate"
        );
    }

    #[test]
    fn render_marks_nonempty_seconds_even_when_dwarfed_by_the_peak() {
        // A single request in a second must not render as a blank line, or a
        // slow tail reads as an outage.
        let rendered = render_histogram(&[100, 1], 50);
        let tail = rendered.lines().nth(1).expect("second bucket");
        assert!(tail.contains('█'), "got: {tail:?}");
    }

    // ── sample collection ───────────────────────────────────────

    #[test]
    fn drain_recovers_samples_while_another_arc_clone_is_alive() {
        // The first live run reported "requests 0" alongside 6 observed
        // throttles: collection used `Arc::try_unwrap`, which fails while the
        // aborted progress ticker still holds a clone, and the fallback
        // returned an empty Vec. Holding a clone here reproduces exactly that
        // condition.
        let samples = Arc::new(Mutex::new(vec![sample_at(1.0, 80), sample_at(0.5, 80)]));
        let _ticker_still_holds_a_clone = Arc::clone(&samples);

        let drained = drain(&samples);

        assert_eq!(drained.len(), 2, "samples must survive a live Arc clone");
        assert!(
            drained[0].finished_at < drained[1].finished_at,
            "drain must return samples in completion order"
        );
    }

    #[test]
    fn drain_empties_the_buffer() {
        let samples = Arc::new(Mutex::new(vec![sample_at(1.0, 80)]));
        assert_eq!(drain(&samples).len(), 1);
        assert!(
            drain(&samples).is_empty(),
            "second drain sees an empty buffer"
        );
    }

    // ── error grouping ──────────────────────────────────────────

    #[test]
    fn distinct_errors_counts_and_orders_by_frequency() {
        let err = |message: &str| Sample {
            finished_at: Duration::ZERO,
            latency: Duration::ZERO,
            error: Some(message.to_owned()),
        };
        let samples = vec![
            err("timeout"),
            err("429"),
            err("timeout"),
            sample_at(0.0, 1),
        ];
        assert_eq!(
            distinct_errors(&samples),
            vec![("timeout".to_owned(), 2), ("429".to_owned(), 1)]
        );
    }

    // ── argument parsing ────────────────────────────────────────

    fn parse(args: &[&str]) -> Result<Option<Config>, String> {
        Config::from_args(args.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn defaults_match_the_shipped_client_configuration() {
        let config = parse(&[]).unwrap().unwrap();
        assert_eq!(config.concurrency, DEFAULT_CONCURRENCY);
        assert_eq!(config.limit, 50);
        assert_eq!(config.duration, Duration::from_secs(180));
        assert_eq!(
            config.rate, None,
            "without --rate the client's own limiter sets the pace"
        );
    }

    #[test]
    fn rate_is_stored_as_the_interval_between_sends() {
        let config = parse(&["--rate", "12"]).unwrap().unwrap();
        let interval = config.rate.expect("--rate should be set");
        assert!(
            (interval.as_secs_f64() - 1.0 / 12.0).abs() < 1e-9,
            "12 req/s should pace at {:?}, got {interval:?}",
            Duration::from_secs_f64(1.0 / 12.0)
        );
    }

    #[test]
    fn a_rate_at_or_above_the_clients_own_is_refused() {
        // Above the client's sustained rate its limiter is the slower of the
        // two and binds first, so the run would measure polyoxide's pacing
        // rather than the server's tolerance — a clean result that means
        // nothing. Refusing beats silently measuring the wrong thing.
        for rate in ["13.4", "14", "100"] {
            let err = parse(&["--rate", rate]).unwrap_err();
            assert!(
                err.contains("client's own limiter"),
                "--rate {rate} should be refused with an explanation, got: {err}"
            );
        }
    }

    #[test]
    fn a_rate_below_the_clients_own_is_accepted() {
        assert!(parse(&["--rate", "13.3"]).unwrap().unwrap().rate.is_some());
    }

    #[test]
    fn a_non_positive_rate_is_refused() {
        for rate in ["0", "-1", "nan", "inf"] {
            assert!(
                parse(&["--rate", rate]).is_err(),
                "--rate {rate} should be refused"
            );
        }
    }

    #[test]
    fn request_cap_scales_with_a_caller_supplied_duration() {
        let config = parse(&["--duration", "10"]).unwrap().unwrap();
        assert_eq!(config.max_requests, 10 * MAX_REQUESTS_PER_SEC);
    }

    #[test]
    fn explicit_request_cap_survives_a_duration_flag() {
        let config = parse(&["--duration", "10", "--max-requests", "7"])
            .unwrap()
            .unwrap();
        assert_eq!(config.max_requests, 7);
    }

    #[test]
    fn limit_above_the_server_ceiling_is_rejected_locally() {
        assert!(parse(&["--limit", "51"]).is_err());
    }

    #[test]
    fn zero_concurrency_is_rejected() {
        // Would otherwise spawn no workers and report a vacuous PASS.
        assert!(parse(&["--concurrency", "0"]).is_err());
    }

    #[test]
    fn zero_duration_is_rejected() {
        assert!(parse(&["--duration", "0"]).is_err());
    }

    #[test]
    fn missing_value_is_rejected() {
        assert!(parse(&["--duration"]).is_err());
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert!(parse(&["--forever"]).is_err());
    }

    #[test]
    fn help_short_circuits() {
        assert!(parse(&["--help"]).unwrap().is_none());
    }
}
