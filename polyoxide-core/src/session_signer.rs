//! Venue scopes a Deposit Wallet session key can be authorized for.
//!
//! Shared by `polyoxide-clob` (which lists them) and `polyoxide-relay` (which
//! requests them). The venue adds venues over time, so an unknown string is
//! kept as [`SessionSignerScope::Other`] rather than rejected.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A trading venue a session key may act on.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SessionSignerScope {
    /// The central limit order book (`"CLOB"`).
    Clob,
    /// Combos RFQ trading (`"COMBOSRFQ"`).
    CombosRfq,
    /// Every current and future venue (`"ALL"`). Must be requested alone.
    All,
    /// A scope this crate does not know yet, kept verbatim.
    Other(String),
}

impl SessionSignerScope {
    /// The wire spelling.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Clob => "CLOB",
            Self::CombosRfq => "COMBOSRFQ",
            Self::All => "ALL",
            Self::Other(s) => s,
        }
    }

    /// Parse a wire spelling; anything unrecognised becomes [`Self::Other`].
    pub fn from_wire(s: &str) -> Self {
        match s {
            "CLOB" => Self::Clob,
            "COMBOSRFQ" => Self::CombosRfq,
            "ALL" => Self::All,
            other => Self::Other(other.to_string()),
        }
    }
}

impl fmt::Display for SessionSignerScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for SessionSignerScope {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SessionSignerScope {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_wire(&s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_scopes_round_trip_their_wire_strings() {
        for (scope, wire) in [
            (SessionSignerScope::Clob, "\"CLOB\""),
            (SessionSignerScope::CombosRfq, "\"COMBOSRFQ\""),
            (SessionSignerScope::All, "\"ALL\""),
        ] {
            assert_eq!(serde_json::to_string(&scope).unwrap(), wire);
            assert_eq!(
                serde_json::from_str::<SessionSignerScope>(wire).unwrap(),
                scope
            );
        }
    }

    #[test]
    fn unknown_scopes_are_preserved_not_rejected() {
        let scope: SessionSignerScope = serde_json::from_str("\"PERPS\"").unwrap();
        assert_eq!(scope, SessionSignerScope::Other("PERPS".into()));
        assert_eq!(serde_json::to_string(&scope).unwrap(), "\"PERPS\"");
        assert_eq!(scope.to_string(), "PERPS");
    }
}
