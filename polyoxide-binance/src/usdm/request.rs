//! The send loop every route goes through.

use std::marker::PhantomData;

use polyoxide_core::{retry_after_header, truncate_for_log, ApiError, HttpClient};
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::{
    error::{retry_after_secs, BinanceError},
    weight::{Cost, Route, WeightBudget, DEFAULT_BAN},
};

/// The header carrying the IP's weight used in the current minute.
pub const USED_WEIGHT_HEADER: &str = "x-mbx-used-weight-1m";

/// A REST request that knows its weight. Call [`send`](Self::send).
#[must_use = "a request does nothing until it is sent"]
pub struct WeightedRequest<T> {
    http: HttpClient,
    budget: WeightBudget,
    route: Route,
    query: Vec<(&'static str, String)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> WeightedRequest<T> {
    pub(crate) fn new(http: &HttpClient, budget: &WeightBudget, route: Route) -> Self {
        Self {
            http: http.clone(),
            budget: budget.clone(),
            route,
            query: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Sets a query parameter, replacing an earlier value for the same key.
    pub(crate) fn query(mut self, key: &'static str, value: impl ToString) -> Self {
        let value = value.to_string();
        match self.query.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.query.push((key, value)),
        }
        self
    }

    /// Replaces the route, for a parameter that changes the weight.
    pub(crate) fn route(mut self, route: Route) -> Self {
        self.route = route;
        self
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.route.cost()
    }
}

impl<T: DeserializeOwned> WeightedRequest<T> {
    /// Waits for the budget, sends, and decodes the response.
    ///
    /// A `429` starts a cooldown every request on the budget waits out, then
    /// this request is retried on core's schedule; with no retry left and no
    /// `Retry-After`, the cooldown runs to the next UTC minute. A `418` starts a
    /// cooldown for its `Retry-After`, or 2 minutes, and is not retried.
    ///
    /// The wait has no deadline of its own: a request can wait out a minute's
    /// spent budget or a ban, which Binance documents at up to 3 days. Wrap the
    /// call in `tokio::time::timeout` where a caller needs a deadline.
    pub async fn send(self) -> Result<T, BinanceError> {
        let text = self.send_text().await?;
        polyoxide_core::decode_json(self.route.path(), &text)
            .map_err(|err| BinanceError::from(ApiError::from(err)))
    }

    async fn send_text(&self) -> Result<String, BinanceError> {
        let path = self.route.path();
        let url = self.http.base_url.join(path)?;
        let cost = self.route.cost();
        let mut attempt = 0u32;

        loop {
            // The permit first, then the budget: the charge then stays next to
            // its send, which keeps the weight in flight under the budget's
            // reserve. Charging first would let charged requests queue for a
            // permit across a minute boundary, where their headers are dropped.
            let permit = self.http.acquire_concurrency().await;
            let charge = self.budget.acquire(cost).await;

            let mut request = self.http.client.get(url.clone());
            if !self.query.is_empty() {
                request = request.query(&self.query);
            }
            let response = request.send().await?;
            let status = response.status();
            let retry_after = retry_after_header(&response);

            // Before anything else, and whatever the status: a refused request
            // still spent weight, and the server's count is the truth.
            if let Some(used) = used_weight(&response) {
                self.budget.record_used(charge, used);
            }

            if status == StatusCode::IM_A_TEAPOT {
                let ban = retry_after_secs(retry_after.as_deref()).unwrap_or(DEFAULT_BAN);
                self.budget.begin_cooldown(ban);
                // The body is Binance's -1003 text, which names when the ban
                // ends; the error's fields have no room for it, so it is logged.
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(
                    "418 on {path}: IP banned, every request held {ban:?}: {}",
                    truncate_for_log(&body)
                );
                return Err(BinanceError::from_response_parts(
                    status.as_u16(),
                    retry_after.as_deref(),
                    &body,
                ));
            }
            if status == StatusCode::TOO_MANY_REQUESTS {
                // Retry-After only ever extends the wait, as in core. With no
                // retry left and no Retry-After, every request still holds until
                // the weight window resets: sending again into a spent minute is
                // how a 429 becomes a 418 ban.
                let retry = self
                    .http
                    .should_retry(status, attempt, retry_after.as_deref());
                let asked = retry_after_secs(retry_after.as_deref());
                let cooldown = match (retry, asked) {
                    (None, None) => self.budget.hold_until_next_minute(),
                    _ => {
                        let cooldown = retry.unwrap_or_default().max(asked.unwrap_or_default());
                        self.budget.begin_cooldown(cooldown);
                        cooldown
                    }
                };
                if retry.is_some() {
                    attempt += 1;
                    tracing::warn!("429 on {path}, retry {attempt} after {cooldown:?}");
                    drop(permit);
                    continue;
                }
                tracing::warn!("429 on {path}, no retry left: every request held {cooldown:?}");
            }

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(BinanceError::from_response_parts(
                    status.as_u16(),
                    retry_after.as_deref(),
                    &body,
                ));
            }
            return Ok(response.text().await?);
        }
    }
}

fn used_weight(response: &Response) -> Option<u32> {
    response
        .headers()
        .get(USED_WEIGHT_HEADER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}
