//! Events: the sets of markets Kalshi groups.

use std::sync::Arc;

use crate::predictions::{
    request::{routes, Routed},
    types::EventResponse,
    Inner, PredictionsError,
};

/// Events namespace.
#[derive(Debug, Clone)]
pub struct EventsApi {
    pub(crate) inner: Arc<Inner>,
}

impl EventsApi {
    /// `GET /trade-api/v2/events/{event_ticker}`: an event the host does not list is a `404`.
    pub fn get(&self, event_ticker: &str) -> GetEvent {
        GetEvent {
            request: Routed::new(&self.inner, routes::EVENT, &[event_ticker]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/events/{event_ticker}`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetEvent {
    pub(crate) request: Routed<EventResponse>,
}

impl GetEvent {
    polyoxide_core::query_setters! {
        /// Nest the event's markets in `event.markets`, as many as the event has.
        with_nested_markets: bool => "with_nested_markets",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<EventResponse, PredictionsError> {
        self.request.send().await
    }
}
