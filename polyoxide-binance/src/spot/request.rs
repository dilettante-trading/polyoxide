//! What every route builder sends through: core's request builder, with the
//! route's weight as its cost. The routed request itself is the crate's
//! shared one; this module fixes its route and error type to spot's.

use crate::spot::{error::SpotError, weight::Route};

pub use crate::shared::weight::USED_WEIGHT_HEADER;

/// A route and its query, sent as core's `Request` with the route's cost.
///
/// The route builders wrap one. Setting a query parameter twice replaces it,
/// and a parameter that changes the weight replaces the route with it.
pub(crate) type Routed<T> = crate::shared::request::Routed<T, Route, SpotError>;

/// A route builder with no parameters of its own beyond those its namespace
/// method sets: `cost` and `send`, like the builders with setters.
macro_rules! route_builder {
    ($(#[$doc:meta])* $name:ident => $output:ty) => {
        $crate::shared::request::route_builder! {
            $(#[$doc])* $name => $output; module = spot, error = SpotError
        }
    };
}

pub(crate) use route_builder;
