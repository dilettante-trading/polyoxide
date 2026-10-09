//! The hooks a venue gives [`HttpClient::send`](crate::HttpClient::send).
//!
//! The loop is core's; a venue supplies three hooks and composes its state
//! inside them:
//!
//! - a [`Throttle`], which charges each attempt before it is sent, sees every
//!   response, and holds every request when the server says to;
//! - a [`RetryPolicy`], which decides from a response whether the request is
//!   done, retried or failed, and whether everyone is held;
//! - an [`Authenticator`], which signs each attempt afresh.
//!
//! Each attempt runs: concurrency permit, [`Throttle::acquire`],
//! [`Authenticator::sign`], send, [`Throttle::observe`], [`RetryPolicy::decide`],
//! then [`Throttle::hold`] when the decision carries a hold.

use std::future::Future;
use std::time::Duration;

use polyoxide_venue::{Class, Classify};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::{Method, StatusCode};

use crate::error::ApiError;
use crate::rate_limit::RetryConfig;

/// A throttle layer, named by the venue that declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(pub &'static str);

impl std::fmt::Display for LayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// What a request costs one layer of a throttle.
///
/// The venue's request builder computes these from its route table. A layer
/// that counts requests finds its own bucket from the method and path, and
/// needs no entry here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    /// The layer charged.
    pub layer: LayerId,
    /// Tokens the request costs that layer.
    pub units: u32,
    /// Whether `units` is the true cost. Only an exact cost may be refused
    /// before sending; an inexact one is a floor, and the server decides.
    pub exact: bool,
}

/// What one layer charged for one attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerCharge {
    /// The layer charged.
    pub layer: LayerId,
    /// Tokens taken.
    pub units: u32,
    /// The window the tokens were taken from, for a layer that counts by
    /// window (Binance's UTC minute).
    pub window: Option<u64>,
}

/// Everything a throttle charged for one attempt, handed back to it in
/// [`Throttle::observe`]. The loop never reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Charge {
    layers: Vec<LayerCharge>,
}

impl Charge {
    /// A charge of nothing.
    pub fn none() -> Self {
        Self::default()
    }

    /// This charge, plus one layer's.
    pub fn with(mut self, charge: LayerCharge) -> Self {
        self.layers.push(charge);
        self
    }

    /// Each layer's charge, in the order they were taken.
    pub fn layers(&self) -> &[LayerCharge] {
        &self.layers
    }
}

/// A cost a layer can never hold, refused before anything is sent.
///
/// Not a throttle: no wait can satisfy it, so the request is never retried.
/// Splitting it is the only remedy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the {layer} layer holds at most {capacity} units, and this request costs {units}")]
pub struct Refused {
    /// The layer that refused.
    pub layer: LayerId,
    /// What the request would cost it.
    pub units: u32,
    /// The most the layer ever holds.
    pub capacity: u32,
}

/// [`Class::InvalidRequest`]: the client refused a cost no layer can ever
/// hold, and nothing was sent.
impl Classify for Refused {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

/// The request an attempt is about to send, as a throttle sees it.
#[derive(Debug, Clone, Copy)]
pub struct RequestMeta<'a> {
    /// The HTTP method.
    pub method: &'a Method,
    /// The path, without the query.
    pub path: &'a str,
    /// The query parameters.
    pub query: &'a [(String, String)],
    /// What the request costs each layer that does not count requests.
    pub costs: &'a [Cost],
}

/// A response, as a throttle and a policy see it.
#[derive(Debug, Clone, Copy)]
pub struct ResponseMeta<'a> {
    /// The status.
    pub status: StatusCode,
    /// The headers.
    pub headers: &'a HeaderMap,
}

impl ResponseMeta<'_> {
    /// The `Retry-After` header, when present and valid UTF-8.
    pub fn retry_after(&self) -> Option<&str> {
        self.headers.get(RETRY_AFTER)?.to_str().ok()
    }
}

/// Which attempt a response answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttemptInfo {
    /// The attempt, from 0.
    pub attempt: u32,
    /// Retries the loop still allows after this attempt.
    pub retries_left: u32,
}

/// A request before it is sent. The loop clones it for each attempt and
/// hands the clone to [`Authenticator::sign`].
#[derive(Debug, Clone)]
pub struct RequestParts {
    /// The HTTP method.
    pub method: Method,
    /// The path, resolved against the client's base URL.
    pub path: String,
    /// The query parameters.
    pub query: Vec<(String, String)>,
    /// The headers.
    pub headers: HeaderMap,
    /// The body, sent as given.
    pub body: Option<String>,
}

impl RequestParts {
    /// A request with no query, headers or body.
    pub fn new(method: Method, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            headers: HeaderMap::new(),
            body: None,
        }
    }
}

/// What the loop does with a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Return the response.
    Done,
    /// Send again, after at least this long. The loop never sleeps less than
    /// its own backoff floor, and never retries past its `max_retries`.
    Retry(Duration),
    /// Return the response, which the caller decodes as an error.
    Fail,
}

/// A policy's answer to one response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// What the loop does next.
    pub outcome: Outcome,
    /// Hold every request on the throttle for this long, whatever the outcome.
    pub hold: Option<Duration>,
}

/// Charges each attempt, sees each response, and holds every request on it.
///
/// One throttle is shared by every client that shares a budget, and its hold
/// with it.
#[dynosaur::dynosaur(pub DynThrottle = dyn(box) Throttle)]
pub trait Throttle: Send + Sync {
    /// Wait until the request may be sent, and charge it.
    ///
    /// Waits out any hold before charging, and again if the hold moved while
    /// this call waited on a bucket of its own.
    ///
    /// # Errors
    ///
    /// [`Refused`] when a cost exceeds what a layer can ever hold. Nothing is
    /// sent, and the request is not retried.
    fn acquire(
        &self,
        meta: &RequestMeta<'_>,
    ) -> impl Future<Output = Result<Charge, Refused>> + Send;

    /// See a response to an attempt `acquire` charged. Runs on every
    /// response, before the policy decides. Records counts and tiers only.
    fn observe(&self, charge: &Charge, response: &ResponseMeta<'_>, attempt: &AttemptInfo);

    /// Hold every request on this throttle for `delay`. A hold only ever
    /// extends.
    fn hold(&self, delay: Duration);
}

/// Decides from a response what the loop does next.
#[dynosaur::dynosaur(pub DynRetryPolicy = dyn(box) RetryPolicy)]
pub trait RetryPolicy: Send + Sync {
    /// Decide what follows `response`. `schedule` is the loop's own
    /// [`RetryConfig`], for a policy that sizes a hold from it.
    fn decide(
        &self,
        response: &ResponseMeta<'_>,
        attempt: &AttemptInfo,
        schedule: &RetryConfig,
    ) -> Decision;
}

/// Signs an attempt before it is sent.
#[dynosaur::dynosaur(pub DynAuthenticator = dyn(box) Authenticator)]
pub trait Authenticator: Send + Sync {
    /// Sign `parts`, adding headers or query parameters. Runs on every
    /// attempt, so a timestamp in the signature is fresh each time.
    ///
    /// # Errors
    ///
    /// The [`ApiError`] the signing failed with. The request is not sent.
    fn sign(
        &self,
        parts: &mut RequestParts,
        attempt: u32,
    ) -> impl Future<Output = Result<(), ApiError>> + Send;
}

/// A throttle that charges nothing and holds nothing: a client built without
/// one.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoThrottle;

impl Throttle for NoThrottle {
    async fn acquire(&self, _meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        Ok(Charge::none())
    }

    fn observe(&self, _charge: &Charge, _response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {}

    fn hold(&self, _delay: Duration) {}
}

/// Core's policy: a `429` is retried and holds every request, a 2xx is done,
/// and every other status fails.
///
/// The hold is the schedule's first delay, [`RetryConfig::retry_delay`] at
/// attempt 0, whatever attempt saw the `429`; the attempt's own wait is the
/// loop's floor. A `429` with no retry left still holds: a 429 is a fact
/// about the host, and the request out of attempts still has to publish it.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultRetryPolicy;

impl RetryPolicy for DefaultRetryPolicy {
    fn decide(
        &self,
        response: &ResponseMeta<'_>,
        _attempt: &AttemptInfo,
        schedule: &RetryConfig,
    ) -> Decision {
        if response.status == StatusCode::TOO_MANY_REQUESTS {
            Decision {
                outcome: Outcome::Retry(Duration::ZERO),
                hold: Some(schedule.retry_delay(0, response.retry_after())),
            }
        } else if response.status.is_success() {
            Decision {
                outcome: Outcome::Done,
                hold: None,
            }
        } else {
            Decision {
                outcome: Outcome::Fail,
                hold: None,
            }
        }
    }
}
