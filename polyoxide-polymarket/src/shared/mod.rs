//! What more than one Polymarket module uses: the hooks they give core's send
//! loop, the HMAC signer and the session-signer types.
//!
//! - [`PolymarketRetryPolicy`], the one retry policy, which every HTTP module
//!   installs on [`HttpClient::send`](polyoxide_core::HttpClient::send).
//! - The five window-quota tables, one per API surface: [`clob_limits`],
//!   [`gamma_limits`], [`data_limits`], [`relay_limits`] and [`perps_limits`],
//!   each behind its module's feature, and each with an `_on` twin
//!   ([`clob_limits_on`] and so on) that keeps time on a given runtime.
//! - [`ClobThrottle`] (`clob`), which composes the CLOB's IP table and the
//!   per-signer layer ([`signer_limit`]) over one hold, with [`signer_cost`]
//!   and the [`LayerId`](polyoxide_core::LayerId)s [`CLOUDFLARE`],
//!   [`SIGNER_ORDER`] and [`SIGNER_CANCEL`].
//! - The HMAC [`Signer`] ([`auth`]) and the session-signer types
//!   ([`session_signer`]), which clob and relay share (`clob` or `relay`).
//! - [`keys`], the constructors of Polymarket's market keys: the clob's
//!   (`clob`, `gamma` or `data`) and perps' (`perps`).
//! - `protocol` (`clob` or `relay`; crate-private), the one rule that tells a
//!   Protocol V2 position id from a CTF token id, which the CLOB's order signing
//!   and balance reads, the relay and Gamma's `event_group` query all apply.
//!
//! Each item is built by the features of the modules that use it.

mod limits;
mod policy;
#[cfg(feature = "clob")]
mod throttle;

#[cfg(any(feature = "clob", feature = "relay"))]
pub mod auth;
#[cfg(any(
    feature = "clob",
    feature = "gamma",
    feature = "data",
    feature = "perps"
))]
pub mod keys;
#[cfg(any(feature = "clob", feature = "relay"))]
pub(crate) mod protocol;
#[cfg(any(feature = "clob", feature = "relay"))]
pub mod session_signer;
#[cfg(feature = "clob")]
pub mod signer_limit;

#[cfg(feature = "clob")]
pub use limits::{clob_limits, clob_limits_on};
#[cfg(feature = "data")]
pub use limits::{data_limits, data_limits_on};
#[cfg(feature = "gamma")]
pub use limits::{gamma_limits, gamma_limits_on};
#[cfg(feature = "perps")]
pub use limits::{perps_limits, perps_limits_on};
#[cfg(feature = "relay")]
pub use limits::{relay_limits, relay_limits_on};
pub use policy::PolymarketRetryPolicy;
#[cfg(feature = "clob")]
pub use throttle::{
    clob_throttle, clob_throttle_on, signer_cost, ClobThrottle, CLOUDFLARE, SIGNER_CANCEL,
    SIGNER_ORDER,
};

#[cfg(any(feature = "clob", feature = "relay"))]
pub use auth::{current_timestamp, Base64Format, Signer};
#[cfg(any(feature = "clob", feature = "relay"))]
pub use session_signer::{DepositWalletRole, SessionSignerScope};
#[cfg(feature = "clob")]
pub use signer_limit::{
    BurstCapacityExceeded, RateLimitStatus, SignerLimiter, Tier, TradingBucket, TradingRequest,
};
