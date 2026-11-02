//! How a connection is dialled: the socket transport seam, and the native
//! transport behind it.

use std::{fmt, sync::Arc, time::Duration};

use futures_util::{Sink, Stream};
use polyoxide_rt::{BoxFuture, Runtime};

use crate::{
    tungstenite::{self, http::Request, Message},
    WsError,
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
pub struct TungsteniteTransport;

#[cfg(not(target_arch = "wasm32"))]
impl TungsteniteTransport {
    /// The plain dial: tungstenite's own connect, which resolves the host and
    /// connects to the first address it returns.
    pub const fn new() -> Self {
        Self
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
        Box::pin(async move {
            crate::ensure_crypto_provider();
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
