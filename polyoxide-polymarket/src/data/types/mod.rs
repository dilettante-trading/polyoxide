//! Response and parameter types for Data API v2.
//!
//! Response structs follow the spec mechanically: a property that is required
//! and not nullable is a plain field, anything else is an `Option`. A `None`
//! means the value was unavailable, never zero. Every `Option` serializes as
//! `null` rather than being skipped.

#[macro_use]
mod common;
mod boards;
mod feeds;
mod markets;
mod service;
mod wallet;

pub use boards::*;
pub use common::*;
pub use feeds::*;
pub use markets::*;
pub use service::*;
pub use wallet::*;
