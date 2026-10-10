//! The full header set one request goes out with (AD-18, Story 3.13).
//!
//! The expected set is a literal, never read from the client under test. CI
//! runs this file in this crate alone with `--no-default-features`, and again
//! in the workspace build, so a change to what a client sends fails here in
//! one build or the other, whether a code default (0.37.0's gzip regression
//! was core's builder turning gzip off) or a reqwest feature another member
//! switches on.

use polyoxide_gamma::Gamma;
use polyoxide_test_support::query::headers_sent;

/// One unauthenticated `GET /markets/1`, and the headers it carried.
#[tokio::test]
async fn a_market_lookup_sends_exactly_the_default_headers() {
    let mut base = String::new();
    let headers = headers_sent("/markets/1", |url| {
        base = url.clone();
        async move {
            let gamma = Gamma::builder().base_url(url).build().unwrap();
            let _ = gamma.markets().get("1").send().await;
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
        "`markets().get` sent a header set other than reqwest's defaults with gzip"
    );
}
