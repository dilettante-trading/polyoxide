//! The full header set one request goes out with (AD-18).
//!
//! The expected set is a literal, never read from the client under test. CI runs this file in this
//! crate alone with `--no-default-features --features predictions`, in the workspace build, and
//! again beside the umbrella with `--features polyoxide/full`, so a change to what a client sends
//! fails here in one of the three builds, whether a code default (0.37.0's gzip regression was
//! core's builder turning gzip off) or a reqwest feature another member, or another venue's
//! module, switches on.

use polyoxide_kalshi::predictions::Predictions;
use polyoxide_test_support::query::headers_sent;

/// One unauthenticated `GET /trade-api/v2/exchange/status`, and the headers it carried.
#[tokio::test]
async fn a_status_request_sends_exactly_the_default_headers() {
    let mut base = String::new();
    let headers = headers_sent("/trade-api/v2/exchange/status", |url| {
        base = url.clone();
        async move {
            let kalshi = Predictions::builder().base_url(url).build().unwrap();
            let _ = kalshi.exchange().status().send().await;
        }
    })
    .await;

    let host = base.trim_start_matches("http://");
    assert_eq!(
        headers,
        [
            ("accept", "*/*"),
            ("accept-encoding", "gzip"),
            ("host", host)
        ]
        .map(|(name, value)| (name.to_owned(), value.to_owned())),
        "`exchange().status()` sent a header set other than reqwest's defaults with gzip"
    );
}
