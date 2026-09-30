//! API namespaces, one module per group of routes.

pub mod exchange;
pub mod health;
pub mod market;
pub mod public;

use polyoxide_core::{HttpClient, Request};
use serde::de::DeserializeOwned;

use crate::error::PerpsError;

/// A route with nothing left to set. Call [`Fetch::send`].
pub struct Fetch<T> {
    pub(crate) request: Request<T, PerpsError>,
}

impl<T: DeserializeOwned> Fetch<T> {
    /// Execute the request.
    pub async fn send(self) -> Result<T, PerpsError> {
        self.request.send().await
    }
}

pub(crate) fn fetch<T>(http_client: &HttpClient, path: &str) -> Fetch<T> {
    Fetch {
        request: Request::new(http_client.clone(), path),
    }
}

/// A chained query-parameter setter on a request builder that holds its
/// `Request` in a field named `request`. The parameter type is explicit so a
/// caller cannot hand a route the wrong vocabulary; string parameters take
/// `impl Into<String>`.
macro_rules! setter {
    ($(#[$meta:meta])* $name:ident: impl Into<String> => $key:literal) => {
        $(#[$meta])*
        pub fn $name(mut self, value: impl Into<String>) -> Self {
            self.request = self.request.query($key, value.into());
            self
        }
    };
    ($(#[$meta:meta])* $name:ident: $ty:ty => $key:literal) => {
        $(#[$meta])*
        pub fn $name(mut self, value: $ty) -> Self {
            self.request = self.request.query($key, value);
            self
        }
    };
}
pub(crate) use setter;
