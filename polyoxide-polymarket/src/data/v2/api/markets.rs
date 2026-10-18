//! `markets` routes: holders, live volume, open interest, price history,
//! resolutions and token lookups.

use polyoxide_core::{csv, QueryBuilder, Request};

use crate::data::{
    v2::{
        envelope::{Envelope, Paged},
        types::{
            LiveVolume, MetaHolder, OpenInterest, PricePoint, PricesInterval, Resolution,
            ResolutionKey, TokenKey, TokenRef,
        },
        DataV2,
    },
    DataError,
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

    /// `GET /v2/tokens`: resolve token ids, or the tokens of conditions, to
    /// their condition, module, outcome, market, event and neg-risk question
    /// index in one call, by exactly one selector family.
    pub fn tokens(&self, key: TokenKey) -> GetTokens {
        let request = Request::new(self.http_client.clone(), "/v2/tokens");
        let request = match key {
            TokenKey::TokenIds(ids) => request.query_opt("token_id", csv(ids)),
            TokenKey::Conditions(ids) => request.query_opt("condition", csv(ids)),
        };
        GetTokens { request }
    }
}

/// Builder for `GET /v2/holders`.
pub struct ListHolders {
    inner: Paged<MetaHolder>,
}

impl ListHolders {
    polyoxide_core::query_setters! { self.inner;
        /// Minimum net balance in shares. Upstream default: `0`.
        min_balance: f64 => "min_balance",
        /// Add per-holder entry cost and PnL, and switch to per-side gross
        /// balances. Upstream then requires exactly one condition and a `limit`
        /// of at most 100.
        include_pnl: bool => "include_pnl",
        /// Rows per outcome token (at most 1000, or 100 with `include_pnl`).
        limit: u32 => "limit",
    }

    paged_builder_methods!(MetaHolder);
}

/// Builder for `GET /v2/live-volume`.
pub struct GetLiveVolume {
    request: Request<Envelope<LiveVolume>, DataError>,
}

impl GetLiveVolume {
    /// Fetch the volume.
    pub async fn send(self) -> Result<LiveVolume, DataError> {
        Ok(self.request.send().await?.data)
    }
}

/// Builder for `GET /v2/oi`.
pub struct GetOpenInterest {
    request: Request<Envelope<Vec<OpenInterest>>, DataError>,
}

impl GetOpenInterest {
    polyoxide_core::query_setters! {
        /// Only these markets (at most 20). An id missing from the result did not
        /// resolve to a servable market. An empty list is omitted.
        conditions: csv<I, S> => "condition",
    }

    /// Fetch the rows.
    pub async fn send(self) -> Result<Vec<OpenInterest>, DataError> {
        Ok(self.request.send().await?.data)
    }
}

/// Builder for `GET /v2/prices-history`.
pub struct ListPricesHistory {
    inner: Paged<PricePoint>,
}

impl ListPricesHistory {
    polyoxide_core::query_setters! { self.inner;
        /// Window start, epoch seconds, inclusive. Alone it means "up to now", capped
        /// at 15 days back. Pass [`end`](Self::end) too when walking pages.
        start: i64 => "start",
        /// Window end, epoch seconds, exclusive. Requires [`start`](Self::start).
        end: i64 => "end",
        /// Relative window, instead of `start`/`end`.
        interval: PricesInterval => "interval",
        /// Bucket width in seconds (60 to 86400). Omit it to let the server size
        /// the width to the window.
        bucket_seconds: u32 => "bucket_seconds",
        /// Point-in-time read: the latest observation at or before this epoch
        /// second. Cannot be combined with a window.
        as_of: i64 => "as_of",
        /// First-page size (at most 10,000, which is also the upstream default).
        limit: u32 => "limit",
    }

    paged_builder_methods!(PricePoint);
}

/// Builder for `GET /v2/resolutions`.
pub struct GetResolutions {
    request: Request<Envelope<Vec<Resolution>>, DataError>,
}

impl GetResolutions {
    /// Fetch the rows; a miss is an empty list.
    pub async fn send(self) -> Result<Vec<Resolution>, DataError> {
        Ok(self.request.send().await?.data)
    }
}

/// Builder for `GET /v2/tokens`.
pub struct GetTokens {
    request: Request<Envelope<Vec<TokenRef>>, DataError>,
}

impl GetTokens {
    /// Fetch the rows. Not paginated. By token id, the rows come in request
    /// order and an unknown id is simply absent, never a `404`; by condition,
    /// they are grouped by selector in request order, then by question index,
    /// market, module and outcome slot, and an unknown value contributes
    /// nothing. An empty selector list sends no parameter, which upstream
    /// refuses with a `400`.
    pub async fn send(self) -> Result<Vec<TokenRef>, DataError> {
        Ok(self.request.send().await?.data)
    }
}
