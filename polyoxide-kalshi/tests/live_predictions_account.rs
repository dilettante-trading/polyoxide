//! Live tests against Kalshi's demo host, signed with the owner's demo key.
//!
//! These hit the real host and need network access and the demo key, so they are `#[ignore]`d.
//! The key comes from `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` (the PEM text). A
//! run without them fails as `auth-gated`, which the nightly skips. Run with:
//! ```sh
//! cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_account -- --ignored
//! ```

use polyoxide_kalshi::predictions::{Credentials, Predictions, DEMO};
use polyoxide_test_support::{load_env, ResultExt};

/// A client on the demo host that signs with the demo key. Fails the test as `auth-gated` when
/// either variable is absent or empty.
fn client() -> Predictions {
    let creds = load_env(&["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"])
        .unwrap_or_else(|missing| missing.or_auth_gated());
    let credentials = Credentials::from_pem(
        creds.get("KALSHI_DEMO_API_KEY_ID"),
        creds.get("KALSHI_DEMO_PRIVATE_KEY"),
    )
    .or_fail("the demo key parses");
    Predictions::builder()
        .base_url(DEMO)
        .credentials(credentials)
        .build()
        .or_fail("the demo client builds")
}

#[tokio::test]
#[ignore]
async fn live_account_limits_are_at_least_basics() {
    let limits = client()
        .account()
        .limits()
        .send()
        .await
        .or_fail("account limits");

    // Basic is the floor of every tier: Read holds 600 tokens and refills 200 a second.
    assert!(limits.read.bucket_capacity >= 600);
    assert!(limits.read.refill_rate >= 200);
    assert!(!limits.usage_tier.is_empty());
}

/// A signed public read is accepted. It does not show the signature verifies: the demo host
/// answered a public read signed with an unknown key id with a `200` (docs/specs/kalshi/OBSERVED.md),
/// so `live_account_limits_are_at_least_basics` is the test that exercises the signature.
#[tokio::test]
#[ignore]
async fn live_a_signed_public_read_succeeds() {
    let page = client()
        .markets()
        .list()
        .limit(1)
        .send()
        .await
        .or_fail("a signed markets list");

    assert!(page.markets.len() <= 1, "the limit was not honoured");
}
