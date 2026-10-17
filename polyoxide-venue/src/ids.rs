//! Opaque ids for orders, fills and the caller's own order names, as each
//! venue spells them.

use std::fmt;
use std::sync::Arc;

macro_rules! opaque_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        ///
        /// Opaque: compared and hashed as the venue's text, never parsed.
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Arc<str>);

        impl $name {
            /// The id the venue sent, or the caller chose.
            pub fn new(id: impl Into<Arc<str>>) -> Self {
                Self(id.into())
            }

            /// The id, as the venue spells it.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(id: &str) -> Self {
                Self(id.into())
            }
        }

        impl From<String> for $name {
            fn from(id: String) -> Self {
                Self(id.into())
            }
        }
    };
}

opaque_id! {
    /// The venue's id for an order.
    OrderId
}

opaque_id! {
    /// The venue's id for a fill: one order's part in a trade.
    FillId
}

opaque_id! {
    /// The id a caller gives an order, which the venue echoes back or the
    /// implementation keeps for the orders it placed.
    ClientOrderId
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_its_text() {
        let id = OrderId::from("ee587a1c-0001");
        assert_eq!(id.as_str(), "ee587a1c-0001");
        assert_eq!(id.to_string(), "ee587a1c-0001");
        assert_eq!(format!("{id:?}"), r#"OrderId("ee587a1c-0001")"#);
        assert_eq!(id, OrderId::new(String::from("ee587a1c-0001")));
        assert_eq!(FillId::new("f").as_str(), "f");
        assert_eq!(ClientOrderId::from(String::from("c")).as_str(), "c");
    }

    #[test]
    fn ids_compare_and_hash_by_their_text() {
        let mut seen = std::collections::HashSet::new();
        assert!(seen.insert(OrderId::from("a")));
        assert!(!seen.insert(OrderId::new("a")), "one text is one id");
        assert!(seen.insert(OrderId::from("b")));
        assert!(OrderId::from("a") < OrderId::from("b"));
        assert_ne!(OrderId::from("a"), OrderId::from("A"));
    }
}
