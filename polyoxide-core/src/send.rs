//! [`HttpClient::send`]: core's one send loop.

use reqwest::Response;
use serde::de::DeserializeOwned;
use url::Url;

use crate::client::HttpClient;
use crate::error::{ApiError, ErrorResponse};
use crate::hooks::{
    Authenticator, Cost, DynAuthenticator, Outcome, RequestMeta, RequestParts, ResponseMeta,
    RetryPolicy, Throttle,
};

impl HttpClient {
    /// Send `parts`, retrying as the client's policy decides.
    ///
    /// Each attempt takes a concurrency permit, waits while the throttle
    /// charges it ([`Throttle::acquire`], with `costs`), is signed by `auth`
    /// when one is given, and is sent, within `parts.timeout` when it is set.
    /// The throttle then sees the response
    /// ([`Throttle::observe`]), the policy decides what follows
    /// ([`RetryPolicy::decide`]), and a hold the decision carries is applied
    /// ([`Throttle::hold`]) whether or not the request is retried.
    ///
    /// A retry releases the permit and sleeps the longer of the policy's wait
    /// and the loop's own floor, [`RetryConfig::retry_delay`] for the attempt.
    /// The floor belongs to the loop, so no policy can retry sooner. The loop
    /// never retries past [`RetryConfig::max_retries`], whatever the policy
    /// says.
    ///
    /// Each retry logs a `WARN` under the `polyoxide_core` target,
    /// `Retriable status <code> on <path>, retry <n> after <ms>ms`, where
    /// `<ms>` is the longer of the retry's sleep and the hold its decision
    /// set, since the retry waits out both. A hold that is not a retry logs
    /// one too: `Status <code> on <path>, not retried: every request held
    /// <ms>ms`.
    ///
    /// Returns the response the policy is done with, whatever its status; one
    /// it fails, or would retry with no retry left, is an error.
    ///
    /// # Errors
    ///
    /// - [`ApiError::Response`] when the policy fails the last response.
    /// - [`ApiError::Url`] when `parts.path` does not join the base URL.
    /// - [`ApiError::Refused`] when the throttle refuses a cost, or the
    ///   signing error when `auth` fails. Nothing is sent.
    /// - [`ApiError::Network`] when no response arrives. That attempt is
    ///   neither observed nor retried.
    ///
    /// [`RetryConfig::retry_delay`]: crate::RetryConfig::retry_delay
    /// [`RetryConfig::max_retries`]: crate::RetryConfig::max_retries
    pub async fn send(
        &self,
        parts: RequestParts,
        costs: &[Cost],
        auth: Option<&DynAuthenticator<'_>>,
    ) -> Result<Response, ApiError> {
        let url = self.base_url.join(&parts.path)?;
        let mut attempt = 0u32;

        loop {
            let permit = self.acquire_concurrency().await;
            let request = RequestMeta {
                method: &parts.method,
                path: &parts.path,
                query: &parts.query,
                costs,
            };
            let charge = self.throttle.acquire(&request).await?;

            // Signed afresh on every attempt, so a timestamp in it is current.
            let mut signed = parts.clone();
            if let Some(auth) = auth {
                auth.sign(&mut signed, attempt).await?;
            }
            let response = self.request(&url, signed).send().await?;

            let status = response.status();
            let info = self.retry_config.attempt_info(attempt);
            let meta = ResponseMeta {
                status,
                headers: response.headers(),
            };
            self.throttle.observe(&charge, &meta, &info);
            let decision = self.policy.decide(&meta, &info, &self.retry_config);

            // Before the retry decision, and whatever it is: a hold is a fact
            // about the host, and a request with no retry left still has to
            // publish it to every request sharing this throttle.
            if let Some(hold) = decision.hold {
                self.throttle.hold(hold);
            }

            match decision.outcome {
                Outcome::Retry(wait) if info.retries_left > 0 => {
                    let floor = self.retry_config.retry_delay(attempt, meta.retry_after());
                    let sleep = floor.max(wait);
                    attempt += 1;
                    // The retry also waits out the hold this decision set, so
                    // the logged wait is the longer of the two.
                    tracing::warn!(
                        "Retriable status {} on {}, retry {} after {}ms",
                        status,
                        parts.path,
                        attempt,
                        decision
                            .hold
                            .map_or(sleep, |hold| hold.max(sleep))
                            .as_millis()
                    );
                    drop(permit);
                    tokio::time::sleep(sleep).await;
                }
                Outcome::Done | Outcome::Fail | Outcome::Retry(_) => {
                    // A hold outlives this request, so it is logged even when
                    // nothing is retried (DRIFT R10).
                    if let Some(hold) = decision.hold {
                        tracing::warn!(
                            "Status {} on {}, not retried: every request held {}ms",
                            status,
                            parts.path,
                            hold.as_millis()
                        );
                    }
                    // `Done` hands the response back whatever its status. A
                    // `Fail`, or a retry with none left, is the error AD-8
                    // names: the status, headers, body and `Retry-After`.
                    return match decision.outcome {
                        Outcome::Done => Ok(response),
                        Outcome::Fail | Outcome::Retry(_) => {
                            Err(ErrorResponse::read(response).await.into())
                        }
                    };
                }
            }
        }
    }

    /// One attempt's request, built from its signed parts.
    fn request(&self, url: &Url, parts: RequestParts) -> reqwest::RequestBuilder {
        let mut request = self
            .client
            .request(parts.method, url.clone())
            .headers(parts.headers);
        if !parts.query.is_empty() {
            request = request.query(&parts.query);
        }
        if let Some(body) = parts.body {
            request = request.body(body);
        }
        if let Some(timeout) = parts.timeout {
            request = request.timeout(timeout);
        }
        request
    }
}

/// Decode a response body as JSON, logging a failure once.
///
/// A failure logs one ERROR line under `polyoxide_core`,
/// `Failed to decode {path}: {err}: {body}`, with the body truncated for the
/// log, and returns the error for the caller to map into its own type.
pub fn decode_json<T: DeserializeOwned>(path: &str, text: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(text).map_err(|err| {
        tracing::error!(
            "Failed to decode {path}: {err}: {}",
            crate::truncate_for_log(text)
        );
        err
    })
}
