//! Events: the sets of markets Kalshi groups.

use std::sync::Arc;

use crate::predictions::{
    request::{routes, Routed},
    types::{EventResponse, EventStatusFilter, EventsPage},
    Inner, PredictionsError,
};

/// Events namespace.
#[derive(Debug, Clone)]
pub struct EventsApi {
    pub(crate) inner: Arc<Inner>,
}

impl EventsApi {
    /// `GET /trade-api/v2/events`: one page of events, multivariate events excluded.
    ///
    /// Kalshi combines these filters and refuses none of them, unlike `GET /markets`
    /// (`docs/specs/kalshi/OBSERVED.md`). Nested markets leave out those settled before the
    /// historical cutoff, which `historical().markets` reads.
    pub fn list(&self) -> ListEvents {
        ListEvents {
            request: Routed::new(&self.inner, routes::EVENTS, &[]),
        }
    }

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

/// Request builder for `GET /trade-api/v2/events`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListEvents {
    pub(crate) request: Routed<EventsPage>,
}

impl ListEvents {
    polyoxide_core::query_setters! {
        /// Events per page, up to 200, the host's default.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
        /// Nest each event's markets in `event.markets`.
        with_nested_markets: bool => "with_nested_markets",
        /// Only events in this state.
        status: EventStatusFilter => "status",
        /// Only the events of this series.
        series_ticker: impl Into<String> => "series_ticker",
        /// Only events with a market that closes after this Unix time, in seconds.
        min_close_ts: i64 => "min_close_ts",
        /// Only events whose metadata changed after this Unix time, in seconds.
        min_updated_ts: i64 => "min_updated_ts",
    }

    /// Only these events, comma-joined. An empty set, or one past
    /// [`TICKERS_QUERY_BUDGET`](crate::predictions::TICKERS_QUERY_BUDGET), is refused when sent,
    /// with nothing sent. Setting it again replaces the set and any refusal.
    pub fn tickers<S: Into<String>>(mut self, tickers: impl IntoIterator<Item = S>) -> Self {
        let tickers: Vec<String> = tickers.into_iter().map(Into::into).collect();
        let texts: Vec<&str> = tickers.iter().map(String::as_str).collect();
        self.request = self.request.tickers_joined("tickers", &texts);
        self
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<EventsPage, PredictionsError> {
        self.request.send().await
    }
}
