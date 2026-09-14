//! `wallet` routes: approvals, positions, combo positions, PnL, stats, volume
//! and portfolio value.

use polyoxide_core::{QueryBuilder, Request};

use crate::{
    v2::{envelope::Envelope, types::UserStats, DataV2},
    DataApiError,
};

impl DataV2 {
    /// `GET /v2/user-stats`: one wallet's profile card.
    ///
    /// Resolves to `None` when the wallet is not a known user. A known user who
    /// has never traded is `Some` with zeros, which is a different answer.
    pub fn user_stats(&self, user: impl Into<String>) -> GetUserStats {
        GetUserStats {
            request: Request::new(self.http_client.clone(), "/v2/user-stats")
                .query("user", user.into()),
        }
    }
}

/// Builder for `GET /v2/user-stats`.
pub struct GetUserStats {
    request: Request<Envelope<Option<UserStats>>, DataApiError>,
}

impl GetUserStats {
    /// Fetch the stats; `None` for an unknown wallet.
    pub async fn send(self) -> Result<Option<UserStats>, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}
