//! Liveness: a ping of the exchange-status route.

use std::sync::Arc;

use polyoxide_core::Pong;

use crate::predictions::{error::PredictionsError, request::routes, Inner};

/// Health namespace.
#[derive(Debug, Clone)]
pub struct Health {
    pub(crate) inner: Arc<Inner>,
}

impl Health {
    /// Pings the host with `GET /trade-api/v2/exchange/status`, which is public: a ping is never
    /// signed, whether or not the client holds a key.
    ///
    /// The ping runs on the client's send loop, so it waits for the Read bucket and is retried and
    /// held as every request is, and its round trip is that of the attempt that answered. Its body
    /// is not read: the route answers whether the host does, and
    /// [`exchange().status()`](crate::predictions::Predictions::exchange) says whether the exchange
    /// is trading. A non-2xx answer is the error Kalshi's decode makes of it.
    pub async fn ping(&self) -> Result<Pong, PredictionsError> {
        self.inner.before_send().await;
        let route = routes::EXCHANGE_STATUS;
        let cost = self.inner.read_cost(&route);
        self.inner
            .http
            .health::<PredictionsError>(&route.path(&[]), &[cost])
            .await
    }
}
