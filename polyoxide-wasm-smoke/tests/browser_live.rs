//! The nightly's live browser checks (spec D14, D17): the browser implementations against the
//! real hosts, run by `scripts/wasm_live.sh` in headless Chrome inside a dedicated Worker.
//!
//! They are gated by the `live` feature, not `#[ignore]`: nextest does not run wasm, and the
//! script selects the target. A failure is tagged through `polyoxide-test-support`, whose tags
//! build on wasm32, so the classifier reads the same `polyoxide-class=<tag>` line it reads for the
//! native suites; an untagged panic is `real`.

#![cfg(all(target_arch = "wasm32", feature = "live"))]

use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_dedicated_worker);

/// The self-check `scripts/wasm_live.sh` runs first: built with `POLYOXIDE_LIVE_PROBE` set, this
/// test fails with a `transient` tag, and the script checks that the classifier reads that tag
/// from the runner's output. Built without it, the test passes at once.
#[wasm_bindgen_test]
fn probe_tag_line() {
    if option_env!("POLYOXIDE_LIVE_PROBE").is_none() {
        return;
    }
    polyoxide_test_support::transient("the runner's output must carry this test's tag line");
}
