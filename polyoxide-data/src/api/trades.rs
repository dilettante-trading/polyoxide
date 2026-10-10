use polyoxide_core::{HttpClient, Request};

use crate::{
    error::DataApiError,
    types::{Trade, TradeFilterType, TradeSide},
};

/// Trades namespace for trade-related operations
#[derive(Clone)]
pub struct Trades {
    pub(crate) http_client: HttpClient,
}

impl Trades {
    /// List trades with optional filtering
    pub fn list(&self) -> ListTrades {
        ListTrades {
            request: Request::new(self.http_client.clone(), "/trades"),
        }
    }
}

/// Request builder for listing trades
pub struct ListTrades {
    request: Request<Vec<Trade>, DataApiError>,
}

impl ListTrades {
    polyoxide_core::query_setters! {
        /// Filter by user address (0x-prefixed, 40 hex chars)
        user: impl Into<String> => "user",
        /// Filter by market condition IDs (comma-separated)
        /// Note: Mutually exclusive with `event_id`
        market: csv impl IntoIterator<Item = impl ToString> => "market",
        /// Filter by event IDs (comma-separated)
        /// Note: Mutually exclusive with `market`
        event_id: csv impl IntoIterator<Item = impl ToString> => "eventId",
        /// Filter by trade side (BUY or SELL)
        side: TradeSide => "side",
        /// Filter for taker trades only (default: true)
        taker_only: bool => "takerOnly",
        /// Set filter type (must be paired with `filter_amount`)
        filter_type: TradeFilterType => "filterType",
        /// Set filter amount (must be paired with `filter_type`)
        filter_amount: f64 => "filterAmount",
        /// Set maximum number of results (0-10000, default: 100)
        limit: u32 => "limit",
        /// Set pagination offset (0-10000, default: 0)
        ///
        /// Requests past the cap are rejected with a 400 rather than silently
        /// clamped. To read deeper than offset 10000, page inside successive
        /// [`start`](Self::start)/[`end`](Self::end) windows — each window has its
        /// own offset budget.
        offset: u32 => "offset",
        /// Lower-bound timestamp (epoch seconds) for the trade window.
        ///
        /// Omit or pass `0` for the default window (most recent ~3 years); pass a
        /// positive epoch (e.g. `1`) to retrieve full history on user-scoped
        /// requests. Market- and event-scoped requests keep the ~3-year floor, so
        /// `start` can only narrow their window.
        start: u64 => "start",
        /// Upper-bound timestamp (epoch seconds) for the trade window.
        ///
        /// Omit for the default (current time); rows newer than `end` are excluded.
        end: u64 => "end",
    }

    /// Execute the request
    pub async fn send(self) -> Result<Vec<Trade>, DataApiError> {
        self.request.send().await
    }
}
