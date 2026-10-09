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

/// Generates a request builder's query-parameter setters.
///
/// Each setter writes its parameter to `self.request`, or to
/// `self.<field>` when the invocation starts with `self.<field>;`. The field
/// may be core's `Request`, or any type with a `query(key, impl ToString)`
/// method of its own, which is then preferred (and `query_many`, for the
/// `many` arm). Each setter keeps the attributes written above it, its doc
/// comment included.
///
/// The arms, one per setter, separated by commas:
///
/// - `name: T => "key"` takes a `T` and sends it as `key`.
/// - `name: impl Into<String> => "key"` takes anything that converts into a
///   `String`.
/// - `name: many T => "key"` takes `T`, an iterator, and repeats `key` once
///   per value.
/// - `name: csv T => "key"` takes `T`, an iterator, and sends its values
///   comma-joined, or nothing when the joined value is empty.
/// - `name: csv<I, S> => "key"` is the same, with explicit generics
///   (`I: IntoIterator<Item = S>`, `S: ToString`).
/// - `name(arg: T) => "key" = expr` sends `expr`, computed from `arg`.
/// - `name(arg: T) => "key" if cond` sends `arg` only when `cond` holds.
/// - `name(arg: T) => csv "key" = expr` comma-joins the iterator `expr`.
///
/// ```
/// use polyoxide_core::{ApiError, HttpClientBuilder, Request};
///
/// pub struct ListThings {
///     request: Request<Vec<String>, ApiError>,
/// }
///
/// impl ListThings {
///     polyoxide_core::query_setters! {
///         /// Page size.
///         limit: u32 => "limit",
///         /// Only things with these ids.
///         ids: many impl IntoIterator<Item = i64> => "id",
///         /// Only open things.
///         open(open: bool) => "closed" = !open,
///     }
/// }
///
/// let http = HttpClientBuilder::new("https://example.com").build().unwrap();
/// let _things = ListThings { request: Request::new(http, "/things") }
///     .limit(10)
///     .ids([1, 2])
///     .open(true);
/// ```
#[macro_export]
macro_rules! query_setters {
    (@setters $field:ident;) => {};
    (@setters $field:ident;
        $(#[$meta:meta])* $name:ident: impl Into<String> => $key:literal
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub fn $name(mut self, value: impl Into<String>) -> Self {
            #[allow(unused_imports)]
            use $crate::QueryBuilder as _;
            self.$field = self.$field.query($key, value.into());
            self
        }

        $crate::query_setters!(@setters $field; $($($rest)*)?);
    };
    (@setters $field:ident;
        $(#[$meta:meta])* $name:ident: many $ty:ty => $key:literal
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub fn $name(mut self, values: $ty) -> Self {
            #[allow(unused_imports)]
            use $crate::QueryBuilder as _;
            self.$field = self.$field.query_many($key, values);
            self
        }

        $crate::query_setters!(@setters $field; $($($rest)*)?);
    };
    (@setters $field:ident;
        $(#[$meta:meta])* $name:ident: csv<I, S> => $key:literal
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub fn $name<I, S>(mut self, values: I) -> Self
        where
            I: IntoIterator<Item = S>,
            S: ToString,
        {
            #[allow(unused_imports)]
            use $crate::QueryBuilder as _;
            if let Some(joined) = $crate::csv(values) {
                self.$field = self.$field.query($key, joined);
            }
            self
        }

        $crate::query_setters!(@setters $field; $($($rest)*)?);
    };
    (@setters $field:ident;
        $(#[$meta:meta])* $name:ident: csv $ty:ty => $key:literal
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub fn $name(mut self, values: $ty) -> Self {
            #[allow(unused_imports)]
            use $crate::QueryBuilder as _;
            if let Some(joined) = $crate::csv(values) {
                self.$field = self.$field.query($key, joined);
            }
            self
        }

        $crate::query_setters!(@setters $field; $($($rest)*)?);
    };
    (@setters $field:ident;
        $(#[$meta:meta])* $name:ident($arg:ident: $ty:ty) => csv $key:literal = $value:expr
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub fn $name(mut self, $arg: $ty) -> Self {
            #[allow(unused_imports)]
            use $crate::QueryBuilder as _;
            if let Some(joined) = $crate::csv($value) {
                self.$field = self.$field.query($key, joined);
            }
            self
        }

        $crate::query_setters!(@setters $field; $($($rest)*)?);
    };
    (@setters $field:ident;
        $(#[$meta:meta])* $name:ident($arg:ident: $ty:ty) => $key:literal = $value:expr
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub fn $name(mut self, $arg: $ty) -> Self {
            #[allow(unused_imports)]
            use $crate::QueryBuilder as _;
            self.$field = self.$field.query($key, $value);
            self
        }

        $crate::query_setters!(@setters $field; $($($rest)*)?);
    };
    (@setters $field:ident;
        $(#[$meta:meta])* $name:ident($arg:ident: $ty:ty) => $key:literal if $cond:expr
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub fn $name(mut self, $arg: $ty) -> Self {
            #[allow(unused_imports)]
            use $crate::QueryBuilder as _;
            if $cond {
                self.$field = self.$field.query($key, $arg);
            }
            self
        }

        $crate::query_setters!(@setters $field; $($($rest)*)?);
    };
    (@setters $field:ident;
        $(#[$meta:meta])* $name:ident: $ty:ty => $key:literal
        $(, $($rest:tt)*)?
    ) => {
        $(#[$meta])*
        pub fn $name(mut self, value: $ty) -> Self {
            #[allow(unused_imports)]
            use $crate::QueryBuilder as _;
            self.$field = self.$field.query($key, value);
            self
        }

        $crate::query_setters!(@setters $field; $($($rest)*)?);
    };
    (self.$field:ident; $($setters:tt)*) => {
        $crate::query_setters!(@setters $field; $($setters)*);
    };
    ($($setters:tt)*) => {
        $crate::query_setters!(@setters request; $($setters)*);
    };
}
