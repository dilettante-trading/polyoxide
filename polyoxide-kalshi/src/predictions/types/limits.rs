//! The account's limits: `GET /trade-api/v2/account/limits`.

use serde::{Deserialize, Serialize};

/// An account's token buckets and the grants behind its tier.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountLimits {
    /// The account's usage tier.
    pub usage_tier: String,
    /// The Read bucket, which every read request is charged against.
    pub read: BucketLimit,
    /// The Write bucket. Recorded, and not built: it comes with Kalshi's `Trading`.
    pub write: BucketLimit,
    /// The grants the tier comes from.
    #[serde(default)]
    pub grants: Vec<ApiUsageGrant>,
}

/// One token bucket, as whole tokens.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BucketLimit {
    /// Tokens added a second.
    pub refill_rate: u32,
    /// The most tokens the bucket holds.
    pub bucket_capacity: u32,
}

/// A grant of API usage to the account.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ApiUsageGrant {
    /// The exchange instance the grant applies to.
    pub exchange_instance: String,
    /// The level the grant gives.
    pub level: String,
    /// When the grant expires, in seconds since the epoch, if it does.
    pub expires_ts: Option<i64>,
    /// What gave the grant.
    pub source: String,
}
