//! # polyoxide-venue
//!
//! The vocabulary every polyoxide crate shares, whichever venue it talks to.
//!
//! - [`Class`] sorts every failure into one of eight classes, and [`Classify`],
//!   implemented by every public error type in the workspace, reports an
//!   error's class, whether it is a fault, and how long the server asked the
//!   caller to wait. [`ClassifiedError`] carries any of them.
//! - [`class_for_status`], [`class_for_close_code`] and
//!   [`class_for_handshake_status`] are the one status and close-code table,
//!   and [`impl_ws_classification!`] expands the one socket table for
//!   `polyoxide_ws::WsError` in the venue crate that wraps it.
//! - [`parse_retry_after`] is the one `Retry-After` parser, and
//!   [`retry_delay`] lets it only lengthen the client's own backoff.
//! - [`Secret`] holds a value its `Debug` never prints, and [`UnixMillis`] is
//!   the time venues stamp things with.
//! - [`wire_enum!`] and [`open_enum!`] declare an enum whose variants each have
//!   one wire spelling; a closed one refuses an unknown spelling with
//!   [`UnknownVariant`], and an open one keeps it in `Other`.
//!   [`specta_as_string!`] gives open enums their `specta` shape.
//! - With the `decimal` feature, `positional` reads and writes rows sent as
//!   bare arrays of decimal strings.
//! - With the `keys` feature, `MarketKey` names one instrument as
//!   `venue.product:id`. Only a product's `KeySpace` makes one, so two keys
//!   are equal exactly when they name the same instrument; `RawKey` is text
//!   split into its parts and checked by no product yet.
//! - With the `market` feature, the traits a venue's products implement so
//!   market data reads the same on every venue: `MarketData`, which every
//!   product implements, and the capability traits `Instruments`, `Trades`,
//!   `Candles`, `Funding` and `EventGroups`, each with a `Dyn` wrapper. They
//!   return the records market data is read into (`Instrument`, `Quote`,
//!   `Book`, `Trade`, `Candle`, `FundingRate` and `EventGroup`), each carrying
//!   `Extensions`, a venue's own facts on a shared record, one value per type.
//! - With the `trading` feature, `Trading` places and cancels orders, reads
//!   open orders, positions and balances, and streams a venue's
//!   acknowledgements and fills as `TradingEvents`, which begins with a
//!   `Resync` and carries another after every reconnect. It works in
//!   `OrderRequest`, `Order`, `Fill`, `Position` and `Balance`, with the opaque
//!   `OrderId`, `FillId` and `ClientOrderId`, each record carrying
//!   `Extensions`; `Extensions::ensure_declared` is how a venue refuses an
//!   option it does not declare.
//!
//! By default the crate depends on nothing, so a credential-free socket crate
//! can use it without building an HTTP or signing stack. The enum macros name
//! `::serde`, so a crate that invokes them depends on serde itself, with
//! serde's `derive` feature for `wire_enum!`, which derives the two traits;
//! `open_enum!` implements them by hand.
//!
//! ```
//! use polyoxide_venue::{class_for_status, Class};
//!
//! let class = class_for_status(503).unwrap().with_code("dependency_unavailable");
//! assert!(class.is_retriable());
//! assert_eq!(class.code(), Some("dependency_unavailable"));
//! assert_eq!(class_for_status(451), Some(Class::Restricted));
//! ```

#![warn(missing_docs)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

mod class;
#[cfg(feature = "market")]
mod extensions;
#[cfg(feature = "trading")]
mod ids;
#[cfg(feature = "keys")]
mod key;
#[cfg(feature = "market")]
mod market;
#[cfg(feature = "decimal")]
pub mod positional;
#[cfg(feature = "market")]
mod records;
mod retry_after;
mod secret;
mod socket;
mod status;
mod time;
#[cfg(feature = "trading")]
mod trading;
mod wire;
mod ws_table;

pub use class::{Class, ClassifiedError, Classify};
#[cfg(feature = "market")]
pub use extensions::{DuplicateExtension, Extensions, UndeclaredExtension};
#[cfg(feature = "trading")]
pub use ids::{ClientOrderId, FillId, OrderId};
#[cfg(feature = "keys")]
pub use key::{KeyError, KeySpace, MarketKey, ProductClass, ProductId, RawKey, VenueId};
#[cfg(feature = "market")]
pub use market::{
    Candles, DynCandles, DynEventGroups, DynFunding, DynInstruments, DynMarketData, DynTrades,
    EventGroups, Funding, Instruments, MarketData, Trades,
};
#[cfg(feature = "market")]
pub use records::{
    Book, Candle, CandleInterval, CandleQuery, EventGroup, EventOutcome, FundingRate, Instrument,
    Level, NotListed, Quote, Side, SizeUnit, Trade, TradeId, UnservedInterval,
};
pub use retry_after::{parse_retry_after, retry_delay};
pub use secret::Secret;
pub use socket::{class_for_close_code, class_for_handshake_status};
pub use status::class_for_status;
pub use time::UnixMillis;
#[cfg(feature = "trading")]
pub use trading::{
    Balance, DynTrading, Fill, KillReason, Liquidity, Order, OrderRequest, OrderStatus, Position,
    TimeInForce, Trading, TradingEvent, TradingEvents,
};
pub use wire::UnknownVariant;

// Every public error type implements `Classify`; one without it fails the
// build here. `.github/scripts/tests/test_classify_coverage.py` fails when a
// public error type is missing from this list.
const _: fn() = || {
    fn is<T: crate::Classify>() {}
    is::<UnknownVariant>();
    #[cfg(feature = "keys")]
    is::<KeyError>();
    #[cfg(feature = "market")]
    is::<DuplicateExtension>();
    #[cfg(feature = "market")]
    is::<NotListed>();
    #[cfg(feature = "market")]
    is::<UndeclaredExtension>();
    #[cfg(feature = "market")]
    is::<UnservedInterval>();
};
