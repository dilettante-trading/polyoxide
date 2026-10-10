//! The query keys and pairs a request builder sends, read off a mock server,
//! and the keys a vendored OpenAPI document declares for the route.
//!
//! A builder called with every argument and setter it has must send exactly
//! the documented parameter names. Comparing [`keys_sent`] with
//! [`documented_parameters`] catches a camelCase key, a setter the route does
//! not take, and a parameter nothing sets.

use std::{
    collections::BTreeSet,
    fmt::Display,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use mockito::{Matcher, Server};
use serde_json::Value;

/// Serves `body` on `GET path` from a mock server, calls `fire` with the
/// server's base URL, and returns the query keys of the request it sent.
///
/// `fire` builds the client against that URL and sends one request. Its
/// response must decode: `body` is the route's captured response, `fixture`
/// its name for the message, so this also proves the builder's return type
/// fits the route. Panics when the request is not sent or does not decode.
pub async fn keys_sent<F, Fut, E>(
    path: &str,
    fixture: &str,
    body: String,
    fire: F,
) -> BTreeSet<String>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Result<(), E>>,
    E: Display,
{
    let mut server = Server::new_async().await;
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = Arc::clone(&seen);
    let mock = server
        .mock("GET", path)
        .match_query(Matcher::Any)
        .match_request(move |request| {
            sink.lock()
                .unwrap()
                .push(request.path_and_query().to_owned());
            true
        })
        .with_status(200)
        .with_body(body)
        .create_async()
        .await;

    let decoded = fire(server.url()).await;
    mock.assert_async().await;
    if let Err(e) = decoded {
        panic!("{path}: the builder did not decode `{fixture}.json`: {e}");
    }

    let seen = seen.lock().unwrap();
    let url = url::Url::parse(&format!("http://mock{}", seen.last().unwrap())).unwrap();
    url.query_pairs().map(|(key, _)| key.into_owned()).collect()
}

/// Answers `GET path` on a mock server, calls `fire` with the server's base
/// URL, and returns the request's query as ordered `(key, value)` pairs,
/// decoded, a repeated key once per value.
///
/// `fire` builds the client against that URL and sends one request; what it
/// returns is ignored, so the body (`{}`) need not decode as the route's
/// type. Panics when no request reaches `path`.
pub async fn pairs_sent<F, Fut>(path: &str, fire: F) -> Vec<(String, String)>
where
    F: FnOnce(String) -> Fut,
    Fut: Future,
{
    let mut server = Server::new_async().await;
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = Arc::clone(&seen);
    let mock = server
        .mock("GET", path)
        .match_query(Matcher::Any)
        .match_request(move |request| {
            sink.lock()
                .unwrap()
                .push(request.path_and_query().to_owned());
            true
        })
        .with_status(200)
        .with_body("{}")
        // Without it a mock expects exactly one hit, and a request sent twice
        // would read as none sent.
        .expect_at_least(1)
        .create_async()
        .await;

    fire(server.url()).await;
    if !mock.matched_async().await {
        panic!("{path}: no request was sent");
    }

    let seen = seen.lock().unwrap();
    let url = url::Url::parse(&format!("http://mock{}", seen.last().unwrap())).unwrap();
    url.query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

/// Answers `GET path` on a mock server, calls `fire` with the server's base
/// URL, and returns every header of the request it sent as ordered
/// `(lowercased name, value)` pairs, a repeated header once per value.
///
/// `fire` builds the client against that URL and sends one request; what it
/// returns is ignored, as with [`pairs_sent`]. Panics when no request reaches
/// `path`, or when a header's value is not visible ASCII.
pub async fn headers_sent<F, Fut>(path: &str, fire: F) -> Vec<(String, String)>
where
    F: FnOnce(String) -> Fut,
    Fut: Future,
{
    let mut server = Server::new_async().await;
    let seen = Arc::new(Mutex::new(Vec::<Vec<(String, String)>>::new()));
    let sink = Arc::clone(&seen);
    let mock = server
        .mock("GET", path)
        .match_query(Matcher::Any)
        .match_request(move |request| {
            let headers = request
                .headers()
                .iter()
                .map(|(name, value)| {
                    let value = value.to_str().unwrap_or_else(|_| {
                        panic!("{name}: a header value that is not visible ASCII")
                    });
                    (name.as_str().to_ascii_lowercase(), value.to_owned())
                })
                .collect();
            sink.lock().unwrap().push(headers);
            true
        })
        .with_status(200)
        .with_body("{}")
        .expect_at_least(1)
        .create_async()
        .await;

    fire(server.url()).await;
    if !mock.matched_async().await {
        panic!("{path}: no request was sent");
    }

    let seen = seen.lock().unwrap();
    seen.last().unwrap().clone()
}

/// Sends one request builder's call against the mock server at the given
/// base URL; what it returns is ignored.
pub type Fire = fn(String) -> Pin<Box<dyn Future<Output = ()> + Send>>;

/// A builder, the path it sends to, a call of every setter it has, and the
/// pairs that call sends.
pub struct Case {
    /// The builder's name, for the failure message.
    pub builder: &'static str,
    /// The path the call sends to.
    pub path: &'static str,
    /// The call.
    pub fire: Fire,
    /// The `(key, value)` pairs it sends, in order.
    pub sends: &'static [(&'static str, &'static str)],
}

/// Fires each case through [`pairs_sent`] and asserts it sends exactly its
/// pairs, in order. A failure names the builder and the path.
pub async fn assert_cases(cases: &[Case]) {
    for case in cases {
        let pairs = pairs_sent(case.path, case.fire).await;
        let sent: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(sent, case.sends, "{} on {}", case.builder, case.path);
    }
}

/// The parameter names `spec` documents for `GET path`, or `None` when the
/// document has no such operation. A documented operation without
/// `parameters` documents none. Panics on a parameter without a name.
#[track_caller]
pub fn documented_parameters(spec: &Value, path: &str) -> Option<BTreeSet<String>> {
    let operation = &spec["paths"][path]["get"];
    if !operation.is_object() {
        return None;
    }
    // A loop, not a closure: `#[track_caller]` does not reach into one.
    let mut names = BTreeSet::new();
    for param in operation["parameters"].as_array().into_iter().flatten() {
        names.insert(param["name"].as_str().unwrap().to_owned());
    }
    Some(names)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn keys_sent_reads_the_query_off_the_request() {
        let keys = keys_sent("/v1/rows", "rows", "[]".to_owned(), |base| async move {
            let body =
                polyoxide_core::reqwest::get(format!("{base}/v1/rows?limit=2&cursor=c&limit=3"))
                    .await
                    .map_err(|e| e.to_string())?
                    .text()
                    .await
                    .map_err(|e| e.to_string())?;
            serde_json::from_str::<Vec<u8>>(&body)
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
        .await;
        assert_eq!(
            keys,
            BTreeSet::from(["cursor".to_owned(), "limit".to_owned()])
        );
    }

    #[tokio::test]
    #[should_panic(expected = "/v1/rows: the builder did not decode `rows.json`: refused")]
    async fn a_response_that_does_not_decode_fails() {
        keys_sent("/v1/rows", "rows", "[]".to_owned(), |base| async move {
            polyoxide_core::reqwest::get(format!("{base}/v1/rows"))
                .await
                .unwrap();
            Err::<(), _>("refused")
        })
        .await;
    }

    #[tokio::test]
    async fn pairs_sent_reads_every_pair_in_order() {
        let pairs = pairs_sent("/v1/rows", |base| async move {
            polyoxide_core::reqwest::get(format!("{base}/v1/rows?limit=2&id=a&id=b&q=x%20y"))
                .await
                .unwrap();
        })
        .await;
        assert_eq!(
            pairs,
            [("limit", "2"), ("id", "a"), ("id", "b"), ("q", "x y")]
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
        );
    }

    #[tokio::test]
    async fn pairs_sent_does_not_need_the_body_to_decode() {
        let pairs = pairs_sent("/v1/rows", |base| async move {
            let body = polyoxide_core::reqwest::get(format!("{base}/v1/rows?limit=2"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            serde_json::from_str::<Vec<u8>>(&body).unwrap_err()
        })
        .await;
        assert_eq!(pairs, [("limit".to_owned(), "2".to_owned())]);
    }

    #[tokio::test]
    async fn headers_sent_reads_every_header_lowercased() {
        let mut base = String::new();
        let headers = headers_sent("/v1/rows", |url| {
            base = url.clone();
            async move {
                polyoxide_core::reqwest::Client::new()
                    .get(format!("{url}/v1/rows?limit=2"))
                    .header("X-Trace", "t-1")
                    .header("x-trace", "t-2")
                    .send()
                    .await
                    .unwrap();
            }
        })
        .await;
        let host = base.trim_start_matches("http://");
        for expected in [("x-trace", "t-1"), ("x-trace", "t-2"), ("host", host)] {
            assert!(
                headers.contains(&(expected.0.to_owned(), expected.1.to_owned())),
                "{expected:?} is not in {headers:?}"
            );
        }
    }

    #[test]
    fn documented_parameters_distinguish_none_from_no_route() {
        let spec = json!({"paths": {
            "/v1/rows": {"get": {"parameters": [{"name": "limit"}, {"name": "cursor"}]}},
            "/v1/time": {"get": {}}
        }});
        assert_eq!(
            documented_parameters(&spec, "/v1/rows"),
            Some(BTreeSet::from(["cursor".to_owned(), "limit".to_owned()]))
        );
        assert_eq!(
            documented_parameters(&spec, "/v1/time"),
            Some(BTreeSet::new())
        );
        assert_eq!(documented_parameters(&spec, "/v1/gone"), None);
    }
}
