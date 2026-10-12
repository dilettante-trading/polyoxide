//! The one socket classification table, as a macro a venue crate expands.

/// Expands, in the invoking module, to the one socket classification table
/// (AD-15) for `polyoxide_ws::WsError`: three private functions a venue's
/// [`Classify`](crate::Classify) impl calls on its `WsError`-carrying arm.
///
/// - `fn ws_class(err: &::polyoxide_ws::WsError) -> Class`
/// - `fn ws_is_fault(err: &::polyoxide_ws::WsError) -> bool`
/// - `fn ws_retry_after(err: &::polyoxide_ws::WsError) -> Option<Duration>`
///
/// It is a macro, not an impl, because neither crate can write the impl:
/// this crate's build does not depend on `polyoxide-ws` (AD-2), so it cannot
/// name `WsError`, and `polyoxide-ws` does not depend on this crate, so it cannot
/// see `Classify`. In a venue crate both the trait and the type are foreign,
/// and the orphan rule forbids the impl there too, so the venue crate expands
/// the table as functions its own enum's impl calls. The expansion names only
/// `::polyoxide_ws` and this crate, so the invoking crate needs `polyoxide-ws`
/// and no tungstenite of its own.
///
/// No lint reports a function the expansion defines and the venue never
/// calls: rustc does not warn about dead code expanded from another crate's
/// macro. `.github/scripts/tests/test_classify_coverage.py` is the guard
/// instead. It fails when a type with a `WsError` field sits in a file that
/// does not expand the macro, and when a file that expands it does not call
/// each of the three in code, so expand it in the file that declares the
/// wrapper, where the fence checks it.
///
/// | `Kind` | Class |
/// | --- | --- |
/// | `Io` (a TLS EOF included), `Protocol`, `ConnectionClosed`, `Other`, `ConnectTimeout`, `NoAnswer` | [`Network`](crate::Class::Network) |
/// | `Closed { code }` | [`class_for_close_code`](crate::class_for_close_code): 1000, 1001, 1006, 1011–1013 and no code are `Network`; 1002, 1003, 1007–1010 and 4000–4999 are `VenueRefusal` with the code |
/// | `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity`, `AlreadyClosed` | [`InvalidRequest`](crate::Class::InvalidRequest) |
/// | `Refused { status, retry_after }` | [`class_for_handshake_status`](crate::class_for_handshake_status), with the `Retry-After` read by [`parse_retry_after`](crate::parse_retry_after) and no clamp: a 429 carries its wait, a zero none; a 418 and a 451 are `Restricted` |
///
/// Any kind a later `polyoxide-ws` adds is `Network`. `ws_is_fault` is
/// `false` only for an upgrade refused with 451, a region the venue does not
/// serve; a 418 ban is a fault the client earned. `ws_retry_after` is a
/// refused upgrade's `Retry-After`, whatever its status, read by
/// [`parse_retry_after`](crate::parse_retry_after) with no clamp, and `None`
/// for a zero, for no header and for every other failure: a socket error
/// surfaces the wait as HTTP's `ErrorResponse::retry_after` does, though only
/// a 429's class carries it. A venue's own override of a status, such as
/// a firewall's `403` (D14), goes in its own arm ahead of the call.
///
/// Whether to reconnect is a separate decision, the venue's recovery:
/// `polyoxide_ws::retrying_can_fix` takes this crate's status rule,
/// `|status| class_for_handshake_status(status).is_retriable()`, so the two
/// agree on every refused status.
///
/// ```
/// use std::{error::Error, fmt};
/// use polyoxide_venue::{Class, Classify};
/// use polyoxide_ws::WsError;
///
/// polyoxide_venue::impl_ws_classification!();
///
/// #[derive(Debug)]
/// enum FeedError {
///     Connection(WsError),
///     Stalled,
/// }
///
/// impl fmt::Display for FeedError {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         match self {
///             Self::Connection(err) => write!(f, "{err}"),
///             Self::Stalled => f.write_str("the feed stalled"),
///         }
///     }
/// }
///
/// impl Error for FeedError {}
///
/// impl Classify for FeedError {
///     fn class(&self) -> Class {
///         match self {
///             Self::Connection(err) => ws_class(err),
///             Self::Stalled => Class::Network,
///         }
///     }
///
///     fn is_fault(&self) -> bool {
///         !matches!(self, Self::Connection(err) if !ws_is_fault(err))
///     }
///
///     fn retry_after(&self) -> Option<std::time::Duration> {
///         match self {
///             Self::Connection(err) => ws_retry_after(err),
///             Self::Stalled => None,
///         }
///     }
/// }
///
/// let policy = FeedError::Connection(WsError::closed(Some(1008), "policy"));
/// assert_eq!(policy.class(), Class::VenueRefusal { code: Some("1008".into()) });
/// assert!(FeedError::Connection(WsError::closed(Some(1001), "")).is_retriable());
/// assert_eq!(policy.retry_after(), None);
/// ```
#[macro_export]
macro_rules! impl_ws_classification {
    () => {
        /// The class of a socket failure, by the one socket table
        /// (`polyoxide_venue::impl_ws_classification!`).
        fn ws_class(err: &::polyoxide_ws::WsError) -> $crate::Class {
            match err.kind() {
                ::polyoxide_ws::Kind::Closed { code, .. } => $crate::class_for_close_code(code),
                ::polyoxide_ws::Kind::Refused {
                    status,
                    retry_after,
                } => $crate::class_for_handshake_status(status).with_retry_after(
                    retry_after.and_then(|value| {
                        $crate::parse_retry_after(value, ::core::time::Duration::MAX)
                    }),
                ),
                ::polyoxide_ws::Kind::Url
                | ::polyoxide_ws::Kind::HttpFormat
                | ::polyoxide_ws::Kind::Tls
                | ::polyoxide_ws::Kind::AttackAttempt
                | ::polyoxide_ws::Kind::Capacity
                | ::polyoxide_ws::Kind::AlreadyClosed => $crate::Class::InvalidRequest,
                // Io (a TLS EOF included), Protocol, ConnectionClosed, Other,
                // ConnectTimeout, NoAnswer, and any kind added later.
                _ => $crate::Class::Network,
            }
        }

        /// Whether a socket failure is a fault: every one but an upgrade
        /// refused with 451, a region the venue does not serve
        /// (`polyoxide_venue::impl_ws_classification!`).
        fn ws_is_fault(err: &::polyoxide_ws::WsError) -> bool {
            !::core::matches!(
                err.kind(),
                ::polyoxide_ws::Kind::Refused { status: 451, .. }
            )
        }

        /// The wait a socket failure asked for: a refused upgrade's
        /// `Retry-After`, whatever its status, unclamped, and `None` for a
        /// zero (`polyoxide_venue::impl_ws_classification!`).
        fn ws_retry_after(
            err: &::polyoxide_ws::WsError,
        ) -> ::core::option::Option<::core::time::Duration> {
            match err.kind() {
                ::polyoxide_ws::Kind::Refused {
                    retry_after: ::core::option::Option::Some(value),
                    ..
                } => $crate::parse_retry_after(value, ::core::time::Duration::MAX),
                _ => ::core::option::Option::None,
            }
        }
    };
}
