//! Vocabulary and response rows for USDⓈ-M futures.
//!
//! Prices, quantities and rates are [`Decimal`],
//! decoded from the decimal strings Binance sends, so no value passes through
//! an `f64`. Timestamps are Unix milliseconds. Where the wire uses one-letter
//! keys (`aggTrades`, `depth`), fields take spelled-out names through serde
//! renames.

use std::{fmt, str::FromStr};

use rust_decimal::Decimal;
use serde::{
    de::{self, DeserializeOwned, IgnoredAny, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_json::Value;

/// A Binance symbol such as `BTCUSDT`, `BTCUSDT_261225` or `币安人生USDT`.
///
/// [`Symbol::new`] accepts 1 to 32 characters, each a Unicode letter, a digit
/// or `_`. Binance lists Chinese-character symbols, and quarterly contracts
/// carry their delivery date after an underscore; on 2026-10-07 the 924 listed
/// symbols used no other character and the longest had 17. ASCII letters are
/// uppercased: no listed symbol has a lowercase one, stream names spell every
/// symbol in lowercase, and uppercasing is what lets a stream name be parsed
/// back to the symbol that built it.
///
/// Response rows carry symbols as `String`, taken as sent, so a symbol this
/// type would refuse never fails a whole response.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(String);

/// A string [`Symbol::new`] refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a Binance symbol: 1 to 32 letters, digits or underscores")]
pub struct InvalidSymbol(pub String);

/// An `InvalidRequest`: the caller named a symbol no request can carry.
impl polyoxide_venue::Classify for InvalidSymbol {
    fn class(&self) -> polyoxide_venue::Class {
        polyoxide_venue::Class::InvalidRequest
    }
}

impl Symbol {
    /// The longest symbol [`Symbol::new`] accepts, in characters.
    pub const MAX_LEN: usize = 32;

    /// Checks a symbol and uppercases its ASCII letters.
    pub fn new(symbol: impl Into<String>) -> Result<Self, InvalidSymbol> {
        let symbol = symbol.into();
        let chars = symbol.chars().count();
        let valid = (1..=Self::MAX_LEN).contains(&chars)
            && symbol.chars().all(|c| c.is_alphanumeric() || c == '_');
        if valid {
            Ok(Self(symbol.to_ascii_uppercase()))
        } else {
            Err(InvalidSymbol(symbol))
        }
    }

    /// The symbol, as REST spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Symbol {
    type Err = InvalidSymbol;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A string that is not one of a closed set's wire spellings.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{value:?} is not a valid {type_name}")]
pub struct UnknownVariant {
    /// The Rust type being parsed.
    pub type_name: &'static str,
    /// The offending input.
    pub value: String,
}

/// An `InvalidRequest`: the caller parsed a spelling no variant has. Only
/// `FromStr` builds one, on a value the caller supplies; a stream name in a
/// server frame that does not parse becomes a `UsdmWsError::Frame` instead.
impl polyoxide_venue::Classify for UnknownVariant {
    fn class(&self) -> polyoxide_venue::Class {
        polyoxide_venue::Class::InvalidRequest
    }
}

/// A closed set the client sends: one wire spelling per variant, and an `ALL`
/// table so a test can walk every spelling.
macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $( $(#[$vmeta])* #[serde(rename = $wire)] $variant, )+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [$name] = &[$( $name::$variant, )+];

            /// The wire spelling.
            pub fn as_str(self) -> &'static str {
                match self { $( $name::$variant => $wire, )+ }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = UnknownVariant;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $( $wire => Ok($name::$variant), )+
                    _ => Err(UnknownVariant { type_name: stringify!($name), value: s.to_owned() }),
                }
            }
        }
    };
}

/// A set Binance reports and extends over time: a value this version does not
/// know is kept verbatim in `Other` instead of failing the response.
macro_rules! open_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
            /// A value this version of the SDK does not recognise, kept verbatim.
            Other(String),
        }

        impl $name {
            /// Every variant this SDK knows, in declaration order.
            pub const ALL: &'static [Self] = &[$( Self::$variant ),+];

            /// The wire spelling.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $wire, )+
                    Self::Other(raw) => raw,
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = std::convert::Infallible;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(match s {
                    $( $wire => Self::$variant, )+
                    other => Self::Other(other.to_owned()),
                })
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                let Ok(value) = raw.parse();
                Ok(value)
            }
        }
    };
}

wire_enum! {
    /// Kline width, shared by REST `klines` and the kline stream. The docs'
    /// list also has `1s`, which this host refuses (`-1120`).
    Interval {
        M1 => "1m", M3 => "3m", M5 => "5m", M15 => "15m", M30 => "30m",
        H1 => "1h", H2 => "2h", H4 => "4h", H6 => "6h", H8 => "8h", H12 => "12h",
        D1 => "1d", D3 => "3d", W1 => "1w",
        /// One calendar month.
        Mo1 => "1M",
    }
}

wire_enum! {
    /// Levels per side `depth` can return. Any other `limit` is refused
    /// (`-4021`).
    DepthLimit {
        Five => "5", Ten => "10", Twenty => "20", Fifty => "50",
        Hundred => "100", FiveHundred => "500", Thousand => "1000",
    }
}

open_enum! {
    /// A contract's type, from `exchangeInfo`.
    ContractType {
        Perpetual => "PERPETUAL",
        /// A perpetual on a traditional-finance underlying (equities, metals, FX).
        TradifiPerpetual => "TRADIFI_PERPETUAL",
        CurrentMonth => "CURRENT_MONTH",
        NextMonth => "NEXT_MONTH",
        CurrentQuarter => "CURRENT_QUARTER",
        NextQuarter => "NEXT_QUARTER",
        PerpetualDelivering => "PERPETUAL_DELIVERING",
    }
}

open_enum! {
    /// A contract's status, from `exchangeInfo`. The variants are the list in
    /// Binance's common definitions; `exchangeInfo` used three of them on
    /// 2026-10-07.
    SymbolStatus {
        PendingTrading => "PENDING_TRADING",
        Trading => "TRADING",
        PreDelivering => "PRE_DELIVERING",
        Delivering => "DELIVERING",
        Delivered => "DELIVERED",
        PreSettle => "PRE_SETTLE",
        /// A delisted perpetual. 134 of 924 symbols on 2026-10-07.
        Settling => "SETTLING",
        Close => "CLOSE",
        TradingHalt => "TRADING_HALT",
        TradingCancelOnly => "TRADING_CANCEL_ONLY",
    }
}

open_enum! {
    /// What a contract's underlying is, from `exchangeInfo`. The values seen on
    /// 2026-10-07; Binance's docs give no list.
    UnderlyingType {
        Coin => "COIN",
        Index => "INDEX",
        Premarket => "PREMARKET",
        Commodity => "COMMODITY",
        Equity => "EQUITY",
        CnEquity => "CN_EQUITY",
        HkEquity => "HK_EQUITY",
        KrEquity => "KR_EQUITY",
        Fx => "FX",
    }
}

/// `serde(with)` for a decimal string that is empty when the value is unknown.
///
/// `fundingRate` sends `"markPrice": ""` for funding events before about 2022,
/// so a plain `Decimal` would fail a backfill on its first old page.
mod decimal_or_empty {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &Option<Decimal>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.collect_str(value),
            None => serializer.serialize_str(""),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Decimal>, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw.is_empty() {
            return Ok(None);
        }
        raw.parse().map(Some).map_err(de::Error::custom)
    }
}

/// `GET /fapi/v1/time`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ServerTime {
    /// Server clock, Unix milliseconds.
    pub server_time: u64,
}

/// `GET /fapi/v1/exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ExchangeInfo {
    /// Always `UTC`.
    pub timezone: String,
    /// Server clock, Unix milliseconds.
    pub server_time: u64,
    /// `U_MARGINED` on this host.
    pub futures_type: String,
    /// The IP's request-weight and order limits.
    pub rate_limits: Vec<RateLimit>,
    /// Exchange-wide filters; empty on 2026-10-07.
    pub exchange_filters: Vec<Filter>,
    /// Margin assets.
    pub assets: Vec<AssetInfo>,
    /// Every listed contract, including delisted (`SETTLING`) ones.
    pub symbols: Vec<SymbolInfo>,
}

/// One row of `exchangeInfo`'s `rateLimits`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct RateLimit {
    /// `REQUEST_WEIGHT` or `ORDERS`.
    pub rate_limit_type: String,
    /// `MINUTE` or `SECOND`.
    pub interval: String,
    /// Intervals per window.
    pub interval_num: u32,
    /// The limit per window.
    pub limit: u32,
}

/// One margin asset in `exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AssetInfo {
    /// Asset name, such as `USDT`.
    pub asset: String,
    /// Whether the asset can be margin.
    pub margin_available: bool,
    /// Binance's auto-exchange threshold.
    #[serde(with = "rust_decimal::serde::str")]
    pub auto_asset_exchange: Decimal,
}

/// One contract in `exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SymbolInfo {
    /// The contract's symbol.
    pub symbol: String,
    /// The underlying pair, `BTCUSDT` for `BTCUSDT_261225`.
    pub pair: String,
    /// Perpetual, TradFi perpetual or a delivery contract.
    pub contract_type: ContractType,
    /// Delivery time; far in the future for a perpetual.
    pub delivery_date: u64,
    /// Listing time.
    pub onboard_date: u64,
    /// Trading, settling (delisted) and so on.
    pub status: SymbolStatus,
    /// Ignore; Binance documents it so.
    #[serde(with = "rust_decimal::serde::str")]
    pub maint_margin_percent: Decimal,
    /// Ignore; Binance documents it so.
    #[serde(with = "rust_decimal::serde::str")]
    pub required_margin_percent: Decimal,
    /// Base asset.
    pub base_asset: String,
    /// Quote asset.
    pub quote_asset: String,
    /// Margin asset.
    pub margin_asset: String,
    /// Decimal places in a price. Use `PRICE_FILTER`'s tick size to round.
    pub price_precision: u32,
    /// Decimal places in a quantity. Use `LOT_SIZE`'s step size to round.
    pub quantity_precision: u32,
    /// Decimal places of the base asset.
    pub base_asset_precision: u32,
    /// Decimal places of the quote asset.
    pub quote_precision: u32,
    /// What the underlying is.
    pub underlying_type: UnderlyingType,
    /// Free-text tags such as `Layer-2` or `TradFi`.
    pub underlying_sub_type: Vec<String>,
    /// Threshold for algo orders with `priceProtect`.
    #[serde(with = "rust_decimal::serde::str")]
    pub trigger_protect: Decimal,
    /// Liquidation fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub liquidation_fee: Decimal,
    /// The most a market order may deviate from the mark price.
    #[serde(with = "rust_decimal::serde::str")]
    pub market_take_bound: Decimal,
    /// Most orders a move-order request may touch.
    pub max_move_order_limit: u32,
    /// Order filters.
    pub filters: Vec<Filter>,
    /// Order types the contract accepts.
    pub order_types: Vec<String>,
    /// Times in force the contract accepts.
    pub time_in_force: Vec<String>,
    /// Trading products the contract is open to (`GRID`, `COPY`, …).
    pub permission_sets: Vec<String>,
}

/// An `exchangeInfo` filter, named after its wire `filterType`.
///
/// A filter type this version does not know is kept whole in [`Filter::Other`],
/// so a new one never fails the response. A known type that loses or retypes a
/// field still fails; one that gains a field decodes without it, and the
/// wire-agreement tests are what notice.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Filter {
    /// `PRICE_FILTER`.
    #[non_exhaustive]
    PriceFilter {
        /// Lowest price.
        min_price: Decimal,
        /// Highest price.
        max_price: Decimal,
        /// Price increment.
        tick_size: Decimal,
    },
    /// `LOT_SIZE`: limit orders.
    #[non_exhaustive]
    LotSize {
        /// Smallest quantity.
        min_qty: Decimal,
        /// Largest quantity.
        max_qty: Decimal,
        /// Quantity increment.
        step_size: Decimal,
    },
    /// `MARKET_LOT_SIZE`: market orders.
    #[non_exhaustive]
    MarketLotSize {
        /// Smallest quantity.
        min_qty: Decimal,
        /// Largest quantity.
        max_qty: Decimal,
        /// Quantity increment.
        step_size: Decimal,
    },
    /// `MAX_NUM_ORDERS`.
    #[non_exhaustive]
    MaxNumOrders {
        /// Most open orders.
        limit: u32,
    },
    /// `MIN_NOTIONAL`.
    #[non_exhaustive]
    MinNotional {
        /// Smallest order value.
        notional: Decimal,
    },
    /// `PERCENT_PRICE`.
    #[non_exhaustive]
    PercentPrice {
        /// Highest price as a multiple of the mark price.
        multiplier_up: Decimal,
        /// Lowest price as a multiple of the mark price.
        multiplier_down: Decimal,
        /// Decimal places of the multipliers.
        multiplier_decimal: Decimal,
    },
    /// `POSITION_RISK_CONTROL`.
    #[non_exhaustive]
    PositionRiskControl {
        /// Which side's position is controlled, such as `NONE`.
        position_control_side: String,
    },
    /// A filter type this version does not model.
    Other {
        /// The wire `filterType`.
        filter_type: String,
        /// The whole filter object, `filterType` included.
        raw: Value,
    },
}

/// The wire bodies of the known filters, which `Filter`'s serde goes through.
mod filter_wire {
    use super::*;

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Price {
        #[serde(with = "rust_decimal::serde::str")]
        pub min_price: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub max_price: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub tick_size: Decimal,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Lot {
        #[serde(with = "rust_decimal::serde::str")]
        pub min_qty: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub max_qty: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub step_size: Decimal,
    }

    #[derive(Serialize, Deserialize)]
    pub struct MaxNumOrders {
        pub limit: u32,
    }

    #[derive(Serialize, Deserialize)]
    pub struct MinNotional {
        #[serde(with = "rust_decimal::serde::str")]
        pub notional: Decimal,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PercentPrice {
        #[serde(with = "rust_decimal::serde::str")]
        pub multiplier_up: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub multiplier_down: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub multiplier_decimal: Decimal,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PositionRiskControl {
        pub position_control_side: String,
    }
}

impl<'de> Deserialize<'de> for Filter {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use filter_wire as wire;

        fn parse<T: DeserializeOwned, E: de::Error>(raw: Value) -> Result<T, E> {
            serde_json::from_value(raw).map_err(E::custom)
        }

        let raw = Value::deserialize(deserializer)?;
        let filter_type = raw
            .get("filterType")
            .and_then(Value::as_str)
            .ok_or_else(|| de::Error::custom("a filter without a string filterType"))?
            .to_owned();
        Ok(match filter_type.as_str() {
            "PRICE_FILTER" => {
                let f: wire::Price = parse(raw)?;
                Self::PriceFilter {
                    min_price: f.min_price,
                    max_price: f.max_price,
                    tick_size: f.tick_size,
                }
            }
            "LOT_SIZE" => {
                let f: wire::Lot = parse(raw)?;
                Self::LotSize {
                    min_qty: f.min_qty,
                    max_qty: f.max_qty,
                    step_size: f.step_size,
                }
            }
            "MARKET_LOT_SIZE" => {
                let f: wire::Lot = parse(raw)?;
                Self::MarketLotSize {
                    min_qty: f.min_qty,
                    max_qty: f.max_qty,
                    step_size: f.step_size,
                }
            }
            "MAX_NUM_ORDERS" => {
                let f: wire::MaxNumOrders = parse(raw)?;
                Self::MaxNumOrders { limit: f.limit }
            }
            "MIN_NOTIONAL" => {
                let f: wire::MinNotional = parse(raw)?;
                Self::MinNotional {
                    notional: f.notional,
                }
            }
            "PERCENT_PRICE" => {
                let f: wire::PercentPrice = parse(raw)?;
                Self::PercentPrice {
                    multiplier_up: f.multiplier_up,
                    multiplier_down: f.multiplier_down,
                    multiplier_decimal: f.multiplier_decimal,
                }
            }
            "POSITION_RISK_CONTROL" => {
                let f: wire::PositionRiskControl = parse(raw)?;
                Self::PositionRiskControl {
                    position_control_side: f.position_control_side,
                }
            }
            _ => Self::Other { filter_type, raw },
        })
    }
}

impl Serialize for Filter {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use filter_wire as wire;

        fn tagged<T: Serialize, E: serde::ser::Error>(
            filter_type: &str,
            body: &T,
        ) -> Result<Value, E> {
            let mut value = serde_json::to_value(body).map_err(E::custom)?;
            if let Value::Object(map) = &mut value {
                map.insert(
                    "filterType".to_owned(),
                    Value::String(filter_type.to_owned()),
                );
            }
            Ok(value)
        }

        let value = match self {
            Self::PriceFilter {
                min_price,
                max_price,
                tick_size,
            } => tagged(
                "PRICE_FILTER",
                &wire::Price {
                    min_price: *min_price,
                    max_price: *max_price,
                    tick_size: *tick_size,
                },
            )?,
            Self::LotSize {
                min_qty,
                max_qty,
                step_size,
            } => tagged(
                "LOT_SIZE",
                &wire::Lot {
                    min_qty: *min_qty,
                    max_qty: *max_qty,
                    step_size: *step_size,
                },
            )?,
            Self::MarketLotSize {
                min_qty,
                max_qty,
                step_size,
            } => tagged(
                "MARKET_LOT_SIZE",
                &wire::Lot {
                    min_qty: *min_qty,
                    max_qty: *max_qty,
                    step_size: *step_size,
                },
            )?,
            Self::MaxNumOrders { limit } => {
                tagged("MAX_NUM_ORDERS", &wire::MaxNumOrders { limit: *limit })?
            }
            Self::MinNotional { notional } => tagged(
                "MIN_NOTIONAL",
                &wire::MinNotional {
                    notional: *notional,
                },
            )?,
            Self::PercentPrice {
                multiplier_up,
                multiplier_down,
                multiplier_decimal,
            } => tagged(
                "PERCENT_PRICE",
                &wire::PercentPrice {
                    multiplier_up: *multiplier_up,
                    multiplier_down: *multiplier_down,
                    multiplier_decimal: *multiplier_decimal,
                },
            )?,
            Self::PositionRiskControl {
                position_control_side,
            } => tagged(
                "POSITION_RISK_CONTROL",
                &wire::PositionRiskControl {
                    position_control_side: position_control_side.clone(),
                },
            )?,
            Self::Other { raw, .. } => raw.clone(),
        };
        value.serialize(serializer)
    }
}

/// One row of `GET /fapi/v1/fundingInfo`: the symbols whose funding cap,
/// floor or interval was adjusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct FundingInfo {
    /// The contract. This host's list also carries COIN-M perpetuals
    /// (`BTCUSD_PERP`), which `exchangeInfo` here does not.
    pub symbol: String,
    /// Highest funding rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub adjusted_funding_rate_cap: Decimal,
    /// Lowest funding rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub adjusted_funding_rate_floor: Decimal,
    /// Hours between funding events.
    pub funding_interval_hours: u32,
    /// Whether Binance shows a disclaimer for the contract.
    pub disclaimer: bool,
    /// When the adjustment was made. `null` on 57 of 805 rows on 2026-10-07,
    /// `BTCUSDT` among them.
    pub update_time: Option<u64>,
}

/// `GET /fapi/v1/ticker/24hr`: rolling 24-hour statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Ticker24h {
    /// The contract.
    pub symbol: String,
    /// Last price less the open price.
    #[serde(with = "rust_decimal::serde::str")]
    pub price_change: Decimal,
    /// The change as a percentage.
    #[serde(with = "rust_decimal::serde::str")]
    pub price_change_percent: Decimal,
    /// Volume-weighted average price.
    #[serde(with = "rust_decimal::serde::str")]
    pub weighted_avg_price: Decimal,
    /// Last traded price.
    #[serde(with = "rust_decimal::serde::str")]
    pub last_price: Decimal,
    /// Last traded quantity.
    #[serde(with = "rust_decimal::serde::str")]
    pub last_qty: Decimal,
    /// Price 24 hours ago.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_price: Decimal,
    /// Highest price.
    #[serde(with = "rust_decimal::serde::str")]
    pub high_price: Decimal,
    /// Lowest price.
    #[serde(with = "rust_decimal::serde::str")]
    pub low_price: Decimal,
    /// Base-asset volume.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Quote-asset volume.
    #[serde(with = "rust_decimal::serde::str")]
    pub quote_volume: Decimal,
    /// Window start.
    pub open_time: u64,
    /// Window end.
    pub close_time: u64,
    /// First trade id in the window.
    pub first_id: i64,
    /// Last trade id in the window.
    pub last_id: i64,
    /// Trades in the window.
    pub count: u64,
}

/// `GET /fapi/v1/premiumIndex`: mark price, index price and funding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PremiumIndex {
    /// The contract.
    pub symbol: String,
    /// Mark price.
    #[serde(with = "rust_decimal::serde::str")]
    pub mark_price: Decimal,
    /// Index price.
    #[serde(with = "rust_decimal::serde::str")]
    pub index_price: Decimal,
    /// Estimated settle price, meaningful only in the hour before a settlement.
    #[serde(with = "rust_decimal::serde::str")]
    pub estimated_settle_price: Decimal,
    /// The funding rate of the current period.
    #[serde(with = "rust_decimal::serde::str")]
    pub last_funding_rate: Decimal,
    /// Interest rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub interest_rate: Decimal,
    /// Next funding event, or `0` for a contract with none scheduled.
    pub next_funding_time: u64,
    /// When the values were computed.
    pub time: u64,
}

/// One candle of `GET /fapi/v1/klines`, a positional array on the wire.
///
/// The wire sends a twelfth element Binance documents as "ignore"; it is
/// dropped, and any further elements are tolerated.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Kline {
    /// Candle start.
    pub open_time: u64,
    /// Open price.
    pub open: Decimal,
    /// High price.
    pub high: Decimal,
    /// Low price.
    pub low: Decimal,
    /// Close price.
    pub close: Decimal,
    /// Base-asset volume.
    pub volume: Decimal,
    /// Candle end, inclusive.
    pub close_time: u64,
    /// Quote-asset volume.
    pub quote_volume: Decimal,
    /// Trades in the candle.
    pub trade_count: u64,
    /// Base-asset volume bought by takers.
    pub taker_buy_base_volume: Decimal,
    /// Quote-asset volume bought by takers.
    pub taker_buy_quote_volume: Decimal,
}

/// A decimal string inside a positional array.
struct DecimalStr(Decimal);

impl<'de> Deserialize<'de> for DecimalStr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        rust_decimal::serde::str::deserialize(deserializer).map(Self)
    }
}

fn next<'de, T: Deserialize<'de>, A: SeqAccess<'de>>(
    seq: &mut A,
    index: usize,
    what: &str,
) -> Result<T, A::Error> {
    seq.next_element()?
        .ok_or_else(|| de::Error::invalid_length(index, &what))
}

fn drain<'de, A: SeqAccess<'de>>(seq: &mut A) -> Result<(), A::Error> {
    while seq.next_element::<IgnoredAny>()?.is_some() {}
    Ok(())
}

impl<'de> Deserialize<'de> for Kline {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KlineVisitor;

        impl<'de> Visitor<'de> for KlineVisitor {
            type Value = Kline;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a kline array of at least 11 elements")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Kline, A::Error> {
                const WHAT: &str = "a kline array of at least 11 elements";
                let kline = Kline {
                    open_time: next(&mut seq, 0, WHAT)?,
                    open: next::<DecimalStr, _>(&mut seq, 1, WHAT)?.0,
                    high: next::<DecimalStr, _>(&mut seq, 2, WHAT)?.0,
                    low: next::<DecimalStr, _>(&mut seq, 3, WHAT)?.0,
                    close: next::<DecimalStr, _>(&mut seq, 4, WHAT)?.0,
                    volume: next::<DecimalStr, _>(&mut seq, 5, WHAT)?.0,
                    close_time: next(&mut seq, 6, WHAT)?,
                    quote_volume: next::<DecimalStr, _>(&mut seq, 7, WHAT)?.0,
                    trade_count: next(&mut seq, 8, WHAT)?,
                    taker_buy_base_volume: next::<DecimalStr, _>(&mut seq, 9, WHAT)?.0,
                    taker_buy_quote_volume: next::<DecimalStr, _>(&mut seq, 10, WHAT)?.0,
                };
                drain(&mut seq)?;
                Ok(kline)
            }
        }

        deserializer.deserialize_seq(KlineVisitor)
    }
}

impl Serialize for Kline {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(11))?;
        seq.serialize_element(&self.open_time)?;
        for value in [&self.open, &self.high, &self.low, &self.close, &self.volume] {
            seq.serialize_element(&value.to_string())?;
        }
        seq.serialize_element(&self.close_time)?;
        seq.serialize_element(&self.quote_volume.to_string())?;
        seq.serialize_element(&self.trade_count)?;
        seq.serialize_element(&self.taker_buy_base_volume.to_string())?;
        seq.serialize_element(&self.taker_buy_quote_volume.to_string())?;
        seq.end()
    }
}

/// One row of `GET /fapi/v1/fundingRate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct FundingRate {
    /// The contract.
    pub symbol: String,
    /// When funding was paid.
    pub funding_time: u64,
    /// The rate paid.
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_rate: Decimal,
    /// The mark price at funding. `None` for events before about 2022, which
    /// the wire sends as `""`.
    #[serde(with = "decimal_or_empty")]
    pub mark_price: Option<Decimal>,
    /// `Regular` on every row seen on 2026-10-07.
    pub rate_type: String,
}

/// `GET /fapi/v1/openInterest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct OpenInterest {
    /// The contract.
    pub symbol: String,
    /// Open interest in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest: Decimal,
    /// When it was computed.
    pub time: u64,
}

/// One row of `GET /fapi/v1/aggTrades`: trades at one price, taken by one
/// order, merged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AggTrade {
    /// Aggregate trade id.
    #[serde(rename = "a")]
    pub id: u64,
    /// Price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Quantity without the trades involving RPI (Retail Price Improvement)
    /// orders, as Binance documents `nq`.
    #[serde(rename = "nq", with = "rust_decimal::serde::str")]
    pub normal_quantity: Decimal,
    /// First trade id merged.
    #[serde(rename = "f")]
    pub first_trade_id: u64,
    /// Last trade id merged.
    #[serde(rename = "l")]
    pub last_trade_id: u64,
    /// Trade time.
    #[serde(rename = "T")]
    pub time: u64,
    /// Whether the buyer was the maker, so the taker sold.
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// `GET /fapi/v1/depth`: an order book snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Depth {
    /// Book update id of the snapshot.
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    /// When the message was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// When the book was last changed.
    #[serde(rename = "T")]
    pub transaction_time: u64,
    /// Bids, best first.
    pub bids: Vec<Level>,
    /// Asks, best first.
    pub asks: Vec<Level>,
}

/// A price level, a `[price, quantity]` pair on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Level {
    /// Price.
    pub price: Decimal,
    /// Quantity at the price.
    pub quantity: Decimal,
}

impl Level {
    /// A level from a price and a quantity.
    pub fn new(price: Decimal, quantity: Decimal) -> Self {
        Self { price, quantity }
    }
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct LevelVisitor;

        impl<'de> Visitor<'de> for LevelVisitor {
            type Value = Level;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a [price, quantity] array")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Level, A::Error> {
                const WHAT: &str = "a [price, quantity] array";
                let level = Level {
                    price: next::<DecimalStr, _>(&mut seq, 0, WHAT)?.0,
                    quantity: next::<DecimalStr, _>(&mut seq, 1, WHAT)?.0,
                };
                drain(&mut seq)?;
                Ok(level)
            }
        }

        deserializer.deserialize_seq(LevelVisitor)
    }
}

impl Serialize for Level {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.price.to_string())?;
        seq.serialize_element(&self.quantity.to_string())?;
        seq.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_symbol_and_an_unknown_spelling_are_invalid_requests() {
        use polyoxide_venue::{Class, Classify};

        let symbol = Symbol::new("BTC USDT").unwrap_err();
        assert_eq!(symbol.class(), Class::InvalidRequest);
        assert!(symbol.is_fault());
        assert_eq!(symbol.retry_after(), None);
        assert!(!symbol.is_retriable());

        let spelling = UnknownVariant {
            type_name: "Interval",
            value: "2m".into(),
        };
        assert_eq!(spelling.class(), Class::InvalidRequest);
        assert!(spelling.is_fault());
        assert_eq!(spelling.retry_after(), None);
        assert!(!spelling.is_retriable());
    }

    #[test]
    fn symbols_take_letters_digits_and_underscores_in_any_script() {
        for good in ["BTCUSDT", "BTCUSDT_261225", "币安人生USDT", "1000PEPEUSDT"] {
            assert_eq!(Symbol::new(good).unwrap().as_str(), good);
        }
        for bad in [
            "",
            "BTC USDT",
            "BTC@USDT",
            "BTC/USDT",
            "BTC-USDT",
            &"A".repeat(33),
        ] {
            assert_eq!(
                Symbol::new(bad),
                Err(InvalidSymbol(bad.to_owned())),
                "{bad:?}"
            );
        }
        assert!(Symbol::new("A".repeat(32)).is_ok());
    }

    #[test]
    fn ascii_letters_are_uppercased_and_nothing_else_changes() {
        assert_eq!(
            Symbol::new("btcusdt").unwrap(),
            Symbol::new("BTCUSDT").unwrap()
        );
        assert_eq!(
            Symbol::new("btcusdt_261225").unwrap().as_str(),
            "BTCUSDT_261225"
        );
        assert_eq!(
            Symbol::new("币安人生usdt").unwrap().as_str(),
            "币安人生USDT"
        );
    }

    #[test]
    fn symbols_are_measured_in_characters_and_only_ascii_changes_case() {
        // `to_uppercase` would turn ß into SS, and `len` would count bytes.
        assert_eq!(Symbol::new("straße").unwrap().as_str(), "STRAßE");
        assert!(Symbol::new("币".repeat(32)).is_ok());
        assert!(Symbol::new("币".repeat(33)).is_err());
    }

    #[test]
    fn wire_spellings_are_binance_s() {
        // `as_str` and `from_str` share each literal, so a round trip cannot
        // see a wrong one; these are the spellings the host accepted.
        let intervals: Vec<&str> = Interval::ALL.iter().map(|i| i.as_str()).collect();
        assert_eq!(
            intervals,
            [
                "1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "6h", "8h", "12h", "1d", "3d",
                "1w", "1M"
            ]
        );
        let limits: Vec<&str> = DepthLimit::ALL.iter().map(|l| l.as_str()).collect();
        assert_eq!(limits, ["5", "10", "20", "50", "100", "500", "1000"]);
    }

    #[test]
    fn every_interval_round_trips_its_wire_spelling() {
        assert_eq!(Interval::ALL.len(), 15);
        for interval in Interval::ALL {
            assert_eq!(interval.as_str().parse::<Interval>(), Ok(*interval));
        }
        assert!("1s".parse::<Interval>().is_err(), "this host refuses 1s");
    }

    #[test]
    fn an_unknown_contract_type_is_kept_verbatim() {
        let parsed: ContractType = serde_json::from_str(r#""NEXT_DECADE""#).unwrap();
        assert_eq!(parsed, ContractType::Other("NEXT_DECADE".to_owned()));
        assert_eq!(serde_json::to_string(&parsed).unwrap(), r#""NEXT_DECADE""#);
        let known: ContractType = serde_json::from_str(r#""TRADIFI_PERPETUAL""#).unwrap();
        assert_eq!(known, ContractType::TradifiPerpetual);
    }

    #[test]
    fn an_unseen_filter_type_is_kept_and_the_known_ones_parse() {
        let filters: Vec<Filter> = serde_json::from_str(
            r#"[{"filterType":"PRICE_FILTER","minPrice":"261.10","maxPrice":"809484","tickSize":"0.10"},
                {"filterType":"MAX_NUM_ICEBERG_ORDERS","maxNumIcebergOrders":5}]"#,
        )
        .unwrap();
        assert!(
            matches!(&filters[0], Filter::PriceFilter { tick_size, .. } if *tick_size == Decimal::new(10, 2))
        );
        let Filter::Other { filter_type, raw } = &filters[1] else {
            panic!("an unseen filter type must not fail the response");
        };
        assert_eq!(filter_type, "MAX_NUM_ICEBERG_ORDERS");
        assert_eq!(raw["maxNumIcebergOrders"], 5);
        let again: Vec<Value> =
            serde_json::from_value(serde_json::to_value(&filters).unwrap()).unwrap();
        assert_eq!(again[0]["filterType"], "PRICE_FILTER");
        assert_eq!(again[1]["maxNumIcebergOrders"], 5);
    }

    #[test]
    fn a_known_filter_with_a_missing_field_fails_loudly() {
        let err = serde_json::from_str::<Filter>(r#"{"filterType":"LOT_SIZE","minQty":"0.001"}"#)
            .unwrap_err();
        assert!(err.to_string().contains("maxQty"), "{err}");
    }

    #[test]
    fn a_kline_drops_the_ignored_element_and_tolerates_more() {
        let wire = r#"[1791354000000,"84125.70","84125.70","84099.90","84106.90","214.561",1791354059999,"18046628.85200",2912,"38.078","3202701.86390","0","extra"]"#;
        let kline: Kline = serde_json::from_str(wire).unwrap();
        assert_eq!(kline.close, "84106.90".parse::<Decimal>().unwrap());
        assert_eq!(kline.trade_count, 2912);
        assert_eq!(
            serde_json::to_string(&kline).unwrap(),
            r#"[1791354000000,"84125.70","84125.70","84099.90","84106.90","214.561",1791354059999,"18046628.85200",2912,"38.078","3202701.86390"]"#
        );
        assert!(serde_json::from_str::<Kline>("[1791354000000]").is_err());
    }

    #[test]
    fn a_level_keeps_every_digit() {
        // More significant digits than an f64 holds.
        let level: Level = serde_json::from_str(r#"["84141.123456789012345","14.663"]"#).unwrap();
        assert_eq!(level.price.to_string(), "84141.123456789012345");
        assert_eq!(
            serde_json::to_string(&level).unwrap(),
            r#"["84141.123456789012345","14.663"]"#
        );
    }

    #[test]
    fn an_old_funding_rate_has_no_mark_price() {
        // Captured 2026-10-07: `fundingRate?symbol=BTCUSDT&startTime=1568102400000`.
        let old: FundingRate = serde_json::from_str(
            r#"{"symbol":"BTCUSDT","fundingTime":1568102400000,"fundingRate":"0.00010000","markPrice":"","rateType":"Regular"}"#,
        )
        .unwrap();
        assert_eq!(old.mark_price, None);
        assert_eq!(serde_json::to_value(&old).unwrap()["markPrice"], "");

        let new: FundingRate = serde_json::from_str(
            r#"{"symbol":"BTCUSDT","fundingTime":1791273600001,"fundingRate":"-0.00001592","markPrice":"85514.00007496","rateType":"Regular"}"#,
        )
        .unwrap();
        assert_eq!(new.mark_price, Some("85514.00007496".parse().unwrap()));
    }
}
