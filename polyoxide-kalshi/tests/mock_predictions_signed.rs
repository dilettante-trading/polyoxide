//! Signed requests against a mock server: the three headers on every route but the ping, what
//! the signature covers, a fresh signature for every attempt, and an anonymous client's silence.
//!
//! Every test answers from one catch-all that records what it was sent. The request is what is
//! under test and the answer is not, so most routes answer `404`, and a test that only looks at
//! the request does not read the result.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::{ApiError, RetryConfig};
use polyoxide_kalshi::predictions::key::Ticker;
use polyoxide_kalshi::predictions::{Credentials, Predictions, PredictionsError};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use ring::signature::{UnparsedPublicKey, ED25519, RSA_PSS_2048_8192_SHA256};
use serde_json::Value;

const STATUS: &str = "/trade-api/v2/exchange/status";
const LIMITS: &str = "/trade-api/v2/account/limits";
const KEY_ID: &str = "test-key-id";

/// An answer of `/account/limits` for a Premier account, with one grant that expires and one that
/// does not.
const LIMITS_BODY: &str = r#"{"usage_tier":"Premier","read":{"refill_rate":1200,"bucket_capacity":1200},"write":{"refill_rate":600,"bucket_capacity":600},"grants":[{"exchange_instance":"main","level":"premier","expires_ts":1790000000,"source":"volume"},{"exchange_instance":"main","level":"basic","source":"default"}]}"#;

/// One request as the server saw it.
#[derive(Debug, Clone)]
struct Sent {
    path: String,
    query: Option<String>,
    key: Option<String>,
    timestamp: Option<String>,
    signature: Option<String>,
    /// How many of its headers start with `kalshi-`.
    kalshi_headers: usize,
}

impl Sent {
    fn of(request: &mockito::Request) -> Self {
        let header = |name: &str| {
            request
                .header(name)
                .first()
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        Self {
            path: request.path().to_owned(),
            query: request
                .path_and_query()
                .split_once('?')
                .map(|(_, query)| query.to_owned()),
            key: header("kalshi-access-key"),
            timestamp: header("kalshi-access-timestamp"),
            signature: header("kalshi-access-signature"),
            kalshi_headers: request
                .headers()
                .keys()
                .filter(|name| name.as_str().starts_with("kalshi-"))
                .count(),
        }
    }
}

/// What the server saw, in order.
#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<Sent>>>);

impl Seen {
    fn push(&self, sent: Sent) {
        self.0.lock().unwrap().push(sent);
    }

    fn all(&self) -> Vec<Sent> {
        self.0.lock().unwrap().clone()
    }

    fn last(&self) -> Sent {
        self.all().pop().expect("the server saw no request")
    }

    /// The requests that went to `path`.
    fn to(&self, path: &str) -> Vec<Sent> {
        let all = self.all();
        all.into_iter().filter(|sent| sent.path == path).collect()
    }
}

/// A catch-all that records every request. The status route answers `status_route` in turn, the
/// last repeating; any other path in `bodies` answers `200` with its body, and every other path
/// answers `404`.
async fn recording(
    server: &mut ServerGuard,
    status_route: &[usize],
    bodies: &[(&str, &str)],
    seen: &Seen,
) -> Mock {
    let status_route = status_route.to_vec();
    let served = AtomicUsize::new(0);
    let seen = seen.clone();
    let bodies: Vec<(String, String)> = bodies
        .iter()
        .map(|(path, body)| ((*path).to_owned(), (*body).to_owned()))
        .collect();
    let status_of = bodies.clone();
    server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .with_status_code_from_request(move |request| {
            seen.push(Sent::of(request));
            if request.path() == STATUS {
                let n = served.fetch_add(1, Ordering::SeqCst);
                status_route[n.min(status_route.len() - 1)]
            } else if status_of.iter().any(|(path, _)| path == request.path()) {
                200
            } else {
                404
            }
        })
        .with_header("content-type", "application/json")
        .with_body_from_request(move |request| {
            bodies
                .iter()
                .find(|(path, _)| path == request.path())
                .map_or_else(|| b"{}".to_vec(), |(_, body)| body.clone().into_bytes())
        })
        .expect_at_least(1)
        .create_async()
        .await
}

/// A client on the mock server with a short retry schedule.
fn client(server: &ServerGuard, credentials: Option<Credentials>) -> Predictions {
    let mut builder = Predictions::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 3,
            initial_backoff_ms: 20,
            max_backoff_ms: 50,
        });
    if let Some(credentials) = credentials {
        builder = builder.credentials(credentials);
    }
    builder.build().unwrap()
}

fn vectors() -> Value {
    fixtures!("predictions/signing").json("vectors")
}

fn field(vectors: &Value, key: &str, name: &str) -> String {
    vectors[key][name]
        .as_str()
        .unwrap_or_else(|| panic!("vectors.json has no {key}.{name}"))
        .to_owned()
}

/// A PEM block around `body`: the vectors hold each key as base64 lines with no armour line.
fn armour(label: &str, body: &str) -> String {
    format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n")
}

/// The vectors' Ed25519 key as PEM.
fn ed25519_pem() -> String {
    armour("PRIVATE KEY", &field(&vectors(), "ed25519", "pkcs8_body"))
}

fn ed25519() -> Credentials {
    Credentials::from_pem(KEY_ID, &ed25519_pem()).unwrap()
}

fn rsa() -> Credentials {
    let pem = armour("RSA PRIVATE KEY", &field(&vectors(), "rsa", "pkcs1_body"));
    Credentials::from_pem(KEY_ID, &pem).unwrap()
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("the vectors hold valid hex"))
        .collect()
}

/// Whether `(message, signature)` verifies under the vectors' Ed25519 public key.
fn ed25519_verifier() -> impl Fn(&[u8], &[u8]) -> bool {
    let public_key = hex(&field(&vectors(), "ed25519", "public_key_hex"));
    move |message, signature| {
        UnparsedPublicKey::new(&ED25519, &public_key)
            .verify(message, signature)
            .is_ok()
    }
}

/// Whether `(message, signature)` verifies under the vectors' RSA public key, in PSS.
fn rsa_verifier() -> impl Fn(&[u8], &[u8]) -> bool {
    let public_key = STANDARD
        .decode(field(&vectors(), "rsa", "public_key_pkcs1_der_b64"))
        .expect("the vectors hold valid base64");
    move |message, signature| {
        UnparsedPublicKey::new(&RSA_PSS_2048_8192_SHA256, &public_key)
            .verify(message, signature)
            .is_ok()
    }
}

/// Whether `sent` carries a signature over `timestamp + GET + path`, the path without its query.
fn verifies(sent: &Sent, verify: &dyn Fn(&[u8], &[u8]) -> bool) -> bool {
    let (Some(timestamp), Some(signature)) = (&sent.timestamp, &sent.signature) else {
        return false;
    };
    let Ok(signature) = STANDARD.decode(signature) else {
        return false;
    };
    verify(
        format!("{timestamp}GET{}", sent.path).as_bytes(),
        &signature,
    )
}

/// The last request seen went to `path` and is signed by the key `verify` holds.
fn assert_signed(seen: &Seen, path: &str, verify: &dyn Fn(&[u8], &[u8]) -> bool) {
    let sent = seen.last();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.kalshi_headers, 3,
        "{path} carries three KALSHI- headers"
    );
    assert_eq!(sent.key.as_deref(), Some(KEY_ID), "{path}");
    assert!(
        verifies(&sent, verify),
        "{path}: the signature does not verify over timestamp + GET + path"
    );
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_millis()
}

fn timestamp(sent: &Sent) -> u128 {
    sent.timestamp
        .as_deref()
        .expect("the request is signed")
        .parse()
        .expect("the timestamp is milliseconds")
}

#[tokio::test]
async fn a_keyed_client_signs_every_route_but_the_ping() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let client = client(&server, Some(ed25519()));
    let verify = ed25519_verifier();
    let ticker = Ticker::new("KXTEST-1").unwrap();

    // Each answer is a 404 nobody reads: the request is what is under test.
    let _ = client.exchange().status().send().await;
    assert_signed(&seen, STATUS, &verify);
    let _ = client.markets().list().limit(5).send().await;
    assert_signed(&seen, "/trade-api/v2/markets", &verify);
    let _ = client.markets().get(&ticker).send().await;
    assert_signed(&seen, "/trade-api/v2/markets/KXTEST-1", &verify);
    let _ = client.markets().orderbook(&ticker).send().await;
    assert_signed(&seen, "/trade-api/v2/markets/KXTEST-1/orderbook", &verify);
    let _ = client.markets().trades().send().await;
    assert_signed(&seen, "/trade-api/v2/markets/trades", &verify);
    let _ = client.events().get("KXEVENT-1").send().await;
    assert_signed(&seen, "/trade-api/v2/events/KXEVENT-1", &verify);
    let _ = client.account().endpoint_costs().send().await;
    assert_signed(&seen, "/trade-api/v2/account/endpoint_costs", &verify);
}

#[tokio::test]
async fn the_ping_is_never_signed_even_by_a_keyed_client() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[200], &[], &seen).await;
    let client = client(&server, Some(ed25519()));

    let _ = client.health().ping().await;

    let sent = seen.last();
    assert_eq!(sent.path, STATUS);
    assert_eq!(sent.kalshi_headers, 0, "the ping carried a KALSHI- header");
}

#[tokio::test]
async fn the_signature_covers_the_path_and_not_the_query() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let client = client(&server, Some(ed25519()));
    let verify = ed25519_verifier();

    let _ = client.markets().list().limit(5).send().await;

    let sent = seen.last();
    assert_eq!(
        sent.query.as_deref(),
        Some("limit=5"),
        "the test's own premise"
    );
    assert!(
        verifies(&sent, &verify),
        "signed over timestamp + GET + path"
    );
    // The same request with its query on the signed path is a different message.
    let with_query = Sent {
        path: format!("{}?limit=5", sent.path),
        ..sent.clone()
    };
    assert!(
        !verifies(&with_query, &verify),
        "the signature also verifies over a path with its query"
    );
}

#[tokio::test]
async fn an_rsa_key_signs_in_pss_with_a_new_salt_each_time() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let client = client(&server, Some(rsa()));
    let verify = rsa_verifier();

    let _ = client.exchange().status().send().await;
    let first = seen.last();
    let _ = client.exchange().status().send().await;
    let second = seen.last();

    assert!(verifies(&first, &verify), "{first:?}");
    assert!(verifies(&second, &verify), "{second:?}");
    assert_ne!(first.signature, second.signature);
}

#[tokio::test]
async fn the_timestamp_is_milliseconds_within_a_second_of_the_clock() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let client = client(&server, Some(ed25519()));

    let before = now_ms();
    let _ = client.exchange().status().send().await;
    let after = now_ms();

    let stamp = timestamp(&seen.last());
    assert!(
        stamp + 1_000 >= before && stamp <= after + 1_000,
        "{stamp} is not within a second of {before}..{after}"
    );
}

#[tokio::test]
async fn a_retried_attempt_is_signed_afresh() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[429, 200], &[], &seen).await;
    let client = client(&server, Some(ed25519()));
    let verify = ed25519_verifier();

    let _ = client.exchange().status().send().await;

    let attempts = seen.to(STATUS);
    assert_eq!(attempts.len(), 2, "a 429, then the retry: {attempts:?}");
    for attempt in &attempts {
        assert!(verifies(attempt, &verify), "{attempt:?}");
    }
    assert!(
        timestamp(&attempts[1]) > timestamp(&attempts[0]),
        "the retry reused the first attempt's timestamp"
    );
    assert_ne!(attempts[0].signature, attempts[1].signature);
}

#[tokio::test]
async fn a_key_id_that_cannot_be_a_header_value_fails_signing_and_sends_nothing() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let credentials = Credentials::from_pem("bad\nid", &ed25519_pem()).unwrap();
    let client = client(&server, Some(credentials));

    let Err(error) = client.exchange().status().send().await else {
        panic!("an unsignable request succeeded");
    };

    assert!(
        matches!(&error, PredictionsError::Api(ApiError::Sign(_))),
        "{error:?}"
    );
    assert_eq!(error.class(), Class::InvalidRequest);
    assert!(seen.to(STATUS).is_empty(), "an unsignable request was sent");
}

#[tokio::test]
async fn an_anonymous_client_sends_no_kalshi_header() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let client = client(&server, None);

    let _ = client.exchange().status().send().await;
    let _ = client.markets().list().send().await;
    let _ = client.account().endpoint_costs().send().await;

    let all = seen.all();
    assert!(all.len() >= 3, "the server saw {all:?}");
    assert!(
        all.iter().all(|sent| sent.kalshi_headers == 0),
        "an anonymous client sent a KALSHI- header"
    );
}

#[tokio::test]
async fn limits_decodes_the_accounts_buckets_and_is_signed() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[(LIMITS, LIMITS_BODY)], &seen).await;
    let client = client(&server, Some(ed25519()));

    let limits = client.account().limits().send().await.unwrap();

    assert_eq!(limits.usage_tier, "Premier");
    assert_eq!(
        (limits.read.bucket_capacity, limits.read.refill_rate),
        (1200, 1200)
    );
    assert_eq!(
        (limits.write.bucket_capacity, limits.write.refill_rate),
        (600, 600)
    );
    assert_eq!(limits.grants.len(), 2);
    assert_eq!(limits.grants[0].level, "premier");
    assert_eq!(limits.grants[0].expires_ts, Some(1_790_000_000));
    assert_eq!(limits.grants[1].expires_ts, None);
    let verify = ed25519_verifier();
    let sent = seen.to(LIMITS);
    assert!(!sent.is_empty(), "the route was never read");
    for request in &sent {
        assert!(verifies(request, &verify), "{request:?}");
    }
}

#[tokio::test]
async fn limits_without_credentials_sends_nothing() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[(LIMITS, LIMITS_BODY)], &seen).await;
    let client = client(&server, None);

    let error = client.account().limits().send().await.unwrap_err();

    match error {
        PredictionsError::Api(ApiError::Validation(message)) => assert_eq!(
            message,
            "account().limits() needs credentials: build the client with PredictionsBuilder::credentials"
        ),
        other => panic!("expected a validation refusal, got {other:?}"),
    }
    assert!(
        seen.to(LIMITS).is_empty(),
        "the refused call reached the server"
    );
}
