//! The browser smoke suite: `BrowserRuntime`, `FetchTransport` and `WebSocketTransport` in
//! headless Chrome, inside a dedicated Worker, against `wasm-smoke-server`.
//!
//! Run it with `scripts/wasm_smoke.sh`, which starts the server and exports the addresses the
//! tests read with `env!`: a build without them does not compile, so the suite cannot pass empty.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::wasm_bindgen_test_configure;

wasm_bindgen_test_configure!(run_in_dedicated_worker);

mod runtime {
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::Duration,
    };

    use polyoxide_rt::{interval, spawn, timeout, BrowserRuntime, MissedTick, Runtime};
    use wasm_bindgen_test::wasm_bindgen_test;

    fn runtime() -> Arc<dyn Runtime> {
        Arc::new(BrowserRuntime)
    }

    #[wasm_bindgen_test]
    async fn sleep_waits_for_its_duration() {
        let rt = runtime();
        let start = rt.now();
        rt.sleep(Duration::from_millis(50)).await;
        let elapsed = rt.now().saturating_duration_since(start);
        assert!(elapsed >= Duration::from_millis(45), "slept {elapsed:?}");
        assert!(elapsed < Duration::from_secs(2), "slept {elapsed:?}");
    }

    #[wasm_bindgen_test]
    async fn sleep_until_waits_for_its_deadline() {
        let rt = runtime();
        let deadline = rt.now() + Duration::from_millis(50);
        rt.sleep_until(deadline).await;
        let early = deadline.saturating_duration_since(rt.now());
        assert!(early < Duration::from_millis(5), "woke {early:?} early");
    }

    #[wasm_bindgen_test]
    async fn sleep_until_a_deadline_already_past_returns_promptly() {
        let rt = runtime();
        let start = rt.now();
        rt.sleep_until(start).await;
        let elapsed = rt.now().saturating_duration_since(start);
        assert!(elapsed < Duration::from_secs(1), "waited {elapsed:?}");
    }

    #[wasm_bindgen_test]
    async fn now_is_monotonic_and_advances() {
        let rt = runtime();
        let mut last = rt.now();
        for _ in 0..1_000 {
            let next = rt.now();
            assert!(
                next >= last,
                "the clock went backwards: {last:?} then {next:?}"
            );
            last = next;
        }
        let before = rt.now();
        rt.sleep(Duration::from_millis(20)).await;
        assert!(rt.now() > before, "the clock did not move across a sleep");
    }

    #[wasm_bindgen_test]
    async fn timeout_expires_a_future_that_never_finishes() {
        let rt = runtime();
        let start = rt.now();
        let outcome = timeout(
            &*rt,
            Duration::from_millis(40),
            std::future::pending::<u8>(),
        )
        .await;
        assert_eq!(outcome, None);
        let elapsed = rt.now().saturating_duration_since(start);
        assert!(
            elapsed >= Duration::from_millis(35),
            "expired after {elapsed:?}"
        );
    }

    #[wasm_bindgen_test]
    async fn timeout_returns_a_ready_future_even_at_zero() {
        let rt = runtime();
        assert_eq!(timeout(&*rt, Duration::ZERO, async { 4u8 }).await, Some(4));
        assert_eq!(
            timeout(&*rt, Duration::from_secs(5), async { 5u8 }).await,
            Some(5)
        );
    }

    #[wasm_bindgen_test]
    async fn timeout_beats_a_slower_sleep() {
        let rt = runtime();
        let slow = rt.sleep(Duration::from_secs(5));
        assert_eq!(timeout(&*rt, Duration::from_millis(30), slow).await, None);
    }

    #[wasm_bindgen_test]
    async fn interval_ticks_at_once_then_every_period() {
        let rt = runtime();
        let period = Duration::from_millis(40);
        let mut ticks = interval(rt.clone(), period);
        assert_eq!(ticks.period(), period);
        let start = rt.now();
        ticks.tick().await;
        let first = rt.now().saturating_duration_since(start);
        assert!(
            first < Duration::from_millis(30),
            "the first tick took {first:?}"
        );
        ticks.tick().await;
        ticks.tick().await;
        let three = rt.now().saturating_duration_since(start);
        assert!(
            three >= Duration::from_millis(75),
            "three ticks took {three:?}"
        );
    }

    #[wasm_bindgen_test]
    async fn interval_reset_moves_the_next_tick_a_period_out() {
        let rt = runtime();
        let mut ticks = interval(rt.clone(), Duration::from_millis(40));
        ticks.tick().await;
        ticks.reset();
        let before = rt.now();
        ticks.tick().await;
        let waited = rt.now().saturating_duration_since(before);
        assert!(
            waited >= Duration::from_millis(35),
            "waited {waited:?} after a reset"
        );
    }

    #[wasm_bindgen_test]
    async fn burst_fires_missed_ticks_at_once() {
        let rt = runtime();
        let mut ticks = interval(rt.clone(), Duration::from_millis(40));
        ticks.tick().await;
        rt.sleep(Duration::from_millis(130)).await;
        let before = rt.now();
        for _ in 0..3 {
            ticks.tick().await;
        }
        let took = rt.now().saturating_duration_since(before);
        assert!(
            took < Duration::from_millis(30),
            "three missed ticks took {took:?}"
        );
    }

    #[wasm_bindgen_test]
    async fn delay_waits_a_period_after_a_late_tick() {
        let rt = runtime();
        let mut ticks = interval(rt.clone(), Duration::from_millis(40));
        ticks.set_missed_tick(MissedTick::Delay);
        ticks.tick().await;
        rt.sleep(Duration::from_millis(130)).await;
        ticks.tick().await;
        let before = rt.now();
        ticks.tick().await;
        let waited = rt.now().saturating_duration_since(before);
        assert!(
            waited >= Duration::from_millis(35),
            "waited {waited:?} after a late tick"
        );
    }

    #[wasm_bindgen_test]
    async fn spawn_runs_the_future_and_returns_its_output() {
        let rt = runtime();
        let task = spawn(&*rt, async { 7u8 });
        assert_eq!(task.await, Some(7));
    }

    #[wasm_bindgen_test]
    async fn abort_stops_a_task_before_it_finishes() {
        let rt = runtime();
        let ran = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran);
        let sleeper = rt.clone();
        let task = spawn(&*rt, async move {
            sleeper.sleep(Duration::from_millis(200)).await;
            flag.store(true, Ordering::SeqCst);
        });
        rt.sleep(Duration::from_millis(20)).await;
        assert!(!task.is_finished(), "the task ended before it was aborted");
        task.abort();
        assert_eq!(task.await, None);
        rt.sleep(Duration::from_millis(400)).await;
        assert!(
            !ran.load(Ordering::SeqCst),
            "an aborted task must not run on"
        );
    }

    #[wasm_bindgen_test]
    async fn is_finished_turns_true_once_the_task_ends() {
        let rt = runtime();
        let task = spawn(&*rt, async {});
        assert!(
            !task.is_finished(),
            "a spawned task cannot have run before the caller yields"
        );
        rt.sleep(Duration::from_millis(30)).await;
        assert!(task.is_finished());
        assert_eq!(task.await, Some(()));
    }

    #[wasm_bindgen_test]
    async fn a_sleep_past_i32_max_millis_has_not_fired_after_a_second() {
        let rt = runtime();
        // setTimeout fires at once for a delay above i32::MAX ms; the runtime chains shorter ones.
        let past = Duration::from_millis(i32::MAX as u64 + 5_000);
        assert_eq!(
            timeout(&*rt, Duration::from_secs(1), rt.sleep(past)).await,
            None
        );
        assert_eq!(
            timeout(&*rt, Duration::from_millis(100), rt.sleep(Duration::MAX)).await,
            None
        );
    }
}

mod fetch {
    use std::{sync::Arc, time::Duration};

    use polyoxide_core::{
        http::{header::RETRY_AFTER, StatusCode},
        ApiError, FetchTransport, HttpClientBuilder, Request, RetryConfig, TransportKind,
    };
    use polyoxide_rt::{BrowserRuntime, Runtime};
    use serde_json::Value;
    use wasm_bindgen_test::wasm_bindgen_test;

    const BASE: &str = env!("POLYOXIDE_SMOKE_HTTP");
    // The slow route's server is its own: the browser may run these tests concurrently, and a
    // sleeping slow route must not hold up another test's request.
    const SLOW_BASE: &str = env!("POLYOXIDE_SMOKE_SLOW");

    /// A request through an `HttpClient` on the fetch transport and the browser runtime, with no
    /// retries, so a 429 is the first answer and a timeout the only wait. `HttpRequest` has no
    /// public constructor, so the transport is driven the way a venue crate drives it: through a
    /// client built with `http_transport(..)`.
    fn request_to(base: &str, path: &str, timeout_ms: u64) -> Request<Value, ApiError> {
        let client = HttpClientBuilder::new(base)
            .runtime(Arc::new(BrowserRuntime))
            .http_transport(Arc::new(FetchTransport::new()))
            .with_retry_config(RetryConfig {
                max_retries: 0,
                initial_backoff_ms: 1,
                max_backoff_ms: 1,
            })
            .timeout_ms(timeout_ms)
            .build()
            .expect("a client on the fetch transport");
        Request::new(client, path)
    }

    fn request(path: &str, timeout_ms: u64) -> Request<Value, ApiError> {
        request_to(BASE, path, timeout_ms)
    }

    #[wasm_bindgen_test]
    async fn a_200_arrives_whole_through_fetch() {
        let response = request("/ok", 10_000)
            .send_raw()
            .await
            .expect("a 200 through fetch");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.expect("a body"), r#"{"ok":true}"#);
    }

    #[wasm_bindgen_test]
    async fn a_200_decodes_as_json() {
        let value = request("/ok", 10_000).send().await.expect("a decoded 200");
        assert_eq!(value["ok"], true);
    }

    #[wasm_bindgen_test]
    async fn a_slow_route_times_out_as_a_transport_timeout() {
        let start = BrowserRuntime.now();
        let err = request_to(SLOW_BASE, "/slow", 300)
            .send_raw()
            .await
            .err()
            .expect("the slow route must time out");
        match err {
            ApiError::Transport(err) => assert_eq!(err.kind(), TransportKind::Timeout, "{err}"),
            other => panic!("expected a transport timeout, got {other:?}"),
        }
        let elapsed = BrowserRuntime.now().saturating_duration_since(start);
        assert!(
            elapsed < Duration::from_millis(1400),
            "the timeout fired after {elapsed:?}, not before the server's 1500 ms answer"
        );
    }

    #[wasm_bindgen_test]
    async fn a_429_that_exposes_retry_after_carries_it_into_the_error_response() {
        let err = request("/limited-exposed", 10_000)
            .send_raw()
            .await
            .err()
            .expect("a 429 is an error");
        let ApiError::Response(response) = err else {
            panic!("expected an error response, got {err:?}");
        };
        assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.message, "slow down");
        assert!(response.headers.get(RETRY_AFTER).is_some());
        assert_eq!(response.retry_after, Some(Duration::from_secs(7)));
    }

    #[wasm_bindgen_test]
    async fn a_429_that_hides_retry_after_shows_none_to_the_browser() {
        let err = request("/limited-hidden", 10_000)
            .send_raw()
            .await
            .err()
            .expect("a 429 is an error");
        let ApiError::Response(response) = err else {
            panic!("expected an error response, got {err:?}");
        };
        assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.message, "slow down");
        // The server sent it, but a browser reads only what Access-Control-Expose-Headers names.
        assert!(response.headers.get(RETRY_AFTER).is_none());
        assert_eq!(response.retry_after, None);
    }
}
