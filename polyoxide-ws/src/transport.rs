//! How a connection is dialled: the socket transport seam, and the native
//! transport behind it.

#[cfg(not(target_arch = "wasm32"))]
use std::net::{IpAddr, SocketAddr};
use std::{fmt, sync::Arc, time::Duration};

use futures_util::{Sink, Stream};
use polyoxide_rt::{BoxFuture, Runtime};
#[cfg(not(target_arch = "wasm32"))]
use tokio::net::TcpStream;

use crate::{
    tungstenite::{self, http::Request, Message},
    WsError,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    tungstenite::{error::UrlError, http::Uri},
    Kind,
};

/// An open connection as the kit reads and writes it: a stream of frames and a
/// sink for them, both failing with tungstenite's error.
///
/// Every type that is both, and `Send + Unpin`, is one, so a transport returns
/// its own socket boxed as a [`BoxSocket`].
pub trait SocketIo:
    Stream<Item = Result<Message, tungstenite::Error>>
    + Sink<Message, Error = tungstenite::Error>
    + Send
    + Unpin
{
}

impl<T> SocketIo for T where
    T: Stream<Item = Result<Message, tungstenite::Error>>
        + Sink<Message, Error = tungstenite::Error>
        + Send
        + Unpin
{
}

/// An open connection, boxed: what a [`SocketTransport`] returns and
/// [`Connection`](crate::Connection) reads.
pub type BoxSocket = Box<dyn SocketIo>;

/// How a WebSocket connection is dialled.
///
/// A builder holds one, or leaves it to [`default_socket_transport`], so a
/// consumer injects nothing unless it wants its own dial. The transport
/// bounds each of its own steps with the `runtime` it is given, so one
/// transport serves clients with different runtimes, and a connect that does
/// not open within `timeout` fails as [`Kind::ConnectTimeout`](crate::Kind::ConnectTimeout).
/// Every other failure is the transport's: a refused upgrade is a
/// [`Kind::Refused`](crate::Kind::Refused) where the transport can see its
/// status.
pub trait SocketTransport: Send + Sync + fmt::Debug + 'static {
    /// Open the connection `request` describes, within `timeout`.
    fn connect(
        &self,
        request: Request<()>,
        timeout: Duration,
        runtime: Arc<dyn Runtime>,
    ) -> BoxFuture<Result<BoxSocket, WsError>>;

    /// What this transport can do, so a protocol adapts instead of failing.
    fn capabilities(&self) -> Capabilities;
}

/// What a [`SocketTransport`] can do.
///
/// A protocol reads it to choose how it keeps a connection alive: a transport
/// that cannot send a ping frame gets an application-level ping, and one that
/// never shows the server's ping frames cannot count them as liveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Capabilities {
    /// Whether the client can send ping and pong frames.
    pub client_pings: bool,
    /// Whether the server's ping frames reach the reader as messages.
    pub sees_server_pings: bool,
    /// Whether the handshake request may carry headers beyond the ones the
    /// WebSocket protocol generates.
    pub handshake_headers: bool,
}

impl Capabilities {
    /// Every capability: what a native socket has.
    pub const FULL: Capabilities = Capabilities::new(true, true, true);

    /// A transport's capabilities.
    pub const fn new(client_pings: bool, sees_server_pings: bool, handshake_headers: bool) -> Self {
        Self {
            client_pings,
            sees_server_pings,
            handshake_headers,
        }
    }
}

/// The transport a builder uses when none was injected: the native one on this
/// target.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_socket_transport() -> Arc<dyn SocketTransport> {
    Arc::new(TungsteniteTransport::new())
}

/// The native transport: tokio-tungstenite over TCP, with TLS from rustls.
///
/// It installs the rustls `CryptoProvider` before it dials
/// ([`ensure_crypto_provider`](crate::ensure_crypto_provider)), since a
/// consumer that builds two rustls backends gets no default and a direct
/// connect would panic. Its capabilities are all `true`.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, Default)]
pub struct TungsteniteTransport {
    per_address: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl TungsteniteTransport {
    /// The plain dial: tungstenite's own connect, which resolves the host and
    /// connects to the first address it returns.
    pub const fn new() -> Self {
        Self { per_address: false }
    }

    /// The dial of a host that has several addresses, some of which may not
    /// answer: it resolves every address up front, tries each in
    /// family-interleaved order (IPv6 and IPv4 alternating, RFC 8305 section 4),
    /// and bounds the TCP connect and, separately, the TLS and WebSocket
    /// upgrade of each by the connect timeout, so a blackholed route costs one
    /// timeout and not one per address. It falls back to the next address on
    /// any failure but a refused upgrade, which it returns at once: another
    /// address would repeat the refusal, and its own failure would hide this
    /// one's status and `Retry-After`.
    pub const fn per_address() -> Self {
        Self { per_address: true }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SocketTransport for TungsteniteTransport {
    fn connect(
        &self,
        request: Request<()>,
        timeout: Duration,
        runtime: Arc<dyn Runtime>,
    ) -> BoxFuture<Result<BoxSocket, WsError>> {
        let per_address = self.per_address;
        Box::pin(async move {
            crate::ensure_crypto_provider();
            if per_address {
                return dial_per_address(request, timeout, runtime.as_ref()).await;
            }
            let (socket, _response) = crate::connect::bounded(
                runtime.as_ref(),
                timeout,
                tokio_tungstenite::connect_async(request),
            )
            .await??;
            Ok::<BoxSocket, WsError>(Box::new(socket))
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::FULL
    }
}

/// Interleave IPv6 and IPv4 addresses, Happy-Eyeballs style (RFC 8305 §4).
///
/// A lookup typically returns all AAAA records before all A records, so a
/// network with broken IPv6 would burn one full timeout per v6 address before
/// ever trying v4. Alternating families bounds the damage to a single timeout.
#[cfg(not(target_arch = "wasm32"))]
fn interleave_address_families(addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    let (v6, v4): (Vec<_>, Vec<_>) = addrs.into_iter().partition(SocketAddr::is_ipv6);
    let mut out = Vec::with_capacity(v6.len() + v4.len());
    let (mut v6, mut v4) = (v6.into_iter(), v4.into_iter());
    loop {
        match (v6.next(), v4.next()) {
            (None, None) => return out,
            (a, b) => out.extend(a.into_iter().chain(b)),
        }
    }
}

/// An I/O failure while connecting, as the kit reports a transport's.
#[cfg(not(target_arch = "wasm32"))]
fn io_error(err: std::io::Error) -> WsError {
    WsError::from(tungstenite::Error::Io(err))
}

/// A host that resolved to no address: a DNS miss, so an I/O failure like any
/// other, not a refusal of the request.
#[cfg(not(target_arch = "wasm32"))]
fn no_addresses(uri: &Uri) -> WsError {
    io_error(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("no addresses resolved for {uri}"),
    ))
}

/// What a request names to dial: an address as written, or a name to resolve.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, PartialEq, Eq)]
enum Target<'a> {
    Ip(IpAddr),
    Name(&'a str),
}

/// The host a URI names and the port to dial: the URI's own, or its scheme's
/// default (80 for `ws://`, 443 otherwise). `None` when it has no host.
#[cfg(not(target_arch = "wasm32"))]
fn dial_target(uri: &Uri) -> Option<(Target<'_>, u16)> {
    let host = uri.host()?;
    let port = uri.port_u16().unwrap_or(match uri.scheme_str() {
        Some("ws") => 80,
        _ => 443,
    });
    let literal = host.trim_start_matches('[').trim_end_matches(']');
    Some(match literal.parse::<IpAddr>() {
        Ok(ip) => (Target::Ip(ip), port),
        Err(_) => (Target::Name(host), port),
    })
}

/// Open `request` address by address, within `timeout` for each step.
#[cfg(not(target_arch = "wasm32"))]
async fn dial_per_address(
    request: Request<()>,
    timeout: Duration,
    runtime: &dyn Runtime,
) -> Result<BoxSocket, WsError> {
    let uri = request.uri().clone();
    let (target, port) = dial_target(&uri)
        .ok_or_else(|| WsError::from(tungstenite::Error::Url(UrlError::NoHostName)))?;

    // An address as written needs no lookup; only a name is resolved.
    let addrs: Vec<SocketAddr> = match target {
        Target::Ip(ip) => vec![SocketAddr::new(ip, port)],
        Target::Name(name) => tokio::net::lookup_host((name, port))
            .await
            .map_err(io_error)?
            .collect(),
    };

    let mut last_err = no_addresses(&uri);
    for addr in interleave_address_families(addrs) {
        let stream = match crate::connect::bounded(runtime, timeout, TcpStream::connect(addr)).await
        {
            Ok(Ok(stream)) => stream,
            Ok(Err(err)) => {
                last_err = io_error(err);
                continue;
            }
            Err(err) => {
                last_err = err;
                continue;
            }
        };
        // TCP is up; bound the TLS and WebSocket handshake by the same budget.
        match crate::connect::bounded(
            runtime,
            timeout,
            crate::connect_over(request.clone(), stream),
        )
        .await
        {
            Ok(Ok(socket)) => return Ok(socket),
            // The host answered and refused the upgrade.
            Ok(Err(err)) if matches!(err.kind(), Kind::Refused { .. }) => return Err(err),
            Ok(Err(err)) => last_err = err,
            Err(err) => last_err = err,
        }
    }
    Err(last_err)
}

/// Generates the transport setters a socket builder offers, each writing a
/// field of the builder: `runtime(Arc<dyn Runtime>)` writes `self.runtime`, an
/// `Option<Arc<dyn polyoxide_rt::Runtime>>`, and `socket_transport(Arc<dyn
/// SocketTransport>)` writes `self.socket_transport`, an
/// `Option<Arc<dyn SocketTransport>>`.
///
/// Invoke it in the builder's `impl` as [`timing_setters!`](crate::timing_setters)
/// is, listing the setters wanted, each with its own doc comment. The
/// invoking crate declares `polyoxide-rt`. A builder's connect falls back to
/// [`default_runtime`](polyoxide_rt::default_runtime) and its default
/// transport for a field left `None`.
///
/// ```
/// use std::sync::Arc;
///
/// #[derive(Default)]
/// pub struct FeedBuilder {
///     runtime: Option<Arc<dyn polyoxide_rt::Runtime>>,
///     socket_transport: Option<Arc<dyn polyoxide_ws::SocketTransport>>,
/// }
///
/// impl FeedBuilder {
///     polyoxide_ws::transport_setters! {
///         /// The runtime the feed's timers and tasks run on.
///         runtime;
///         /// How the feed dials.
///         socket_transport;
///     }
/// }
///
/// let builder = FeedBuilder::default()
///     .runtime(polyoxide_rt::default_runtime())
///     .socket_transport(polyoxide_ws::default_socket_transport());
/// assert!(builder.runtime.is_some() && builder.socket_transport.is_some());
/// ```
#[macro_export]
macro_rules! transport_setters {
    () => {};
    ($(#[$meta:meta])* runtime; $($rest:tt)*) => {
        $(#[$meta])*
        pub fn runtime(
            mut self,
            runtime: ::std::sync::Arc<dyn ::polyoxide_rt::Runtime>,
        ) -> Self {
            self.runtime = ::core::option::Option::Some(runtime);
            self
        }
        $crate::transport_setters! { $($rest)* }
    };
    ($(#[$meta:meta])* socket_transport; $($rest:tt)*) => {
        $(#[$meta])*
        pub fn socket_transport(
            mut self,
            transport: ::std::sync::Arc<dyn $crate::SocketTransport>,
        ) -> Self {
            self.socket_transport = ::core::option::Option::Some(transport);
            self
        }
        $crate::transport_setters! { $($rest)* }
    };
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use futures_util::StreamExt;
    use polyoxide_rt::{default_runtime, BoxFuture, Instant, Runtime};

    use super::*;
    use crate::{
        test_server::{Script, ScriptedServer},
        tungstenite::{client::IntoClientRequest, Message},
        Kind,
    };

    /// A runtime whose clock has already run out: every sleep is over at once.
    #[derive(Debug)]
    struct Elapsed;

    impl Runtime for Elapsed {
        fn spawn(&self, _task: BoxFuture<()>) {}

        fn sleep(&self, _duration: Duration) -> BoxFuture<()> {
            Box::pin(std::future::ready(()))
        }

        fn now(&self) -> Instant {
            Instant::now()
        }
    }

    #[test]
    fn capabilities_are_what_they_were_built_with() {
        let caps = Capabilities::new(false, true, false);
        assert!(!caps.client_pings);
        assert!(caps.sees_server_pings);
        assert!(!caps.handshake_headers);
        assert_eq!(Capabilities::FULL, Capabilities::new(true, true, true));
        assert_ne!(Capabilities::FULL, caps);
    }

    #[test]
    fn the_native_default_transport_can_do_everything() {
        assert_eq!(
            default_socket_transport().capabilities(),
            Capabilities::FULL
        );
        assert_eq!(
            TungsteniteTransport::new().capabilities(),
            Capabilities::FULL
        );
    }

    #[tokio::test]
    async fn the_plain_transport_dials_and_the_socket_reads_a_frame() {
        let server = ScriptedServer::start(vec![Script::frames(&["hello"])]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let mut socket = TungsteniteTransport::new()
            .connect(request, Duration::from_secs(2), default_runtime())
            .await
            .expect("connect");
        let first = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("a frame within the window")
            .expect("the stream has not ended")
            .expect("a frame, not an error");
        assert_eq!(first, Message::Text("hello".into()));
    }

    #[tokio::test]
    async fn a_refused_upgrade_keeps_its_status() {
        let server = ScriptedServer::start(vec![Script::refuse_upgrade(503)]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let Err(err) = TungsteniteTransport::new()
            .connect(request, Duration::from_secs(2), default_runtime())
            .await
        else {
            panic!("a refused upgrade must not connect");
        };
        assert_eq!(
            err.kind(),
            Kind::Refused {
                status: 503,
                retry_after: None
            }
        );
    }

    #[tokio::test]
    async fn the_transport_bounds_its_connect_with_the_runtime_it_is_given() {
        // The server never answers the upgrade and the timeout is an hour, so
        // only the given runtime's timer, which is already over, can end it.
        let server = ScriptedServer::start(vec![Script::stall()]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let after = Duration::from_secs(3600);
        let Err(err) = TungsteniteTransport::new()
            .connect(request, after, Arc::new(Elapsed))
            .await
        else {
            panic!("a stalled handshake must not connect");
        };
        assert_eq!(err.kind(), Kind::ConnectTimeout { after });
    }

    #[test]
    fn the_setters_write_the_builders_fields() {
        #[derive(Default)]
        struct Builder {
            runtime: Option<Arc<dyn Runtime>>,
            socket_transport: Option<Arc<dyn SocketTransport>>,
        }

        impl Builder {
            crate::transport_setters! {
                /// The runtime.
                runtime;
                /// The transport.
                socket_transport;
            }
        }

        let bare = Builder::default();
        assert!(bare.runtime.is_none() && bare.socket_transport.is_none());
        let set = Builder::default()
            .runtime(default_runtime())
            .socket_transport(default_socket_transport());
        assert!(set.runtime.is_some() && set.socket_transport.is_some());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod connect_tests {
    use std::{net::SocketAddr, time::Duration};

    use futures_util::StreamExt;
    use polyoxide_rt::default_runtime;

    use super::*;
    use crate::{
        test_server::{Script, ScriptedServer},
        tungstenite::client::IntoClientRequest,
        Kind,
    };

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn address_families_are_interleaved() {
        // Resolvers return all AAAA then all A; a broken v6 network must not
        // pay one timeout per v6 address before the first v4 attempt.
        let addrs = vec![
            addr("[2001:db8::1]:443"),
            addr("[2001:db8::2]:443"),
            addr("192.0.2.1:443"),
            addr("192.0.2.2:443"),
            addr("192.0.2.3:443"),
        ];
        let out = interleave_address_families(addrs);
        assert_eq!(
            out,
            vec![
                addr("[2001:db8::1]:443"),
                addr("192.0.2.1:443"),
                addr("[2001:db8::2]:443"),
                addr("192.0.2.2:443"),
                addr("192.0.2.3:443"),
            ]
        );
    }

    #[test]
    fn single_family_lists_pass_through_unchanged() {
        let v4 = vec![addr("192.0.2.1:443"), addr("192.0.2.2:443")];
        assert_eq!(interleave_address_families(v4.clone()), v4);
        assert!(interleave_address_families(Vec::new()).is_empty());
    }

    #[test]
    fn a_uri_without_a_port_dials_its_schemes_default() {
        for (uri, port) in [
            ("ws://127.0.0.1/", 80),
            ("wss://127.0.0.1/", 443),
            ("ws://127.0.0.1:9/", 9),
            ("wss://example.com/ws/market", 443),
        ] {
            let parsed: Uri = uri.parse().unwrap();
            let (_, dialled) = dial_target(&parsed).expect(uri);
            assert_eq!(dialled, port, "{uri}");
        }
        assert_eq!(
            dial_target(&"ws://[::1]/".parse().unwrap()),
            Some((Target::Ip(std::net::Ipv6Addr::LOCALHOST.into()), 80))
        );
        assert_eq!(
            dial_target(&"wss://example.com/x".parse().unwrap()),
            Some((Target::Name("example.com"), 443))
        );
    }

    #[test]
    fn a_dns_miss_is_an_io_failure_and_says_what_it_could_not_resolve() {
        let err = no_addresses(
            &"wss://ws.example.invalid/feed"
                .parse()
                .expect("a valid uri"),
        );
        assert_eq!(err.kind(), Kind::Io);
        assert_eq!(
            err.to_string(),
            "IO error: no addresses resolved for wss://ws.example.invalid/feed"
        );
    }

    #[tokio::test]
    async fn per_address_returns_the_upgraded_socket_with_nothing_read() {
        let server = ScriptedServer::start(vec![Script::frames(&["hello"])]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let mut socket = TungsteniteTransport::per_address()
            .connect(request, Duration::from_secs(2), default_runtime())
            .await
            .expect("connect");
        let first = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("a frame within the window")
            .expect("the stream has not ended")
            .expect("a frame, not an error");
        assert_eq!(first, Message::Text("hello".into()));
    }

    #[tokio::test]
    async fn per_address_returns_a_refused_upgrade_at_once() {
        let server = ScriptedServer::start(vec![Script::refuse_upgrade(503)]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let Err(err) = TungsteniteTransport::per_address()
            .connect(request, Duration::from_secs(2), default_runtime())
            .await
        else {
            panic!("a refused upgrade must not connect");
        };
        assert_eq!(
            err.kind(),
            Kind::Refused {
                status: 503,
                retry_after: None
            }
        );
        assert_eq!(server.connection_count(), 1, "no other address is tried");
    }

    #[tokio::test]
    async fn per_address_bounds_the_handshake_by_the_timeout() {
        let server = ScriptedServer::start(vec![Script::stall()]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let after = Duration::from_millis(200);
        let Err(err) = TungsteniteTransport::per_address()
            .connect(request, after, default_runtime())
            .await
        else {
            panic!("a stalled handshake must not connect");
        };
        assert_eq!(err.kind(), Kind::ConnectTimeout { after });
    }

    #[tokio::test]
    async fn a_request_naming_no_host_is_a_url_failure_before_any_dial() {
        let request = Request::builder().uri("/feed").body(()).unwrap();
        let Err(err) = TungsteniteTransport::per_address()
            .connect(request, Duration::from_secs(2), default_runtime())
            .await
        else {
            panic!("a request with no host must not connect");
        };
        assert_eq!(err.kind(), Kind::Url);
    }
}
