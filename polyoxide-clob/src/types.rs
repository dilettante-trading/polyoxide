use std::fmt;

use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error when parsing a tick size from an invalid value
#[derive(Error, Debug, Clone, PartialEq)]
#[error("invalid tick size: {0}. Valid values are 0.1, 0.01, 0.001, or 0.0001")]
pub struct ParseTickSizeError(String);

/// Side of an order (buy or sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    /// Returns the uppercase string representation ("BUY" / "SELL") for API queries.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

impl Serialize for OrderSide {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Buy => serializer.serialize_str("BUY"),
            Self::Sell => serializer.serialize_str("SELL"),
        }
    }
}

impl fmt::Display for OrderSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buy => write!(f, "0"),
            Self::Sell => write!(f, "1"),
        }
    }
}

/// Order type/kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderKind {
    /// Good-till-Cancelled
    Gtc,
    /// Fill-or-Kill
    Fok,
    /// Good-till-Date
    Gtd,
    /// Fill-and-Kill
    Fak,
}

impl fmt::Display for OrderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gtc => write!(f, "GTC"),
            Self::Fok => write!(f, "FOK"),
            Self::Gtd => write!(f, "GTD"),
            Self::Fak => write!(f, "FAK"),
        }
    }
}

/// Signature type for order signing (EOA, Proxy, or Gnosis Safe).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SignatureType {
    #[default]
    Eoa = 0,
    PolyProxy = 1,
    PolyGnosisSafe = 2,
}

impl SignatureType {
    /// Returns true if the signature type indicates a proxy wallet
    pub fn is_proxy(&self) -> bool {
        matches!(self, Self::PolyProxy | Self::PolyGnosisSafe)
    }
}

impl Serialize for SignatureType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for SignatureType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = u8::deserialize(deserializer)?;
        match v {
            0 => Ok(Self::Eoa),
            1 => Ok(Self::PolyProxy),
            2 => Ok(Self::PolyGnosisSafe),
            _ => Err(serde::de::Error::custom(format!(
                "invalid signature type: {}",
                v
            ))),
        }
    }
}

impl fmt::Display for SignatureType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eoa => write!(f, "eoa"),
            Self::PolyProxy => write!(f, "poly-proxy"),
            Self::PolyGnosisSafe => write!(f, "poly-gnosis-safe"),
        }
    }
}

/// Tick size (minimum price increment)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TickSize {
    /// 0.1
    Tenth,
    /// 0.01
    Hundredth,
    /// 0.001
    Thousandth,
    /// 0.0001
    TenThousandth,
}

impl TickSize {
    /// Returns the tick size as an `f64` value.
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::Tenth => 0.1,
            Self::Hundredth => 0.01,
            Self::Thousandth => 0.001,
            Self::TenThousandth => 0.0001,
        }
    }

    /// Returns the number of decimal places for this tick size.
    pub fn decimals(&self) -> u32 {
        match self {
            Self::Tenth => 1,
            Self::Hundredth => 2,
            Self::Thousandth => 3,
            Self::TenThousandth => 4,
        }
    }
}

/// Options for creating an order
#[derive(Debug, Clone, Copy, Default)]
pub struct PartialCreateOrderOptions {
    pub tick_size: Option<TickSize>,
    pub neg_risk: Option<bool>,
}

impl TryFrom<&str> for TickSize {
    type Error = ParseTickSizeError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "0.1" => Ok(Self::Tenth),
            "0.01" => Ok(Self::Hundredth),
            "0.001" => Ok(Self::Thousandth),
            "0.0001" => Ok(Self::TenThousandth),
            _ => Err(ParseTickSizeError(s.to_string())),
        }
    }
}

impl TryFrom<f64> for TickSize {
    type Error = ParseTickSizeError;

    fn try_from(n: f64) -> Result<Self, Self::Error> {
        const EPSILON: f64 = 1e-10;
        if (n - 0.1).abs() < EPSILON {
            Ok(Self::Tenth)
        } else if (n - 0.01).abs() < EPSILON {
            Ok(Self::Hundredth)
        } else if (n - 0.001).abs() < EPSILON {
            Ok(Self::Thousandth)
        } else if (n - 0.0001).abs() < EPSILON {
            Ok(Self::TenThousandth)
        } else {
            Err(ParseTickSizeError(n.to_string()))
        }
    }
}

impl std::str::FromStr for TickSize {
    type Err = ParseTickSizeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

fn serialize_salt<S>(salt: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // Parse the string as u128 and serialize it as a number
    let val = salt
        .parse::<u128>()
        .map_err(|_| serde::ser::Error::custom("invalid salt"))?;
    serializer.serialize_u128(val)
}

/// Unsigned order
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde(serialize_with = "serialize_salt")]
    pub salt: String,
    pub maker: Address,
    pub signer: Address,
    pub taker: Address,
    pub token_id: String,
    pub maker_amount: String,
    pub taker_amount: String,
    pub expiration: String,
    pub nonce: String,
    pub fee_rate_bps: String,
    pub side: OrderSide,
    pub signature_type: SignatureType,
    #[serde(skip)]
    pub neg_risk: bool,
}

/// Arguments for creating a market order
#[derive(Debug, Clone)]
pub struct MarketOrderArgs {
    pub token_id: String,
    /// For BUY: Amount in USDC to spend
    /// For SELL: Amount of token to sell
    pub amount: f64,
    pub side: OrderSide,
    /// Worst acceptable price to fill at.
    /// If None, it will be calculated from the orderbook.
    pub price: Option<f64>,
    pub fee_rate_bps: Option<u16>,
    pub nonce: Option<u64>,
    pub funder: Option<Address>,
    pub signature_type: Option<SignatureType>,
    pub order_type: Option<OrderKind>,
}

/// Signed order
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedOrder {
    #[serde(flatten)]
    pub order: Order,
    pub signature: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;
    use std::str::FromStr;

    #[test]
    fn test_order_serialization() {
        let order = Order {
            salt: "123".to_string(),
            maker: Address::from_str("0x0000000000000000000000000000000000000001").unwrap(),
            signer: Address::from_str("0x0000000000000000000000000000000000000002").unwrap(),
            taker: Address::ZERO,
            token_id: "456".to_string(),
            maker_amount: "1000".to_string(),
            taker_amount: "2000".to_string(),
            expiration: "0".to_string(),
            nonce: "789".to_string(),
            fee_rate_bps: "0".to_string(),
            side: OrderSide::Buy,
            signature_type: SignatureType::Eoa,
            neg_risk: false,
        };

        let signed_order = SignedOrder {
            order,
            signature: "0xabc".to_string(),
        };

        let json = serde_json::to_value(&signed_order).unwrap();

        // Check camelCase
        assert!(json.get("makerAmount").is_some());
        assert!(json.get("takerAmount").is_some());
        assert!(json.get("tokenId").is_some());
        assert!(json.get("feeRateBps").is_some());
        assert!(json.get("signatureType").is_some());

        // Check flattened fields
        assert!(json.get("signature").is_some());
        assert!(json.get("salt").is_some());

        // Check values
        assert_eq!(json["makerAmount"], "1000");
        assert_eq!(json["side"], "BUY");
        assert_eq!(json["signatureType"], 0);
        assert_eq!(json["nonce"], "789");
    }

    #[test]
    fn order_side_serde_roundtrip() {
        let buy: OrderSide = serde_json::from_str("\"BUY\"").unwrap();
        let sell: OrderSide = serde_json::from_str("\"SELL\"").unwrap();
        assert_eq!(buy, OrderSide::Buy);
        assert_eq!(sell, OrderSide::Sell);

        assert_eq!(serde_json::to_string(&OrderSide::Buy).unwrap(), "\"BUY\"");
        assert_eq!(serde_json::to_string(&OrderSide::Sell).unwrap(), "\"SELL\"");
    }

    #[test]
    fn order_side_display_is_numeric() {
        // Display uses 0/1 for EIP-712 encoding
        assert_eq!(OrderSide::Buy.to_string(), "0");
        assert_eq!(OrderSide::Sell.to_string(), "1");
    }

    #[test]
    fn order_side_rejects_lowercase() {
        let result = serde_json::from_str::<OrderSide>("\"buy\"");
        assert!(result.is_err(), "Should reject lowercase order side");
    }

    #[test]
    fn order_kind_serde_roundtrip() {
        for (variant, expected) in [
            (OrderKind::Gtc, "GTC"),
            (OrderKind::Fok, "FOK"),
            (OrderKind::Gtd, "GTD"),
            (OrderKind::Fak, "FAK"),
        ] {
            let serialized = serde_json::to_string(&variant).unwrap();
            assert_eq!(serialized, format!("\"{}\"", expected));

            let deserialized: OrderKind = serde_json::from_str(&serialized).unwrap();
            assert_eq!(deserialized, variant);
        }
    }

    #[test]
    fn order_kind_display() {
        assert_eq!(OrderKind::Gtc.to_string(), "GTC");
        assert_eq!(OrderKind::Fok.to_string(), "FOK");
        assert_eq!(OrderKind::Gtd.to_string(), "GTD");
        assert_eq!(OrderKind::Fak.to_string(), "FAK");
    }

    #[test]
    fn signature_type_serde_as_u8() {
        assert_eq!(serde_json::to_string(&SignatureType::Eoa).unwrap(), "0");
        assert_eq!(
            serde_json::to_string(&SignatureType::PolyProxy).unwrap(),
            "1"
        );
        assert_eq!(
            serde_json::to_string(&SignatureType::PolyGnosisSafe).unwrap(),
            "2"
        );

        let eoa: SignatureType = serde_json::from_str("0").unwrap();
        assert_eq!(eoa, SignatureType::Eoa);
        let proxy: SignatureType = serde_json::from_str("1").unwrap();
        assert_eq!(proxy, SignatureType::PolyProxy);
        let gnosis: SignatureType = serde_json::from_str("2").unwrap();
        assert_eq!(gnosis, SignatureType::PolyGnosisSafe);
    }

    #[test]
    fn signature_type_rejects_invalid_u8() {
        let result = serde_json::from_str::<SignatureType>("3");
        assert!(result.is_err(), "Should reject invalid signature type 3");

        let result = serde_json::from_str::<SignatureType>("255");
        assert!(result.is_err(), "Should reject invalid signature type 255");
    }

    #[test]
    fn signature_type_display() {
        assert_eq!(SignatureType::Eoa.to_string(), "eoa");
        assert_eq!(SignatureType::PolyProxy.to_string(), "poly-proxy");
        assert_eq!(
            SignatureType::PolyGnosisSafe.to_string(),
            "poly-gnosis-safe"
        );
    }

    #[test]
    fn signature_type_default_is_eoa() {
        assert_eq!(SignatureType::default(), SignatureType::Eoa);
    }

    #[test]
    fn signature_type_is_proxy() {
        assert!(!SignatureType::Eoa.is_proxy());
        assert!(SignatureType::PolyProxy.is_proxy());
        assert!(SignatureType::PolyGnosisSafe.is_proxy());
    }

    #[test]
    fn tick_size_from_str() {
        assert_eq!(TickSize::try_from("0.1").unwrap(), TickSize::Tenth);
        assert_eq!(TickSize::try_from("0.01").unwrap(), TickSize::Hundredth);
        assert_eq!(TickSize::try_from("0.001").unwrap(), TickSize::Thousandth);
        assert_eq!(
            TickSize::try_from("0.0001").unwrap(),
            TickSize::TenThousandth
        );
    }

    #[test]
    fn tick_size_from_str_rejects_invalid() {
        assert!(TickSize::try_from("0.5").is_err());
        assert!(TickSize::try_from("1.0").is_err());
        assert!(TickSize::try_from("abc").is_err());
        assert!(TickSize::try_from("0.00001").is_err());
    }

    #[test]
    fn tick_size_from_f64() {
        assert_eq!(TickSize::try_from(0.1).unwrap(), TickSize::Tenth);
        assert_eq!(TickSize::try_from(0.01).unwrap(), TickSize::Hundredth);
        assert_eq!(TickSize::try_from(0.001).unwrap(), TickSize::Thousandth);
        assert_eq!(TickSize::try_from(0.0001).unwrap(), TickSize::TenThousandth);
    }

    #[test]
    fn tick_size_from_f64_rejects_invalid() {
        assert!(TickSize::try_from(0.5).is_err());
        assert!(TickSize::try_from(0.0).is_err());
        assert!(TickSize::try_from(1.0).is_err());
    }

    #[test]
    fn tick_size_as_f64() {
        assert!((TickSize::Tenth.as_f64() - 0.1).abs() < f64::EPSILON);
        assert!((TickSize::Hundredth.as_f64() - 0.01).abs() < f64::EPSILON);
        assert!((TickSize::Thousandth.as_f64() - 0.001).abs() < f64::EPSILON);
        assert!((TickSize::TenThousandth.as_f64() - 0.0001).abs() < f64::EPSILON);
    }

    #[test]
    fn tick_size_decimals() {
        assert_eq!(TickSize::Tenth.decimals(), 1);
        assert_eq!(TickSize::Hundredth.decimals(), 2);
        assert_eq!(TickSize::Thousandth.decimals(), 3);
        assert_eq!(TickSize::TenThousandth.decimals(), 4);
    }

    #[test]
    fn tick_size_from_str_trait() {
        let ts: TickSize = "0.01".parse().unwrap();
        assert_eq!(ts, TickSize::Hundredth);
    }

    #[test]
    fn parse_tick_size_error_display() {
        let err = TickSize::try_from("bad").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("bad"),
            "Error should contain invalid value: {}",
            msg
        );
        assert!(
            msg.contains("0.1"),
            "Error should list valid values: {}",
            msg
        );
    }

    #[test]
    fn order_neg_risk_skipped_in_serialization() {
        let order = Order {
            salt: "1".to_string(),
            maker: Address::ZERO,
            signer: Address::ZERO,
            taker: Address::ZERO,
            token_id: "1".to_string(),
            maker_amount: "1".to_string(),
            taker_amount: "1".to_string(),
            expiration: "0".to_string(),
            nonce: "0".to_string(),
            fee_rate_bps: "0".to_string(),
            side: OrderSide::Buy,
            signature_type: SignatureType::Eoa,
            neg_risk: true,
        };
        let json = serde_json::to_value(&order).unwrap();
        assert!(
            json.get("neg_risk").is_none() && json.get("negRisk").is_none(),
            "neg_risk should be skipped in serialization: {}",
            json
        );
    }

    #[test]
    fn salt_serialized_as_number() {
        let order = Order {
            salt: "12345678901234567890".to_string(),
            maker: Address::ZERO,
            signer: Address::ZERO,
            taker: Address::ZERO,
            token_id: "1".to_string(),
            maker_amount: "1".to_string(),
            taker_amount: "1".to_string(),
            expiration: "0".to_string(),
            nonce: "0".to_string(),
            fee_rate_bps: "0".to_string(),
            side: OrderSide::Buy,
            signature_type: SignatureType::Eoa,
            neg_risk: false,
        };
        let json = serde_json::to_value(&order).unwrap();
        // Salt should be serialized as a number, not a string
        assert!(
            json["salt"].is_number(),
            "Salt should be a number: {:?}",
            json["salt"]
        );
    }
}
