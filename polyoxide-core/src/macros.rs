//! Macros for reducing boilerplate in API client implementations

/// Implements standard error conversions for API error wrapper types
///
/// This macro generates `From` implementations for `reqwest::Error` and `url::ParseError`
/// that wrap them in the `Api` variant of the error type.
///
/// # Example
///
/// ```ignore
/// use polyoxide_core::{ApiError, RequestError};
/// use thiserror::Error;
///
/// #[derive(Error, Debug)]
/// pub enum MyApiError {
///     #[error(transparent)]
///     Api(#[from] ApiError),
/// }
///
/// impl RequestError for MyApiError {
///     async fn from_response(response: reqwest::Response) -> Self {
///         Self::Api(ApiError::from_response(response).await)
///     }
/// }
///
/// // Instead of writing these manually:
/// // impl From<reqwest::Error> for MyApiError { ... }
/// // impl From<url::ParseError> for MyApiError { ... }
///
/// // Use the macro:
/// polyoxide_core::impl_api_error_conversions!(MyApiError);
/// ```
#[macro_export]
macro_rules! impl_api_error_conversions {
    ($error_type:ty) => {
        impl From<reqwest::Error> for $error_type {
            fn from(err: reqwest::Error) -> Self {
                Self::Api($crate::ApiError::Network(err))
            }
        }

        impl From<url::ParseError> for $error_type {
            fn from(err: url::ParseError) -> Self {
                Self::Api($crate::ApiError::Url(err))
            }
        }
    };
}

/// Generates a client builder's five transport setters over a
/// `ClientConfig` field: `base_url`, `timeout_ms`, `pool_size`,
/// `with_retry_config` and `max_concurrent`.
///
/// Invoke it inside the builder's `impl`, naming the field that holds the
/// config. The builder's `build` reads the config back through
/// `ClientConfig::http_builder`.
///
/// ```
/// use polyoxide_core::ClientConfig;
///
/// pub struct MyBuilder {
///     config: ClientConfig,
/// }
///
/// impl MyBuilder {
///     polyoxide_core::client_config_setters!(config);
/// }
///
/// let builder = MyBuilder { config: ClientConfig::new("https://example.com", 4) }
///     .timeout_ms(5_000)
///     .max_concurrent(2);
/// assert_eq!(builder.config.timeout_ms, 5_000);
/// assert_eq!(builder.config.max_concurrent, Some(2));
/// ```
#[macro_export]
macro_rules! client_config_setters {
    ($config:ident) => {
        /// Set the base URL for the API.
        pub fn base_url(mut self, url: impl Into<String>) -> Self {
            self.$config.base_url = url.into();
            self
        }

        /// Set the request timeout in milliseconds (default 30,000).
        pub fn timeout_ms(mut self, timeout: u64) -> Self {
            self.$config.timeout_ms = timeout;
            self
        }

        /// Set the number of idle connections kept per host (default 10).
        pub fn pool_size(mut self, size: usize) -> Self {
            self.$config.pool_size = size;
            self
        }

        /// Set the retry schedule: how many retries, and the backoff between
        /// them.
        pub fn with_retry_config(mut self, config: $crate::RetryConfig) -> Self {
            self.$config.retry_config = Some(config);
            self
        }

        /// Set the maximum number of concurrent in-flight requests.
        ///
        /// Each client has its own default. At least 1: zero admits no
        /// request, so every send waits forever.
        pub fn max_concurrent(mut self, max: usize) -> Self {
            self.$config.max_concurrent = Some(max);
            self
        }
    };
}

/// Generates a client's namespace accessors: each clones the listed client
/// fields into the same-named fields of its namespace struct.
///
/// The invocation starts with the fields every namespace takes, then lists
/// one accessor per namespace, with its attributes. An accessor may instead
/// name its own field mapping, `name: Type { to: from, .. }`, which clones
/// `self.from` into `Type::to`. The namespace structs stay declared by hand,
/// since their paths are public API.
///
/// ```
/// #[derive(Clone)]
/// pub struct Http;
///
/// pub struct Markets {
///     http_client: Http,
/// }
///
/// pub struct Pnl {
///     http_client: Http,
/// }
///
/// pub struct Client {
///     http_client: Http,
///     pnl_http_client: Http,
/// }
///
/// impl Client {
///     polyoxide_core::namespaces! { http_client;
///         /// The markets namespace.
///         markets: Markets,
///         /// The PnL namespace, on its own host.
///         pnl: Pnl { http_client: pnl_http_client },
///     }
/// }
/// ```
#[macro_export]
macro_rules! namespaces {
    (@accessors [$($field:ident),+]) => {};
    (@accessors [$($field:ident),+]
        $(#[$meta:meta])* $name:ident: $ns:path { $($to:ident: $from:ident),+ $(,)? }
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub fn $name(&self) -> $ns {
            $ns { $($to: self.$from.clone()),+ }
        }

        $crate::namespaces!(@accessors [$($field),+] $($($rest)*)?);
    };
    (@accessors [$($field:ident),+]
        $(#[$meta:meta])* $name:ident: $ns:path
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub fn $name(&self) -> $ns {
            $ns { $($field: self.$field.clone()),+ }
        }

        $crate::namespaces!(@accessors [$($field),+] $($($rest)*)?);
    };
    ($($field:ident),+; $($accessors:tt)*) => {
        $crate::namespaces!(@accessors [$($field),+] $($accessors)*);
    };
}
