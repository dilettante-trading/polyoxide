//! The browser's HTTP transport.
//!
//! `FetchTransport`, built only for `wasm32`, sends through the platform's
//! `fetch`, by way of reqwest's wasm client. What it decides itself is which
//! `TransportKind` a failure is, and that is plain Rust, built on every
//! target so the host's tests pin it.

#[cfg(any(target_arch = "wasm32", test))]
use std::time::Duration;

#[cfg(any(target_arch = "wasm32", test))]
use crate::TransportKind;

/// Where in an exchange a failure struck.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Before the response head arrived.
    Fetch,
    /// While the body was being read.
    Body,
}

/// The longest timeout reqwest's wasm client takes: it converts the
/// milliseconds to an `i32` and panics on a larger one. A longer timeout is
/// held to this bound, the one the browser runtime's timers use.
#[cfg(any(target_arch = "wasm32", test))]
fn clamped(timeout: Duration) -> Duration {
    timeout.min(Duration::from_millis(i32::MAX as u64))
}

/// Whether a failed exchange timed out.
///
/// reqwest's wasm client reports a timeout only for one that struck before the
/// response head: a body read cut short by the same timer comes back as a
/// decode error. So the transport decides from its own clock too: a failure at
/// or after the request's deadline timed out, whatever reqwest says.
#[cfg(any(target_arch = "wasm32", test))]
fn timed_out(reqwest_says: bool, elapsed: Duration, timeout: Duration) -> bool {
    reqwest_says || elapsed >= timeout
}

/// The kind of a failed exchange.
///
/// A timeout (`timed_out`) is a timeout wherever it struck. A request the browser would not
/// build (a header value it cannot carry) is the caller's: `Request`, which
/// no retry fixes. Any other failed `fetch` is `Connect`: the browser tells a
/// network failure and a CORS refusal apart for no page. A failed body read is
/// `Body`.
#[cfg(any(target_arch = "wasm32", test))]
fn kind_of(stage: Stage, timed_out: bool, malformed: bool) -> TransportKind {
    if timed_out {
        TransportKind::Timeout
    } else if malformed {
        TransportKind::Request
    } else {
        match stage {
            Stage::Fetch => TransportKind::Connect,
            Stage::Body => TransportKind::Body,
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::time::Duration;

    use bytes::Bytes;
    use polyoxide_rt::{default_runtime, BoxFuture, Instant};
    use send_wrapper::SendWrapper;

    use super::{clamped, kind_of, timed_out, Stage};
    use crate::{HttpRequest, HttpTransport, Response, TransportError};

    /// An `HttpTransport` over the browser's `fetch`, the default of a
    /// `wasm32` client.
    ///
    /// The request's timeout is set per request, held to `i32::MAX`
    /// milliseconds, and bounds the whole exchange, body included; a failure
    /// at or after that deadline is a `Timeout` whichever stage it struck. The browser negotiates compression itself and follows
    /// redirects. Each exchange sits in a `SendWrapper`, since its JS handles
    /// are `!Send`: the `Send` bound of `HttpTransport` holds, and the handles
    /// stay on the one thread a browser build has (a build with atomics is
    /// unsupported).
    #[derive(Debug, Clone, Default)]
    #[non_exhaustive]
    pub struct FetchTransport {}

    impl FetchTransport {
        /// A transport over the page's `fetch`.
        pub fn new() -> Self {
            Self {}
        }
    }

    impl HttpTransport for FetchTransport {
        fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>> {
            Box::pin(SendWrapper::new(fetch(request)))
        }
    }

    async fn fetch(request: HttpRequest) -> Result<Response, TransportError> {
        let timeout = clamped(request.timeout);
        // The transport is built with no runtime, so the deadline reads the target's own.
        let runtime = default_runtime();
        let started = runtime.now();
        let mut builder = reqwest::Client::new()
            .request(request.method, request.url)
            .headers(request.headers)
            .timeout(timeout);
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        let response = builder
            .send()
            .await
            .map_err(|error| failure(Stage::Fetch, error, runtime.now(), started, timeout))?;
        let status = response.status();
        let headers = response.headers().clone();
        let url = response.url().clone();
        // The body is read lazily, as `Response` promises; the reqwest response
        // keeps the timeout's abort guard alive until it is read or dropped.
        let body: BoxFuture<Result<Bytes, TransportError>> =
            Box::pin(SendWrapper::new(async move {
                response
                    .bytes()
                    .await
                    .map_err(|error| failure(Stage::Body, error, runtime.now(), started, timeout))
            }));
        Ok(Response::new(status, headers, url, body))
    }

    fn failure(
        stage: Stage,
        error: reqwest::Error,
        now: Instant,
        started: Instant,
        timeout: Duration,
    ) -> TransportError {
        let elapsed = now.saturating_duration_since(started);
        let timed_out = timed_out(error.is_timeout(), elapsed, timeout);
        TransportError::new(kind_of(stage, timed_out, error.is_builder()), error)
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::FetchTransport;

#[cfg(test)]
mod tests {
    use polyoxide_venue::Classify;

    use std::time::Duration;

    use super::{clamped, kind_of, timed_out, Stage};
    use crate::{TransportError, TransportKind};

    const TIMEOUT: Duration = Duration::from_secs(30);

    #[test]
    fn a_timeout_is_a_timeout_in_either_stage() {
        for stage in [Stage::Fetch, Stage::Body] {
            assert_eq!(kind_of(stage, true, false), TransportKind::Timeout);
        }
    }

    #[test]
    fn a_timeout_outranks_a_request_the_browser_refused() {
        assert_eq!(kind_of(Stage::Fetch, true, true), TransportKind::Timeout);
    }

    #[test]
    fn a_failed_fetch_is_a_connect_failure() {
        // A network failure and a CORS refusal look alike to a page.
        assert_eq!(kind_of(Stage::Fetch, false, false), TransportKind::Connect);
    }

    #[test]
    fn a_failed_body_read_is_a_body_failure() {
        assert_eq!(kind_of(Stage::Body, false, false), TransportKind::Body);
    }

    #[test]
    fn a_request_the_browser_refused_to_build_is_a_request_failure() {
        for stage in [Stage::Fetch, Stage::Body] {
            assert_eq!(kind_of(stage, false, true), TransportKind::Request);
        }
    }

    #[test]
    fn a_body_failure_after_the_deadline_is_a_timeout() {
        // reqwest reports a body read cut short by its timer as a decode
        // error, not a timeout.
        let late = timed_out(false, TIMEOUT + Duration::from_millis(1), TIMEOUT);
        assert_eq!(kind_of(Stage::Body, late, false), TransportKind::Timeout);
        let at = timed_out(false, TIMEOUT, TIMEOUT);
        assert_eq!(kind_of(Stage::Body, at, false), TransportKind::Timeout);
    }

    #[test]
    fn a_body_failure_before_the_deadline_is_a_body_failure() {
        let early = timed_out(false, TIMEOUT - Duration::from_millis(1), TIMEOUT);
        assert_eq!(kind_of(Stage::Body, early, false), TransportKind::Body);
    }

    #[test]
    fn reqwests_own_timeout_flag_is_a_timeout_whatever_the_clock_says() {
        assert!(timed_out(true, Duration::ZERO, TIMEOUT));
        assert!(timed_out(true, TIMEOUT * 2, TIMEOUT));
    }

    #[test]
    fn a_timeout_above_the_bound_is_held_to_it() {
        let bound = Duration::from_millis(i32::MAX as u64);
        assert_eq!(clamped(bound + Duration::from_millis(1)), bound);
        assert_eq!(clamped(Duration::MAX), bound);
        assert_eq!(clamped(bound), bound);
        assert_eq!(clamped(TIMEOUT), TIMEOUT);
        assert_eq!(clamped(Duration::ZERO), Duration::ZERO);
    }

    #[test]
    fn a_failed_fetch_is_retried_and_a_refused_request_is_not() {
        let lost = TransportError::new(kind_of(Stage::Fetch, false, false), "network error");
        assert!(lost.is_retriable());
        let refused = TransportError::new(kind_of(Stage::Fetch, false, true), "bad header value");
        assert!(!refused.is_retriable());
    }
}
