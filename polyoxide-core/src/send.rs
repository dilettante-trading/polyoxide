//! [`HttpClient::send`]: core's one send loop.

use reqwest::Response;
use url::Url;

use crate::client::HttpClient;
use crate::error::ApiError;
use crate::hooks::{
    AttemptInfo, Authenticator, Cost, DynAuthenticator, Outcome, RequestMeta, RequestParts,
    ResponseMeta, RetryPolicy, Throttle,
};

impl HttpClient {
    /// Send `parts`, retrying as the client's policy decides.
    ///
    /// Each attempt takes a concurrency permit, waits while the throttle
    /// charges it ([`Throttle::acquire`], with `costs`), is signed by `auth`
    /// when one is given, and is sent. The throttle then sees the response
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
    /// Returns the last response, whatever its status, for the caller to
    /// decode.
    ///
    /// # Errors
    ///
    /// - [`ApiError::Url`] when `parts.path` does not join the base URL.
    /// - [`ApiError::Refused`] when the throttle refuses a cost. Nothing is
    ///   sent.
    /// - The signing error, when `auth` fails. Nothing is sent.
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
            let info = AttemptInfo {
                attempt,
                retries_left: self.retry_config.max_retries.saturating_sub(attempt),
            };
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
                    tracing::warn!(
                        "Retriable status {} on {}, retry {} after {}ms",
                        status,
                        parts.path,
                        attempt,
                        sleep.as_millis()
                    );
                    drop(permit);
                    tokio::time::sleep(sleep).await;
                }
                Outcome::Done | Outcome::Fail | Outcome::Retry(_) => return Ok(response),
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
        request
    }
}
