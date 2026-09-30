//! API namespaces, one module per group of routes.

pub mod exchange;
pub mod health;

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
/// `Request` in a field named `request`.
macro_rules! setter {
    ($(#[$meta:meta])* $name:ident => $key:literal) => {
        $(#[$meta])*
        pub fn $name(mut self, value: impl ToString) -> Self {
            self.request = self.request.query($key, value);
            self
        }
    };
}
pub(crate) use setter;
