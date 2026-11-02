//! The seam between the send loop and the network.
//!
//! [`HttpTransport`] takes one built [`HttpRequest`] and answers with a
//! [`Response`], or with a [`TransportError`] when no response head arrived.
//! The send loop decides everything else: whether a status is a failure,
//! whether to retry, what to hold. A client sends on the target's default
//! transport unless one is injected.

use std::fmt;
use std::time::Duration;

use bytes::Bytes;
use http::{HeaderMap, Method};
use polyoxide_rt::BoxFuture;
use polyoxide_venue::{Class, Classify};
use thiserror::Error;
use url::Url;

use crate::response::Response;

/// One attempt's request, as the send loop builds it.
///
/// Only core builds one: the struct is `non_exhaustive` and has no public
/// constructor, so a transport reads the fields it is handed.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HttpRequest {
    /// The HTTP method.
    pub method: Method,
    /// The full URL: the base URL joined with the path, with the query
    /// form-urlencoded onto it.
    pub url: Url,
    /// The headers, signed.
    pub headers: HeaderMap,
    /// The body, sent as given.
    pub body: Option<Bytes>,
    /// How long the whole exchange may take, the body's read included: the
    /// request's own timeout, else the client's. Always set.
    pub timeout: Duration,
}

/// Sends one request and answers with its response.
///
/// The send loop is the only caller. A client holds one transport and shares
/// it with its `with_base_url` siblings.
pub trait HttpTransport: Send + Sync + fmt::Debug + 'static {
    /// Send `request` and answer with its response, whatever its status.
    ///
    /// An implementation:
    ///
    /// - returns a [`Response`] for every status, an error status included:
    ///   judging a status is the send loop's job, never the transport's;
    /// - fails with a [`TransportError`] only when no response head arrived;
    /// - bounds the whole exchange, the body's read included, by
    ///   `request.timeout`;
    /// - leaves the body unread until the [`Response`] is asked for it.
    fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>>;
}

/// Where a transport's exchange broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransportKind {
    /// The connection could not be made.
    Connect,
    /// The timeout ran out.
    Timeout,
    /// The body could not be read to its end.
    Body,
    /// The request could not be built, or sent as built.
    Request,
    /// Anything else.
    Other,
}

impl fmt::Display for TransportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Connect => "connect",
            Self::Timeout => "timeout",
            Self::Body => "body",
            Self::Request => "request",
            Self::Other => "other",
        })
    }
}

/// Why no response arrived, or why its body could not be read.
///
/// [`kind`](Self::kind) says where the exchange broke, and the boxed source is
/// the transport's own error, kept for its message and its chain. A custom
/// [`HttpTransport`] builds one with [`new`](Self::new). Its class is
/// [`Class::Network`] for every kind but [`TransportKind::Request`], a request
/// that could not be built, which is [`Class::InvalidRequest`].
#[derive(Debug, Error)]
#[non_exhaustive]
#[error("{kind}: {source}")]
pub struct TransportError {
    kind: TransportKind,
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl TransportError {
    /// A transport failure of `kind`, caused by `source`.
    pub fn new(
        kind: TransportKind,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self {
            kind,
            source: source.into(),
        }
    }

    /// Where the exchange broke.
    pub fn kind(&self) -> TransportKind {
        self.kind
    }
}

impl Classify for TransportError {
    fn class(&self) -> Class {
        match self.kind {
            TransportKind::Request => Class::InvalidRequest,
            TransportKind::Connect
            | TransportKind::Timeout
            | TransportKind::Body
            | TransportKind::Other => Class::Network,
        }
    }
}

/// The transport the native targets use: reqwest.
///
/// The client builder makes one from its timeout, pool size and `gzip`; build
/// one with [`new`](Self::new) to send on a client of your own.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// A transport that sends on `client`.
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// The default native transport, built from a client builder's settings:
    /// its timeout, its idle connections per host and its `gzip` choice
    /// (unset keeps reqwest's default, so every client asks for gzip), with a
    /// ten second connect timeout and no redirects.
    pub(crate) fn from_settings(
        timeout: Duration,
        pool_size: usize,
        gzip: Option<bool>,
    ) -> Result<Self, TransportError> {
        let mut builder = reqwest::Client::builder();
        if let Some(enabled) = gzip {
            builder = builder.gzip(enabled);
        }
        let client = builder
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .pool_max_idle_per_host(pool_size)
            .build()
            .map_err(map_error)?;
        Ok(Self::new(client))
    }
}

/// The default transport, built from a client builder's settings. The browser
/// story gates this and `ReqwestTransport` to the native targets and adds the
/// wasm32 twin, over `fetch`.
pub(crate) fn default_transport(
    timeout: Duration,
    pool_size: usize,
    gzip: Option<bool>,
) -> Result<std::sync::Arc<dyn HttpTransport>, TransportError> {
    Ok(std::sync::Arc::new(ReqwestTransport::from_settings(
        timeout, pool_size, gzip,
    )?))
}

impl HttpTransport for ReqwestTransport {
    fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>> {
        let mut builder = self
            .client
            .request(request.method, request.url)
            .headers(request.headers)
            .timeout(request.timeout);
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        Box::pin(async move {
            let response = builder.send().await.map_err(map_error)?;
            let status = response.status();
            let headers = response.headers().clone();
            let url = response.url().clone();
            Ok(Response::new(
                status,
                headers,
                url,
                Box::pin(async move { response.bytes().await.map_err(map_error) }),
            ))
        })
    }
}

/// The transport error for a reqwest one, by the rule core has always used:
/// a failure in transit is `Network` whatever reqwest labels it, so it is
/// decided before a request that could not be built.
fn map_error(err: reqwest::Error) -> TransportError {
    let flags = Flags {
        timeout: err.is_timeout(),
        connect: err.is_connect(),
        body: err.is_body() || err.is_decode(),
        builder: err.is_builder(),
    };
    TransportError::new(kind_of(flags, &err), err)
}

/// What a reqwest error reports about itself.
#[derive(Clone, Copy)]
struct Flags {
    timeout: bool,
    connect: bool,
    /// A body that broke off; reqwest labels that one a decode error too.
    body: bool,
    builder: bool,
}

/// The kind for a reqwest error's flags and its source chain: a timeout, a
/// failed connect, a body that broke off, or an I/O error or a body error
/// anywhere in the chain is a failure in transit, checked before a request
/// that could not be built; anything else is `Other`.
fn kind_of(flags: Flags, err: &(dyn std::error::Error + 'static)) -> TransportKind {
    if flags.timeout {
        TransportKind::Timeout
    } else if flags.connect {
        TransportKind::Connect
    } else if flags.body {
        TransportKind::Body
    } else if failed_in_transit(err) {
        TransportKind::Other
    } else if flags.builder {
        TransportKind::Request
    } else {
        TransportKind::Other
    }
}

/// Whether an I/O error or a body-kind reqwest error is anywhere in the
/// error's source chain, however reqwest labels the error itself.
fn failed_in_transit(err: &(dyn std::error::Error + 'static)) -> bool {
    let mut source = err.source();
    while let Some(inner) = source {
        if inner.is::<std::io::Error>()
            || inner
                .downcast_ref::<reqwest::Error>()
                .is_some_and(reqwest::Error::is_body)
        {
            return true;
        }
        source = inner.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use http::{HeaderValue, StatusCode};

    use super::*;
    use crate::error::ApiError;

    #[test]
    fn a_kind_displays_in_lower_case() {
        for (kind, text) in [
            (TransportKind::Connect, "connect"),
            (TransportKind::Timeout, "timeout"),
            (TransportKind::Body, "body"),
            (TransportKind::Request, "request"),
            (TransportKind::Other, "other"),
        ] {
            assert_eq!(kind.to_string(), text);
        }
    }

    #[test]
    fn an_error_displays_its_kind_then_its_source() {
        let err = TransportError::new(TransportKind::Timeout, "no answer");
        assert_eq!(err.kind(), TransportKind::Timeout);
        assert_eq!(err.to_string(), "timeout: no answer");
        assert_eq!(
            std::error::Error::source(&err).map(ToString::to_string),
            Some("no answer".to_owned())
        );
        assert_eq!(
            ApiError::from(err).to_string(),
            "Transport error: timeout: no answer"
        );
    }

    #[test]
    fn only_a_request_that_could_not_be_built_is_not_a_network_failure() {
        for kind in [
            TransportKind::Connect,
            TransportKind::Timeout,
            TransportKind::Body,
            TransportKind::Other,
        ] {
            let err = TransportError::new(kind, "x");
            assert_eq!(err.class(), Class::Network, "{kind}");
            assert!(err.is_retriable(), "{kind}");
            assert!(err.is_fault(), "{kind}");
        }
        let err = TransportError::new(TransportKind::Request, "x");
        assert_eq!(err.class(), Class::InvalidRequest);
        assert!(!err.is_retriable());
    }

    /// An error with an I/O error behind it, as a reqwest error that failed
    /// in transit has.
    #[derive(Debug, Error)]
    #[error("wrapped")]
    struct Wrapped(#[source] std::io::Error);

    #[test]
    fn an_io_error_in_the_source_chain_is_never_a_request_error() {
        let err = Wrapped(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
        for (flags, expected) in [
            (
                Flags {
                    timeout: false,
                    connect: false,
                    body: false,
                    builder: true,
                },
                TransportKind::Other,
            ),
            (
                Flags {
                    timeout: false,
                    connect: false,
                    body: false,
                    builder: false,
                },
                TransportKind::Other,
            ),
            (
                Flags {
                    timeout: true,
                    connect: false,
                    body: false,
                    builder: true,
                },
                TransportKind::Timeout,
            ),
            (
                Flags {
                    timeout: false,
                    connect: true,
                    body: false,
                    builder: true,
                },
                TransportKind::Connect,
            ),
            (
                Flags {
                    timeout: false,
                    connect: false,
                    body: true,
                    builder: true,
                },
                TransportKind::Body,
            ),
        ] {
            let kind = kind_of(flags, &err);
            assert_eq!(kind, expected);
            assert_eq!(TransportError::new(kind, "x").class(), Class::Network);
        }
        // With no I/O error behind it, a builder error stays a request error.
        let plain = TransportError::new(
            kind_of(
                Flags {
                    timeout: false,
                    connect: false,
                    body: false,
                    builder: true,
                },
                &fmt::Error,
            ),
            "x",
        );
        assert_eq!(plain.kind(), TransportKind::Request);
    }

    fn request(url: &str) -> HttpRequest {
        HttpRequest {
            method: Method::GET,
            url: Url::parse(url).unwrap(),
            headers: HeaderMap::new(),
            body: None,
            timeout: Duration::from_secs(5),
        }
    }

    /// A transport on a client with no proxy, so a proxy set in the
    /// environment cannot answer for the local addresses below.
    fn transport() -> ReqwestTransport {
        ReqwestTransport::new(reqwest::Client::builder().no_proxy().build().unwrap())
    }

    #[tokio::test]
    async fn the_head_comes_back_whole_and_the_body_is_read_on_demand() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/v1/rows")
            .match_query(mockito::Matcher::UrlEncoded("limit".into(), "2".into()))
            .match_header("x-trace", "t-1")
            .with_status(201)
            .with_header("x-served-by", "mock")
            .with_body("rows")
            .create_async()
            .await;
        let mut sent = request(&format!("{}/v1/rows?limit=2", server.url()));
        sent.headers
            .insert("x-trace", HeaderValue::from_static("t-1"));

        let response = transport().send(sent.clone()).await.unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response
                .headers()
                .get("x-served-by")
                .and_then(|value| value.to_str().ok()),
            Some("mock")
        );
        assert_eq!(response.url(), &sent.url);
        assert_eq!(response.text().await.unwrap(), "rows");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn the_method_and_the_body_are_sent() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/order")
            .match_body(r#"{"size":1}"#)
            .with_status(200)
            .with_body("{}")
            .create_async()
            .await;
        let mut sent = request(&format!("{}/v1/order", server.url()));
        sent.method = Method::POST;
        sent.body = Some(Bytes::from_static(br#"{"size":1}"#));

        let response = transport().send(sent).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn an_error_status_is_a_response_and_never_a_transport_error() {
        let mut server = mockito::Server::new_async().await;
        let _down = server
            .mock("GET", "/v1/rows")
            .with_status(503)
            .with_body("down")
            .create_async()
            .await;

        let response = transport()
            .send(request(&format!("{}/v1/rows", server.url())))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.text().await.unwrap(), "down");
    }

    #[tokio::test]
    async fn a_refused_connection_is_a_connect_failure() {
        // Bind a port, then free it, so nothing is listening there.
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        let err = transport()
            .send(request(&format!("http://127.0.0.1:{port}/")))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), TransportKind::Connect, "{err:?}");
        assert_eq!(err.class(), Class::Network);
    }

    #[tokio::test]
    async fn a_host_that_never_answers_is_a_timeout() {
        // Accepted by the kernel, never answered.
        let silent = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut sent = request(&format!("http://{}/", silent.local_addr().unwrap()));
        sent.timeout = Duration::from_millis(200);

        let err = transport().send(sent).await.unwrap_err();

        assert_eq!(err.kind(), TransportKind::Timeout, "{err:?}");
        assert_eq!(err.class(), Class::Network);
    }

    /// A server that answers with a head promising 100 bytes and six of them,
    /// then holds the connection for `hold` before closing it.
    fn short_body_server(hold: Duration) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.read(&mut [0; 1024]);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{\"a\":");
            std::thread::sleep(hold);
        });
        (url, server)
    }

    #[tokio::test]
    async fn a_body_that_stops_short_is_a_body_failure() {
        let (url, server) = short_body_server(Duration::ZERO);

        let response = transport().send(request(&url)).await.unwrap();
        let err = response.bytes().await.unwrap_err();
        server.join().unwrap();

        assert!(
            matches!(&err, ApiError::Transport(e) if e.kind() == TransportKind::Body),
            "{err:?}"
        );
        assert_eq!(err.class(), Class::Network);
    }

    #[tokio::test]
    async fn the_timeout_bounds_the_body_as_well_as_the_head() {
        let (url, server) = short_body_server(Duration::from_millis(1_500));
        let mut sent = request(&url);
        sent.timeout = Duration::from_millis(300);

        // The head arrives at once, so the send succeeds; the body then
        // stalls past the timeout.
        let response = transport().send(sent).await.unwrap();
        let err = response.bytes().await.unwrap_err();
        server.join().unwrap();

        assert!(
            matches!(&err, ApiError::Transport(e) if e.kind() == TransportKind::Timeout),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn a_scheme_the_client_refuses_is_a_request_failure() {
        let err = transport()
            .send(request("ftp://127.0.0.1/file"))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), TransportKind::Request, "{err:?}");
        assert_eq!(err.class(), Class::InvalidRequest);
    }

    #[tokio::test]
    async fn the_default_transport_follows_no_redirect() {
        let mut server = mockito::Server::new_async().await;
        let _moved = server
            .mock("GET", "/moved")
            .with_status(302)
            .with_header("location", "/elsewhere")
            .create_async()
            .await;
        let elsewhere = server
            .mock("GET", "/elsewhere")
            .with_status(200)
            .expect(0)
            .create_async()
            .await;
        let http = crate::HttpClientBuilder::new(server.url()).build().unwrap();

        let err = http
            .send(crate::RequestParts::new(Method::GET, "/moved"), &[], None)
            .await
            .unwrap_err();

        assert!(
            matches!(&err, ApiError::Response(r) if r.status == StatusCode::FOUND),
            "{err:?}"
        );
        elsewhere.assert_async().await;
    }
}
