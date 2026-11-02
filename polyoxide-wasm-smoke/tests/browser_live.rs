//! The nightly's live browser checks (spec D14, D17): the browser implementations against the
//! real hosts, run by `scripts/wasm_live.sh` in headless Chrome inside a dedicated Worker.
//!
//! They are gated by the `live` feature, not `#[ignore]`: nextest does not run wasm, and the
//! script selects the target. A failure is tagged through `polyoxide-test-support`, whose tags
//! build on wasm32, so the classifier reads the same `polyoxide-class=<tag>` line it reads for the
//! native suites; an untagged panic is `real`.
//!
//! Per D17 each check retries once after 5 s. A second failure is `real`, except a socket that
//! opened and then dropped, which is `transient`, and a host the runner cannot reach for a reason
//! of its own (a region block), which is `environmental`. A GET is bounded by 10 s, a socket held
//! until its first data frame or 20 s.

#![cfg(all(target_arch = "wasm32", feature = "live"))]

use std::{future::Future, sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use polyoxide_core::{ApiError, FetchTransport, HttpClientBuilder, Request, Response, RetryConfig};
use polyoxide_rt::{timeout, BrowserRuntime, Runtime};
use polyoxide_test_support::{environmental, tag_for, transient, Tag};
use polyoxide_venue::Classify;
use polyoxide_ws::{BoxSocket, SocketTransport, WebSocketTransport};
use serde_json::Value;
use tungstenite::{http::Request as Upgrade, Message};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_dedicated_worker);

const GET_LIMIT_MS: u64 = 10_000;
const CONNECT_LIMIT: Duration = Duration::from_secs(10);
const SOCKET_LIMIT: Duration = Duration::from_secs(20);
const ANSWER_LIMIT: Duration = Duration::from_secs(5);
const RETRY_AFTER: Duration = Duration::from_secs(5);
/// Binance's pacing of requests on one socket is 200 ms.
const REQUEST_GAP: Duration = Duration::from_millis(250);

/// What a check found.
enum Failure {
    /// The expectation did not hold, or the host stayed down through the retry.
    Real(String),
    /// The socket opened and then dropped.
    Transient(String),
    /// The runner cannot test the host, such as a region block.
    Environmental(String),
}

/// Fails untagged, which the classifier files as `real`.
fn real(reason: &str) -> ! {
    panic!("{reason}")
}

/// Run `check`; on a failure other than an environmental one, run it once more after
/// [`RETRY_AFTER`]; a second failure fails the test with its tag.
async fn checked<F, Fut>(what: &str, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(), Failure>>,
{
    let mut failure = match check().await {
        Ok(()) => return,
        Err(failure) => failure,
    };
    if !matches!(failure, Failure::Environmental(_)) {
        BrowserRuntime.sleep(RETRY_AFTER).await;
        match check().await {
            Ok(()) => return,
            Err(second) => failure = second,
        }
    }
    match failure {
        Failure::Environmental(reason) => environmental(&format!("{what}: {reason}")),
        Failure::Transient(reason) => transient(&format!("{what}: {reason}")),
        Failure::Real(reason) => real(&format!("{what}: {reason}")),
    }
}

struct Host {
    base: &'static str,
    path: &'static str,
}

const CLOB: Host = Host {
    base: "https://clob.polymarket.com",
    path: "/time",
};
const GAMMA: Host = Host {
    base: "https://gamma-api.polymarket.com",
    path: "/status",
};
const DATA: Host = Host {
    base: "https://data-api.polymarket.com",
    path: "/",
};
const PERPS: Host = Host {
    base: "https://api.perpetuals.polymarket.com",
    path: "/v1/info/ping",
};
const FAPI: Host = Host {
    base: "https://fapi.binance.com",
    path: "/fapi/v1/ping",
};

/// One public GET through `FetchTransport`, with no retry of its own. A cross-origin refusal and
/// a network failure look alike to a browser, and both fail here.
async fn get(host: &Host) -> Result<Response, Failure> {
    let client = HttpClientBuilder::new(host.base)
        .runtime(Arc::new(BrowserRuntime))
        .http_transport(Arc::new(FetchTransport::new()))
        .with_retry_config(RetryConfig {
            max_retries: 0,
            initial_backoff_ms: 1,
            max_backoff_ms: 1,
        })
        .timeout_ms(GET_LIMIT_MS)
        .build()
        .map_err(|err| Failure::Real(format!("building the client: {err}")))?;
    Request::<Value, ApiError>::new(client, host.path)
        .send_raw()
        .await
        .map_err(|err| {
            let detail = format!("GET {}{}: {err}", host.base, host.path);
            match tag_for(&err.class(), err.is_fault()) {
                Tag::Environmental => Failure::Environmental(detail),
                _ => Failure::Real(detail),
            }
        })
}

#[wasm_bindgen_test]
async fn get_clob() {
    checked("clob GET /time", || async { get(&CLOB).await.map(drop) }).await;
}

#[wasm_bindgen_test]
async fn get_gamma() {
    checked("gamma GET /status", || async {
        get(&GAMMA).await.map(drop)
    })
    .await;
}

#[wasm_bindgen_test]
async fn get_data() {
    checked("data GET /", || async { get(&DATA).await.map(drop) }).await;
}

/// The one host in scope that names a header in `Access-Control-Expose-Headers`: `x-pmp-pod` is
/// readable here, as `Retry-After` is.
#[wasm_bindgen_test]
async fn get_perps() {
    checked("perps GET /v1/info/ping", || async {
        let response = get(&PERPS).await?;
        if response.headers().get("x-pmp-pod").is_none() {
            return Err(Failure::Real(
                "x-pmp-pod is not readable: perps no longer exposes it".into(),
            ));
        }
        Ok(())
    })
    .await;
}

/// Binance sends `X-MBX-USED-WEIGHT-1M` and does not expose it, so a browser reads none: the
/// weight accounting cannot work there. If this fails, the host started exposing it.
#[wasm_bindgen_test]
async fn get_fapi() {
    checked("fapi GET /fapi/v1/ping", || async {
        let response = get(&FAPI).await?;
        if response.headers().get("x-mbx-used-weight-1m").is_some() {
            return Err(Failure::Real(
                "X-MBX-USED-WEIGHT-1M became readable: fapi now exposes it".into(),
            ));
        }
        Ok(())
    })
    .await;
}

struct SocketHost {
    url: &'static str,
    /// The first frame, for a host that sends nothing until it is spoken to.
    hello: Option<&'static str>,
}

const CLOB_MARKET: SocketHost = SocketHost {
    url: "wss://ws-subscriptions-clob.polymarket.com/ws/market",
    // The market channel closes 1008 on anything but a subscription, `PING` included. An empty
    // one is the shape the supervised socket replays and the host keeps open.
    hello: Some(r#"{"assets_ids":[],"type":"market"}"#),
};
const SPORTS: SocketHost = SocketHost {
    url: "wss://sports-api.polymarket.com/ws",
    // The feed sends data only while a match is live: held for the window, it has passed.
    hello: None,
};
const PERPS_WS: SocketHost = SocketHost {
    url: "wss://ws.perpetuals.polymarket.com/v1/ws",
    hello: Some(r#"{"id":1,"req":"post","op":{"type":"ping"}}"#),
};
const FSTREAM: SocketHost = SocketHost {
    url: "wss://fstream.binance.com/market/stream",
    hello: Some(r#"{"method":"SUBSCRIBE","params":["btcusdt@markPrice@1s"],"id":1}"#),
};

async fn open(url: &str) -> Result<BoxSocket, Failure> {
    let request = Upgrade::builder()
        .uri(url)
        .body(())
        .map_err(|err| Failure::Real(format!("building the upgrade request: {err}")))?;
    let runtime: Arc<dyn Runtime> = Arc::new(BrowserRuntime);
    WebSocketTransport::new()
        .connect(request, CONNECT_LIMIT, runtime)
        .await
        .map_err(|err| Failure::Real(format!("{url}: the upgrade failed: {err}")))
}

/// Open `host`, speak first if it needs it, and hold the socket until its first data frame or the
/// window ends. A socket that closes or errors first opened and then dropped.
async fn hold(host: &SocketHost) -> Result<(), Failure> {
    let mut socket = open(host.url).await?;
    if let Some(hello) = host.hello {
        socket
            .send(Message::Text(hello.into()))
            .await
            .map_err(|err| Failure::Real(format!("sending the first frame: {err}")))?;
    }
    let outcome = match timeout(&BrowserRuntime, SOCKET_LIMIT, socket.next()).await {
        None => Ok(()),
        Some(Some(Ok(Message::Text(_) | Message::Binary(_)))) => Ok(()),
        Some(Some(Ok(other))) => Err(Failure::Transient(format!(
            "the first frame was {other:?}, not data"
        ))),
        Some(Some(Err(err))) => Err(Failure::Transient(format!("the socket dropped: {err}"))),
        Some(None) => Err(Failure::Transient("the socket closed with no frame".into())),
    };
    let _ = socket.close().await;
    outcome
}

#[wasm_bindgen_test]
async fn socket_clob_market() {
    checked("clob market socket", || hold(&CLOB_MARKET)).await;
}

#[wasm_bindgen_test]
async fn socket_sports() {
    checked("sports socket", || hold(&SPORTS)).await;
}

#[wasm_bindgen_test]
async fn socket_perps() {
    checked("perps socket", || hold(&PERPS_WS)).await;
}

/// A browser cannot read the status of a refused upgrade, so a Binance region block (a 451 on the
/// REST host as well) shows as a failed connect: look at the REST host before calling it real.
async fn fstream() -> Result<(), Failure> {
    match hold(&FSTREAM).await {
        Err(Failure::Real(reason)) => match get(&FAPI).await {
            Err(Failure::Environmental(region)) => Err(Failure::Environmental(format!(
                "{reason}; and the REST host: {region}"
            ))),
            _ => Err(Failure::Real(reason)),
        },
        other => other,
    }
}

#[wasm_bindgen_test]
async fn socket_fstream() {
    checked("fstream socket", fstream).await;
}

/// Binance's `LIST_SUBSCRIPTIONS`, the request the supervised protocol's keepalive is, since a
/// browser cannot send a protocol ping: it must be answered, naming the stream subscribed.
async fn list_subscriptions() -> Result<(), Failure> {
    let mut socket = open(FSTREAM.url).await?;
    for (id, request) in [
        (
            1,
            r#"{"method":"SUBSCRIBE","params":["btcusdt@markPrice@1s"],"id":1}"#,
        ),
        (2, r#"{"method":"LIST_SUBSCRIPTIONS","id":2}"#),
    ] {
        if id > 1 {
            BrowserRuntime.sleep(REQUEST_GAP).await;
        }
        socket
            .send(Message::Text(request.into()))
            .await
            .map_err(|err| Failure::Real(format!("sending request {id}: {err}")))?;
    }
    let answered = timeout(&BrowserRuntime, ANSWER_LIMIT, async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else {
                        continue;
                    };
                    if value["id"] == 2 {
                        return Ok(value);
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(err)) => {
                    return Err(Failure::Transient(format!("the socket dropped: {err}")))
                }
                None => {
                    return Err(Failure::Transient(
                        "the socket closed before it answered".into(),
                    ))
                }
            }
        }
    })
    .await;
    let _ = socket.close().await;
    match answered {
        None => Err(Failure::Real(
            "LIST_SUBSCRIPTIONS was not answered within 5 s".into(),
        )),
        Some(Err(failure)) => Err(failure),
        Some(Ok(value)) => {
            let listed = value["result"].as_array().is_some_and(|names| {
                names
                    .iter()
                    .any(|name| name.as_str() == Some("btcusdt@markPrice@1s"))
            });
            if listed {
                Ok(())
            } else {
                Err(Failure::Real(format!(
                    "LIST_SUBSCRIPTIONS answered without the stream: {value}"
                )))
            }
        }
    }
}

#[wasm_bindgen_test]
async fn fstream_keepalive() {
    checked("fstream LIST_SUBSCRIPTIONS", list_subscriptions).await;
}

/// The self-check `scripts/wasm_live.sh` runs first: built with `POLYOXIDE_LIVE_PROBE` set, this
/// test fails with a `transient` tag, and the script checks that the classifier reads that tag
/// from the runner's output. Built without it, the test passes at once.
#[wasm_bindgen_test]
fn probe_tag_line() {
    if option_env!("POLYOXIDE_LIVE_PROBE").is_none() {
        return;
    }
    transient("the runner's output must carry this test's tag line");
}
