//! The browser's socket transport.
//!
//! `WebSocketTransport`, built only for `wasm32`, dials with the platform's
//! `WebSocket`. A browser socket cannot set a handshake header, send a ping or
//! pong frame or apply backpressure. What the transport decides before it
//! touches the platform (which requests it refuses, which close codes it may
//! send, which frames it cannot) is plain Rust, built on every target so the
//! host's tests pin it.

/// The message of the `Kind::Unsupported` error a transport that sends no
/// ping frames (a browser's) gives for a ping: a caller that refuses a ping
/// before sending it builds its error from this text, so the refusal reads
/// the same as the transport's own.
pub const PING_UNSUPPORTED: &str = "a ping frame is not supported by the browser transport";

// The test server's browser-like transport (`test-server`) refuses a frame by
// `outgoing`, so the two refuse alike; the rest is wasm32's and the tests'.
#[cfg(any(target_arch = "wasm32", test, feature = "test-server"))]
#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
pub(crate) mod plain {
    use std::io;

    use super::PING_UNSUPPORTED;
    use crate::tungstenite::{
        error::UrlError,
        http::{header, HeaderName, Request},
        Error, Message,
    };

    const HEADER_UNSUPPORTED: &str = "a handshake header is not supported by the browser transport";
    const PONG_UNSUPPORTED: &str = "a pong frame is not supported by the browser transport";
    const FRAME_UNSUPPORTED: &str = "a raw frame is not supported by the browser transport";

    /// The longest close reason a browser takes: it throws on more.
    const CLOSE_REASON_MAX: usize = 123;

    /// An operation the browser cannot perform, in the form `WsError::kind`
    /// reads as `Kind::Unsupported`.
    fn unsupported(message: &'static str) -> Error {
        Error::Io(io::Error::new(io::ErrorKind::Unsupported, message))
    }

    /// What the browser needs to open a socket.
    #[derive(Debug, PartialEq, Eq)]
    pub(super) struct Dial {
        pub(super) url: String,
        pub(super) protocols: Vec<String>,
    }

    /// The headers tungstenite generates for every handshake request, which
    /// the browser generates for itself.
    fn is_generated(name: &HeaderName) -> bool {
        [
            header::HOST,
            header::CONNECTION,
            header::UPGRADE,
            header::SEC_WEBSOCKET_VERSION,
            header::SEC_WEBSOCKET_KEY,
        ]
        .contains(name)
    }

    /// Read a handshake request as the browser can act on it.
    ///
    /// The URL must be `ws` or `wss`. A `Sec-WebSocket-Protocol` header
    /// becomes the subprotocol list; any other header beyond the generated
    /// ones is refused, since a browser cannot set it.
    // The error is tungstenite's, the type `SocketIo` and `WsError` take.
    #[allow(clippy::result_large_err)]
    pub(super) fn dial_plan(request: &Request<()>) -> Result<Dial, Error> {
        let uri = request.uri();
        if !matches!(uri.scheme_str(), Some("ws" | "wss")) {
            return Err(Error::Url(UrlError::UnsupportedUrlScheme));
        }
        let mut protocols = Vec::new();
        for (name, value) in request.headers() {
            if *name == header::SEC_WEBSOCKET_PROTOCOL {
                let list = value
                    .to_str()
                    .map_err(|_| unsupported(HEADER_UNSUPPORTED))?;
                protocols.extend(
                    list.split(',')
                        .map(str::trim)
                        .filter(|protocol| !protocol.is_empty())
                        .map(str::to_owned),
                );
            } else if !is_generated(name) {
                return Err(unsupported(HEADER_UNSUPPORTED));
            }
        }
        Ok(Dial {
            url: uri.to_string(),
            protocols,
        })
    }

    /// The close code a browser may send: 1000, or one in 3000-4999. A browser
    /// throws on any other, so any other goes out as 1000.
    pub(super) const fn close_code(code: u16) -> u16 {
        if matches!(code, 1000 | 3000..=4999) {
            code
        } else {
            1000
        }
    }

    /// The close reason cut to what a browser takes, on a character boundary.
    pub(super) fn close_reason(reason: &str) -> &str {
        &reason[..reason.floor_char_boundary(CLOSE_REASON_MAX)]
    }

    /// A frame the browser can send, with the form it takes there.
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) enum Outgoing<'a> {
        Text(&'a str),
        Binary(&'a [u8]),
        Close { code: u16, reason: &'a str },
    }

    /// What sending `message` means to the browser, or why it cannot.
    // The error is tungstenite's, the type `SocketIo` and `WsError` take.
    #[allow(clippy::result_large_err)]
    pub(crate) fn outgoing(message: &Message) -> Result<Outgoing<'_>, Error> {
        match message {
            Message::Text(text) => Ok(Outgoing::Text(text.as_str())),
            Message::Binary(bytes) => Ok(Outgoing::Binary(bytes.as_ref())),
            Message::Close(Some(frame)) => Ok(Outgoing::Close {
                code: close_code(u16::from(frame.code)),
                reason: close_reason(frame.reason.as_str()),
            }),
            Message::Close(None) => Ok(Outgoing::Close {
                code: 1000,
                reason: "",
            }),
            Message::Ping(_) => Err(unsupported(PING_UNSUPPORTED)),
            Message::Pong(_) => Err(unsupported(PONG_UNSUPPORTED)),
            Message::Frame(_) => Err(unsupported(FRAME_UNSUPPORTED)),
        }
    }

    /// What a send means to a browser socket in its current state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum SendState {
        /// The socket is open: hand the frame to the platform.
        Accept,
        /// The socket is closing and its close has not been yielded: take the
        /// frame and drop it. The close event that ends the stream is on its
        /// way, as it is on a native socket whose peer has closed but whose
        /// close frame has not been read yet.
        Drop,
        /// The stream has ended: the send fails.
        AlreadyClosed,
    }

    /// Decide a send from the socket's `readyState` (the WHATWG values:
    /// 0 connecting, 1 open, 2 closing, 3 closed) and whether the stream has
    /// yielded its close.
    ///
    /// A send in the window between the browser starting to close and the
    /// close event reaching the reader must not fail: a supervisor reads
    /// `AlreadyClosed` as the end of the stream, where a routine server close
    /// should reconnect.
    pub(super) const fn send_state(ready_state: u16, ended: bool) -> SendState {
        if ended {
            SendState::AlreadyClosed
        } else if ready_state >= 2 {
            SendState::Drop
        } else {
            SendState::Accept
        }
    }

    #[cfg(test)]
    mod tests {
        use std::io;

        use crate::tungstenite::{
            client::IntoClientRequest,
            error::UrlError,
            http::{header, HeaderValue, Request},
            protocol::{frame::coding::CloseCode, frame::Frame, CloseFrame},
            Error, Message,
        };
        use crate::{retrying_can_fix, Kind, WsError};

        use super::{
            close_code, close_reason, dial_plan, outgoing, send_state, Dial, Outgoing, SendState,
        };

        fn request(url: &str) -> Request<()> {
            url.into_client_request().expect("a valid WebSocket URL")
        }

        fn unsupported_message(err: &Error) -> Option<String> {
            match err {
                Error::Io(inner) if inner.kind() == io::ErrorKind::Unsupported => {
                    Some(inner.to_string())
                }
                _ => None,
            }
        }

        #[test]
        fn the_request_tungstenite_builds_is_dialled_by_url_alone() {
            let plan = dial_plan(&request("wss://stream.example.invalid/ws?x=1"));
            assert_eq!(
                plan.unwrap(),
                Dial {
                    url: "wss://stream.example.invalid/ws?x=1".to_owned(),
                    protocols: Vec::new(),
                }
            );
        }

        #[test]
        fn a_subprotocol_header_becomes_the_subprotocol_list() {
            let mut req = request("ws://stream.example.invalid/");
            req.headers_mut().append(
                header::SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_static("json, v2"),
            );
            req.headers_mut().append(
                header::SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_static("extra"),
            );
            assert_eq!(dial_plan(&req).unwrap().protocols, ["json", "v2", "extra"]);
        }

        #[test]
        fn any_other_handshake_header_is_refused_with_its_message() {
            let mut req = request("wss://stream.example.invalid/");
            req.headers_mut()
                .insert("x-api-key", HeaderValue::from_static("secret"));
            let err = dial_plan(&req).unwrap_err();
            assert_eq!(
                unsupported_message(&err).as_deref(),
                Some("a handshake header is not supported by the browser transport")
            );
            assert_eq!(WsError::from(err).kind(), Kind::Unsupported);
        }

        #[test]
        fn a_url_that_is_not_a_websocket_url_is_refused_before_dialling() {
            let req = Request::builder()
                .uri("https://stream.example.invalid/")
                .body(())
                .unwrap();
            let err = dial_plan(&req).unwrap_err();
            assert!(matches!(err, Error::Url(UrlError::UnsupportedUrlScheme)));
            let err = WsError::from(err);
            assert_eq!(err.kind(), Kind::Url);
            assert!(!retrying_can_fix(&err, |_| true));
        }

        #[test]
        fn a_close_code_goes_out_only_when_a_browser_may_send_it() {
            for code in [1000, 3000, 3999, 4000, 4999] {
                assert_eq!(close_code(code), code, "{code}");
            }
            // A browser throws on every other code, 1001 and 1011 included.
            for code in [0, 999, 1001, 1002, 1005, 1006, 1011, 2999, 5000, u16::MAX] {
                assert_eq!(close_code(code), 1000, "{code}");
            }
        }

        #[test]
        fn a_close_reason_is_cut_to_the_browsers_123_bytes_on_a_char_boundary() {
            assert_eq!(close_reason("going away"), "going away");
            let exact = "a".repeat(123);
            assert_eq!(close_reason(&exact), exact);
            assert_eq!(close_reason(&"a".repeat(124)).len(), 123);
            // 100 two-byte characters: 123 falls inside one, so 122 bytes stay.
            let wide = "é".repeat(100);
            let cut = close_reason(&wide);
            assert_eq!(cut.len(), 122);
            assert!(wide.starts_with(cut));
        }

        #[test]
        fn text_binary_and_close_frames_are_sent() {
            assert_eq!(
                outgoing(&Message::text("hi")).unwrap(),
                Outgoing::Text("hi")
            );
            assert_eq!(
                outgoing(&Message::binary(vec![1, 2, 3])).unwrap(),
                Outgoing::Binary(&[1, 2, 3])
            );
            let away = Message::Close(Some(CloseFrame {
                code: CloseCode::from(1001),
                reason: "bye".into(),
            }));
            assert_eq!(
                outgoing(&away).unwrap(),
                Outgoing::Close {
                    code: 1000,
                    reason: "bye"
                }
            );
            let library = Message::Close(Some(CloseFrame {
                code: CloseCode::from(4000),
                reason: "".into(),
            }));
            assert_eq!(
                outgoing(&library).unwrap(),
                Outgoing::Close {
                    code: 4000,
                    reason: ""
                }
            );
            assert_eq!(
                outgoing(&Message::Close(None)).unwrap(),
                Outgoing::Close {
                    code: 1000,
                    reason: ""
                }
            );
        }

        #[test]
        fn a_send_on_a_live_socket_is_accepted() {
            for ready_state in [0, 1] {
                assert_eq!(send_state(ready_state, false), SendState::Accept);
            }
        }

        #[test]
        fn a_send_while_the_close_is_still_to_come_is_dropped_not_failed() {
            // CLOSING, and CLOSED with the close event still in the channel.
            for ready_state in [2, 3] {
                assert_eq!(send_state(ready_state, false), SendState::Drop);
            }
        }

        #[test]
        fn a_send_after_the_stream_has_ended_is_already_closed() {
            for ready_state in 0..=3 {
                assert_eq!(send_state(ready_state, true), SendState::AlreadyClosed);
            }
        }

        #[test]
        fn ping_pong_and_raw_frames_are_refused_with_their_messages() {
            let cases = [
                (
                    Message::Ping(Vec::<u8>::new().into()),
                    "a ping frame is not supported by the browser transport",
                ),
                (
                    Message::Pong(Vec::<u8>::new().into()),
                    "a pong frame is not supported by the browser transport",
                ),
                (
                    Message::Frame(Frame::ping(Vec::<u8>::new())),
                    "a raw frame is not supported by the browser transport",
                ),
            ];
            for (message, text) in cases {
                let err = outgoing(&message).unwrap_err();
                assert_eq!(unsupported_message(&err).as_deref(), Some(text));
                assert_eq!(WsError::from(err).kind(), Kind::Unsupported);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::{
        io,
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
        time::Duration,
    };

    use futures_util::{Sink, Stream};
    use polyoxide_rt::{BoxFuture, Runtime};
    use send_wrapper::SendWrapper;
    use tokio::sync::mpsc;
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

    use super::plain::{dial_plan, outgoing, send_state, Dial, Outgoing, SendState};
    use crate::tungstenite::{
        error::UrlError,
        http::Request,
        protocol::{frame::coding::CloseCode, CloseFrame},
        Error, Message,
    };
    use crate::{BoxSocket, Capabilities, SocketTransport, WsError};

    /// The browser could not open the socket; a refused upgrade, a blocked
    /// host and a lost network look alike to a page.
    const NOT_OPENED: &str = "the browser could not open the WebSocket connection";
    /// The browser reported an error event on an open socket.
    const FAILED: &str = "the browser reported a WebSocket error";

    /// What the platform's callbacks hand the socket.
    enum Incoming {
        Open,
        Message(Message),
        Failed,
        Closed(CloseFrame),
    }

    /// The four callbacks, kept alive as long as the socket.
    struct Callbacks {
        _open: Closure<dyn FnMut(Event)>,
        _message: Closure<dyn FnMut(MessageEvent)>,
        _error: Closure<dyn FnMut(Event)>,
        _close: Closure<dyn FnMut(CloseEvent)>,
    }

    /// A text or binary frame as the kit sees it; anything else a browser
    /// might deliver is dropped.
    fn decode(data: &JsValue) -> Option<Message> {
        if let Some(text) = data.as_string() {
            return Some(Message::text(text));
        }
        if data.is_instance_of::<js_sys::ArrayBuffer>() {
            return Some(Message::binary(js_sys::Uint8Array::new(data).to_vec()));
        }
        None
    }

    fn attach(ws: &WebSocket, tx: &mpsc::UnboundedSender<Incoming>) -> Callbacks {
        let sender = tx.clone();
        let open = Closure::<dyn FnMut(Event)>::new(move |_| {
            let _ = sender.send(Incoming::Open);
        });
        ws.set_onopen(Some(open.as_ref().unchecked_ref()));

        let sender = tx.clone();
        let message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if let Some(message) = decode(&event.data()) {
                let _ = sender.send(Incoming::Message(message));
            }
        });
        ws.set_onmessage(Some(message.as_ref().unchecked_ref()));

        let sender = tx.clone();
        let error = Closure::<dyn FnMut(Event)>::new(move |_| {
            let _ = sender.send(Incoming::Failed);
        });
        ws.set_onerror(Some(error.as_ref().unchecked_ref()));

        let sender = tx.clone();
        let close = Closure::<dyn FnMut(CloseEvent)>::new(move |event: CloseEvent| {
            let _ = sender.send(Incoming::Closed(CloseFrame {
                code: CloseCode::from(event.code()),
                reason: event.reason().into(),
            }));
        });
        ws.set_onclose(Some(close.as_ref().unchecked_ref()));

        Callbacks {
            _open: open,
            _message: message,
            _error: error,
            _close: close,
        }
    }

    fn js_error(error: JsValue) -> Error {
        Error::Io(io::Error::other(format!("{error:?}")))
    }

    /// One open browser socket. The `WebSocket` and its callbacks are `!Send`
    /// JS handles, so each sits in a `SendWrapper`: the socket is `Send`, as
    /// `BoxSocket` requires, and every touch of a handle (including its drop)
    /// is on the one thread a browser build has. Events arrive on an
    /// unbounded channel, since a browser socket cannot apply backpressure.
    struct BrowserSocket {
        ws: SendWrapper<WebSocket>,
        _callbacks: SendWrapper<Callbacks>,
        incoming: mpsc::UnboundedReceiver<Incoming>,
        ended: bool,
    }

    impl Unpin for BrowserSocket {}

    impl BrowserSocket {
        fn open(dial: &Dial) -> Result<Self, WsError> {
            let ws = if dial.protocols.is_empty() {
                WebSocket::new(&dial.url)
            } else {
                let list = js_sys::Array::new();
                for protocol in &dial.protocols {
                    list.push(&JsValue::from_str(protocol));
                }
                WebSocket::new_with_str_sequence(&dial.url, &list)
            }
            .map_err(|error| {
                WsError::from(Error::Url(UrlError::UnableToConnect(format!("{error:?}"))))
            })?;
            ws.set_binary_type(BinaryType::Arraybuffer);
            let (tx, incoming) = mpsc::unbounded_channel();
            let callbacks = attach(&ws, &tx);
            Ok(Self {
                ws: SendWrapper::new(ws),
                _callbacks: SendWrapper::new(callbacks),
                incoming,
                ended: false,
            })
        }

        /// Wait for the upgrade. A failure or close before it opens is a lost
        /// connection: a browser shows no status for a refused upgrade.
        async fn opened(&mut self) -> Result<(), WsError> {
            loop {
                match self.incoming.recv().await {
                    Some(Incoming::Open) => return Ok(()),
                    Some(Incoming::Message(_)) => {}
                    Some(Incoming::Failed) => {
                        return Err(WsError::from(Error::Io(io::Error::new(
                            io::ErrorKind::ConnectionRefused,
                            NOT_OPENED,
                        ))))
                    }
                    Some(Incoming::Closed(frame)) => {
                        return Err(WsError::from_close_frame(Some(&frame)))
                    }
                    None => return Err(WsError::from(Error::ConnectionClosed)),
                }
            }
        }

        /// What a send means now. Only a send after the stream has yielded its
        /// close fails; one while the browser is closing is dropped, since the
        /// close event is coming and ends the stream.
        fn send_state(&self) -> SendState {
            send_state(self.ws.ready_state(), self.ended)
        }
    }

    impl Drop for BrowserSocket {
        fn drop(&mut self) {
            // Detach first, so a late event never reaches a freed closure.
            self.ws.set_onopen(None);
            self.ws.set_onmessage(None);
            self.ws.set_onerror(None);
            self.ws.set_onclose(None);
            if self.ws.ready_state() < WebSocket::CLOSING {
                let _ = self.ws.close();
            }
        }
    }

    impl Stream for BrowserSocket {
        type Item = Result<Message, Error>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let this = &mut *self;
            loop {
                if this.ended {
                    return Poll::Ready(None);
                }
                return match this.incoming.poll_recv(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(None) => {
                        this.ended = true;
                        Poll::Ready(None)
                    }
                    Poll::Ready(Some(Incoming::Open)) => continue,
                    Poll::Ready(Some(Incoming::Message(message))) => Poll::Ready(Some(Ok(message))),
                    Poll::Ready(Some(Incoming::Failed)) => {
                        Poll::Ready(Some(Err(Error::Io(io::Error::other(FAILED)))))
                    }
                    Poll::Ready(Some(Incoming::Closed(frame))) => {
                        this.ended = true;
                        Poll::Ready(Some(Ok(Message::Close(Some(frame)))))
                    }
                };
            }
        }
    }

    impl Sink<Message> for BrowserSocket {
        type Error = Error;

        fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
            Poll::Ready(match self.send_state() {
                SendState::AlreadyClosed => Err(Error::AlreadyClosed),
                SendState::Accept | SendState::Drop => Ok(()),
            })
        }

        fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Error> {
            let frame = outgoing(&item)?;
            match self.send_state() {
                SendState::AlreadyClosed => return Err(Error::AlreadyClosed),
                SendState::Drop => return Ok(()),
                SendState::Accept => {}
            }
            let sent = match frame {
                Outgoing::Text(text) => self.ws.send_with_str(text),
                Outgoing::Binary(bytes) => self.ws.send_with_u8_array(bytes),
                Outgoing::Close { code, reason } => {
                    self.ws.close_with_code_and_reason(code, reason)
                }
            };
            sent.map_err(js_error)
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
            // A browser sends as it is told; there is nothing to flush.
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
            if self.ws.ready_state() < WebSocket::CLOSING {
                let _ = self.ws.close();
            }
            Poll::Ready(Ok(()))
        }
    }

    /// A `SocketTransport` over the browser's `WebSocket`, the default of a
    /// `wasm32` socket.
    ///
    /// Its capabilities are all `false`: a browser cannot send a ping, reports
    /// no server ping to the page and takes no handshake header. A request
    /// with a header beyond those tungstenite generates (a
    /// `Sec-WebSocket-Protocol` becomes the subprotocol list) is refused as
    /// `Kind::Unsupported` before dialling, and so is sending a ping, pong or
    /// raw frame. A refused upgrade shows no status to a page, so it arrives
    /// as a lost connection, not `Kind::Refused`. A close event yields
    /// `Message::Close` with its code and reason, then the stream ends.
    /// A send while the browser is closing, before that close event has been
    /// read, is dropped and returns `Ok`, as a native socket's send does until
    /// it reads the peer's close; only a send after the stream has ended is
    /// `AlreadyClosed`.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct WebSocketTransport;

    impl WebSocketTransport {
        /// A transport over the page's `WebSocket`.
        pub const fn new() -> Self {
            Self
        }
    }

    impl SocketTransport for WebSocketTransport {
        fn connect(
            &self,
            request: Request<()>,
            timeout: Duration,
            runtime: Arc<dyn Runtime>,
        ) -> BoxFuture<Result<BoxSocket, WsError>> {
            Box::pin(SendWrapper::new(async move {
                let dial = dial_plan(&request).map_err(WsError::from)?;
                let mut socket = BrowserSocket::open(&dial)?;
                let outcome = polyoxide_rt::timeout(&*runtime, timeout, socket.opened()).await;
                match outcome {
                    Some(Ok(())) => Ok(Box::new(socket) as BoxSocket),
                    Some(Err(error)) => Err(error),
                    // Dropping `socket` closes the half-open connection.
                    None => Err(WsError::connect_timeout(timeout)),
                }
            }))
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::new(false, false, false)
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::WebSocketTransport;
