//! Core's send loop, driven against a mock server with hooks that record what
//! the loop asks of them.

use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::polymarket::PolymarketRetryPolicy;
use polyoxide_core::{
    ApiError, AttemptInfo, Authenticator, Charge, Decision, DynAuthenticator, HttpClient,
    HttpClientBuilder, LayerId, Refused, RequestMeta, RequestParts, ResponseMeta, RetryConfig,
    RetryPolicy, Throttle,
};
use polyoxide_venue::{Class, Classify};
use reqwest::{Method, StatusCode};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, SubscriberExt};

/// What the hooks and the server saw, in order.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<String>>>);

impl Log {
    fn push(&self, entry: impl Into<String>) {
        self.0.lock().unwrap().push(entry.into());
    }

    fn entries(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }

    fn count(&self, prefix: &str) -> usize {
        self.entries()
            .iter()
            .filter(|e| e.starts_with(prefix))
            .count()
    }
}

/// A throttle that records each call, and refuses every charge when told to.
#[derive(Clone, Default)]
struct Recorder {
    log: Log,
    holds: Arc<Mutex<Vec<Duration>>>,
    refuse: bool,
}

impl Recorder {
    fn holds(&self) -> Vec<Duration> {
        self.holds.lock().unwrap().clone()
    }
}

impl Throttle for Recorder {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        self.log
            .push(format!("acquire {} {}", meta.method, meta.path));
        if self.refuse {
            return Err(Refused {
                layer: LayerId("write"),
                units: 20,
                capacity: 10,
            });
        }
        Ok(Charge::none())
    }

    fn observe(&self, _charge: &Charge, response: &ResponseMeta<'_>, attempt: &AttemptInfo) {
        self.log.push(format!(
            "observe {} attempt {} left {}",
            response.status.as_u16(),
            attempt.attempt,
            attempt.retries_left
        ));
    }

    fn hold(&self, delay: Duration) {
        self.log.push("hold");
        self.holds.lock().unwrap().push(delay);
    }
}

/// A policy that records each decision and defers to Polymarket's.
struct Deciding(Log);

impl RetryPolicy for Deciding {
    fn decide(
        &self,
        response: &ResponseMeta<'_>,
        attempt: &AttemptInfo,
        schedule: &RetryConfig,
    ) -> Decision {
        self.0.push(format!("decide {}", response.status.as_u16()));
        PolymarketRetryPolicy.decide(response, attempt, schedule)
    }
}

/// An authenticator that records the attempt it signs and stamps it on the
/// request, where the server reads it back.
struct Stamp(Log);

impl Authenticator for Stamp {
    async fn sign(&self, parts: &mut RequestParts, attempt: u32) -> Result<(), ApiError> {
        self.0.push(format!("sign {attempt}"));
        parts.headers.insert("x-attempt", attempt.into());
        Ok(())
    }
}

/// A server answering `GET path` with `statuses` in turn, then the last of
/// them for good, logging each request it serves as `send <x-attempt>`. The
/// mock expects exactly `hits` requests.
async fn scripted(path: &str, statuses: &[usize], hits: usize, log: &Log) -> (ServerGuard, Mock) {
    capture_warnings();
    let mut server = Server::new_async().await;
    let statuses = statuses.to_vec();
    let served = AtomicUsize::new(0);
    let log = log.clone();
    let mock = server
        .mock("GET", path)
        .match_query(Matcher::Any)
        .with_status_code_from_request(move |request| {
            let stamp = request
                .header("x-attempt")
                .first()
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-")
                .to_owned();
            log.push(format!("send {stamp}"));
            let n = served.fetch_add(1, Ordering::SeqCst);
            statuses[n.min(statuses.len() - 1)]
        })
        .with_body("ok")
        .expect(hits)
        .create_async()
        .await;
    (server, mock)
}

fn schedule(max_retries: u32, initial_backoff_ms: u64) -> RetryConfig {
    RetryConfig {
        max_retries,
        initial_backoff_ms,
        max_backoff_ms: 10_000,
    }
}

/// A client on `server` with Polymarket's policy, recording through `log`.
fn client(server: &ServerGuard, throttle: &Recorder, config: RetryConfig) -> HttpClient {
    HttpClientBuilder::new(server.url())
        .with_throttle(throttle.clone())
        .with_retry_policy(Deciding(throttle.log.clone()))
        .with_retry_config(config)
        .build()
        .unwrap()
}

fn rows() -> RequestParts {
    RequestParts::new(Method::GET, "/v1/rows")
}

#[tokio::test]
async fn each_attempt_runs_acquire_sign_send_observe_decide_hold_in_order() {
    let throttle = Recorder::default();
    let log = throttle.log.clone();
    let (server, mock) = scripted("/v1/rows", &[429, 200], 2, &log).await;
    let http = client(&server, &throttle, schedule(3, 1));
    let stamp = Stamp(log.clone());

    let response = http
        .send(rows(), &[], Some(DynAuthenticator::from_ref(&stamp)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    mock.assert_async().await;

    assert_eq!(
        log.entries(),
        [
            "acquire GET /v1/rows",
            "sign 0",
            "send 0",
            "observe 429 attempt 0 left 3",
            "decide 429",
            "hold",
            "acquire GET /v1/rows",
            "sign 1",
            "send 1",
            "observe 200 attempt 1 left 2",
            "decide 200",
        ]
    );
}

#[tokio::test]
async fn acquire_waits_for_the_permit() {
    let throttle = Recorder::default();
    let (server, mock) = scripted("/v1/rows", &[200], 1, &throttle.log).await;
    let http = HttpClientBuilder::new(server.url())
        .with_throttle(throttle.clone())
        .with_max_concurrent(1)
        .build()
        .unwrap();

    let permit = http.acquire_concurrency().await.unwrap();
    let sending = {
        let http = http.clone();
        tokio::spawn(async move { http.send(rows(), &[], None).await })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        throttle.log.count("acquire"),
        0,
        "the throttle charged a request that had no permit"
    );

    drop(permit);
    let response = sending.await.unwrap().unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(throttle.log.count("acquire"), 1);
    mock.assert_async().await;
}

#[tokio::test]
async fn sign_runs_on_every_attempt_with_its_number() {
    let throttle = Recorder::default();
    let log = throttle.log.clone();
    let (server, mock) = scripted("/v1/rows", &[429, 425, 200], 3, &log).await;
    let http = client(&server, &throttle, schedule(3, 1));
    let stamp = Stamp(log.clone());

    http.send(rows(), &[], Some(DynAuthenticator::from_ref(&stamp)))
        .await
        .unwrap();
    mock.assert_async().await;

    let signed: Vec<_> = log
        .entries()
        .into_iter()
        .filter(|e| e.starts_with("sign") || e.starts_with("send"))
        .collect();
    assert_eq!(
        signed,
        ["sign 0", "send 0", "sign 1", "send 1", "sign 2", "send 2"],
        "each attempt is signed afresh, and the server sees that attempt's signature"
    );
}

#[tokio::test]
async fn observe_sees_the_last_attempt() {
    let throttle = Recorder::default();
    let (server, mock) = scripted("/v1/rows", &[429], 2, &throttle.log).await;
    let http = client(&server, &throttle, schedule(1, 1));

    let err = http.send(rows(), &[], None).await.unwrap_err();
    assert_eq!(failed(&err), StatusCode::TOO_MANY_REQUESTS);
    mock.assert_async().await;

    let observed: Vec<_> = throttle
        .log
        .entries()
        .into_iter()
        .filter(|e| e.starts_with("observe"))
        .collect();
    assert_eq!(
        observed,
        [
            "observe 429 attempt 0 left 1",
            "observe 429 attempt 1 left 0"
        ],
        "the attempt with no retry left must be observed too"
    );
}

#[tokio::test]
async fn a_zero_wait_still_sleeps_the_floor() {
    // A 425 is retried with a wait of zero and no hold, so nothing but the
    // loop's own floor stands between the two attempts.
    let throttle = Recorder::default();
    let (server, mock) = scripted("/v1/rows", &[425, 200], 2, &throttle.log).await;
    let http = client(&server, &throttle, schedule(3, 300));

    let start = Instant::now();
    let response = http.send(rows(), &[], None).await.unwrap();
    let elapsed = start.elapsed();
    assert_eq!(response.status(), StatusCode::OK);
    mock.assert_async().await;
    assert!(throttle.holds().is_empty(), "a 425 holds nothing");
    assert!(
        elapsed >= Duration::from_millis(225),
        "the retry went out after {elapsed:?}: a policy's zero wait replaced the \
         loop's floor of at least 225ms"
    );
}

#[tokio::test]
async fn a_429_with_no_retry_left_still_holds() {
    let throttle = Recorder::default();
    let (server, mock) = scripted("/v1/rows", &[429], 1, &throttle.log).await;
    let http = client(&server, &throttle, schedule(0, 400));

    let err = http.send(rows(), &[], None).await.unwrap_err();
    assert_eq!(failed(&err), StatusCode::TOO_MANY_REQUESTS);
    mock.assert_async().await;

    let holds = throttle.holds();
    assert_eq!(
        holds.len(),
        1,
        "a 429 with no retry left must still hold every request: {holds:?}"
    );
    assert!(
        (300..=500).contains(&holds[0].as_millis()),
        "the hold is the schedule's first delay, 300-500ms: {:?}",
        holds[0]
    );
}

#[tokio::test]
async fn the_429_hold_is_retry_delay_zero_not_the_attempts_wait() {
    // Three 429s at a 40ms base: the retries wait 30-50ms, 60-100ms and
    // 120-200ms, while every hold stays at the first delay.
    let throttle = Recorder::default();
    let (server, mock) = scripted("/v1/rows", &[429, 429, 429, 200], 4, &throttle.log).await;
    let http = client(&server, &throttle, schedule(3, 40));

    let start = Instant::now();
    let response = http.send(rows(), &[], None).await.unwrap();
    let elapsed = start.elapsed();
    assert_eq!(response.status(), StatusCode::OK);
    mock.assert_async().await;

    let holds = throttle.holds();
    assert_eq!(holds.len(), 3, "{holds:?}");
    for hold in &holds {
        assert!(
            (30..=50).contains(&hold.as_millis()),
            "a hold of {hold:?} is not retry_delay(0); the later attempts' waits must not \
             become the client-wide hold"
        );
    }
    assert!(
        elapsed >= Duration::from_millis(210),
        "three retries took {elapsed:?}; each waits its own attempt's backoff"
    );
}

#[tokio::test]
async fn a_425_retries_without_holding() {
    let throttle = Recorder::default();
    let (server, mock) = scripted("/v1/rows", &[425, 200], 2, &throttle.log).await;
    let http = client(&server, &throttle, schedule(3, 1));

    let response = http.send(rows(), &[], None).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    mock.assert_async().await;
    assert!(
        throttle.holds().is_empty(),
        "a 425 is the matching engine restarting; its siblings are not held"
    );
}

#[tokio::test]
async fn the_default_policy_does_not_retry_425() {
    let throttle = Recorder::default();
    let (server, mock) = scripted("/v1/rows", &[425], 1, &throttle.log).await;
    let http = HttpClientBuilder::new(server.url())
        .with_throttle(throttle.clone())
        .with_retry_config(schedule(3, 1))
        .build()
        .unwrap();

    let err = http.send(rows(), &[], None).await.unwrap_err();
    assert_eq!(failed(&err), StatusCode::TOO_EARLY);
    mock.assert_async().await;
    assert!(throttle.holds().is_empty());
}

#[tokio::test]
async fn a_5xx_and_a_408_are_not_retried() {
    for status in [500, 502, 503, 408] {
        let throttle = Recorder::default();
        let (server, mock) = scripted("/v1/rows", &[status], 1, &throttle.log).await;
        let http = client(&server, &throttle, schedule(3, 1));

        let err = http.send(rows(), &[], None).await.unwrap_err();
        assert_eq!(failed(&err).as_u16(), status as u16);
        mock.assert_async().await;
        assert!(throttle.holds().is_empty(), "{status} held the client");
    }
}

#[tokio::test]
async fn a_transport_error_is_not_retried_and_skips_observe_and_decide() {
    // Bind a port, then free it, so nothing is listening there.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let throttle = Recorder::default();
    let http = HttpClientBuilder::new(format!("http://127.0.0.1:{port}"))
        .with_throttle(throttle.clone())
        .with_retry_policy(Deciding(throttle.log.clone()))
        .with_retry_config(schedule(3, 1))
        .build()
        .unwrap();

    let err = http.send(rows(), &[], None).await.unwrap_err();
    assert!(matches!(err, ApiError::Network(_)), "{err:?}");
    assert_eq!(err.class(), Class::Network);
    assert_eq!(
        throttle.log.entries(),
        ["acquire GET /v1/rows"],
        "no response, so nothing to observe or decide, and no retry"
    );
}

#[tokio::test]
async fn a_refused_charge_sends_nothing() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/rows")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let throttle = Recorder {
        refuse: true,
        ..Recorder::default()
    };
    let http = client(&server, &throttle, schedule(3, 1));
    let stamp = Stamp(throttle.log.clone());

    let err = http
        .send(rows(), &[], Some(DynAuthenticator::from_ref(&stamp)))
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            ApiError::Refused(Refused {
                layer: LayerId("write"),
                units: 20,
                capacity: 10
            })
        ),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::InvalidRequest);
    assert_eq!(throttle.log.entries(), ["acquire GET /v1/rows"]);
    mock.assert_async().await;
}

#[tokio::test]
async fn get_bytes_retries_a_429_and_holds() {
    // Core's default policy: get_bytes runs on the loop, so a 429 is retried
    // and holds the client like any other request.
    let throttle = Recorder::default();
    let (server, mock) = scripted("/v1/rows", &[429, 200], 2, &throttle.log).await;
    let http = HttpClientBuilder::new(server.url())
        .with_throttle(throttle.clone())
        .with_retry_config(schedule(3, 1))
        .build()
        .unwrap();

    let body = http.get_bytes("/v1/rows", &[]).await.unwrap();
    assert_eq!(body, b"ok");
    mock.assert_async().await;
    assert_eq!(throttle.holds().len(), 1);
    assert_eq!(throttle.log.count("acquire"), 2);
}

/// Every `WARN` logged while this binary runs, as `(target, message)`.
static WARNINGS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Installs, once and before any request is sent, the global subscriber that
/// fills [`WARNINGS`]. A subscriber scoped to one test would race: a callsite
/// first reached from a test with none caches `Interest::never` for every
/// thread, and the scoped test then sees nothing.
fn capture_warnings() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        tracing::subscriber::set_global_default(tracing_subscriber::registry().with(Warnings))
            .expect("this binary installs no other subscriber");
    });
}

/// The warnings that name `path`, which each test keeps to itself.
fn warnings_on(path: &str) -> Vec<(String, String)> {
    let needle = format!(" on {path},");
    WARNINGS
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, message)| message.contains(&needle))
        .cloned()
        .collect()
}

struct Warnings;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Warnings {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        if *event.metadata().level() != tracing::Level::WARN {
            return;
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        WARNINGS
            .lock()
            .unwrap()
            .push((event.metadata().target().to_owned(), message.0));
    }
}

struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

#[tokio::test]
async fn a_retry_warns_once_under_polyoxide_core() {
    for (status, reason) in [(429, "Too Many Requests"), (425, "Too Early")] {
        let path = format!("/v1/retry-{status}");
        let throttle = Recorder::default();
        let (server, mock) = scripted(&path, &[status, 200], 2, &throttle.log).await;
        let http = client(&server, &throttle, schedule(3, 1));

        http.send(RequestParts::new(Method::GET, &path), &[], None)
            .await
            .unwrap();
        mock.assert_async().await;

        let seen = warnings_on(&path);
        assert_eq!(seen.len(), 1, "one retry, one warning: {seen:?}");
        let (target, message) = &seen[0];
        assert!(target.starts_with("polyoxide_core"), "{target}");
        let expected = format!("Retriable status {status} {reason} on {path}, retry 1 after ");
        assert!(
            message.starts_with(&expected) && message.ends_with("ms"),
            "{message:?} is not {expected:?}<ms>ms"
        );
    }
}

#[tokio::test]
async fn a_hold_with_no_retry_left_warns() {
    let path = "/v1/held";
    let throttle = Recorder::default();
    let (server, mock) = scripted(path, &[429], 1, &throttle.log).await;
    let http = client(&server, &throttle, schedule(0, 400));

    http.send(RequestParts::new(Method::GET, path), &[], None)
        .await
        .unwrap_err();
    mock.assert_async().await;

    let seen = warnings_on(path);
    assert_eq!(seen.len(), 1, "{seen:?}");
    let (target, message) = &seen[0];
    assert!(target.starts_with("polyoxide_core"), "{target}");
    let held = throttle.holds()[0].as_millis();
    assert_eq!(
        message,
        &format!(
            "Status 429 Too Many Requests on {path}, not retried: every request held {held}ms"
        )
    );
}

#[test]
fn a_429_with_no_retry_left_is_fail_with_its_hold() {
    let schedule = schedule(3, 400);
    let headers = reqwest::header::HeaderMap::new();
    let decide = |status: StatusCode, attempt: u32| {
        PolymarketRetryPolicy.decide(
            &ResponseMeta {
                status,
                headers: &headers,
            },
            &schedule.attempt_info(attempt),
            &schedule,
        )
    };

    let last = decide(StatusCode::TOO_MANY_REQUESTS, 3);
    assert_eq!(last.outcome, polyoxide_core::Outcome::Fail);
    let hold = last.hold.expect("a 429 with no retry left still holds");
    assert!(
        (300..=500).contains(&hold.as_millis()),
        "the hold is the schedule's first delay, 300-500ms: {hold:?}"
    );

    // With a retry left, the same 429 is retried with the same hold.
    let earlier = decide(StatusCode::TOO_MANY_REQUESTS, 2);
    assert_eq!(
        earlier.outcome,
        polyoxide_core::Outcome::Retry(Duration::ZERO)
    );
    assert!(earlier.hold.is_some());

    // A 425 out of attempts fails too, and still holds nobody.
    let early = decide(StatusCode::TOO_EARLY, 3);
    assert_eq!(early.outcome, polyoxide_core::Outcome::Fail);
    assert_eq!(early.hold, None);
}

/// Retries a 429 at once, holding the throttle for a long time, as Binance's
/// policy does when the `Retry-After` outlasts the loop's floor.
struct LongHold;

impl RetryPolicy for LongHold {
    fn decide(
        &self,
        response: &ResponseMeta<'_>,
        _attempt: &AttemptInfo,
        _schedule: &RetryConfig,
    ) -> Decision {
        if response.status == StatusCode::TOO_MANY_REQUESTS {
            Decision {
                outcome: polyoxide_core::Outcome::Retry(Duration::ZERO),
                hold: Some(Duration::from_secs(5)),
            }
        } else {
            Decision {
                outcome: polyoxide_core::Outcome::Done,
                hold: None,
            }
        }
    }
}

#[tokio::test]
async fn a_retry_s_warning_names_the_hold_it_waits_out() {
    // The retry sleeps the loop's 1ms floor, then waits out the 5s hold in
    // the throttle: the warning names the wait, not the floor. This throttle
    // records the hold without enforcing it, so the test does not wait.
    let path = "/v1/long-hold";
    let throttle = Recorder::default();
    let (server, mock) = scripted(path, &[429, 200], 2, &throttle.log).await;
    let http = HttpClientBuilder::new(server.url())
        .with_throttle(throttle.clone())
        .with_retry_policy(LongHold)
        .with_retry_config(schedule(3, 1))
        .build()
        .unwrap();

    http.send(RequestParts::new(Method::GET, path), &[], None)
        .await
        .unwrap();
    mock.assert_async().await;

    let seen = warnings_on(path);
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(
        seen[0].1,
        format!("Retriable status 429 Too Many Requests on {path}, retry 1 after 5000ms")
    );
}

/// The status of the response a failed send carries.
fn failed(err: &ApiError) -> StatusCode {
    match err {
        ApiError::Response(response) => response.status,
        other => panic!("expected a response, got {other:?}"),
    }
}

/// Fails a 200 and is done with a 404: the outcome, not the status, decides
/// whether `send` returns the response.
struct Inverted;

impl RetryPolicy for Inverted {
    fn decide(
        &self,
        response: &ResponseMeta<'_>,
        _attempt: &AttemptInfo,
        _schedule: &RetryConfig,
    ) -> Decision {
        Decision {
            outcome: if response.status.is_success() {
                polyoxide_core::Outcome::Fail
            } else {
                polyoxide_core::Outcome::Done
            },
            hold: None,
        }
    }
}

#[tokio::test]
async fn fail_is_an_error_and_done_is_the_response_whatever_the_status() {
    let throttle = Recorder::default();
    let (ok_server, ok_mock) = scripted("/v1/rows", &[200], 1, &throttle.log).await;
    let http = HttpClientBuilder::new(ok_server.url())
        .with_retry_policy(Inverted)
        .build()
        .unwrap();
    let err = http.send(rows(), &[], None).await.unwrap_err();
    ok_mock.assert_async().await;
    match &err {
        ApiError::Response(response) => {
            assert_eq!(response.status, StatusCode::OK);
            assert_eq!(response.body, "ok");
        }
        other => panic!("a `Fail` on a 200 is still an error: {other:?}"),
    }

    let (missing_server, missing_mock) = scripted("/v1/rows", &[404], 2, &throttle.log).await;
    let http = HttpClientBuilder::new(missing_server.url())
        .with_retry_policy(Inverted)
        .build()
        .unwrap();
    let response = http.send(rows(), &[], None).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // `Request::send_raw` never hands a caller a failed response as data.
    let raw = polyoxide_core::Request::<(), ApiError>::new(http, "/v1/rows")
        .send_raw()
        .await
        .unwrap_err();
    missing_mock.assert_async().await;
    assert_eq!(failed(&raw), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_failed_response_carries_its_headers_body_and_retry_after() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/rows")
        .with_status(429)
        .with_header("retry-after", "7")
        .with_header("x-trace", "t-1")
        .with_body(r#"{"error":"slow down"}"#)
        .expect(1)
        .create_async()
        .await;
    let http = HttpClientBuilder::new(server.url())
        .with_retry_config(schedule(0, 1))
        .build()
        .unwrap();

    let err = http.send(rows(), &[], None).await.unwrap_err();
    mock.assert_async().await;
    let ApiError::Response(response) = &err else {
        panic!("expected a response, got {err:?}");
    };
    assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers["x-trace"], "t-1");
    assert_eq!(response.body, r#"{"error":"slow down"}"#);
    assert_eq!(response.message, "slow down");
    assert_eq!(response.retry_after, Some(Duration::from_secs(7)));
    assert_eq!(
        err.class(),
        Class::RateLimited {
            retry_after: Some(Duration::from_secs(7))
        }
    );
    assert_eq!(err.retry_after(), Some(Duration::from_secs(7)));
}

/// A venue's error, whose `From<ApiError>` is its one decode: a 404 becomes
/// its own variant, and anything else stays core's.
#[derive(Debug)]
enum VenueError {
    NotFound(String),
    Core(#[allow(dead_code)] ApiError),
}

impl From<ApiError> for VenueError {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::Response(response) if response.status == StatusCode::NOT_FOUND => {
                Self::NotFound(response.body)
            }
            other => Self::Core(other),
        }
    }
}

#[tokio::test]
async fn a_done_failure_reaches_the_venue_s_decode() {
    // `Inverted` is done with a 404, so `send` hands it back, and `send_raw`
    // and `send` turn it into the caller's error through its decode, not
    // around it.
    let throttle = Recorder::default();
    let (server, mock) = scripted("/v1/rows", &[404], 2, &throttle.log).await;
    let http = HttpClientBuilder::new(server.url())
        .with_retry_policy(Inverted)
        .build()
        .unwrap();

    let raw =
        polyoxide_core::Request::<serde_json::Value, VenueError>::new(http.clone(), "/v1/rows")
            .send_raw()
            .await
            .unwrap_err();
    assert!(
        matches!(&raw, VenueError::NotFound(body) if body == "ok"),
        "{raw:?}"
    );

    let decoded = polyoxide_core::Request::<serde_json::Value, VenueError>::new(http, "/v1/rows")
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(&decoded, VenueError::NotFound(body) if body == "ok"),
        "{decoded:?}"
    );
    mock.assert_async().await;
}
