//! What every route builder sends through: core's request builder, with the
//! route's weight as its cost.

use std::marker::PhantomData;

use polyoxide_core::{HttpClient, QueryBuilder, Request};
use serde::de::DeserializeOwned;

use crate::{
    error::BinanceError,
    weight::{Cost, Route},
};

/// The header carrying the IP's weight used in the current minute.
pub const USED_WEIGHT_HEADER: &str = "x-mbx-used-weight-1m";

/// A route and its query, sent as core's [`Request`] with the route's cost.
///
/// The route builders wrap one. Setting a query parameter twice replaces it,
/// and a parameter that changes the weight replaces the route with it.
pub(crate) struct Routed<T> {
    http: HttpClient,
    route: Route,
    query: Vec<(&'static str, String)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Routed<T> {
    pub(crate) fn new(http: &HttpClient, route: Route) -> Self {
        Self {
            http: http.clone(),
            route,
            query: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Sets a query parameter, replacing an earlier value for the same key.
    pub(crate) fn query(mut self, key: &'static str, value: impl ToString) -> Self {
        let value = value.to_string();
        match self.query.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.query.push((key, value)),
        }
        self
    }

    /// Replaces the route, for a parameter that changes the weight.
    pub(crate) fn route(mut self, route: Route) -> Self {
        self.route = route;
        self
    }

    /// What sending this request will cost.
    pub(crate) fn cost(&self) -> Cost {
        self.route.cost()
    }
}

impl<T: DeserializeOwned> Routed<T> {
    /// Sends on the client's send loop, which waits for the budget, and
    /// decodes the response.
    pub(crate) async fn send(self) -> Result<T, BinanceError> {
        let mut request = Request::<T, BinanceError>::new(self.http, self.route.path())
            .with_cost(self.route.cost().into());
        for (key, value) in self.query {
            request = request.query(key, value);
        }
        request.send().await
    }
}

/// A route builder with no parameters of its own beyond those its namespace
/// method sets: `cost` and `send`, like the builders with setters.
macro_rules! route_builder {
    ($(#[$doc:meta])* $name:ident => $output:ty) => {
        $(#[$doc])*
        #[must_use = "a request does nothing until it is sent"]
        pub struct $name {
            pub(crate) request: $crate::usdm::request::Routed<$output>,
        }

        impl $name {
            /// What sending this request will cost.
            pub fn cost(&self) -> $crate::weight::Cost {
                self.request.cost()
            }

            /// Waits for the budget, sends, and decodes the response.
            ///
            /// A `429` holds every request on the budget, then this request
            /// is retried on the client's schedule; with no retry left and no
            /// `Retry-After`, the hold runs to the next UTC minute. A `418`
            /// holds every request for its `Retry-After`, or 2 minutes, and
            /// is not retried.
            ///
            /// The wait has no deadline of its own: a request can wait out a
            /// minute's spent budget or a ban, which Binance documents at up
            /// to 3 days. Wrap the call in `tokio::time::timeout` where a
            /// caller needs a deadline.
            pub async fn send(self) -> Result<$output, $crate::BinanceError> {
                self.request.send().await
            }
        }
    };
}

pub(crate) use route_builder;
