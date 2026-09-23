//! `markets` routes: holders, live volume, open interest, price history and
//! resolutions.

use polyoxide_core::{QueryBuilder, Request};

use crate::{
    v2::{
        envelope::{csv, Envelope, Paged},
        types::{
            LiveVolume, MetaHolder, OpenInterest, PricePoint, PricesInterval, Resolution,
            ResolutionKey,
        },
        DataV2,
    },
    DataApiError,
};

impl DataV2 {
    /// `GET /v2/holders`: top holders per outcome token, for these markets
    /// (condition ids, at most 20).
    pub fn holders<I, S>(&self, conditions: I) -> ListHolders
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        let mut inner = Paged::new(Request::new(self.http_client.clone(), "/v2/holders"));
        if let Some(value) = csv(conditions) {
            inner = inner.query("condition", value);
        }
        ListHolders { inner }
    }

    /// `GET /v2/live-volume`: taker volume per market under these Gamma events
    /// (at most 20 distinct ids; more is a `400`).
    pub fn live_volume<I, S>(&self, event_ids: I) -> GetLiveVolume
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        let mut request = Request::new(self.http_client.clone(), "/v2/live-volume");
        if let Some(value) = csv(event_ids) {
            request = request.query("event_id", value);
        }
        GetLiveVolume { request }
    }

    /// `GET /v2/oi`: open interest. With no
    /// [`conditions`](GetOpenInterest::conditions) this is the single global
    /// figure (`condition_id` `"GLOBAL"`).
    pub fn open_interest(&self) -> GetOpenInterest {
        GetOpenInterest {
            request: Request::new(self.http_client.clone(), "/v2/oi"),
        }
    }

    /// `GET /v2/prices-history`: an outcome token's price series.
    pub fn prices_history(&self, token_id: impl Into<String>) -> ListPricesHistory {
        ListPricesHistory {
            inner: Paged::new(Request::new(self.http_client.clone(), "/v2/prices-history"))
                .query("token_id", token_id.into()),
        }
    }

    /// `GET /v2/resolutions`: resolution state, by exactly one selector family.
    pub fn resolutions(&self, key: ResolutionKey) -> GetResolutions {
        let request = Request::new(self.http_client.clone(), "/v2/resolutions");
        let request = match key {
            ResolutionKey::Question(id) => request.query("question_id", id),
            ResolutionKey::Conditions(ids) => request.query_opt("condition", csv(ids)),
            ResolutionKey::Events(ids) => request.query_opt("event_id", csv(ids)),
        };
        GetResolutions { request }
    }
}

/// Builder for `GET /v2/holders`.
pub struct ListHolders {
    inner: Paged<MetaHolder>,
}

impl ListHolders {
    /// Minimum net balance in shares. Upstream default: `0`.
    pub fn min_balance(mut self, min_balance: f64) -> Self {
        self.inner = self.inner.query("min_balance", min_balance);
        self
    }

    /// Add per-holder entry cost and PnL, and switch to per-side gross
    /// balances. Upstream then requires exactly one condition and a `limit`
    /// of at most 100.
    pub fn include_pnl(mut self, include: bool) -> Self {
        self.inner = self.inner.query("include_pnl", include);
        self
    }

    /// Rows per outcome token (at most 1000, or 100 with `include_pnl`).
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.query("limit", limit);
        self
    }

    paged_builder_methods!(MetaHolder);
}

/// Builder for `GET /v2/live-volume`.
pub struct GetLiveVolume {
    request: Request<Envelope<LiveVolume>, DataApiError>,
}

impl GetLiveVolume {
    /// Fetch the volume.
    pub async fn send(self) -> Result<LiveVolume, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}

/// Builder for `GET /v2/oi`.
pub struct GetOpenInterest {
    request: Request<Envelope<Vec<OpenInterest>>, DataApiError>,
}

impl GetOpenInterest {
    /// Only these markets (at most 20). An id missing from the result did not
    /// resolve to a servable market. An empty list is omitted.
    pub fn conditions<I, S>(mut self, conditions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: ToString,
    {
        if let Some(value) = csv(conditions) {
            self.request = self.request.query("condition", value);
        }
        self
    }

    /// Fetch the rows.
    pub async fn send(self) -> Result<Vec<OpenInterest>, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}

/// Builder for `GET /v2/prices-history`.
pub struct ListPricesHistory {
    inner: Paged<PricePoint>,
}

impl ListPricesHistory {
    /// Window start, epoch seconds, inclusive. Alone it means "up to now", capped
    /// at 15 days back. Pass [`end`](Self::end) too when walking pages.
    pub fn start(mut self, start: i64) -> Self {
        self.inner = self.inner.query("start", start);
        self
    }

    /// Window end, epoch seconds, exclusive. Requires [`start`](Self::start).
    pub fn end(mut self, end: i64) -> Self {
        self.inner = self.inner.query("end", end);
        self
    }

    /// Relative window, instead of `start`/`end`.
    pub fn interval(mut self, interval: PricesInterval) -> Self {
        self.inner = self.inner.query("interval", interval);
        self
    }

    /// Bucket width in seconds (60 to 86400). Omit it to let the server size
    /// the width to the window.
    pub fn bucket_seconds(mut self, seconds: u32) -> Self {
        self.inner = self.inner.query("bucket_seconds", seconds);
        self
    }

    /// Point-in-time read: the latest observation at or before this epoch
    /// second. Cannot be combined with a window.
    pub fn as_of(mut self, as_of: i64) -> Self {
        self.inner = self.inner.query("as_of", as_of);
        self
    }

    /// First-page size (at most 10,000, which is also the upstream default).
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.query("limit", limit);
        self
    }

    paged_builder_methods!(PricePoint);
}

/// Builder for `GET /v2/resolutions`.
pub struct GetResolutions {
    request: Request<Envelope<Vec<Resolution>>, DataApiError>,
}

impl GetResolutions {
    /// Fetch the rows; a miss is an empty list.
    pub async fn send(self) -> Result<Vec<Resolution>, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}
