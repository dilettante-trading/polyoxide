//! The eight error classes, the trait every error type implements, and the one
//! error a venue-neutral caller receives.

use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// What kind of failure an error is, whichever venue or transport produced it.
///
/// A caller decides whether to retry, back off, alert or give up from the
/// class alone, so one policy serves every crate. An HTTP status decides the
/// class before any body does (see [`class_for_status`](crate::class_for_status)).
///
/// `code` fields hold the venue's own error code, never the HTTP status, with
/// numeric codes written in decimal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Class {
    /// No response arrived: a failed connect, a timeout, a reset, a TLS EOF, a
    /// DNS failure, or a socket that closed.
    Network,
    /// The host is unwell or busy: a 408, 425 or 5xx, whatever the body says.
    Unavailable {
        /// The venue's error code, when it sent one.
        code: Option<Arc<str>>,
    },
    /// A 429.
    RateLimited {
        /// How long the server asked the caller to wait.
        retry_after: Option<Duration>,
    },
    /// A 401 or 403: the credentials were missing, wrong or not allowed.
    Unauthorized,
    /// The client refused the request itself, or was misused: local
    /// validation, a bad URL, a socket used after it closed. No status ever
    /// produces it.
    InvalidRequest,
    /// The venue refused the request: any 4xx without a class of its own, or a
    /// socket closed with a protocol or application close code.
    VenueRefusal {
        /// The venue's error code, when it sent one.
        code: Option<Arc<str>>,
    },
    /// The venue will not serve this caller: a 418 ban, a 451 region block, or
    /// a firewall refusal a venue documents as one.
    Restricted,
    /// A successful response, or a frame, that did not parse.
    Decode,
}

impl Class {
    /// Whether retrying could plausibly succeed: true for
    /// [`Network`](Self::Network), [`Unavailable`](Self::Unavailable) and
    /// [`RateLimited`](Self::RateLimited).
    ///
    /// This describes the failure, not the operation. A retriable error on an
    /// order placement still needs the caller to judge whether resubmitting
    /// is safe.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            Self::Network | Self::Unavailable { .. } | Self::RateLimited { .. }
        )
    }

    /// The wait a [`RateLimited`](Self::RateLimited) class carries, and `None`
    /// for every other class and for a zero wait.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after } => retry_after.filter(|wait| !wait.is_zero()),
            _ => None,
        }
    }

    /// The venue's code, for [`Unavailable`](Self::Unavailable) and
    /// [`VenueRefusal`](Self::VenueRefusal).
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Unavailable { code } | Self::VenueRefusal { code } => code.as_deref(),
            _ => None,
        }
    }

    /// Sets the venue's code on [`Unavailable`](Self::Unavailable) and
    /// [`VenueRefusal`](Self::VenueRefusal). Every other class is returned
    /// unchanged, since it has nowhere to put one.
    pub fn with_code(self, code: impl Into<Arc<str>>) -> Self {
        match self {
            Self::Unavailable { .. } => Self::Unavailable {
                code: Some(code.into()),
            },
            Self::VenueRefusal { .. } => Self::VenueRefusal {
                code: Some(code.into()),
            },
            other => other,
        }
    }

    /// Sets the wait on [`RateLimited`](Self::RateLimited). A zero wait is
    /// dropped, since it asks for nothing. Every other class is returned
    /// unchanged.
    pub fn with_retry_after(self, retry_after: Option<Duration>) -> Self {
        match self {
            Self::RateLimited { .. } => Self::RateLimited {
                retry_after: retry_after.filter(|wait| !wait.is_zero()),
            },
            other => other,
        }
    }
}

/// Implemented by every public error type in the workspace.
///
/// Only [`class`](Self::class) is required. The defaults suit almost every
/// error: an error is a fault, and the only wait it carries is its class's.
pub trait Classify: Error {
    /// The class of this error.
    fn class(&self) -> Class;

    /// Whether this error is a fault, rather than the venue answering as
    /// designed. False only for defined outcomes, such as a Fill-And-Kill order
    /// that matched nothing, or a venue that does not serve the caller's
    /// region. Alerting should key on this; retrying should not.
    fn is_fault(&self) -> bool {
        true
    }

    /// How long the server asked the caller to wait. Defaults to the class's
    /// own; an error whose wait outlives its class, such as a ban that says
    /// when it lifts, overrides it. Never `Some` of zero.
    fn retry_after(&self) -> Option<Duration> {
        self.class().retry_after()
    }

    /// Whether retrying could plausibly succeed, read from the class (see
    /// [`Class::is_retriable`]).
    fn is_retriable(&self) -> bool {
        self.class().is_retriable()
    }
}

/// An error with its class, from any crate: the one error type a venue-neutral
/// caller handles.
///
/// Built from any [`Classify`] error with `?` or `.into()`, which records the
/// class, [`is_fault`](Self::is_fault) and [`retry_after`](Self::retry_after)
/// as the error reported them. It displays as its source does and continues
/// the source chain from there, so wrapping an error changes nothing an error
/// reporter prints. The original error stays reachable through `source`, by
/// downcasting.
///
/// It does not implement [`Classify`] itself: that would make the blanket
/// `From` overlap `From<T> for T`. Read the class from the `class` field.
#[derive(Debug)]
pub struct ClassifiedError {
    /// The error's class.
    pub class: Class,
    /// The error that was classified.
    pub source: Box<dyn Error + Send + Sync + 'static>,
    fault: bool,
    retry_after: Option<Duration>,
}

impl ClassifiedError {
    /// Classifies an error that does not implement [`Classify`]. It counts as a
    /// fault, and its wait is its class's.
    pub fn new(class: Class, source: impl Into<Box<dyn Error + Send + Sync + 'static>>) -> Self {
        let retry_after = class.retry_after();
        Self {
            class,
            source: source.into(),
            fault: true,
            retry_after,
        }
    }

    /// Whether the error is a fault, as [`Classify::is_fault`] reported it.
    pub fn is_fault(&self) -> bool {
        self.fault
    }

    /// The wait the server asked for, as [`Classify::retry_after`] reported it.
    pub fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }

    /// Whether retrying could plausibly succeed (see [`Class::is_retriable`]).
    pub fn is_retriable(&self) -> bool {
        self.class.is_retriable()
    }
}

impl fmt::Display for ClassifiedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.source, f)
    }
}

impl Error for ClassifiedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.source()
    }
}

impl<E> From<E> for ClassifiedError
where
    E: Classify + Send + Sync + 'static,
{
    fn from(err: E) -> Self {
        Self {
            class: err.class(),
            fault: err.is_fault(),
            // The trait's contract, held even for an impl that breaks it.
            retry_after: err.retry_after().filter(|wait| !wait.is_zero()),
            source: Box::new(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for a venue error: a defined outcome with a wait that
    /// outlives its class, like Binance's ban.
    #[derive(Debug)]
    struct Banned;

    impl fmt::Display for Banned {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("banned")
        }
    }

    impl Error for Banned {}

    impl Classify for Banned {
        fn class(&self) -> Class {
            Class::Restricted
        }
        fn is_fault(&self) -> bool {
            false
        }
        fn retry_after(&self) -> Option<Duration> {
            Some(Duration::from_secs(120))
        }
    }

    #[derive(Debug)]
    struct Throttled;

    impl fmt::Display for Throttled {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("throttled")
        }
    }

    impl Error for Throttled {}

    impl Classify for Throttled {
        fn class(&self) -> Class {
            Class::RateLimited {
                retry_after: Some(Duration::from_secs(3)),
            }
        }
    }

    #[test]
    fn only_network_unavailable_and_rate_limited_are_retriable() {
        let classes = [
            (Class::Network, true),
            (Class::Unavailable { code: None }, true),
            (Class::RateLimited { retry_after: None }, true),
            (Class::Unauthorized, false),
            (Class::InvalidRequest, false),
            (Class::VenueRefusal { code: None }, false),
            (Class::Restricted, false),
            (Class::Decode, false),
        ];
        for (class, retriable) in classes {
            assert_eq!(class.is_retriable(), retriable, "{class:?}");
        }
    }

    #[test]
    fn a_code_and_a_wait_land_only_where_a_class_has_room_for_them() {
        assert_eq!(
            Class::VenueRefusal { code: None }.with_code("-1121").code(),
            Some("-1121")
        );
        assert_eq!(
            Class::Unavailable { code: None }
                .with_code("internal")
                .code(),
            Some("internal")
        );
        assert_eq!(
            Class::Unauthorized.with_code("x"),
            Class::Unauthorized,
            "a class without a code field is unchanged"
        );

        let wait = Some(Duration::from_secs(2));
        assert_eq!(
            Class::RateLimited { retry_after: None }
                .with_retry_after(wait)
                .retry_after(),
            wait
        );
        assert_eq!(
            Class::RateLimited { retry_after: None }
                .with_retry_after(Some(Duration::ZERO))
                .retry_after(),
            None,
            "a zero wait asks for nothing"
        );
        assert_eq!(
            Class::RateLimited {
                retry_after: Some(Duration::ZERO)
            }
            .retry_after(),
            None,
            "nor when the class was built with one"
        );
        assert_eq!(Class::Restricted.with_retry_after(wait), Class::Restricted);
    }

    #[test]
    fn the_defaults_read_the_class() {
        assert!(Throttled.is_fault());
        assert_eq!(
            Classify::retry_after(&Throttled),
            Some(Duration::from_secs(3))
        );
        assert!(Classify::is_retriable(&Throttled));
    }

    #[test]
    fn a_classified_error_keeps_what_the_error_reported() {
        let err = ClassifiedError::from(Banned);
        assert_eq!(err.class, Class::Restricted);
        assert!(!err.is_fault());
        assert_eq!(err.retry_after(), Some(Duration::from_secs(120)));
        assert!(!err.is_retriable());
        assert!(err.source.downcast_ref::<Banned>().is_some());
    }

    #[test]
    fn a_classified_error_displays_as_its_source_and_continues_its_chain() {
        let err: ClassifiedError = Throttled.into();
        assert_eq!(err.to_string(), "throttled");
        assert!(
            Error::source(&err).is_none(),
            "Throttled has no source of its own"
        );
    }

    /// An impl that breaks the contract by reporting a zero wait.
    #[derive(Debug)]
    struct ZeroWait;

    impl fmt::Display for ZeroWait {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("zero wait")
        }
    }

    impl Error for ZeroWait {}

    impl Classify for ZeroWait {
        fn class(&self) -> Class {
            Class::Unavailable { code: None }
        }
        fn retry_after(&self) -> Option<Duration> {
            Some(Duration::ZERO)
        }
    }

    #[test]
    fn a_classified_error_never_keeps_a_zero_wait() {
        assert_eq!(ClassifiedError::from(ZeroWait).retry_after(), None);
    }

    #[test]
    fn an_unclassified_error_takes_the_class_it_is_given() {
        let err = ClassifiedError::new(
            Class::RateLimited {
                retry_after: Some(Duration::from_secs(1)),
            },
            "slow down",
        );
        assert!(err.is_fault());
        assert_eq!(err.retry_after(), Some(Duration::from_secs(1)));
        assert_eq!(err.to_string(), "slow down");
    }

    #[test]
    fn the_question_mark_operator_classifies() {
        fn fails() -> Result<(), ClassifiedError> {
            let banned: Result<(), Banned> = Err(Banned);
            banned?;
            Ok(())
        }
        assert_eq!(fails().unwrap_err().class, Class::Restricted);
    }
}
