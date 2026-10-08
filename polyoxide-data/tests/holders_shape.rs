//! The `market` shape rule `live_api.rs` selects holders markets on, pinned
//! offline. It lived in the live suite, the one test there without `#[ignore]`;
//! every test in a `tests/live_*.rs` is now `#[ignore]`d, and CI checks it.

mod common;

use common::is_hash64;

/// Pins the shape rule against values taken from the live trade feed.
///
/// Needs no network, and the reject case is the actual `conditionId` that broke
/// `live_holders` in the 2026-08-26 nightly (issue #32) — `0x` plus 62 hex
/// digits, zero-padded on the right.
#[test]
fn hash64_shape_matches_what_holders_accepts() {
    assert!(is_hash64(
        "0x94f56a80d387a41395ae464e5e3eb2e23d1a6032014b40590074b75aa3447f90"
    ));

    // Straight from `GET /trades`. 62 hex digits, so 64 characters, not 66.
    assert!(!is_hash64(
        "0x03474f36a86039e6c40479b1844401d81a0000000000000000000000000000"
    ));
    // Missing prefix, over-long, non-hex, and the empty string an absent field
    // would produce — the venue rejects every one of these identically.
    assert!(!is_hash64(
        "94f56a80d387a41395ae464e5e3eb2e23d1a6032014b40590074b75aa3447f90"
    ));
    assert!(!is_hash64(
        "0x94f56a80d387a41395ae464e5e3eb2e23d1a6032014b40590074b75aa3447f900"
    ));
    assert!(!is_hash64(
        "0xZZ4f56a80d387a41395ae464e5e3eb2e23d1a6032014b40590074b75aa3447f9"
    ));
    assert!(!is_hash64(""));
}
