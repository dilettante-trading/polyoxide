//! # polyoxide
//!
//! The multi-venue umbrella: each venue crate under its venue's name, behind
//! features named for the venue and its modules.
//!
//! - `polyoxide::venue` is `polyoxide-venue`, the vocabulary every venue
//!   shares, such as the error classes and the `Classify` trait. It is always
//!   on.
//! - `polyoxide::polymarket` is `polyoxide-polymarket`, under `polymarket`
//!   (its defaults: `clob`, `gamma` and `data`), on by default, or one
//!   `polymarket-<module>` feature per module: `polymarket-clob`,
//!   `polymarket-clob-ws`, `polymarket-gamma`, `polymarket-data`,
//!   `polymarket-relay`, `polymarket-perps`, `polymarket-perps-ws`,
//!   `polymarket-rtds` and `polymarket-sports`. With `clob`, `gamma` and
//!   `data` all on it holds the unified client, `polymarket::Polymarket`.
//! - `polyoxide::binance` is `polyoxide-binance`, under `binance` (its
//!   defaults: `usdm`), `binance-usdm` or `binance-usdm-ws`.
//! - `polyoxide::parse_key` reads a market key, `venue.product:id`, through
//!   the constructors of whichever enabled product it names.
//! - `polyoxide::prelude` re-exports a selection of the enabled modules'
//!   clients, errors and stream types.
//!
//! A `-ws` feature turns its module's own on. `full` turns on every module of
//! every venue, and `keychain`, which stores credentials in the OS keychain.
//!
//! ## Example
//!
//! ```no_run
//! use polyoxide::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load account from environment variables
//!     let account = Account::from_env()?;
//!
//!     // Create unified client
//!     let polymarket = Polymarket::builder(account)
//!         .chain(Chain::PolygonMainnet)
//!         .build()?;
//!
//!     // Use Gamma API to list markets
//!     let markets = polymarket.gamma.markets()
//!         .list()
//!         .open(true)
//!         .limit(10)
//!         .send()
//!         .await?;
//!
//!     // Use Data API to get user positions
//!     let positions = polymarket.data
//!         .user("0x1234567890123456789012345678901234567890")
//!         .list_positions()
//!         .limit(10)
//!         .send()
//!         .await?;
//!
//!     for position in &positions {
//!         println!("Position: {} - size: {}", position.title, position.size);
//!     }
//!
//!     // Use CLOB API to place an order
//!     if let Some(first_market) = markets.first() {
//!         if let Some(token) = first_market.tokens.first() {
//!             let order_params = CreateOrderParams {
//!                 token_id: token.token_id.clone(),
//!                 price: 0.52,
//!                 size: 100.0,
//!                 side: OrderSide::Buy,
//!                 expiration: None,
//!                 order_type: OrderKind::Gtc,
//!                 post_only: false,
//!                 funder: None,
//!                 signature_type: Some(SignatureType::PolyProxy),
//!             };
//!
//!             let response = polymarket.clob.place_order(&order_params, None).await?;
//!             println!("Order placed: {:?}", response.order_id);
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
/// Compiles the crate `README.md` code examples as doctests.
struct ReadmeDoctests;

#[cfg(doctest)]
#[doc = include_str!("../../README.md")]
/// Compiles the workspace root `README.md` code examples as doctests.
struct RootReadmeDoctests;

/// The vocabulary every venue shares, `polyoxide-venue`: always on.
pub use polyoxide_venue as venue;

/// Polymarket's venue crate, `polyoxide-polymarket`: one module per
/// `polymarket-<module>` feature.
#[cfg(any(
    feature = "polymarket",
    feature = "polymarket-clob",
    feature = "polymarket-clob-ws",
    feature = "polymarket-gamma",
    feature = "polymarket-data",
    feature = "polymarket-relay",
    feature = "polymarket-perps",
    feature = "polymarket-perps-ws",
    feature = "polymarket-rtds",
    feature = "polymarket-sports"
))]
pub use polyoxide_polymarket as polymarket;

/// Binance's venue crate, `polyoxide-binance`: one module per
/// `binance-<module>` feature.
#[cfg(any(
    feature = "binance",
    feature = "binance-usdm",
    feature = "binance-usdm-ws"
))]
pub use polyoxide_binance as binance;

/// The key `raw` names, made by the constructors of its product, among the
/// venues and modules this build enables: Polymarket's `clob` (with
/// `polymarket-clob`, `-gamma` or `-data`) and `perps` (`polymarket-perps`),
/// and Binance's `usdm` (`binance-usdm`).
///
/// Each product canonicalizes the id, so text read from configuration or a
/// command line gives the key the product's own constructors give.
///
/// ```
/// # #[cfg(feature = "binance-usdm")]
/// # fn main() -> Result<(), polyoxide::venue::KeyError> {
/// use polyoxide::venue::RawKey;
///
/// let btc = polyoxide::parse_key(&RawKey::parse("binance.usdm:btcusdt")?)?;
/// assert_eq!(btc.to_string(), "binance.usdm:BTCUSDT");
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "binance-usdm"))]
/// # fn main() {}
/// ```
///
/// # Errors
///
/// [`KeyError::UnknownProduct`](venue::KeyError::UnknownProduct) when no
/// enabled product has the key's venue and product, and the product's own
/// error when it does not read the id.
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-gamma",
    feature = "polymarket-data",
    feature = "polymarket-perps",
    feature = "binance-usdm"
))]
pub fn parse_key(raw: &venue::RawKey) -> Result<venue::MarketKey, venue::KeyError> {
    #[cfg(any(
        feature = "polymarket-clob",
        feature = "polymarket-gamma",
        feature = "polymarket-data",
        feature = "polymarket-perps"
    ))]
    if raw.venue() == &polymarket::shared::keys::VENUE {
        return polymarket::shared::keys::parse(raw);
    }
    #[cfg(feature = "binance-usdm")]
    if raw.venue() == &binance::usdm::key::VENUE && raw.product() == &binance::usdm::key::PRODUCT {
        return binance::usdm::key::parse(raw);
    }
    Err(venue::KeyError::unknown(raw))
}

/// Prelude module for convenient imports
pub mod prelude {
    #[cfg(feature = "polymarket-clob-ws")]
    pub use polyoxide_polymarket::clob::ws;
    #[cfg(feature = "polymarket-clob")]
    pub use polyoxide_polymarket::clob::{
        Account, Chain, Clob, ClobBuilder, ClobError, CreateOrderParams, Credentials, OrderKind,
        OrderSide, SignatureType,
    };
    #[cfg(feature = "polymarket-data")]
    pub use polyoxide_polymarket::data::{DataApi, DataError};
    #[cfg(feature = "polymarket-gamma")]
    pub use polyoxide_polymarket::gamma::{Gamma, GammaError};
    #[cfg(feature = "polymarket-perps-ws")]
    pub use polyoxide_polymarket::perps::ws::{
        Channel as PerpsChannel, Event as PerpsEvent, PerpsMembership, PerpsWsBuilder,
    };
    #[cfg(feature = "polymarket-perps")]
    pub use polyoxide_polymarket::perps::{Perps, PerpsError};
    #[cfg(feature = "polymarket-rtds")]
    pub use polyoxide_polymarket::rtds::{
        PriceEvent, PriceUpdate, Rtds, RtdsBuilder, Subscription, Topic, TwapWindow,
    };
    // Aliased: a bare `MatchUpdate` beside the CLOB types reads like an
    // order-match update.
    #[cfg(feature = "polymarket-sports")]
    pub use polyoxide_polymarket::sports::{
        Event as SportsEvent, GameKey as SportsGameKey, MatchUpdate as SportsMatchUpdate,
        SportsError, SportsWs, SportsWsBuilder, SupervisedSportsWs,
    };

    #[cfg(all(
        feature = "polymarket-clob",
        feature = "polymarket-gamma",
        feature = "polymarket-data"
    ))]
    pub use polyoxide_polymarket::{BuildError, Polymarket, PolymarketBuilder};

    #[cfg(feature = "binance-usdm")]
    pub use polyoxide_binance::usdm::{Usdm, UsdmBuilder, UsdmError, WeightBudget};
    // Aliased as perps' and sports' are: a bare `Event` names no venue.
    #[cfg(feature = "binance-usdm-ws")]
    pub use polyoxide_binance::usdm::ws::{
        Event as UsdmEvent, StreamName, UsdmWs, UsdmWsBuilder, UsdmWsError,
    };
}

// Each passthrough names one type of its module through the path it opens,
// outside `cfg(test)`, so `cargo hack --each-feature` compiles that path with
// that feature alone: a feature missing from a `cfg` above fails here.
#[cfg(feature = "polymarket-clob")]
const _: fn(crate::polymarket::clob::Clob) = drop;
#[cfg(feature = "polymarket-clob-ws")]
const _: fn(crate::polymarket::clob::ws::WebSocket) = drop;
#[cfg(feature = "polymarket-gamma")]
const _: fn(crate::polymarket::gamma::Gamma) = drop;
#[cfg(feature = "polymarket-data")]
const _: fn(crate::polymarket::data::DataApi) = drop;
#[cfg(feature = "polymarket-relay")]
const _: fn(crate::polymarket::relay::RelayClient) = drop;
#[cfg(feature = "polymarket-perps")]
const _: fn(crate::polymarket::perps::Perps) = drop;
#[cfg(feature = "polymarket-perps-ws")]
const _: fn(crate::polymarket::perps::ws::PerpsWs) = drop;
#[cfg(feature = "polymarket-rtds")]
const _: fn(crate::polymarket::rtds::Rtds) = drop;
#[cfg(feature = "polymarket-sports")]
const _: fn(crate::polymarket::sports::SportsWs) = drop;
#[cfg(feature = "binance-usdm")]
const _: fn(crate::binance::usdm::Usdm) = drop;
#[cfg(feature = "binance-usdm-ws")]
const _: fn(crate::binance::usdm::ws::UsdmWs) = drop;

// The same for each of the prelude's `cfg` blocks, gated on the feature it is
// meant for, so a block whose `cfg` narrows fails here instead of emptying.
#[cfg(feature = "polymarket-clob")]
const _: fn(crate::prelude::Clob) = drop;
#[cfg(feature = "polymarket-clob-ws")]
const _: fn(crate::prelude::ws::WebSocket) = drop;
#[cfg(feature = "polymarket-data")]
const _: fn(crate::prelude::DataApi) = drop;
#[cfg(feature = "polymarket-gamma")]
const _: fn(crate::prelude::Gamma) = drop;
#[cfg(feature = "polymarket-perps")]
const _: fn(crate::prelude::Perps) = drop;
#[cfg(feature = "polymarket-perps-ws")]
const _: fn(crate::prelude::PerpsWsBuilder) = drop;
#[cfg(feature = "polymarket-rtds")]
const _: fn(crate::prelude::Rtds) = drop;
#[cfg(feature = "polymarket-sports")]
const _: fn(crate::prelude::SportsWs) = drop;
#[cfg(feature = "binance-usdm")]
const _: fn(crate::prelude::Usdm) = drop;
#[cfg(feature = "binance-usdm-ws")]
const _: fn(crate::prelude::UsdmWs) = drop;

// The default, `polymarket`, builds the unified client, so it must keep
// turning on clob, gamma and data.
#[cfg(feature = "polymarket")]
const _: fn(crate::prelude::Polymarket) = drop;
