//! The full header set one request goes out with (AD-18, Story 3.13).
//!
//! The expected set is a literal, never read from the client under test. CI
//! runs this file in this crate alone with `--no-default-features`, and again
//! in the workspace build, so a reqwest feature another member switches on
//! (0.37.0's gzip regression) fails here in one build or the other.

use polyoxide_data::DataApi;
use polyoxide_test_support::query::headers_sent;

/// One unauthenticated `GET /positions`, and the headers it carried.
#[tokio::test]
async fn a_positions_list_sends_exactly_the_default_headers() {
    let mut base = String::new();
    let headers = headers_sent("/positions", |url| {
        base = url.clone();
        async move {
            let data = DataApi::builder().base_url(url).build().unwrap();
            let _ = data.user("0xabc").list_positions().send().await;
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
        "`user().list_positions` sent a header set other than reqwest's defaults with gzip"
    );
}
