//! The browser smoke suite's shared constants.
//!
//! The suite has two halves. `wasm-smoke-server`, a native bin, answers a handful of routes with
//! CORS headers and holds one scripted socket. `tests/browser_smoke.rs`, built for
//! `wasm32-unknown-unknown`, runs in a dedicated Worker under headless Chrome and calls them
//! through the browser implementations. The two halves agree on the paths, the 429's
//! `Retry-After` and the socket's commands here, and nowhere else.

#![warn(missing_docs)]

use std::time::Duration;

/// The environment variable holding the HTTP server's base URL, as the server writes it and the
/// browser tests read it at compile time.
pub const HTTP_ENV: &str = "POLYOXIDE_SMOKE_HTTP";
/// The environment variable holding the socket server's `ws://` URL.
pub const WS_ENV: &str = "POLYOXIDE_SMOKE_WS";
/// The environment variable holding the base URL of the server that answers [`SLOW_PATH`]. It is
/// a server of its own: mockito answers a slow route by sleeping its server's one thread, which
/// would hold up every other route on that server.
pub const SLOW_ENV: &str = "POLYOXIDE_SMOKE_SLOW";

/// A route answering 200 with [`OK_BODY`].
pub const OK_PATH: &str = "/ok";
/// The body of [`OK_PATH`].
pub const OK_BODY: &str = r#"{"ok":true}"#;

/// A route that answers 200 only after [`SLOW_DELAY`], on the server [`SLOW_ENV`] names.
pub const SLOW_PATH: &str = "/slow";
/// How long [`SLOW_PATH`] holds its answer back.
pub const SLOW_DELAY: Duration = Duration::from_millis(1500);

/// A route answering 429 whose `Retry-After` the browser may read, since the response names it
/// in `Access-Control-Expose-Headers`.
pub const LIMITED_EXPOSED_PATH: &str = "/limited-exposed";
/// A route answering 429 with a `Retry-After` the browser may not read: no
/// `Access-Control-Expose-Headers`.
pub const LIMITED_HIDDEN_PATH: &str = "/limited-hidden";
/// The body of both 429 routes.
pub const LIMITED_BODY: &str = r#"{"error":"slow down"}"#;
/// The seconds both 429 routes send as `Retry-After`.
pub const RETRY_AFTER_SECS: u64 = 7;

/// What the socket sends back for a text frame: this, then the frame.
pub const ECHO_PREFIX: &str = "echo:";
/// The text frame that makes the socket close.
pub const CLOSE_COMMAND: &str = "close";
/// The code of the close the socket sends for [`CLOSE_COMMAND`].
pub const CLOSE_CODE: u16 = 4001;
/// The reason of that close.
pub const CLOSE_REASON: &str = "smoke close";
