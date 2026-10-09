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
