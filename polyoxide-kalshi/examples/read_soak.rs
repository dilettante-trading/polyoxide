//! Measures what Kalshi's production host serves an unsigned client, validates the shipped client's
//! sizing against it, and probes which Read bucket a signed public read draws on.
//!
//! Kalshi publishes no limit for unauthenticated requests, so the Predictions client sizes its
//! unsigned Read bucket from this harness's measurement. A ramp steps one route up through fixed
//! rates, each for `--stage-secs`, until the host refuses one; the highest clean rate is what
//! `src/predictions/throttle.rs` sizes from, less a tenth. The stages are requests a second:
//! Basic's Read bucket refills 200 tokens a second, which is 20 reads at the default cost of 10.
//!
//! ```sh
//! # Ramp one route on production, unsigned.
//! cargo run --release -p polyoxide-kalshi --features predictions --example read_soak -- \
//!     --routes market --stages 10,20,40,80,160,240
//!
//! # Validate the shipped client: pace both routes by its own throttle and require zero 429s.
//! cargo run --release -p polyoxide-kalshi --features predictions --example read_soak -- \
//!     --routes all --pace client --concurrency 32 --max-concurrent 32
//!
//! # Probe the demo host with the demo key: does a signed public read draw on the key's bucket?
//! cargo run --release -p polyoxide-kalshi --features predictions --example read_soak -- --signed
//! ```
//!
//! Every URL of a run is distinct, because a repeated URL can be answered from a cache without
//! reaching the origin, and a route is probed once before any stage so a run never measures a path
//! the host does not serve. The ramp's rules are `polyoxide_test_support::soak::verdict::strict`,
//! unit-tested there.
use std::{
    collections::HashSet,
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use polyoxide_core::reqwest::{Client, RequestBuilder};
use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{MarketStatusFilter, MveFilter},
    Credentials, Predictions, DEMO, PRODUCTION,
};
use polyoxide_test_support::{
    load_env,
    soak::{
        self,
        observe::install_observer,
        verdict::strict::{classify, judge, summarize, Reply, Verdict},
        Pacer,
    },
};

const DEFAULT_STAGES: [f64; 6] = [10.0, 20.0, 40.0, 80.0, 160.0, 240.0];
/// The most requests a second a stage or a probe may ask for.
const CEILING_RPS: f64 = 300.0;
/// Pages of the open listing a walk reads, of `LISTING_LIMIT` markets each.
const DEFAULT_PAGES: usize = 40;
const LISTING_LIMIT: u32 = 1000;

/// The shortest and longest a phase of the signed probe waits for a `429`.
const MIN_WINDOW: Duration = Duration::from_secs(10);
const MAX_WINDOW: Duration = Duration::from_secs(60);
/// A throttle earlier than this share of the time the key's bucket takes to empty is not the
/// key's bucket.
const EARLIEST_SHARE: f64 = 0.5;
/// A throttle later than this multiple of that time, and the slack, is not the key's bucket
/// either.
const LATEST_FACTOR: f64 = 2.0;
const SLACK: Duration = Duration::from_secs(1);
/// What the bucket is given to refill between the signed probe's phases, beyond a full refill.
const REFILL_MARGIN: Duration = Duration::from_secs(5);

/// The templates the signed probe looks up in `GET /account/endpoint_costs`.
const PUBLIC_TEMPLATE: &str = "/trade-api/v2/markets/:ticker";
const PRIVATE_TEMPLATE: &str = "/trade-api/v2/account/limits";

// ── Routes and URLs ─────────────────────────────────────────────

/// A public route the harness reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Market,
    Event,
}

/// One request: where it goes, and the path a signature covers.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    url: String,
    path: String,
}

impl Route {
    const ALL: [Route; 2] = [Route::Market, Route::Event];

    fn name(self) -> &'static str {
        match self {
            Route::Market => "market",
            Route::Event => "event",
        }
    }

    /// The request for one ticker, or one event ticker, on `base`.
    fn target(self, base: &str, id: &str) -> Target {
        let path = match self {
            Route::Market => format!("/trade-api/v2/markets/{id}"),
            Route::Event => format!("/trade-api/v2/events/{id}"),
        };
        Target {
            url: format!("{base}{path}"),
            path,
        }
    }
}

fn parse_routes(raw: &str) -> Result<Vec<Route>, String> {
    soak::parse_routes(raw, &Route::ALL, Route::name)
}

fn parse_stages(raw: &str) -> Result<Vec<f64>, String> {
    soak::parse_stages(raw, CEILING_RPS)
}

/// Whether `text` can sit in a URL path as it is, and is not made of dots alone.
fn path_safe(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
        && text.bytes().any(|byte| byte.is_ascii_alphanumeric())
}

/// The tickers and event tickers a listing walk found, each once, in listing order.
#[derive(Debug, Default, PartialEq, Eq)]
struct Listing {
    tickers: Vec<String>,
    events: Vec<String>,
}

impl Listing {
    /// Keeps the first sight of each `(ticker, event ticker)` row's ticker and event ticker, and
    /// drops a ticker the client would not send as it is listed, and an event ticker that would
    /// not sit in a URL path.
    fn from_rows(rows: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut listing = Self::default();
        let mut tickers = HashSet::new();
        let mut events = HashSet::new();
        for (ticker, event) in rows {
            let canonical = Ticker::new(&ticker).is_ok_and(|parsed| parsed.as_str() == ticker);
            if canonical && tickers.insert(ticker.clone()) {
                listing.tickers.push(ticker);
            }
            if path_safe(&event) && events.insert(event.clone()) {
                listing.events.push(event);
            }
        }
        listing
    }

    /// The ids `route` is read for.
    fn ids(&self, route: Route) -> &[String] {
        match route {
            Route::Market => &self.tickers,
            Route::Event => &self.events,
        }
    }
}

/// Ids handed out once each, so no URL of a run repeats.
#[derive(Debug)]
struct Pool {
    ids: Vec<String>,
    next: usize,
}

impl Pool {
    fn new(ids: &[String]) -> Self {
        Self {
            ids: ids.to_vec(),
            next: 0,
        }
    }

    /// The next id, or `None` once every id has gone out.
    fn draw(&mut self) -> Option<String> {
        let id = self.ids.get(self.next)?.clone();
        self.next += 1;
        Some(id)
    }

    /// The next `count` ids, or an error that hands out none when fewer are left.
    fn take(&mut self, count: usize) -> Result<Vec<String>, String> {
        let left = self.ids.len() - self.next;
        if left < count {
            return Err(format!(
                "{count} more distinct URLs are needed and the listing walk left {left}; lower \
                 --stage-secs or the stage rate, or raise --pages"
            ));
        }
        let taken = self.ids[self.next..self.next + count].to_vec();
        self.next += count;
        Ok(taken)
    }
}

/// The distinct URLs a stage of `secs` seconds at `rate` a second can send: one for each slot,
/// and one more for a slot that lands on the deadline.
fn stage_urls(rate: f64, secs: u64) -> usize {
    (rate * secs as f64).ceil() as usize + 1
}

/// The rate of the last stage before the first one that was not clean, and `None` when the first
/// was not.
fn highest_clean(stages: &[(f64, Verdict)]) -> Option<f64> {
    stages
        .iter()
        .take_while(|(_, verdict)| *verdict == Verdict::Clean)
        .last()
        .map(|(rate, _)| *rate)
}

/// When the first `429` arrived, from the start of the phase.
fn first_throttle(replies: &[(Duration, Reply)]) -> Option<Duration> {
    replies
        .iter()
        .find(|(_, reply)| matches!(reply, Reply::Throttled { .. }))
        .map(|(at, _)| *at)
}

// ── The signed probe ────────────────────────────────────────────

/// One phase of the signed probe: reads sent faster than a bucket refills.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drain {
    /// Requests a second the phase sends.
    rate: f64,
    /// When a bucket that starts full empties at that rate, from the first request.
    model: Duration,
    /// How long the phase sends before it stops waiting for a `429`.
    window: Duration,
}

impl Drain {
    /// A phase against a bucket of `capacity` tokens refilling `refill` a second, for reads of
    /// `cost` tokens: twice the rate the refill sustains, so the bucket empties after
    /// `capacity / refill` seconds, unless the harness's ceiling is lower.
    fn new(capacity: u32, refill: u32, cost: u32) -> Result<Self, String> {
        if capacity == 0 || refill == 0 || cost == 0 {
            return Err(format!(
                "a bucket of {capacity} tokens refilling {refill} a second, read at {cost} tokens, \
                 cannot be probed"
            ));
        }
        let (capacity, refill, cost) = (f64::from(capacity), f64::from(refill), f64::from(cost));
        let rate = (2.0 * refill / cost).min(CEILING_RPS);
        let net = rate * cost - refill;
        if net <= 0.0 {
            return Err(format!(
                "{CEILING_RPS} requests a second at {cost} tokens a read do not outrun a refill of \
                 {refill} tokens a second, so the bucket cannot be drained"
            ));
        }
        let model = Duration::from_secs_f64(capacity / net);
        Ok(Self {
            rate,
            model,
            window: (model * 3).clamp(MIN_WINDOW, MAX_WINDOW),
        })
    }
}

/// Where a first `429` falls against the time the key's bucket takes to empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Timing {
    Early,
    OnModel,
    Late,
}

fn timing(model: Duration, observed: Duration) -> Timing {
    if observed < model.mul_f64(EARLIEST_SHARE) {
        Timing::Early
    } else if observed > model.mul_f64(LATEST_FACTOR) + SLACK {
        Timing::Late
    } else {
        Timing::OnModel
    }
}

/// What the signed probe found.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SignedVerdict {
    /// The public read throttled where the key's bucket empties: it draws on the key's bucket.
    Follows,
    /// The public read did not throttle there, or throttled well before: it does not.
    DoesNotFollow,
    /// The control did not behave as the key's bucket predicts, or the public read throttled well
    /// after it: nothing is concluded.
    Inconclusive(String),
}

/// Judges the two phases. The control is a route that needs the key and so certainly draws on its
/// bucket; a public read is judged only when the control throttled where the model says.
fn signed_verdict(
    control_model: Duration,
    control: Option<Duration>,
    public_model: Duration,
    public: Option<Duration>,
) -> SignedVerdict {
    let Some(control) = control else {
        return SignedVerdict::Inconclusive(
            "the control never throttled, so the host did not enforce the key's bucket at this \
             rate and a quiet public read proves nothing"
                .into(),
        );
    };
    if timing(control_model, control) != Timing::OnModel {
        return SignedVerdict::Inconclusive(format!(
            "the control throttled at {control:?}, not where a bucket that empties after \
             {control_model:?} puts it"
        ));
    }
    match public.map(|at| (at, timing(public_model, at))) {
        None | Some((_, Timing::Early)) => SignedVerdict::DoesNotFollow,
        Some((_, Timing::OnModel)) => SignedVerdict::Follows,
        Some((at, Timing::Late)) => SignedVerdict::Inconclusive(format!(
            "the public read throttled at {at:?}, long after the {public_model:?} the key's bucket \
             takes to empty"
        )),
    }
}

/// The three headers Kalshi signs a request with: the key id, `now_ms`, and the signature of
/// `now_ms + "GET" + path`, with `path` as sent and no query.
fn signed_headers(
    credentials: &Credentials,
    path: &str,
    now_ms: u128,
) -> Result<[(&'static str, String); 3], String> {
    let timestamp = now_ms.to_string();
    let signature = credentials
        .sign_message(format!("{timestamp}GET{path}").as_bytes())
        .map_err(|error| error.to_string())?;
    Ok([
        ("KALSHI-ACCESS-KEY", credentials.key_id().to_owned()),
        ("KALSHI-ACCESS-TIMESTAMP", timestamp),
        ("KALSHI-ACCESS-SIGNATURE", signature),
    ])
}

// ── Configuration ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Ramp,
    Validate,
    Signed,
}

#[derive(Debug, Clone)]
struct Config {
    mode: Mode,
    routes: Vec<Route>,
    stages: Vec<f64>,
    stage_secs: u64,
    cooldown_secs: u64,
    concurrency: usize,
    max_concurrent: Option<usize>,
    pages: usize,
    base_url: Option<String>,
}

const USAGE: &str = "\
Measure what Kalshi's production host serves an unsigned client, validate the shipped client
against it, or probe which Read bucket a signed public read draws on.

Usage: read_soak --routes <routes> [options]
       read_soak --routes <routes> --pace client [options]
       read_soak --signed [options]

  --routes <routes>       market | event. A ramp takes one; --pace client takes a comma list
                          or `all`
  --stages <r1,r2,...>    Ramp rates in requests a second, ascending, at most 300
                          (default: 10,20,40,80,160,240)
  --pace client           Validate instead: pace by the shipped client's own throttle and
                          require zero 429s
  --signed                Probe the demo host with KALSHI_DEMO_API_KEY_ID and
                          KALSHI_DEMO_PRIVATE_KEY: does a signed public read draw on the key's
                          Read bucket?
  --stage-secs <n>        Seconds per stage (default: 60 ramp, 120 validation)
  --cooldown-secs <n>     Idle seconds between ramp stages (default: 30)
  --concurrency <n>       Workers (default: 48 ramp and signed, 4 per route validation)
  --max-concurrent <n>    Validation: the client's in-flight cap (default: the client's own)
  --pages <n>             Pages of 1000 markets the listing walk reads (default: 40)
  --base-url <url>        Override the host (default: production, the demo host with --signed)
  -h, --help              Show this message";

impl Config {
    fn from_args(mut args: impl Iterator<Item = String>) -> Result<Option<Self>, String> {
        let mut routes = None;
        let mut stages = None;
        let mut client_paced = false;
        let mut signed = false;
        let mut stage_secs = None;
        let mut cooldown_secs = 30;
        let mut concurrency = None;
        let mut max_concurrent = None;
        let mut pages = DEFAULT_PAGES;
        let mut base_url = None;
        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| format!("{flag} requires a value"))
            };
            match flag.as_str() {
                "-h" | "--help" => return Ok(None),
                "--routes" => routes = Some(parse_routes(&value()?)?),
                "--stages" => stages = Some(parse_stages(&value()?)?),
                "--pace" => match value()?.as_str() {
                    "client" => client_paced = true,
                    other => return Err(format!("--pace takes only `client`, got {other:?}")),
                },
                "--signed" => signed = true,
                "--stage-secs" => {
                    stage_secs = Some(value()?.parse().map_err(|_| "bad --stage-secs")?)
                }
                "--cooldown-secs" => {
                    cooldown_secs = value()?.parse().map_err(|_| "bad --cooldown-secs")?
                }
                "--concurrency" => {
                    concurrency = Some(value()?.parse().map_err(|_| "bad --concurrency")?)
                }
                "--max-concurrent" => {
                    max_concurrent = Some(value()?.parse().map_err(|_| "bad --max-concurrent")?)
                }
                "--pages" => pages = value()?.parse().map_err(|_| "bad --pages")?,
                "--base-url" => base_url = Some(value()?.trim_end_matches('/').to_owned()),
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        let mode = if signed {
            Mode::Signed
        } else if client_paced {
            Mode::Validate
        } else {
            Mode::Ramp
        };
        match mode {
            Mode::Signed => {
                if routes.is_some() || stages.is_some() || client_paced {
                    return Err("--signed takes no --routes, --stages or --pace".into());
                }
            }
            Mode::Validate | Mode::Ramp => {
                let listed = routes.as_ref().ok_or("--routes is required")?;
                if mode == Mode::Ramp && listed.len() != 1 {
                    return Err(
                        "a ramp measures one route at a time; list several only with --pace client"
                            .into(),
                    );
                }
                if mode == Mode::Validate && stages.is_some() {
                    return Err("--stages is a ramp's; --pace client takes none".into());
                }
            }
        }
        if max_concurrent.is_some() && mode != Mode::Validate {
            return Err("--max-concurrent is for --pace client".into());
        }
        if concurrency == Some(0)
            || stage_secs == Some(0)
            || max_concurrent == Some(0)
            || pages == 0
        {
            return Err(
                "--concurrency, --stage-secs, --max-concurrent and --pages must be at least 1"
                    .into(),
            );
        }
        Ok(Some(Self {
            mode,
            routes: routes.unwrap_or_default(),
            stages: stages.unwrap_or_else(|| DEFAULT_STAGES.to_vec()),
            stage_secs: stage_secs.unwrap_or(if mode == Mode::Validate { 120 } else { 60 }),
            cooldown_secs,
            concurrency: concurrency.unwrap_or(if mode == Mode::Validate { 4 } else { 48 }),
            max_concurrent,
            pages,
            base_url,
        }))
    }
}

// ── Drivers ─────────────────────────────────────────────────────

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis())
}

/// The whole seconds that cover `duration`.
fn whole_secs(duration: Duration) -> u64 {
    duration.as_secs_f64().ceil() as u64
}

/// A bare client for the raw requests. It sets no `Accept-Encoding` of its own, so it sends what
/// core's reqwest sends every polyoxide client.
fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())
}

/// Sends `request` and reads the reply as the ramp's rules do: its status, its cache header, its
/// `Retry-After` and its body.
async fn fetch(request: RequestBuilder) -> Reply {
    let Ok(response) = request.send().await else {
        return Reply::Error(0);
    };
    let status = response.status().as_u16();
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let cache = header("x-cache").or_else(|| header("cf-cache-status"));
    let retry_after = header("retry-after");
    let body = response.text().await.unwrap_or_default();
    classify(status, cache.as_deref(), retry_after.as_deref(), &body)
}

/// One request of the harness, signed when `signer` is given.
async fn send(client: &Client, target: &Target, signer: Option<&Credentials>) -> Reply {
    let mut request = client.get(&target.url);
    if let Some(credentials) = signer {
        match signed_headers(credentials, &target.path, now_millis()) {
            Ok(headers) => {
                for (name, value) in headers {
                    request = request.header(name, value);
                }
            }
            Err(_) => return Reply::Error(0),
        }
    }
    fetch(request).await
}

/// Sends `targets` at `rate` a second from `concurrency` workers until `secs` seconds pass, every
/// target has gone out, or the first `429` lands. Returns the replies in arrival order.
async fn drive(
    client: &Client,
    targets: Vec<Target>,
    signer: Option<Arc<Credentials>>,
    rate: f64,
    secs: u64,
    concurrency: usize,
) -> Vec<(Duration, Reply)> {
    let pacer = Arc::new(Pacer::new(Duration::from_secs_f64(1.0 / rate)));
    let replies = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    let deadline = start + Duration::from_secs(secs);
    let mut shares: Vec<Vec<Target>> = (0..concurrency).map(|_| Vec::new()).collect();
    for (n, target) in targets.into_iter().enumerate() {
        shares[n % concurrency].push(target);
    }
    let mut workers = Vec::new();
    for share in shares {
        let client = client.clone();
        let pacer = Arc::clone(&pacer);
        let replies = Arc::clone(&replies);
        let stop = Arc::clone(&stop);
        let signer = signer.clone();
        workers.push(tokio::spawn(async move {
            for target in share {
                if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    break;
                }
                pacer.wait().await;
                // A worker parked in the pacer when another was told 429 wakes after it, so the
                // stop is read again before sending.
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let reply = send(&client, &target, signer.as_deref()).await;
                let throttled = matches!(reply, Reply::Throttled { .. });
                lock(&replies).push((start.elapsed(), reply));
                if throttled {
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }));
    }
    for worker in workers {
        let _ = worker.await;
    }
    let mut out = lock(&replies).clone();
    out.sort_by_key(|(at, _)| *at);
    out
}

/// Walks the open listing, without the multivariate combinations it serves first, for up to
/// `pages` pages.
async fn walk(client: &Predictions, pages: usize) -> Result<Listing, String> {
    let mut rows = Vec::new();
    let mut cursor = String::new();
    for _ in 0..pages {
        let mut request = client
            .markets()
            .list()
            .status(MarketStatusFilter::Open)
            .mve_filter(MveFilter::Exclude)
            .limit(LISTING_LIMIT);
        if !cursor.is_empty() {
            request = request.cursor(cursor.clone());
        }
        let page = request
            .send()
            .await
            .map_err(|error| format!("the listing walk failed: {error}"))?;
        rows.extend(
            page.markets
                .into_iter()
                .map(|market| (market.ticker, market.event_ticker)),
        );
        if page.cursor.is_empty() {
            break;
        }
        cursor = page.cursor;
    }
    Ok(Listing::from_rows(rows))
}

/// One request to a route before any stage uses it, so a run never measures a path the host does
/// not serve: it must answer 2xx.
async fn probe(
    client: &Client,
    label: &str,
    target: &Target,
    signer: Option<&Credentials>,
) -> Result<(), String> {
    match send(client, target, signer).await {
        Reply::Ok | Reply::CacheHit => {
            eprintln!("probe {label}: {} answers 2xx", target.path);
            Ok(())
        }
        other => Err(format!(
            "probe {label}: {} did not answer 2xx: {other:?}",
            target.path
        )),
    }
}

/// A signed phase is a finding only when every reply was a 2xx or a `429`: any other reply means
/// the host did not accept the signed requests, and the phase says nothing about a bucket.
fn check_accepted(label: &str, replies: &[(Duration, Reply)]) -> Result<(), String> {
    let statuses: Vec<u16> = replies
        .iter()
        .filter_map(|(_, reply)| match reply {
            Reply::Error(status) => Some(*status),
            _ => None,
        })
        .collect();
    if let Some(first) = statuses.first() {
        return Err(format!(
            "{label}: {} replies were neither 2xx nor 429, the first with status {first} (0 is no \
             response): {}",
            statuses.len(),
            summarize(replies)
        ));
    }
    Ok(())
}

/// One read through the shipped client.
async fn read(kalshi: &Predictions, route: Route, id: &str) -> Result<(), String> {
    match route {
        Route::Market => {
            let ticker = Ticker::new(id).map_err(|error| error.to_string())?;
            kalshi
                .markets()
                .get(&ticker)
                .send()
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        Route::Event => kalshi
            .events()
            .get(id)
            .send()
            .await
            .map(|_| ())
            .map_err(|error| error.to_string()),
    }
}

/// Steps one route up through `--stages` until the host refuses a stage.
async fn run_ramp(cfg: &Config) -> Result<ExitCode, String> {
    let base = cfg
        .base_url
        .clone()
        .unwrap_or_else(|| PRODUCTION.to_owned());
    let route = cfg.routes[0];
    let kalshi = Predictions::builder()
        .base_url(base.as_str())
        .build()
        .map_err(|error| error.to_string())?;
    let listing = walk(&kalshi, cfg.pages).await?;
    eprintln!(
        "the listing walk found {} tickers and {} events",
        listing.tickers.len(),
        listing.events.len()
    );
    let mut pool = Pool::new(listing.ids(route));
    let http = http_client()?;
    let probed = pool
        .draw()
        .ok_or_else(|| format!("the listing walk found no {} ids", route.name()))?;
    probe(&http, route.name(), &route.target(&base, &probed), None).await?;
    let mut stages = Vec::new();
    for &rate in &cfg.stages {
        let ids = match pool.take(stage_urls(rate, cfg.stage_secs)) {
            Ok(ids) => ids,
            Err(reason) => {
                eprintln!("== stopping before {rate:.1} req/s: {reason}");
                break;
            }
        };
        let targets = ids.iter().map(|id| route.target(&base, id)).collect();
        eprintln!(
            "== {} at {rate:.1} req/s for {}s",
            route.name(),
            cfg.stage_secs
        );
        let replies = drive(&http, targets, None, rate, cfg.stage_secs, cfg.concurrency).await;
        let verdict = judge(&replies, rate, cfg.stage_secs);
        eprintln!("   {} replies, verdict {verdict:?}", replies.len());
        eprintln!("   {}", summarize(&replies));
        let unclean = verdict != Verdict::Clean;
        stages.push((rate, verdict));
        if unclean {
            break;
        }
        eprintln!("   cooling down {}s", cfg.cooldown_secs);
        tokio::time::sleep(Duration::from_secs(cfg.cooldown_secs)).await;
    }
    let refused = stages
        .last()
        .is_some_and(|(_, verdict)| matches!(verdict, Verdict::Throttled { .. }));
    let clean = highest_clean(&stages);
    println!(
        "RESULT ramp route={} highest_clean={clean:?} refused={refused} stages={stages:?}",
        route.name()
    );
    Ok(if clean.is_some() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// Paces reads by the shipped client's own throttle and requires zero `429`s.
async fn run_validation(cfg: &Config) -> Result<ExitCode, String> {
    let observer = install_observer(Instant::now());
    let base = cfg
        .base_url
        .clone()
        .unwrap_or_else(|| PRODUCTION.to_owned());
    // The shipped client, untouched but for the in-flight cap a caller asks to lift: its own
    // throttle paces every request.
    let mut builder = Predictions::builder().base_url(base.as_str());
    if let Some(cap) = cfg.max_concurrent {
        builder = builder.max_concurrent(cap);
    }
    let kalshi = builder.build().map_err(|error| error.to_string())?;
    let listing = walk(&kalshi, cfg.pages).await?;
    eprintln!(
        "the listing walk found {} tickers and {} events",
        listing.tickers.len(),
        listing.events.len()
    );
    let http = http_client()?;
    let mut pools = Vec::new();
    for &route in &cfg.routes {
        let mut pool = Pool::new(listing.ids(route));
        let probed = pool
            .draw()
            .ok_or_else(|| format!("the listing walk found no {} ids", route.name()))?;
        probe(&http, route.name(), &route.target(&base, &probed), None).await?;
        pools.push((route, Arc::new(Mutex::new(pool))));
    }
    let deadline = Instant::now() + Duration::from_secs(cfg.stage_secs);
    let sent = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let exhausted = Arc::new(AtomicBool::new(false));
    let mut workers = Vec::new();
    for (route, pool) in &pools {
        for _ in 0..cfg.concurrency {
            let route = *route;
            let pool = Arc::clone(pool);
            let kalshi = kalshi.clone();
            let sent = Arc::clone(&sent);
            let failed = Arc::clone(&failed);
            let exhausted = Arc::clone(&exhausted);
            workers.push(tokio::spawn(async move {
                while Instant::now() < deadline {
                    let drawn = lock(&pool).draw();
                    let Some(id) = drawn else {
                        exhausted.store(true, Ordering::Relaxed);
                        break;
                    };
                    let result = read(&kalshi, route, &id).await;
                    sent.fetch_add(1, Ordering::Relaxed);
                    if let Err(error) = result {
                        failed.fetch_add(1, Ordering::Relaxed);
                        eprintln!("{}: {error}", route.name());
                    }
                }
            }));
        }
    }
    for worker in workers {
        let _ = worker.await;
    }
    let sent = sent.load(Ordering::Relaxed);
    let failed = failed.load(Ordering::Relaxed);
    let exhausted = exhausted.load(Ordering::Relaxed);
    let throttles = observer.throttles_by_path();
    let throttled: u64 = throttles.values().sum();
    // `sent` counts calls: a call the retry loop re-sends is several requests on the wire but one
    // here; the throttle counts are per request.
    println!(
        "validation: {sent} calls over {}s, {throttled} throttled, {failed} failed{}",
        cfg.stage_secs,
        if exhausted {
            ", and the distinct URLs ran out first"
        } else {
            ""
        }
    );
    for (path, count) in &throttles {
        println!("  {path}: {count} throttled");
    }
    let clean = throttled == 0 && failed == 0 && !exhausted;
    println!("RESULT validation clean={clean} calls={sent} throttled={throttled} failed={failed}");
    Ok(if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// Drains the demo key's Read bucket on a route that needs the key and then on a public route,
/// and says whether the public read's `429` follows the key's bucket.
async fn run_signed(cfg: &Config) -> Result<ExitCode, String> {
    let creds = load_env(&["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"])
        .map_err(|missing| missing.to_string())?;
    let credentials = Credentials::from_pem(
        creds.get("KALSHI_DEMO_API_KEY_ID"),
        creds.get("KALSHI_DEMO_PRIVATE_KEY"),
    )
    .map_err(|error| error.to_string())?;
    let base = cfg.base_url.clone().unwrap_or_else(|| DEMO.to_owned());
    let kalshi = Predictions::builder()
        .base_url(base.as_str())
        .credentials(credentials.clone())
        .build()
        .map_err(|error| error.to_string())?;
    let limits = kalshi
        .account()
        .limits()
        .send()
        .await
        .map_err(|error| format!("account().limits() failed: {error}"))?;
    let costs = kalshi
        .account()
        .endpoint_costs()
        .send()
        .await
        .map_err(|error| format!("account().endpoint_costs() failed: {error}"))?;
    let cost_of = |template: &str| {
        costs
            .endpoint_costs
            .iter()
            .find(|entry| entry.method == "GET" && entry.path == template)
            .map_or(costs.default_cost, |entry| entry.cost)
    };
    let (capacity, refill) = (limits.read.bucket_capacity, limits.read.refill_rate);
    let (public_cost, private_cost) = (cost_of(PUBLIC_TEMPLATE), cost_of(PRIVATE_TEMPLATE));
    let public = Drain::new(capacity, refill, public_cost)?;
    let private = Drain::new(capacity, refill, private_cost)?;
    eprintln!(
        "the key's Read bucket: {capacity} tokens refilling {refill} a second (tier {}); a public \
         read costs {public_cost} tokens and the limits read {private_cost}",
        limits.usage_tier
    );

    let listing = walk(&kalshi, cfg.pages).await?;
    let mut pool = Pool::new(listing.ids(Route::Market));
    let http = http_client()?;
    let signer = Arc::new(credentials);
    let control_target = Target {
        url: format!("{base}{PRIVATE_TEMPLATE}"),
        path: PRIVATE_TEMPLATE.to_owned(),
    };
    probe(
        &http,
        "private control",
        &control_target,
        Some(signer.as_ref()),
    )
    .await?;
    let probed = pool.draw().ok_or("the listing walk found no tickers")?;
    probe(
        &http,
        "public",
        &Route::Market.target(&base, &probed),
        Some(signer.as_ref()),
    )
    .await?;

    let window = whole_secs(private.window);
    eprintln!(
        "== control: {PRIVATE_TEMPLATE}, signed, {:.1} req/s for {window}s; a bucket that starts \
         full empties after {:?}",
        private.rate, private.model
    );
    let targets = vec![control_target; stage_urls(private.rate, window)];
    let control = drive(
        &http,
        targets,
        Some(Arc::clone(&signer)),
        private.rate,
        window,
        cfg.concurrency,
    )
    .await;
    check_accepted("control", &control)?;
    eprintln!("   {}", summarize(&control));

    let refill_wait =
        Duration::from_secs_f64(f64::from(capacity) / f64::from(refill)) + REFILL_MARGIN;
    eprintln!("   waiting {refill_wait:?} for the bucket to refill");
    tokio::time::sleep(refill_wait).await;

    let window = whole_secs(public.window);
    let ids = pool.take(stage_urls(public.rate, window))?;
    let targets: Vec<Target> = ids
        .iter()
        .map(|id| Route::Market.target(&base, id))
        .collect();
    eprintln!(
        "== public: {PUBLIC_TEMPLATE}, signed, {:.1} req/s for {window}s over {} distinct URLs; \
         a bucket that starts full empties after {:?}",
        public.rate,
        targets.len(),
        public.model
    );
    let public_replies = drive(
        &http,
        targets,
        Some(Arc::clone(&signer)),
        public.rate,
        window,
        cfg.concurrency,
    )
    .await;
    check_accepted("public", &public_replies)?;
    eprintln!("   {}", summarize(&public_replies));

    let control_first = first_throttle(&control);
    let public_first = first_throttle(&public_replies);
    let verdict = signed_verdict(private.model, control_first, public.model, public_first);
    println!(
        "RESULT signed verdict={verdict:?} control_first_429={control_first:?} \
         control_model={:?} public_first_429={public_first:?} public_model={:?}",
        private.model, public.model
    );
    Ok(match verdict {
        SignedVerdict::Inconclusive(_) => ExitCode::from(1),
        SignedVerdict::Follows | SignedVerdict::DoesNotFollow => ExitCode::SUCCESS,
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let cfg = match Config::from_args(std::env::args().skip(1)) {
        Ok(Some(cfg)) => cfg,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("error: {error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let outcome = match cfg.mode {
        Mode::Ramp => run_ramp(&cfg).await,
        Mode::Validate => run_validation(&cfg).await,
        Mode::Signed => run_signed(&cfg).await,
    };
    outcome.unwrap_or_else(|error| {
        eprintln!("error: {error}");
        ExitCode::from(2)
    })
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use polyoxide_test_support::fixtures;
    use ring::signature::{UnparsedPublicKey, ED25519};

    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|argument| (*argument).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn config(list: &[&str]) -> Config {
        Config::from_args(args(list))
            .expect("the arguments parse")
            .expect("not a request for help")
    }

    fn refused(list: &[&str]) -> String {
        Config::from_args(args(list)).expect_err("the arguments were refused")
    }

    fn rows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(ticker, event)| ((*ticker).to_owned(), (*event).to_owned()))
            .collect()
    }

    fn ids(count: usize) -> Vec<String> {
        (1..=count).map(|n| format!("T{n}")).collect()
    }

    // ── arguments ───────────────────────────────────────────────

    #[test]
    fn a_ramp_takes_exactly_one_route_and_a_validation_takes_a_list() {
        assert!(refused(&["--routes", "market,event"]).contains("one route at a time"));
        let ramp = config(&["--routes", "event"]);
        assert_eq!((ramp.mode, ramp.routes), (Mode::Ramp, vec![Route::Event]));

        let validation = config(&["--routes", "all", "--pace", "client"]);
        assert_eq!(validation.mode, Mode::Validate);
        assert_eq!(validation.routes, vec![Route::Market, Route::Event]);
        assert_eq!(validation.stage_secs, 120);
        assert_eq!(validation.concurrency, 4);
        assert_eq!(ramp.stage_secs, 60);
        assert_eq!(ramp.concurrency, 48);
    }

    #[test]
    fn a_run_names_its_routes_unless_it_is_the_signed_probe() {
        assert!(refused(&[]).contains("--routes is required"));
        assert!(refused(&["--routes", "orderbook"]).contains("unknown route"));
        let signed = config(&["--signed"]);
        assert_eq!(signed.mode, Mode::Signed);
        assert!(signed.routes.is_empty());
        assert!(refused(&["--signed", "--routes", "market"]).contains("--signed takes no"));
        assert!(refused(&["--signed", "--stages", "10"]).contains("--signed takes no"));
        assert!(refused(&["--signed", "--pace", "client"]).contains("--signed takes no"));
    }

    #[test]
    fn stages_belong_to_a_ramp_and_must_ascend_under_the_ceiling() {
        assert!(
            refused(&["--routes", "market", "--pace", "client", "--stages", "10"])
                .contains("--stages is a ramp's")
        );
        assert!(parse_stages("10,5").is_err());
        assert!(parse_stages("10,301").is_err());
        assert_eq!(parse_stages("5,10").unwrap(), vec![5.0, 10.0]);
        assert_eq!(config(&["--routes", "market"]).stages, DEFAULT_STAGES);
    }

    #[test]
    fn the_default_ramp_starts_below_basics_twenty_reads_and_ends_past_two_hundred() {
        // Basic's 200 tokens a second are 20 reads at 10 tokens, and the spec's 200 is the
        // figure the ramp must reach.
        let stages = config(&["--routes", "market"]).stages;
        assert!(stages.first().is_some_and(|first| *first < 20.0));
        assert!(stages
            .last()
            .is_some_and(|last| *last > 200.0 && *last <= CEILING_RPS));
        assert!(stages.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_zero_count_is_refused_so_a_run_cannot_pass_with_no_workers() {
        for flag in ["--concurrency", "--stage-secs", "--pages"] {
            let message = refused(&["--routes", "market", flag, "0"]);
            assert!(message.contains("at least 1"), "{flag}: {message}");
        }
        let message = refused(&[
            "--routes",
            "all",
            "--pace",
            "client",
            "--max-concurrent",
            "0",
        ]);
        assert!(message.contains("at least 1"), "{message}");
    }

    #[test]
    fn the_client_cap_is_a_validations_and_a_help_request_runs_nothing() {
        assert!(refused(&["--routes", "market", "--max-concurrent", "8"])
            .contains("--max-concurrent is for --pace client"));
        let validation = config(&[
            "--routes",
            "market",
            "--pace",
            "client",
            "--max-concurrent",
            "8",
        ]);
        assert_eq!(validation.max_concurrent, Some(8));
        assert!(Config::from_args(args(&["--help"])).unwrap().is_none());
        assert!(refused(&["--nope"]).contains("unknown argument"));
        assert!(refused(&["--routes"]).contains("requires a value"));
        assert!(refused(&["--pace", "server"]).contains("takes only `client`"));
    }

    #[test]
    fn a_base_url_loses_its_trailing_slash() {
        let config = config(&["--routes", "market", "--base-url", "http://127.0.0.1:9/"]);
        assert_eq!(config.base_url.as_deref(), Some("http://127.0.0.1:9"));
    }

    // ── URLs ────────────────────────────────────────────────────

    #[test]
    fn a_route_target_is_the_hosts_url_and_the_path_a_signature_covers() {
        let market = Route::Market.target("https://host.test", "KXA-1");
        assert_eq!(market.url, "https://host.test/trade-api/v2/markets/KXA-1");
        assert_eq!(market.path, "/trade-api/v2/markets/KXA-1");
        let event = Route::Event.target("https://host.test", "KXA");
        assert_eq!(event.url, "https://host.test/trade-api/v2/events/KXA");
        assert_eq!(event.path, "/trade-api/v2/events/KXA");
    }

    #[test]
    fn a_listing_keeps_each_ticker_and_event_once_in_order_and_drops_what_cannot_be_sent() {
        let listing = Listing::from_rows(rows(&[
            ("KXA-1", "KXA"),
            ("KXA-2", "KXA"),
            ("KXA-1", "KXA"),
            // A lowercase ticker is a 404 on the host, which is case-sensitive.
            ("kxb-1", "KXB"),
            // A space cannot sit in a URL path.
            ("KXC-1", "KX C"),
            ("KXD.1", "KXD"),
            // Dots alone are dot segments.
            ("KXE-1", ".."),
        ]));
        assert_eq!(
            listing.tickers,
            ["KXA-1", "KXA-2", "KXC-1", "KXD.1", "KXE-1"]
        );
        assert_eq!(listing.events, ["KXA", "KXB", "KXD"]);
        assert_eq!(listing.ids(Route::Market), listing.tickers.as_slice());
        assert_eq!(listing.ids(Route::Event), listing.events.as_slice());
    }

    #[test]
    fn a_pool_hands_out_each_id_once_and_refuses_a_stage_it_cannot_fill() {
        let mut pool = Pool::new(&ids(5));
        assert_eq!(pool.draw().as_deref(), Some("T1"));
        assert_eq!(pool.take(2), Ok(vec!["T2".to_owned(), "T3".to_owned()]));

        let short = pool.take(3).unwrap_err();
        assert!(
            short.contains("3 more distinct URLs") && short.contains("left 2"),
            "{short}"
        );
        // The refusal handed out nothing.
        assert_eq!(pool.take(2), Ok(vec!["T4".to_owned(), "T5".to_owned()]));
        assert_eq!(pool.draw(), None);
        assert_eq!(pool.take(0), Ok(vec![]));
    }

    #[test]
    fn a_stage_needs_a_url_for_every_slot_and_one_more() {
        assert_eq!(stage_urls(40.0, 60), 2401);
        assert_eq!(stage_urls(0.5, 1), 2);
        assert_eq!(stage_urls(10.0, 1), 11);
    }

    #[test]
    fn the_highest_clean_stage_stops_at_the_first_that_was_not() {
        let throttled = Verdict::Throttled {
            after: Duration::from_secs(3),
            code: "too many requests".into(),
        };
        let stages = vec![
            (10.0, Verdict::Clean),
            (20.0, Verdict::Clean),
            (40.0, throttled),
            (80.0, Verdict::Clean),
        ];
        assert_eq!(highest_clean(&stages), Some(20.0));
        assert_eq!(
            highest_clean(&[(10.0, Verdict::Invalid { errors: 1 })]),
            None
        );
        assert_eq!(
            highest_clean(&[(10.0, Verdict::UnderDriven { achieved: 4.0 })]),
            None
        );
        assert_eq!(highest_clean(&[]), None);
    }

    #[test]
    fn the_first_throttle_is_when_the_first_429_arrived() {
        let throttled = |at| {
            (
                Duration::from_secs(at),
                Reply::Throttled {
                    code: "x".into(),
                    retry_after: None,
                },
            )
        };
        let replies = vec![
            (Duration::from_secs(1), Reply::Ok),
            throttled(4),
            throttled(5),
        ];
        assert_eq!(first_throttle(&replies), Some(Duration::from_secs(4)));
        assert_eq!(first_throttle(&replies[..1]), None);
        assert_eq!(first_throttle(&[]), None);
    }

    // ── the signed probe ────────────────────────────────────────

    #[test]
    fn a_drain_runs_at_twice_the_refills_reads_and_empties_the_bucket_in_capacity_over_refill() {
        // Basic: 600 tokens refilling 200 a second, 20 reads a second at 10 tokens. Twice that
        // is 40 reads a second, 400 tokens, and 200 of them a second come out of the bucket.
        assert_eq!(
            Drain::new(600, 200, 10),
            Ok(Drain {
                rate: 40.0,
                model: Duration::from_secs(3),
                window: MIN_WINDOW,
            })
        );
        // A bucket that empties in a second still waits the minimum window.
        assert_eq!(
            Drain::new(1200, 1200, 10),
            Ok(Drain {
                rate: 240.0,
                model: Duration::from_secs(1),
                window: MIN_WINDOW,
            })
        );
        // The ceiling caps the rate, and the bucket then empties more slowly.
        assert_eq!(
            Drain::new(6000, 2000, 10),
            Ok(Drain {
                rate: 300.0,
                model: Duration::from_secs(6),
                window: Duration::from_secs(18),
            })
        );
    }

    #[test]
    fn a_drain_waits_at_most_the_maximum_window_and_refuses_a_bucket_it_cannot_empty() {
        let slow = Drain::new(60_000, 1000, 10).unwrap();
        assert_eq!(slow.window, MAX_WINDOW);
        for (capacity, refill, cost) in [(0, 200, 10), (600, 0, 10), (600, 200, 0)] {
            assert!(
                Drain::new(capacity, refill, cost)
                    .unwrap_err()
                    .contains("cannot be probed"),
                "{capacity} {refill} {cost}"
            );
        }
        // At 300 reads of 10 tokens a second the harness spends 3000 tokens a second, which does
        // not outrun a refill of 3000.
        let message = Drain::new(600, 3000, 10).unwrap_err();
        assert!(message.contains("cannot be drained"), "{message}");
        assert!(Drain::new(600, 4000, 10).is_err());
    }

    #[test]
    fn a_throttle_is_on_the_model_between_half_its_time_and_twice_it_with_a_second() {
        let model = Duration::from_secs(3);
        let at = |millis| timing(model, Duration::from_millis(millis));
        assert_eq!(at(0), Timing::Early);
        assert_eq!(at(1499), Timing::Early);
        assert_eq!(at(1500), Timing::OnModel);
        assert_eq!(at(3000), Timing::OnModel);
        assert_eq!(at(7000), Timing::OnModel);
        assert_eq!(at(7001), Timing::Late);
    }

    #[test]
    fn the_verdict_needs_a_control_that_throttled_where_the_keys_bucket_says() {
        let model = Duration::from_secs(3);
        let on = Some(Duration::from_secs(3));
        let early = Some(Duration::from_secs(1));
        let late = Some(Duration::from_secs(20));
        let inconclusive =
            |verdict: SignedVerdict| matches!(verdict, SignedVerdict::Inconclusive(_));

        // A quiet control proves the host did not enforce the bucket at this rate, and a control
        // anywhere but the model proves the model wrong. Neither lets a public read speak.
        assert!(inconclusive(signed_verdict(model, None, model, None)));
        assert!(inconclusive(signed_verdict(model, None, model, on)));
        assert!(inconclusive(signed_verdict(model, early, model, None)));
        assert!(inconclusive(signed_verdict(model, late, model, None)));

        // With a good control, the public read decides.
        assert_eq!(signed_verdict(model, on, model, on), SignedVerdict::Follows);
        assert_eq!(
            signed_verdict(model, on, model, None),
            SignedVerdict::DoesNotFollow
        );
        assert_eq!(
            signed_verdict(model, on, model, early),
            SignedVerdict::DoesNotFollow
        );
        assert!(inconclusive(signed_verdict(model, on, model, late)));
    }

    #[test]
    fn the_verdict_judges_each_phase_against_its_own_model() {
        // The public route costs less, so its bucket empties later: a throttle at 8 s is on its
        // model of 6 s, and would be late judged against the control's 3 s.
        let control = Duration::from_secs(3);
        let public = Duration::from_secs(6);
        let eight = Some(Duration::from_secs(8));
        assert_eq!(
            signed_verdict(control, Some(control), public, eight),
            SignedVerdict::Follows
        );
        assert!(matches!(
            signed_verdict(control, Some(control), control, eight),
            SignedVerdict::Inconclusive(_)
        ));
    }

    /// The signing vectors' Ed25519 key, a throwaway one, as the PEM Kalshi issues.
    pub(super) fn throwaway_key() -> Credentials {
        let vectors = fixtures!("predictions/signing").json("vectors");
        let body = vectors["ed25519"]["pkcs8_body"]
            .as_str()
            .expect("the vectors hold an Ed25519 key body");
        let label = "PRIVATE KEY";
        let pem = format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n");
        Credentials::from_pem("test-key", &pem).expect("the throwaway key parses")
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("the vectors hold hex"))
            .collect()
    }

    #[test]
    fn a_signed_request_carries_the_key_the_time_and_a_signature_over_time_get_path() {
        let path = "/trade-api/v2/markets/KXA-1";
        let headers = signed_headers(&throwaway_key(), path, 1_700_000_000_000).unwrap();

        let names: Vec<&str> = headers.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "KALSHI-ACCESS-KEY",
                "KALSHI-ACCESS-TIMESTAMP",
                "KALSHI-ACCESS-SIGNATURE"
            ]
        );
        assert_eq!(headers[0].1, "test-key");
        assert_eq!(headers[1].1, "1700000000000");
        // The signature verifies under the vectors' public key over the message Kalshi checks.
        let vectors = fixtures!("predictions/signing").json("vectors");
        let public_key = hex(vectors["ed25519"]["public_key_hex"]
            .as_str()
            .expect("the vectors hold the public key"));
        let signature = STANDARD
            .decode(&headers[2].1)
            .expect("the signature is base64");
        let verified = UnparsedPublicKey::new(&ED25519, &public_key)
            .verify(b"1700000000000GET/trade-api/v2/markets/KXA-1", &signature);
        assert!(verified.is_ok(), "the signature does not verify");
    }
}

#[cfg(test)]
mod drive_tests {
    use std::sync::atomic::AtomicUsize;

    use mockito::{Matcher, Mock, Server, ServerGuard};
    use polyoxide_test_support::fixtures;

    use super::tests::throwaway_key;
    use super::*;

    /// What the mock host saw of one request.
    #[derive(Debug, Clone)]
    struct Seen {
        path_and_query: String,
        key: Option<String>,
        timestamp: Option<String>,
        signature: Option<String>,
    }

    #[derive(Clone, Default)]
    struct Log(Arc<Mutex<Vec<Seen>>>);

    impl Log {
        fn all(&self) -> Vec<Seen> {
            lock(&self.0).clone()
        }
    }

    fn ids(count: usize) -> Vec<String> {
        (1..=count).map(|n| format!("T{n}")).collect()
    }

    /// A host that records every GET, answers with `status_of` the request's number counted from
    /// zero, and sends the body `body_of` makes of the request.
    async fn host(
        log: &Log,
        status_of: impl Fn(usize) -> usize + Send + Sync + 'static,
        body_of: impl Fn(&mockito::Request) -> String + Send + Sync + 'static,
    ) -> (ServerGuard, Mock) {
        let mut server = Server::new_async().await;
        let served = AtomicUsize::new(0);
        let recorder = log.clone();
        let mock = server
            .mock("GET", Matcher::Any)
            .match_query(Matcher::Any)
            .with_status_code_from_request(move |request| {
                let header = |name: &str| {
                    request
                        .header(name)
                        .first()
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned)
                };
                lock(&recorder.0).push(Seen {
                    path_and_query: request.path_and_query().to_owned(),
                    key: header("kalshi-access-key"),
                    timestamp: header("kalshi-access-timestamp"),
                    signature: header("kalshi-access-signature"),
                });
                status_of(served.fetch_add(1, Ordering::SeqCst))
            })
            .with_header("content-type", "application/json")
            .with_body_from_request(move |request| body_of(request).into_bytes())
            .create_async()
            .await;
        (server, mock)
    }

    #[tokio::test]
    async fn a_stage_over_distinct_urls_gets_only_origin_replies_and_sends_no_key() {
        let log = Log::default();
        let (server, _mock) = host(&log, |_| 200, |_| "{}".to_owned()).await;
        let base = server.url();
        let targets: Vec<Target> = ids(12)
            .iter()
            .map(|id| Route::Market.target(&base, id))
            .collect();
        let http = http_client().unwrap();

        let replies = drive(&http, targets, None, 20.0, 5, 2).await;

        assert_eq!(replies.len(), 12);
        assert!(replies.iter().all(|(_, reply)| *reply == Reply::Ok));
        let seen = log.all();
        let mut paths: Vec<&str> = seen
            .iter()
            .map(|request| request.path_and_query.as_str())
            .collect();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), 12, "a URL went out twice: {paths:?}");
        assert!(paths
            .iter()
            .all(|path| path.starts_with("/trade-api/v2/markets/T")));
        // An unsigned drive carries none of Kalshi's headers.
        assert!(seen
            .iter()
            .all(|request| request.key.is_none() && request.signature.is_none()));
    }

    #[tokio::test]
    async fn the_first_429_ends_the_stage_and_is_its_verdict() {
        let log = Log::default();
        // Kalshi's documented 429 body.
        let (server, _mock) = host(
            &log,
            |n| if n < 3 { 200 } else { 429 },
            |_| r#"{"error":"too many requests"}"#.to_owned(),
        )
        .await;
        let base = server.url();
        let targets: Vec<Target> = ids(20)
            .iter()
            .map(|id| Route::Market.target(&base, id))
            .collect();
        let http = http_client().unwrap();

        // One worker, so nothing is in flight behind the 429 when the stage ends.
        let replies = drive(&http, targets, None, 50.0, 5, 1).await;

        assert_eq!(replies.len(), 4);
        match judge(&replies, 50.0, 5) {
            Verdict::Throttled { code, .. } => assert_eq!(code, "too many requests"),
            other => panic!("expected a throttled stage, got {other:?}"),
        }
        assert!(first_throttle(&replies).is_some());
    }

    #[tokio::test]
    async fn a_signed_drive_sends_the_three_headers_for_the_path_it_was_given() {
        let log = Log::default();
        let (server, _mock) = host(&log, |_| 200, |_| "{}".to_owned()).await;
        let target = Route::Market.target(&server.url(), "KXA-1");
        let http = http_client().unwrap();

        let replies = drive(
            &http,
            vec![target],
            Some(Arc::new(throwaway_key())),
            10.0,
            5,
            1,
        )
        .await;

        assert_eq!(replies.len(), 1);
        let seen = log.all();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].path_and_query, "/trade-api/v2/markets/KXA-1");
        assert_eq!(seen[0].key.as_deref(), Some("test-key"));
        let timestamp = seen[0].timestamp.as_deref().expect("a timestamp was sent");
        assert!(
            timestamp.len() == 13 && timestamp.bytes().all(|byte| byte.is_ascii_digit()),
            "{timestamp}"
        );
        assert!(seen[0]
            .signature
            .as_deref()
            .is_some_and(|signature| !signature.is_empty()));
    }

    #[tokio::test]
    async fn a_probe_names_the_route_that_does_not_answer_2xx() {
        let log = Log::default();
        let (missing, _missing_mock) = host(
            &log,
            |_| 404,
            |_| r#"{"error":{"code":"not_found","message":"not found"}}"#.to_owned(),
        )
        .await;
        let (found, _found_mock) = host(&log, |_| 200, |_| "{}".to_owned()).await;
        let http = http_client().unwrap();

        let refusal = probe(
            &http,
            "event",
            &Route::Event.target(&missing.url(), "KXA"),
            None,
        )
        .await
        .unwrap_err();
        assert!(
            refusal.contains("probe event")
                && refusal.contains("/trade-api/v2/events/KXA")
                && refusal.contains("Error(404)"),
            "{refusal}"
        );
        probe(
            &http,
            "event",
            &Route::Event.target(&found.url(), "KXA"),
            None,
        )
        .await
        .unwrap();
    }

    #[test]
    fn a_signed_phase_with_a_reply_that_is_neither_2xx_nor_429_is_not_a_finding() {
        let ok = (Duration::from_secs(1), Reply::Ok);
        let throttled = (
            Duration::from_secs(2),
            Reply::Throttled {
                code: "x".into(),
                retry_after: None,
            },
        );
        assert_eq!(check_accepted("control", &[ok.clone(), throttled]), Ok(()));

        let rejected = (Duration::from_secs(3), Reply::Error(401));
        let message = check_accepted("control", &[ok, rejected]).unwrap_err();
        assert!(
            message
                .contains("control: 1 replies were neither 2xx nor 429, the first with status 401"),
            "{message}"
        );
    }

    #[tokio::test]
    async fn a_listing_walk_pages_until_the_cursor_ends_and_leaves_the_combinations_out() {
        let log = Log::default();
        let first = fixtures!("predictions").text("markets_page");
        let mut last: serde_json::Value =
            serde_json::from_str(&first).expect("the fixture is JSON");
        let cursor = last["cursor"]
            .as_str()
            .expect("the fixture's page has a cursor")
            .to_owned();
        last["cursor"] = serde_json::json!("");
        let last = last.to_string();
        let (server, _mock) = host(
            &log,
            |_| 200,
            move |request| {
                let target = request.path_and_query();
                if target.contains("endpoint_costs") {
                    r#"{"default_cost":10,"endpoint_costs":[]}"#.to_owned()
                } else if target.contains("cursor=") {
                    last.clone()
                } else {
                    first.clone()
                }
            },
        )
        .await;
        let kalshi = Predictions::builder()
            .base_url(server.url())
            .build()
            .unwrap();

        let listing = walk(&kalshi, 5).await.unwrap();

        let walks: Vec<String> = log
            .all()
            .into_iter()
            .map(|request| request.path_and_query)
            .filter(|target| target.starts_with("/trade-api/v2/markets?"))
            .collect();
        assert_eq!(walks.len(), 2, "{walks:?}");
        for pair in ["status=open", "mve_filter=exclude", "limit=1000"] {
            assert!(walks[0].contains(pair), "{pair}: {walks:?}");
        }
        assert!(!walks[0].contains("cursor="), "{walks:?}");
        assert!(walks[1].contains(&format!("cursor={cursor}")), "{walks:?}");
        // The fixture's five markets, each once though both pages hold them.
        assert_eq!(listing.tickers.len(), 5);
        assert_eq!(listing.tickers[0], "KXUAEPLSPREAD-26OCT18KHFALW-ALW4");
        assert_eq!(listing.events.len(), 5);
        assert_eq!(listing.events[0], "KXUAEPLSPREAD-26OCT18KHFALW");
    }

    #[tokio::test]
    async fn a_worker_parked_in_the_pacer_when_the_429_lands_sends_nothing_more() {
        let log = Log::default();
        let (server, _mock) = host(
            &log,
            |_| 429,
            |_| {
                r#"{"error":{"code":"too_many_requests","message":"too many requests"}}"#.to_owned()
            },
        )
        .await;
        let base = server.url();
        let targets: Vec<Target> = ids(4)
            .iter()
            .map(|id| Route::Market.target(&base, id))
            .collect();
        let http = http_client().unwrap();

        // Four workers share a slot every 200 ms: the first sends at once, and the other three are
        // parked in the pacer, until 200, 400 and 600 ms, when the first has long since been told
        // `429`. None of them may send after it.
        let replies = drive(&http, targets, None, 5.0, 5, 4).await;

        assert_eq!(replies.len(), 1, "{replies:?}");
        assert!(first_throttle(&replies).is_some());
        assert_eq!(log.all().len(), 1, "{:?}", log.all());
    }
}
