//! Market keys: one canonical name per instrument, whichever venue lists it.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::{Class, Classify};

/// Whether `id` is a venue or product id: an ASCII lowercase letter, then
/// lowercase letters, digits and `-`, the form a crate's registration declares.
const fn is_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut i = 1;
    while i < bytes.len() {
        let b = bytes[i];
        if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
            return false;
        }
        i += 1;
    }
    true
}

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident, $what:literal) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Cow<'static, str>);

        impl $name {
            #[doc = concat!("A ", $what, " id known when the program is built.")]
            ///
            /// # Panics
            ///
            /// Unless `id` is an ASCII lowercase letter followed by lowercase
            /// letters, digits and `-`. In a `const` the panic is a build error.
            pub const fn from_static(id: &'static str) -> Self {
                assert!(
                    is_id(id),
                    "an id is a lowercase letter, then lowercase letters, digits and `-`"
                );
                Self(Cow::Borrowed(id))
            }

            #[doc = concat!("The ", $what, " id.")]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.as_str()).finish()
            }
        }
    };
}

id_type! {
    /// A venue's id: the `venue` of `venue.product:id`.
    ///
    /// A venue crate declares its own with [`VenueId::from_static`], equal to
    /// the `venue` its registration declares.
    VenueId, "venue"
}

id_type! {
    /// A product's id: the `product` of `venue.product:id`, naming the module
    /// that trades the instrument.
    ///
    /// A venue crate declares each with [`ProductId::from_static`], equal to one
    /// of the `products` its registration declares.
    ProductId, "product"
}

/// One instrument's name, written `venue.product:id`, such as
/// `acme.spot:ABCUSD`.
///
/// A key has no public constructor from text or from parts. Only a product's
/// [`KeySpace`] makes one, and it canonicalizes the id first, so two keys are
/// equal exactly when they name the same instrument. On an event-contract
/// venue a key names one outcome, not a market.
///
/// It serializes as its text form. To read one back, parse a [`RawKey`] and
/// hand it to the product's constructors, which canonicalize it again.
///
/// ```
/// use std::borrow::Cow;
/// use polyoxide_venue::{KeySpace, MarketKey, ProductId, RawKey, VenueId};
///
/// /// A product whose ids are ASCII and uppercased.
/// struct Spot;
///
/// impl KeySpace for Spot {
///     const VENUE: VenueId = VenueId::from_static("acme");
///     const PRODUCT: ProductId = ProductId::from_static("spot");
///     type Id = String;
///
///     fn parse_id(text: &str) -> Result<String, String> {
///         if text.is_ascii() {
///             Ok(text.to_ascii_uppercase())
///         } else {
///             Err("an id is ASCII".into())
///         }
///     }
///
///     fn id_text(id: &String) -> Cow<'_, str> {
///         Cow::Borrowed(id)
///     }
/// }
///
/// let key = Spot::parse(&RawKey::parse("acme.spot:abcusd")?)?;
/// assert_eq!(key, Spot::key(&"ABCUSD".to_owned()));
/// assert_eq!(key.to_string(), "acme.spot:ABCUSD");
/// # Ok::<(), polyoxide_venue::KeyError>(())
/// ```
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarketKey {
    venue: VenueId,
    product: ProductId,
    id: Arc<str>,
}

impl MarketKey {
    /// The venue that lists the instrument.
    pub fn venue(&self) -> &VenueId {
        &self.venue
    }

    /// The product, the module that trades it.
    pub fn product(&self) -> &ProductId {
        &self.product
    }

    /// The id, in its product's canonical form. Opaque to everything but its
    /// product.
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for MarketKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}:{}", self.venue, self.product, self.id)
    }
}

impl fmt::Debug for MarketKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MarketKey")
            .field(&format_args!("{self}"))
            .finish()
    }
}

impl Serialize for MarketKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// A key's text split into venue, product and id, checked by no product yet.
///
/// [`RawKey::parse`] splits at the first `:`, then what comes before it at its
/// first `.`. The id is everything after the `:`, so it may hold `:` and `.`
/// of its own. The venue and product must be ids (an ASCII lowercase letter,
/// then lowercase letters, digits and `-`) and the id must not be empty; the
/// id is otherwise kept as written, since only its product knows its
/// canonical form.
///
/// It serializes as its text form and deserializes through
/// [`RawKey::parse`].
///
/// ```
/// use polyoxide_venue::RawKey;
///
/// let raw = RawKey::parse("acme.events:FED-25DEC:yes")?;
/// assert_eq!(raw.venue().as_str(), "acme");
/// assert_eq!(raw.product().as_str(), "events");
/// assert_eq!(raw.id(), "FED-25DEC:yes");
/// assert!(RawKey::parse("acme:spot").is_err());
/// # Ok::<(), polyoxide_venue::KeyError>(())
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RawKey {
    venue: VenueId,
    product: ProductId,
    id: String,
}

impl RawKey {
    /// Splits `text`, written `venue.product:id`.
    ///
    /// # Errors
    ///
    /// [`KeyError::Malformed`] when there is no `:`, no `.` before it, a venue
    /// or product that is not an id, or an empty id.
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        let malformed = |reason| KeyError::Malformed {
            text: text.to_owned(),
            reason,
        };
        let (scope, id) = text
            .split_once(':')
            .ok_or_else(|| malformed("no `:` before the id"))?;
        let (venue, product) = scope
            .split_once('.')
            .ok_or_else(|| malformed("no `.` between the venue and the product"))?;
        if !is_id(venue) {
            return Err(malformed(
                "the venue is not a lowercase letter followed by lowercase letters, digits and `-`",
            ));
        }
        if !is_id(product) {
            return Err(malformed(
                "the product is not a lowercase letter followed by lowercase letters, digits and `-`",
            ));
        }
        if id.is_empty() {
            return Err(malformed("the id is empty"));
        }
        Ok(Self {
            venue: VenueId(Cow::Owned(venue.to_owned())),
            product: ProductId(Cow::Owned(product.to_owned())),
            id: id.to_owned(),
        })
    }

    /// The venue, as written.
    pub fn venue(&self) -> &VenueId {
        &self.venue
    }

    /// The product, as written.
    pub fn product(&self) -> &ProductId {
        &self.product
    }

    /// The id, as written.
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl FromStr for RawKey {
    type Err = KeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl fmt::Display for RawKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}:{}", self.venue, self.product, self.id)
    }
}

impl fmt::Debug for RawKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RawKey")
            .field(&format_args!("{self}"))
            .finish()
    }
}

impl Serialize for RawKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RawKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<'de, str>::deserialize(deserializer)?;
        Self::parse(&text).map_err(de::Error::custom)
    }
}

/// A product's keys. A venue crate implements it once for each product it
/// trades, and its methods are the only way to make a [`MarketKey`].
///
/// The product's [`Id`](Self::Id) type holds an id in canonical form, so
/// [`key`](Self::key) cannot fail, and [`parse_id`](Self::parse_id) is the
/// one place the product checks and canonicalizes text.
pub trait KeySpace {
    /// The venue id, equal to the `venue` the venue crate registers.
    const VENUE: VenueId;
    /// The product id, one of the `products` the venue crate registers.
    const PRODUCT: ProductId;
    /// The product's own id, in canonical form.
    type Id;

    /// Reads an id from text, canonicalizing it.
    ///
    /// # Errors
    ///
    /// Why the text is not one of the product's ids.
    fn parse_id(text: &str) -> Result<Self::Id, String>;

    /// The id's canonical text, the `id` of its key.
    fn id_text(id: &Self::Id) -> Cow<'_, str>;

    /// The key of `id`.
    fn key(id: &Self::Id) -> MarketKey {
        MarketKey {
            venue: Self::VENUE,
            product: Self::PRODUCT,
            id: Arc::from(Self::id_text(id).as_ref()),
        }
    }

    /// The key `raw` names, canonicalized.
    ///
    /// # Errors
    ///
    /// [`KeyError::WrongProduct`] when `raw` names another product, and
    /// [`KeyError::InvalidId`] when its id is not one of this product's.
    fn parse(raw: &RawKey) -> Result<MarketKey, KeyError> {
        Self::id_of(raw.venue(), raw.product(), raw.id()).map(|id| Self::key(&id))
    }

    /// The product's id that `key` names.
    ///
    /// # Errors
    ///
    /// [`KeyError::WrongProduct`] when `key` names another product.
    fn id(key: &MarketKey) -> Result<Self::Id, KeyError> {
        Self::id_of(key.venue(), key.product(), key.id())
    }

    #[doc(hidden)]
    fn id_of(venue: &VenueId, product: &ProductId, id: &str) -> Result<Self::Id, KeyError> {
        if venue != &Self::VENUE || product != &Self::PRODUCT {
            return Err(KeyError::WrongProduct {
                expected: format!("{}.{}", Self::VENUE, Self::PRODUCT),
                found: format!("{venue}.{product}"),
            });
        }
        Self::parse_id(id).map_err(|reason| KeyError::InvalidId {
            product: format!("{}.{}", Self::VENUE, Self::PRODUCT),
            id: id.to_owned(),
            reason,
        })
    }
}

/// Why text or a key named no instrument a product could read. Every one is
/// an [`InvalidRequest`](Class::InvalidRequest): the caller named something
/// no request can carry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyError {
    /// The text is not `venue.product:id`.
    Malformed {
        /// The text, as given.
        text: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// No product this build enables has the key's venue and product ids.
    UnknownProduct {
        /// The venue id, as written.
        venue: String,
        /// The product id, as written.
        product: String,
    },
    /// A product was handed a key of another product.
    WrongProduct {
        /// The product that was asked, as `venue.product`.
        expected: String,
        /// The key's product, as `venue.product`.
        found: String,
    },
    /// The product does not read the id as one of its own.
    InvalidId {
        /// The product, as `venue.product`.
        product: String,
        /// The id, as given.
        id: String,
        /// Why the product refused it.
        reason: String,
    },
}

impl KeyError {
    /// The error for a raw key whose venue and product no enabled product has.
    pub fn unknown(raw: &RawKey) -> Self {
        Self::UnknownProduct {
            venue: raw.venue().as_str().to_owned(),
            product: raw.product().as_str().to_owned(),
        }
    }
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { text, reason } => {
                write!(
                    f,
                    "{text:?} is not a market key (`venue.product:id`): {reason}"
                )
            }
            Self::UnknownProduct { venue, product } => {
                write!(f, "no product this build enables is {venue}.{product}")
            }
            Self::WrongProduct { expected, found } => {
                write!(f, "a {found} key was given where a {expected} key belongs")
            }
            Self::InvalidId {
                product,
                id,
                reason,
            } => write!(f, "{id:?} is not a {product} id: {reason}"),
        }
    }
}

impl Error for KeyError {}

/// An [`InvalidRequest`](Class::InvalidRequest), whatever the case.
impl Classify for KeyError {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ids are ASCII; letters are uppercased.
    struct Upper;

    impl KeySpace for Upper {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("spot");
        type Id = String;

        fn parse_id(text: &str) -> Result<String, String> {
            if text.is_ascii() {
                Ok(text.to_ascii_uppercase())
            } else {
                Err("an id is ASCII".to_owned())
            }
        }

        fn id_text(id: &String) -> Cow<'_, str> {
            Cow::Borrowed(id)
        }
    }

    /// Ids are unsigned integers, written without leading zeros.
    struct Numbered;

    impl KeySpace for Numbered {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("perpetual");
        type Id = u64;

        fn parse_id(text: &str) -> Result<u64, String> {
            text.parse().map_err(|e| format!("{e}"))
        }

        fn id_text(id: &u64) -> Cow<'_, str> {
            Cow::Owned(id.to_string())
        }
    }

    fn raw(text: &str) -> RawKey {
        RawKey::parse(text).unwrap()
    }

    #[test]
    fn text_splits_at_the_first_dot_and_the_first_colon() {
        let key = raw("acme.spot:abc");
        assert_eq!(key.venue().as_str(), "acme");
        assert_eq!(key.product().as_str(), "spot");
        assert_eq!(key.id(), "abc");

        let dotted = raw("acme.spot:a.b");
        assert_eq!(dotted.product().as_str(), "spot");
        assert_eq!(dotted.id(), "a.b");
    }

    #[test]
    fn a_synthetic_id_holding_a_second_colon_is_kept_whole() {
        let key = raw("acme.events:FED-25DEC:yes");
        assert_eq!(key.id(), "FED-25DEC:yes");
        assert_eq!(key.to_string(), "acme.events:FED-25DEC:yes");
        let parsed = Upper::parse(&raw("acme.spot:fed-25dec:yes")).unwrap();
        assert_eq!(parsed.id(), "FED-25DEC:YES");
    }

    #[test]
    fn malformed_text_names_its_fault() {
        for (text, fault) in [
            ("acme-spot-abc", "no `:`"),
            ("acme:abc", "no `.`"),
            ("Acme.spot:abc", "the venue"),
            (".spot:abc", "the venue"),
            ("acme.Spot:abc", "the product"),
            ("acme.sp.ot:abc", "the product"),
            ("acme.:abc", "the product"),
            ("acme.spot:", "the id is empty"),
            (" acme.spot:abc", "the venue"),
        ] {
            let err = RawKey::parse(text).unwrap_err();
            assert!(
                matches!(&err, KeyError::Malformed { text: t, reason } if t == text && reason.contains(fault)),
                "{text:?}: {err}"
            );
        }
    }

    #[test]
    fn case_canonicalization_makes_two_spellings_one_key() {
        let lower = Upper::parse(&raw("acme.spot:abcusd")).unwrap();
        let upper = Upper::parse(&raw("acme.spot:ABCUSD")).unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.id(), "ABCUSD");
        let mut map = std::collections::HashMap::new();
        map.insert(lower, 1);
        assert_eq!(map.get(&upper), Some(&1));
    }

    #[test]
    fn a_numbered_id_loses_its_leading_zeros() {
        let key = Numbered::parse(&raw("acme.perpetual:007")).unwrap();
        assert_eq!(key, Numbered::key(&7));
        assert_eq!(key.id(), "7");
        assert_eq!(Numbered::id(&key), Ok(7));
    }

    #[test]
    fn a_product_refuses_another_products_key_and_an_id_it_cannot_read() {
        let spot = Upper::key(&"ABC".to_owned());
        assert_eq!(
            Numbered::id(&spot),
            Err(KeyError::WrongProduct {
                expected: "acme.perpetual".to_owned(),
                found: "acme.spot".to_owned(),
            })
        );
        assert!(matches!(
            Numbered::parse(&raw("other.perpetual:7")),
            Err(KeyError::WrongProduct { .. })
        ));
        let err = Numbered::parse(&raw("acme.perpetual:seven")).unwrap_err();
        assert!(
            matches!(&err, KeyError::InvalidId { product, id, .. } if product == "acme.perpetual" && id == "seven"),
            "{err}"
        );
        assert_eq!(err.class(), Class::InvalidRequest);
    }

    #[test]
    fn a_key_and_a_raw_key_serialize_as_their_text() {
        let key = Upper::key(&"ABC".to_owned());
        assert_eq!(serde_json::to_string(&key).unwrap(), r#""acme.spot:ABC""#);
        let raw: RawKey = serde_json::from_str(r#""acme.spot:abc""#).unwrap();
        assert_eq!(raw, RawKey::parse("acme.spot:abc").unwrap());
        assert_eq!(serde_json::to_string(&raw).unwrap(), r#""acme.spot:abc""#);
        assert!(serde_json::from_str::<RawKey>(r#""acme-spot""#).is_err());
        assert_eq!(Upper::parse(&raw).unwrap(), key);
    }

    #[test]
    fn debug_shows_the_text_form() {
        let key = Upper::key(&"ABC".to_owned());
        assert_eq!(format!("{key:?}"), "MarketKey(acme.spot:ABC)");
        assert_eq!(
            format!("{:?}", raw("acme.spot:abc")),
            "RawKey(acme.spot:abc)"
        );
        assert_eq!(format!("{:?}", Upper::VENUE), r#"VenueId("acme")"#);
    }

    #[test]
    fn every_key_error_is_an_invalid_request() {
        let errors = [
            RawKey::parse("x").unwrap_err(),
            KeyError::unknown(&raw("acme.spot:abc")),
            Numbered::id(&Upper::key(&"A".to_owned())).unwrap_err(),
            Numbered::parse(&raw("acme.perpetual:x")).unwrap_err(),
        ];
        for err in errors {
            assert_eq!(err.class(), Class::InvalidRequest, "{err}");
            assert!(err.is_fault());
            assert!(!err.is_retriable());
        }
    }

    #[test]
    #[should_panic(expected = "an id is a lowercase letter")]
    fn a_static_id_must_be_an_id() {
        let _ = VenueId::from_static("Acme");
    }
}
