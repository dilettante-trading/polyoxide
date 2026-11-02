//! The browser smoke suite: `BrowserRuntime`, `FetchTransport` and `WebSocketTransport` in
//! headless Chrome, inside a dedicated Worker, against `wasm-smoke-server`.
//!
//! Run it with `scripts/wasm_smoke.sh`, which starts the server and exports the addresses the
//! tests read with `env!`: a build without them does not compile, so the suite cannot pass empty.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::wasm_bindgen_test_configure;

wasm_bindgen_test_configure!(run_in_dedicated_worker);

mod runtime {
    use std::{sync::Arc, time::Duration};

    use polyoxide_rt::{BrowserRuntime, Runtime};
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
            assert!(next >= last, "the clock went backwards: {last:?} then {next:?}");
            last = next;
        }
        let before = rt.now();
        rt.sleep(Duration::from_millis(20)).await;
        assert!(rt.now() > before, "the clock did not move across a sleep");
    }
}
