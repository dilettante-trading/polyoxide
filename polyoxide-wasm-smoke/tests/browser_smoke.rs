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
        http::{
            header::{CONTENT_TYPE, RETRY_AFTER},
            HeaderValue, Method, StatusCode,
        },
        ApiError, FetchTransport, HttpClient, HttpClientBuilder, Request, RequestParts,
        RetryConfig, TransportKind,
    };
    use polyoxide_rt::{BrowserRuntime, Runtime};
    use polyoxide_wasm_smoke::{BINARY_BODY, ECHO_BODY_PATH};
    use serde_json::Value;
    use wasm_bindgen_test::wasm_bindgen_test;

    const BASE: &str = env!("POLYOXIDE_SMOKE_HTTP");
    // The slow route's server is its own: a sleeping slow route holds its server's one thread,
    // and must not hold up the requests that follow it.
    const SLOW_BASE: &str = env!("POLYOXIDE_SMOKE_SLOW");
    // The stalled-body route's server is its own too: a hand-written responder, since mockito
    // cannot send a head and hold the body behind it.
    const STALL_BASE: &str = env!("POLYOXIDE_SMOKE_STALL");

    /// An `HttpClient` on the fetch transport and the browser runtime, with no retries, so a 429
    /// is the first answer and a timeout the only wait. `HttpRequest` has no public constructor,
    /// so the transport is driven the way a venue crate drives it: through a client built with
    /// `http_transport(..)`.
    fn client_to(base: &str, timeout_ms: u64) -> HttpClient {
        HttpClientBuilder::new(base)
            .runtime(Arc::new(BrowserRuntime))
            .http_transport(Arc::new(FetchTransport::new()))
            .with_retry_config(RetryConfig {
                max_retries: 0,
                initial_backoff_ms: 1,
                max_backoff_ms: 1,
            })
            .timeout_ms(timeout_ms)
            .build()
            .expect("a client on the fetch transport")
    }

    /// A request through [`client_to`]'s client.
    fn request_to(base: &str, path: &str, timeout_ms: u64) -> Request<Value, ApiError> {
        Request::new(client_to(base, timeout_ms), path)
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

    // Why: issue #87. The page's fetch sends a body as given only when the send loop hands it
    // bytes, so a body that is not UTF-8 comes back from the echo route byte for byte.
    #[wasm_bindgen_test]
    async fn a_body_that_is_not_utf8_crosses_fetch_byte_for_byte() {
        let mut parts = RequestParts::new(Method::POST, ECHO_BODY_PATH);
        parts
            .headers
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/gzip"));
        parts.body = Some(BINARY_BODY.into());
        let response = client_to(BASE, 10_000)
            .send(parts, &[], None)
            .await
            .expect("the echo route answers");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.bytes().await.expect("a body").as_ref(),
            BINARY_BODY
        );
    }

    #[wasm_bindgen_test]
    async fn a_slow_route_times_out_as_a_transport_timeout() {
        let start = BrowserRuntime.now();
        let err = request_to(SLOW_BASE, "/slow", 300)
            .send_raw()
            .await
            .expect_err("the slow route must time out");
        match err {
            ApiError::Transport(err) => assert_eq!(err.kind(), TransportKind::Timeout, "{err}"),
            other => panic!("expected a transport timeout, got {other:?}"),
        }
        let elapsed = BrowserRuntime.now().saturating_duration_since(start);
        assert!(
            elapsed >= Duration::from_millis(250) && elapsed < Duration::from_millis(1400),
            "the timeout fired after {elapsed:?}, not in the window after the request's 300 ms \
             deadline and before the server's 1500 ms answer"
        );
    }

    // Why: no host test reaches the wasm32 body path. The head comes back at once and the body
    // stalls past the request's deadline; reqwest's wasm client reports that cut-short read as
    // a decode error, so `Timeout` here proves FetchTransport's own clock decided it.
    #[wasm_bindgen_test]
    async fn a_body_read_that_outlives_the_deadline_is_a_transport_timeout() {
        let start = BrowserRuntime.now();
        let response = request_to(STALL_BASE, "/stall", 500)
            .send_raw()
            .await
            .expect("the head arrives before the deadline");
        assert_eq!(response.status(), StatusCode::OK);
        let head_at = BrowserRuntime.now().saturating_duration_since(start);
        assert!(
            head_at < Duration::from_millis(500),
            "the head took {head_at:?}"
        );
        let err = response
            .bytes()
            .await
            .expect_err("the stalled body must not arrive");
        match err {
            ApiError::Transport(err) => assert_eq!(err.kind(), TransportKind::Timeout, "{err}"),
            other => panic!("expected a transport timeout, got {other:?}"),
        }
        let elapsed = BrowserRuntime.now().saturating_duration_since(start);
        assert!(
            elapsed >= Duration::from_millis(450) && elapsed < Duration::from_millis(2_500),
            "the body timed out after {elapsed:?}, not near the 500 ms deadline and before the server's 3000 ms rest"
        );
    }

    #[wasm_bindgen_test]
    async fn a_429_that_exposes_retry_after_carries_it_into_the_error_response() {
        let err = request("/limited-exposed", 10_000)
            .send_raw()
            .await
            .expect_err("a 429 is an error");
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
            .expect_err("a 429 is an error");
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

mod socket {
    use std::{sync::Arc, time::Duration};

    use futures_util::{SinkExt, StreamExt};
    use polyoxide_rt::{timeout, BrowserRuntime, Runtime};
    use polyoxide_ws::{BoxSocket, Kind, SocketTransport, WebSocketTransport, WsError};
    use tungstenite::{http::Request, Message};
    use wasm_bindgen_test::wasm_bindgen_test;

    const URL: &str = env!("POLYOXIDE_SMOKE_WS");
    const WAIT: Duration = Duration::from_secs(5);

    const NO_HEADER: &str = "a handshake header is not supported by the browser transport";
    const NO_PING: &str = "a ping frame is not supported by the browser transport";

    async fn open(header: Option<(&str, &str)>) -> Result<BoxSocket, WsError> {
        let mut request = Request::builder().uri(URL);
        if let Some((name, value)) = header {
            request = request.header(name, value);
        }
        let request = request.body(()).expect("a handshake request");
        let runtime: Arc<dyn Runtime> = Arc::new(BrowserRuntime);
        WebSocketTransport::new()
            .connect(request, WAIT, runtime)
            .await
    }

    async fn next(socket: &mut BoxSocket) -> Message {
        timeout(&BrowserRuntime, WAIT, socket.next())
            .await
            .expect("a frame within five seconds")
            .expect("the stream ended")
            .expect("a frame, not an error")
    }

    #[wasm_bindgen_test]
    async fn a_text_frame_is_echoed() {
        let mut socket = open(None).await.expect("the socket opens");
        socket
            .send(Message::Text("hello".into()))
            .await
            .expect("send");
        assert_eq!(next(&mut socket).await, Message::Text("echo:hello".into()));
    }

    #[wasm_bindgen_test]
    async fn a_close_arrives_with_its_code_and_reason() {
        let mut socket = open(None).await.expect("the socket opens");
        socket
            .send(Message::Text("close".into()))
            .await
            .expect("send");
        let Message::Close(Some(frame)) = next(&mut socket).await else {
            panic!("the command must be answered with a close frame");
        };
        assert_eq!(u16::from(frame.code), 4001);
        assert_eq!(frame.reason.as_str(), "smoke close");
        assert!(
            timeout(&BrowserRuntime, WAIT, socket.next())
                .await
                .expect("the stream ends within five seconds")
                .is_none(),
            "the stream ends after the close"
        );
    }

    /// A send before the close is yielded succeeds, and one after it fails with
    /// `AlreadyClosed`.
    ///
    /// This does not pin the drop while the browser is closing: Chrome on localhost never holds
    /// that window open, so a send here lands before or after it, never in it. The host
    /// `send_state` tests in `polyoxide-ws` pin the drop. What this pins is the end of the
    /// stream: sends between short polls must all succeed until the close arrives, and the one
    /// after it must fail, so it never fails by a race.
    #[wasm_bindgen_test]
    async fn a_send_before_the_close_is_yielded_succeeds_and_one_after_fails() {
        let mut socket = open(None).await.expect("the socket opens");
        socket
            .send(Message::Text("close".into()))
            .await
            .expect("send");
        let started = BrowserRuntime.now();
        let close = loop {
            socket
                .send(Message::Text("again".into()))
                .await
                .expect("a send before the close is yielded must not fail");
            assert!(
                BrowserRuntime.now().saturating_duration_since(started) < WAIT,
                "no close within five seconds"
            );
            match timeout(&BrowserRuntime, Duration::from_millis(1), socket.next()).await {
                Some(Some(Ok(Message::Close(frame)))) => break frame,
                Some(Some(Ok(_))) | None => {}
                Some(Some(Err(error))) => panic!("an error before the close: {error:?}"),
                Some(None) => panic!("the stream ended without yielding its close"),
            }
        };
        assert_eq!(close.map(|frame| u16::from(frame.code)), Some(4001));
        let err = socket
            .send(Message::Text("late".into()))
            .await
            .expect_err("a send after the close must fail");
        assert!(matches!(err, tungstenite::Error::AlreadyClosed), "{err:?}");
    }

    #[wasm_bindgen_test]
    async fn a_handshake_header_is_refused_before_dialling() {
        // The socket is not `Debug`, so `expect_err` is unavailable.
        let Err(err) = open(Some(("x-smoke", "1"))).await else {
            panic!("a handshake header must be refused");
        };
        assert_eq!(err.kind(), Kind::Unsupported, "{err}");
        assert!(err.to_string().contains(NO_HEADER), "{err}");
    }

    #[wasm_bindgen_test]
    async fn a_ping_frame_is_refused_and_the_socket_stays_usable() {
        let mut socket = open(None).await.expect("the socket opens");
        let err = socket
            .send(Message::Ping(vec![1, 2, 3].into()))
            .await
            .expect_err("a ping frame must be refused");
        match err {
            tungstenite::Error::Io(io) => {
                assert_eq!(io.kind(), std::io::ErrorKind::Unsupported);
                assert!(io.to_string().contains(NO_PING), "{io}");
            }
            other => panic!("expected an I/O error, got {other:?}"),
        }
        socket
            .send(Message::Text("still".into()))
            .await
            .expect("send after the refusal");
        assert_eq!(next(&mut socket).await, Message::Text("echo:still".into()));
    }
}

mod signing {
    use polyoxide_polymarket::clob::{Account, Credentials};
    use wasm_bindgen_test::wasm_bindgen_test;

    // Hardhat's well-known test key #0, the key of the host's golden vector.
    const TEST_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    // The signature py-clob-client (with poly_eip712_structs) produces for TEST_KEY, chain 137,
    // timestamp 1700000000 and nonce 42: the golden vector `clob/core/eip712.rs`'s host test
    // `sign_clob_auth_matches_the_reference_implementation` pins.
    const REFERENCE_SIGNATURE: &str = "0x8e61f918d542a48ff9433fb6cc4c172a2763ad53dda8afa07f77321e2e0a1e3055f29fac1408e0020dca977190fd4f01b1c73b5e2078be29c88a32887945fe2a1b";

    // Why: no host test reaches the wasm32 arm of `sign_digest`, which sits alloy's `?Send`
    // `sign_hash` future in a `SendWrapper`. A signature byte-equal to the reference one proves
    // the wrapped future ran to the end in the browser and signed the same digest.
    #[wasm_bindgen_test]
    async fn a_clob_auth_signature_matches_the_reference_vector() {
        let credentials = Credentials {
            key: "k".into(),
            secret: "c2VjcmV0".into(),
            passphrase: "p".into(),
        };
        let account = Account::new(TEST_KEY, credentials).expect("the test key parses");
        let signature = account
            .sign_clob_auth(137, 1_700_000_000, 42)
            .await
            .expect("the browser signs");
        assert_eq!(signature, REFERENCE_SIGNATURE);
    }
}

mod quota {
    use std::{sync::Arc, time::Duration};

    use polyoxide_core::{http::Method, WindowQuotaTable};
    use polyoxide_rt::{BoxFuture, BrowserRuntime, Instant, Runtime};
    use wasm_bindgen_test::wasm_bindgen_test;

    // Why: a window quota keeps time with governor. A second acquisition that waits about one
    // interval in the browser, without panicking, proves the limiter works there. That it paces
    // on the injected runtime's clock, and not the real one, is proved by the fast-clock test
    // below (`the_wait_is_paced_by_the_injected_runtime_not_by_the_real_clock`); quanta does not
    // panic in a Worker.
    #[wasm_bindgen_test]
    async fn a_second_acquisition_waits_the_interval_on_the_runtimes_clock() {
        let runtime: Arc<dyn Runtime> = Arc::new(BrowserRuntime);
        let mut table = WindowQuotaTable::new(10, Duration::from_secs(3));
        table.with_runtime(Arc::clone(&runtime));
        let limiter = table.build();
        let interval = limiter.effective_quota(&Method::GET, "/quota")[0].interval();
        assert!(
            interval >= Duration::from_millis(100),
            "{interval:?} is too short to see"
        );

        let start = runtime.now();
        limiter.acquire("/quota", Some(&Method::GET)).await;
        let first = runtime.now().saturating_duration_since(start);
        assert!(
            first < interval / 2,
            "the first acquisition waited {first:?}"
        );

        limiter.acquire("/quota", Some(&Method::GET)).await;
        let both = runtime.now().saturating_duration_since(start);
        assert!(
            both >= interval.mul_f64(0.9),
            "two acquisitions took {both:?}, less than the {interval:?} interval"
        );
        assert!(
            both < interval + Duration::from_secs(1),
            "two acquisitions took {both:?}, far past the {interval:?} interval"
        );
    }

    /// The browser's runtime with its clock running four times fast: `now` reads four times the
    /// real time since the runtime was made, and `sleep` waits a quarter of the duration.
    #[derive(Debug)]
    struct Fast {
        origin: Instant,
    }

    impl Fast {
        const SPEED: u32 = 4;
    }

    impl Runtime for Fast {
        fn spawn(&self, task: BoxFuture<()>) {
            BrowserRuntime.spawn(task)
        }
        fn sleep(&self, duration: Duration) -> BoxFuture<()> {
            BrowserRuntime.sleep(duration / Self::SPEED)
        }
        fn now(&self) -> Instant {
            let real = BrowserRuntime.now().saturating_duration_since(self.origin);
            self.origin + real * Self::SPEED
        }
    }

    // Why: on a real clock the test above cannot tell the runtime's clock from quanta's, which
    // reads the same `performance.now()` in a Worker. A runtime that runs four times fast can: a
    // limiter on that clock waits the interval in a quarter of the real time, and one that read
    // quanta's would wait the whole interval and show four times it on the runtime's clock.
    #[wasm_bindgen_test]
    async fn the_wait_is_paced_by_the_injected_runtime_not_by_the_real_clock() {
        let runtime: Arc<dyn Runtime> = Arc::new(Fast {
            origin: BrowserRuntime.now(),
        });
        let mut table = WindowQuotaTable::new(10, Duration::from_secs(3));
        table.with_runtime(Arc::clone(&runtime));
        let limiter = table.build();
        let interval = limiter.effective_quota(&Method::GET, "/quota")[0].interval();

        let real_start = BrowserRuntime.now();
        let start = runtime.now();
        limiter.acquire("/quota", Some(&Method::GET)).await;
        limiter.acquire("/quota", Some(&Method::GET)).await;
        let on_runtime = runtime.now().saturating_duration_since(start);
        let on_real = BrowserRuntime.now().saturating_duration_since(real_start);

        assert!(
            on_runtime >= interval.mul_f64(0.9) && on_runtime < interval + Duration::from_millis(600),
            "the wait took {on_runtime:?} on the runtime's clock, not about the {interval:?} interval"
        );
        assert!(
            on_real < interval.mul_f64(0.6),
            "the wait took {on_real:?} of real time: its sleeps did not go through the runtime"
        );
    }
}
