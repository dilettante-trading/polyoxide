//! `boards` routes: biggest winners, the builder boards and the trader
//! leaderboard.

use polyoxide_core::{QueryBuilder, Request};

use crate::{
    v2::{
        envelope::{Envelope, Paged},
        types::{
            BiggestWinner, BuilderStanding, BuilderVolumePoint, LeaderboardBoard, LeaderboardEntry,
            LeaderboardUserEntry, TimePeriod,
        },
        DataV2,
    },
    DataApiError,
};

impl DataV2 {
    /// `GET /v2/biggest-winners`: the largest resolved wins in a window.
    pub fn biggest_winners(&self) -> ListBiggestWinners {
        ListBiggestWinners {
            inner: Paged::new(Request::new(
                self.http_client.clone(),
                "/v2/biggest-winners",
            )),
        }
    }

    /// `GET /v2/builders/leaderboard`: builders ranked by volume.
    pub fn builders_leaderboard(&self) -> ListBuildersLeaderboard {
        ListBuildersLeaderboard {
            inner: Paged::new(Request::new(
                self.http_client.clone(),
                "/v2/builders/leaderboard",
            )),
        }
    }

    /// `GET /v2/builders/volume`: builder volume per time bucket.
    pub fn builder_volume(&self) -> GetBuilderVolume {
        GetBuilderVolume {
            request: Request::new(self.http_client.clone(), "/v2/builders/volume"),
        }
    }

    /// `GET /v2/leaderboard`: a page of the trader board.
    ///
    /// `volume` on these rows is in shares, not USDC, and the windows are
    /// `day`/`week`/`month`/`all`, so this board does not line up with
    /// [`DataApi::rankings`](crate::DataApi::rankings).
    pub fn leaderboard(&self) -> ListLeaderboard {
        ListLeaderboard {
            inner: Paged::new(Request::new(self.http_client.clone(), "/v2/leaderboard")),
        }
    }

    /// `GET /v2/leaderboard?user=`: one wallet's standing on both boards.
    ///
    /// Resolves to `None` for an unknown wallet. A `None` rank inside the entry
    /// means the wallet is unranked on that board.
    pub fn leaderboard_user(&self, user: impl Into<String>) -> GetLeaderboardUser {
        GetLeaderboardUser {
            request: Request::new(self.http_client.clone(), "/v2/leaderboard")
                .query("user", user.into()),
        }
    }
}

/// Builder for `GET /v2/biggest-winners`.
pub struct ListBiggestWinners {
    inner: Paged<BiggestWinner>,
}

impl ListBiggestWinners {
    /// Window on `resolved_at`. Upstream default: `day`.
    pub fn time_period(mut self, period: TimePeriod) -> Self {
        self.inner = self.inner.query("time_period", period);
        self
    }

    /// `overall` (the default), a Gamma category such as `sports`, `combos`,
    /// or `esports`.
    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.inner = self.inner.query("category", category.into());
        self
    }

    /// First-page size (at most 1000).
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.query("limit", limit);
        self
    }

    paged_builder_methods!(BiggestWinner);
}

/// Builder for `GET /v2/builders/leaderboard`.
pub struct ListBuildersLeaderboard {
    inner: Paged<BuilderStanding>,
}

impl ListBuildersLeaderboard {
    /// Window. Upstream default: `day`.
    pub fn time_period(mut self, period: TimePeriod) -> Self {
        self.inner = self.inner.query("time_period", period);
        self
    }

    /// First-page size (at most 1000).
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.query("limit", limit);
        self
    }

    paged_builder_methods!(BuilderStanding);
}

/// Builder for `GET /v2/builders/volume`.
pub struct GetBuilderVolume {
    request: Request<Envelope<Vec<BuilderVolumePoint>>, DataApiError>,
}

impl GetBuilderVolume {
    /// Bucket width. Upstream default: `day`.
    pub fn interval(mut self, interval: TimePeriod) -> Self {
        self.request = self.request.query("interval", interval);
        self
    }

    /// How many of the most recent buckets to return (at most 90; upstream
    /// default 30).
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// Fetch the buckets.
    pub async fn send(self) -> Result<Vec<BuilderVolumePoint>, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}

/// Builder for `GET /v2/leaderboard`.
pub struct ListLeaderboard {
    inner: Paged<LeaderboardEntry>,
}

impl ListLeaderboard {
    /// Window. Upstream default: `day`.
    pub fn time_period(mut self, period: TimePeriod) -> Self {
        self.inner = self.inner.query("time_period", period);
        self
    }

    /// `overall` (the default), a Gamma category such as `sports`, `combos`
    /// (PnL board only), or `esports`.
    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.inner = self.inner.query("category", category.into());
        self
    }

    /// Which board. Upstream default: `PNL`.
    pub fn board(mut self, board: LeaderboardBoard) -> Self {
        self.inner = self.inner.query("sort_by", board);
        self
    }

    /// First-page size (at most 1000).
    pub fn limit(mut self, limit: u32) -> Self {
        self.inner = self.inner.query("limit", limit);
        self
    }

    paged_builder_methods!(LeaderboardEntry);
}

/// Builder for `GET /v2/leaderboard?user=`.
pub struct GetLeaderboardUser {
    request: Request<Envelope<Option<LeaderboardUserEntry>>, DataApiError>,
}

impl GetLeaderboardUser {
    /// Window. Upstream default: `day`.
    pub fn time_period(mut self, period: TimePeriod) -> Self {
        self.request = self.request.query("time_period", period);
        self
    }

    /// `overall` (the default), a Gamma category, `combos` or `esports`.
    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.request = self.request.query("category", category.into());
        self
    }

    /// Fetch the standing; `None` for an unknown wallet.
    pub async fn send(self) -> Result<Option<LeaderboardUserEntry>, DataApiError> {
        Ok(self.request.send().await?.data)
    }
}
