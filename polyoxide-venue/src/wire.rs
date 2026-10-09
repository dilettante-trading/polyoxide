//! Wire enums: one spelling per variant, written once.

use std::fmt;

use crate::class::{Class, Classify};

/// A string that is not one of a closed set's wire spellings.
///
/// A [`wire_enum!`](crate::wire_enum) type's `FromStr` returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownVariant {
    /// The Rust type being parsed.
    pub type_name: &'static str,
    /// The offending input.
    pub value: String,
}

impl fmt::Display for UnknownVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a valid {}", self.value, self.type_name)
    }
}

impl std::error::Error for UnknownVariant {}

/// An `InvalidRequest`: the caller parsed a spelling no variant has. Only
/// `FromStr` builds one, on a value the caller supplies; a server frame whose
/// spelling does not parse is the venue's decode error instead.
impl Classify for UnknownVariant {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

/// Declares a closed set the client sends: one wire spelling per variant.
///
/// The enum derives `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash` and
/// serde's `Serialize` and `Deserialize`, renamed to the wire spellings, and
/// gets `ALL`, `as_str`, `Display` and a `FromStr` whose error is
/// [`UnknownVariant`]. Attributes written above the enum, such as
/// `#[non_exhaustive]`, are kept, and so are those above each variant.
///
/// The expansion names `::serde`, so the calling crate depends on serde.
///
/// ```
/// polyoxide_venue::wire_enum! {
///     /// Side of a trade or position.
///     pub enum Side { Long => "long", Short => "short" }
/// }
///
/// assert_eq!(Side::Long.as_str(), "long");
/// assert_eq!("short".parse::<Side>(), Ok(Side::Short));
/// assert_eq!(Side::ALL, &[Side::Long, Side::Short]);
/// let err = "flat".parse::<Side>().unwrap_err();
/// assert_eq!(err.to_string(), r#""flat" is not a valid Side"#);
/// ```
#[macro_export]
macro_rules! wire_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $wire:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, ::serde::Serialize, ::serde::Deserialize,
        )]
        $vis enum $name {
            $( $(#[$vmeta])* #[serde(rename = $wire)] $variant, )+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[$( Self::$variant ),+];

            /// The wire spelling.
            pub fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $wire, )+
                }
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = $crate::UnknownVariant;

            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                match s {
                    $( $wire => ::core::result::Result::Ok(Self::$variant), )+
                    _ => ::core::result::Result::Err($crate::UnknownVariant {
                        type_name: ::core::stringify!($name),
                        value: ::std::borrow::ToOwned::to_owned(s),
                    }),
                }
            }
        }
    };
}

/// Declares a set the venue reports and extends over time: a spelling this
/// version does not know is kept verbatim in `Other` instead of failing the
/// response.
///
/// The enum is `#[non_exhaustive]` and derives `Debug`, `Clone`, `PartialEq`,
/// `Eq` and `Hash`. It gets `ALL` (the known variants), `as_str`,
/// `from_wire`, `Display`, a `FromStr` that never fails, and serde's
/// `Serialize` and `Deserialize` as a string. Attributes written above the
/// enum and each variant are kept.
///
/// The expansion names `::serde`, so the calling crate depends on serde.
///
/// ```
/// polyoxide_venue::open_enum! {
///     /// Lifecycle state of a transaction.
///     pub enum State {
///         /// Accepted.
///         New => "STATE_NEW",
///         /// Confirmed on chain.
///         Confirmed => "STATE_CONFIRMED",
///     }
/// }
///
/// assert_eq!(State::from_wire("STATE_NEW"), State::New);
/// let queued = State::from_wire("STATE_QUEUED");
/// assert_eq!(queued, State::Other("STATE_QUEUED".into()));
/// assert_eq!(queued.as_str(), "STATE_QUEUED");
/// assert_eq!(State::ALL, &[State::New, State::Confirmed]);
/// ```
#[macro_export]
macro_rules! open_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $wire:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        $vis enum $name {
            $( $(#[$vmeta])* $variant, )+
            /// A value this version of the SDK does not recognise, kept verbatim.
            /// Sending it in a request passes the string through unchanged.
            ///
            /// Build it with `from_wire` or `FromStr`, never directly:
            /// `Other` holding a known spelling serialises as the named variant
            /// does, but does not compare equal to it.
            Other(::std::string::String),
        }

        impl $name {
            /// Every variant this SDK knows, in declaration order. `Other` is
            /// not among them: it holds whatever else the server sends.
            pub const ALL: &'static [Self] = &[$( Self::$variant ),+];

            /// The wire spelling.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $wire, )+
                    Self::Other(raw) => raw,
                }
            }

            /// Parse a wire spelling; anything unrecognised becomes `Other`.
            pub fn from_wire(s: &str) -> Self {
                match s {
                    $( $wire => Self::$variant, )+
                    other => Self::Other(::std::borrow::ToOwned::to_owned(other)),
                }
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = ::core::convert::Infallible;

            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                ::core::result::Result::Ok(Self::from_wire(s))
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                let raw = <::std::string::String as ::serde::Deserialize>::deserialize(deserializer)?;
                ::core::result::Result::Ok(match raw.as_str() {
                    $( $wire => Self::$variant, )+
                    _ => Self::Other(raw),
                })
            }
        }
    };
}

/// Implements `specta::Type` as a `String` for each named type, behind the
/// calling crate's `specta` feature: the shape an [`open_enum!`](crate::open_enum)
/// type has on the wire.
///
/// It is a macro of its own, rather than part of `open_enum!`, because the
/// `cfg(feature = "specta")` it emits is evaluated in the calling crate, and a
/// crate that declares no such feature would fail its `unexpected_cfgs` check.
///
/// ```text
/// polyoxide_venue::specta_as_string!(ProtocolVersion, ResolutionStatus, HomeAway);
/// ```
#[macro_export]
macro_rules! specta_as_string {
    ($($name:ty),+ $(,)?) => {
        $(
            #[cfg(feature = "specta")]
            impl ::specta::Type for $name {
                fn inline(
                    type_map: &mut ::specta::TypeMap,
                    generics: ::specta::Generics,
                ) -> ::specta::DataType {
                    <::std::string::String as ::specta::Type>::inline(type_map, generics)
                }
            }
        )+
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::wire_enum! {
        /// A closed test set, with a non-identifier spelling.
        #[non_exhaustive]
        pub enum Interval {
            /// One minute.
            M1 => "1m",
            /// One month, which differs from `1m` only in case.
            Mo1 => "1M",
        }
    }

    crate::open_enum! {
        /// An open test set.
        pub enum State {
            /// Accepted.
            New => "STATE_NEW",
            /// Confirmed on chain.
            Confirmed => "STATE_CONFIRMED",
        }
    }

    #[test]
    fn an_unknown_variant_names_the_type_and_the_value() {
        let err = UnknownVariant {
            type_name: "Interval",
            value: "2m".into(),
        };
        assert_eq!(err.to_string(), r#""2m" is not a valid Interval"#);
        let boxed: Box<dyn std::error::Error> = Box::new(err);
        assert!(boxed.source().is_none());
    }

    #[test]
    fn an_unknown_variant_is_an_invalid_request() {
        let err = "2m".parse::<Interval>().unwrap_err();
        assert_eq!(err.class(), Class::InvalidRequest);
        assert!(err.is_fault());
        assert!(!err.is_retriable());
        assert_eq!(Classify::retry_after(&err), None);
    }

    #[test]
    fn a_wire_enum_round_trips_and_refuses_an_unknown_spelling() {
        assert_eq!(Interval::ALL, &[Interval::M1, Interval::Mo1]);
        for &variant in Interval::ALL {
            let wire = variant.as_str();
            assert_eq!(variant.to_string(), wire);
            assert_eq!(wire.parse::<Interval>(), Ok(variant));
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, format!("{wire:?}"));
            assert_eq!(serde_json::from_str::<Interval>(&json).unwrap(), variant);
        }
        assert_eq!(
            "2m".parse::<Interval>(),
            Err(UnknownVariant {
                type_name: "Interval",
                value: "2m".into()
            })
        );
        assert!(serde_json::from_str::<Interval>(r#""2m""#).is_err());
    }

    #[test]
    fn an_open_enum_keeps_an_unknown_value_verbatim() {
        let queued: State = serde_json::from_str(r#""STATE_QUEUED""#).unwrap();
        assert_eq!(queued, State::Other("STATE_QUEUED".into()));
        assert_eq!(queued.as_str(), "STATE_QUEUED");
        assert_eq!(queued.to_string(), "STATE_QUEUED");
        assert_eq!(serde_json::to_string(&queued).unwrap(), r#""STATE_QUEUED""#);

        let known: State = serde_json::from_str(r#""STATE_NEW""#).unwrap();
        assert_eq!(known, State::New);
        assert_eq!(State::ALL, &[State::New, State::Confirmed]);
    }

    #[test]
    fn from_wire_and_from_str_agree() {
        for wire in ["STATE_NEW", "STATE_CONFIRMED", "STATE_QUEUED", ""] {
            let Ok(parsed) = wire.parse::<State>();
            assert_eq!(State::from_wire(wire), parsed, "{wire:?}");
            assert_eq!(parsed.as_str(), wire);
        }
    }
}
