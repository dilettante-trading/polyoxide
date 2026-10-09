//! One health ping, on the send loop.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::{Method, Response};

use crate::client::HttpClient;
use crate::error::ApiError;
use crate::hooks::{Authenticator, Cost, DynAuthenticator, RequestParts};
use crate::request::RequestError;

/// A successful [`HttpClient::health`] ping.
#[derive(Debug)]
#[non_exhaustive]
pub struct Pong {
    /// The round trip of the attempt that answered, timed from just before it
    /// was sent. It leaves out the waits for a permit, the throttle and a
    /// retry's backoff, so a ping that was retried reports its last attempt.
    pub round_trip: Duration,
    /// The 2xx response, its body unread, for a venue that checks it.
    pub response: Response,
}

/// Notes when an attempt is signed, which is just before it is sent.
#[derive(Default)]
struct Stopwatch(Mutex<Option<Instant>>);

impl Stopwatch {
    /// Time since the last attempt was signed.
    fn elapsed(&self) -> Duration {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .map_or(Duration::ZERO, |sent| sent.elapsed())
    }
}

impl Authenticator for Stopwatch {
    async fn sign(&self, _parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Instant::now());
        Ok(())
    }
}

impl HttpClient {
    /// Ping `path` with one `GET` on [`send`](Self::send), charging `costs`.
    ///
    /// The ping takes a concurrency permit and the throttle, and is retried
    /// and held by the client's policy, as every other request is. Its
    /// [`Pong::round_trip`] is that of the attempt that answered, so every
    /// venue's ping latency means the same thing.
    ///
    /// # Errors
    ///
    /// A final non-2xx response is `E::from_response`, and a failure before
    /// one (a refused cost, a transport error) is the [`ApiError`] as `E`.
    pub async fn health<E: RequestError>(&self, path: &str, costs: &[Cost]) -> Result<Pong, E> {
        let stopwatch = Stopwatch::default();
        let response = self
            .send(
                RequestParts::new(Method::GET, path),
                costs,
                Some(DynAuthenticator::from_ref(&stopwatch)),
            )
            .await?;
        let round_trip = stopwatch.elapsed();

        if !response.status().is_success() {
            return Err(E::from_response(response).await);
        }

        Ok(Pong {
            round_trip,
            response,
        })
    }
}
