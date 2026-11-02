//! Vocabulary and response rows for Binance spot.
//!
//! Prices and quantities are [`Decimal`], decoded from the decimal strings
//! Binance sends, so no value passes through an `f64`. Timestamps are Unix
//! milliseconds. Response rows carry symbols as `String`, taken as sent, so a
//! symbol [`Symbol::new`] would refuse never fails a whole response.

use rust_decimal::Decimal;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub use crate::shared::symbol::{InvalidSymbol, Symbol};

/// `GET /api/v3/time`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ServerTime {
    /// Server clock, Unix milliseconds.
    pub server_time: u64,
}

/// `GET /api/v3/exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ExchangeInfo {
    /// Always `UTC`.
    pub timezone: String,
    /// Server clock, Unix milliseconds.
    pub server_time: u64,
    /// The IP's request-weight, raw-request and order limits.
    pub rate_limits: Vec<RateLimit>,
    /// Exchange-wide filters.
    pub exchange_filters: Vec<Filter>,
    /// Every listed symbol, including ones that are not trading.
    pub symbols: Vec<SymbolInfo>,
    /// Smart order routing: the symbols an order for a base asset may be
    /// routed across. Absent when the host sends none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sors: Option<Vec<Sor>>,
}

/// One row of `exchangeInfo`'s `rateLimits`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct RateLimit {
    /// `REQUEST_WEIGHT`, `RAW_REQUESTS` or `ORDERS`.
    pub rate_limit_type: String,
    /// `SECOND`, `MINUTE` or `DAY`.
    pub interval: String,
    /// Intervals per window.
    pub interval_num: u32,
    /// The limit per window.
    pub limit: u32,
}

/// One smart-order-routing entry in `exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Sor {
    /// The base asset the routing applies to.
    pub base_asset: String,
    /// The symbols an order may be routed across.
    pub symbols: Vec<String>,
}

/// One symbol in `exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SymbolInfo {
    /// The symbol, as Binance spells it.
    pub symbol: String,
    /// Trading, halted and so on.
    pub status: SymbolStatus,
    /// Base asset.
    pub base_asset: String,
    /// Decimal places of the base asset.
    pub base_asset_precision: u32,
    /// Quote asset.
    pub quote_asset: String,
    /// Decimal places of the quote asset, as Binance's older key spells it.
    pub quote_precision: u32,
    /// Decimal places of the quote asset.
    pub quote_asset_precision: u32,
    /// Decimal places of the base asset's commission.
    pub base_commission_precision: u32,
    /// Decimal places of the quote asset's commission.
    pub quote_commission_precision: u32,
    /// Order types the symbol accepts.
    pub order_types: Vec<String>,
    /// Whether iceberg orders are allowed.
    pub iceberg_allowed: bool,
    /// Whether OCO order lists are allowed.
    pub oco_allowed: bool,
    /// Whether OTO order lists are allowed.
    pub oto_allowed: bool,
    /// Whether OPO order lists are allowed. Every captured row carries the flag;
    /// it is optional so a row without it still decodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opo_allowed: Option<bool>,
    /// Whether a market order may name a quote quantity.
    pub quote_order_qty_market_allowed: bool,
    /// Whether trailing stops are allowed.
    pub allow_trailing_stop: bool,
    /// Whether cancel-replace is allowed.
    pub cancel_replace_allowed: bool,
    /// Whether order amendment is allowed. Every captured row carries the flag;
    /// it is optional so a row without it still decodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amend_allowed: Option<bool>,
    /// Whether pegged orders are allowed. Every captured row carries the flag;
    /// it is optional so a row without it still decodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peg_instructions_allowed: Option<bool>,
    /// Whether the symbol can be traded as spot.
    pub is_spot_trading_allowed: bool,
    /// Whether the symbol can be traded on margin.
    pub is_margin_trading_allowed: bool,
    /// Order filters.
    pub filters: Vec<Filter>,
    /// Permissions the symbol is open to (older key).
    pub permissions: Vec<String>,
    /// Permission sets: an account needs every permission of one set.
    pub permission_sets: Vec<Vec<String>>,
    /// The self-trade-prevention mode applied when an order names none.
    pub default_self_trade_prevention_mode: String,
    /// The self-trade-prevention modes an order may name.
    pub allowed_self_trade_prevention_modes: Vec<String>,
}

polyoxide_venue::open_enum! {
    /// A symbol's trading status, from `exchangeInfo`. The variants are the
    /// list in Binance's common definitions.
    pub enum SymbolStatus {
        Trading => "TRADING",
        Break => "BREAK",
        Halt => "HALT",
        PreTrading => "PRE_TRADING",
        PostTrading => "POST_TRADING",
        EndOfDay => "END_OF_DAY",
        AuctionMatch => "AUCTION_MATCH",
    }
}

/// Declares [`Filter`] and the private wire enum its serde goes through from
/// one table: each entry is a variant, its wire `filterType`, and its fields
/// (camelCase on the wire, a decimal string where `=> "..."` names the
/// `serde(with)`). Every field type is `Copy`, which the serializer relies on.
macro_rules! filters {
    ($(
        $(#[$vmeta:meta])*
        $variant:ident = $tag:literal {
            $( $field:ident : $ty:ty $(=> $with:literal)? ),* $(,)?
        }
    )+) => {
        /// An `exchangeInfo` filter, named after its wire `filterType`.
        ///
        /// A filter type this version does not know is kept whole in
        /// [`Filter::Other`], so a new one never fails the response. A known
        /// type that loses or retypes a field still fails; one that gains a
        /// field decodes without it, and the wire-agreement tests are what
        /// notice.
        #[derive(Debug, Clone, PartialEq)]
        #[non_exhaustive]
        pub enum Filter {
            $(
                $(#[$vmeta])*
                #[non_exhaustive]
                $variant { $( $field: $ty ),* },
            )+
            /// A filter type this version does not model.
            Other {
                /// The wire `filterType`.
                filter_type: String,
                /// The whole filter object, `filterType` included.
                raw: Value,
            },
        }

        /// The wire bodies of the known filters, which `Filter`'s serde goes
        /// through.
        #[derive(Serialize, Deserialize)]
        #[serde(tag = "filterType")]
        enum FilterWire {
            $(
                #[serde(rename = $tag, rename_all = "camelCase")]
                $variant { $( $(#[serde(with = $with)])? $field: $ty ),* },
            )+
        }

        impl FilterWire {
            /// Every `filterType` this version models.
            const KNOWN: &'static [&'static str] = &[$($tag),+];
        }

        impl From<FilterWire> for Filter {
            fn from(wire: FilterWire) -> Self {
                match wire {
                    $( FilterWire::$variant { $($field),* } => Self::$variant { $($field),* }, )+
                }
            }
        }

        impl<'de> Deserialize<'de> for Filter {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = Value::deserialize(deserializer)?;
                let filter_type = raw
                    .get("filterType")
                    .and_then(Value::as_str)
                    .ok_or_else(|| <D::Error as de::Error>::custom("a filter without a string filterType"))?
                    .to_owned();
                if !FilterWire::KNOWN.contains(&filter_type.as_str()) {
                    return Ok(Self::Other { filter_type, raw });
                }
                let wire: FilterWire =
                    serde_json::from_value(raw).map_err(<D::Error as de::Error>::custom)?;
                Ok(wire.into())
            }
        }

        impl Serialize for Filter {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let wire = match self {
                    $( Self::$variant { $($field),* } => FilterWire::$variant { $($field: *$field),* }, )+
                    Self::Other { raw, .. } => return raw.serialize(serializer),
                };
                wire.serialize(serializer)
            }
        }
    };
}

filters! {
    /// `PRICE_FILTER`.
    PriceFilter = "PRICE_FILTER" {
        min_price: Decimal => "rust_decimal::serde::str",
        max_price: Decimal => "rust_decimal::serde::str",
        tick_size: Decimal => "rust_decimal::serde::str",
    }
    /// `PERCENT_PRICE_BY_SIDE`.
    PercentPriceBySide = "PERCENT_PRICE_BY_SIDE" {
        bid_multiplier_up: Decimal => "rust_decimal::serde::str",
        bid_multiplier_down: Decimal => "rust_decimal::serde::str",
        ask_multiplier_up: Decimal => "rust_decimal::serde::str",
        ask_multiplier_down: Decimal => "rust_decimal::serde::str",
        avg_price_mins: u32,
    }
    /// `LOT_SIZE`: limit orders.
    LotSize = "LOT_SIZE" {
        min_qty: Decimal => "rust_decimal::serde::str",
        max_qty: Decimal => "rust_decimal::serde::str",
        step_size: Decimal => "rust_decimal::serde::str",
    }
    /// `NOTIONAL`: the minimum and maximum order value.
    Notional = "NOTIONAL" {
        min_notional: Decimal => "rust_decimal::serde::str",
        apply_min_to_market: bool,
        max_notional: Decimal => "rust_decimal::serde::str",
        apply_max_to_market: bool,
        avg_price_mins: u32,
    }
    /// `ICEBERG_PARTS`.
    IcebergParts = "ICEBERG_PARTS" {
        limit: u32,
    }
    /// `MARKET_LOT_SIZE`: market orders.
    MarketLotSize = "MARKET_LOT_SIZE" {
        min_qty: Decimal => "rust_decimal::serde::str",
        max_qty: Decimal => "rust_decimal::serde::str",
        step_size: Decimal => "rust_decimal::serde::str",
    }
    /// `MAX_NUM_ORDERS`.
    MaxNumOrders = "MAX_NUM_ORDERS" {
        max_num_orders: u32,
    }
    /// `MAX_NUM_ALGO_ORDERS`.
    MaxNumAlgoOrders = "MAX_NUM_ALGO_ORDERS" {
        max_num_algo_orders: u32,
    }
    /// `MAX_POSITION`.
    MaxPosition = "MAX_POSITION" {
        max_position: Decimal => "rust_decimal::serde::str",
    }
    /// `TRAILING_DELTA`.
    TrailingDelta = "TRAILING_DELTA" {
        min_trailing_above_delta: u64,
        max_trailing_above_delta: u64,
        min_trailing_below_delta: u64,
        max_trailing_below_delta: u64,
    }
    /// `MAX_NUM_ORDER_AMENDS`.
    MaxNumOrderAmends = "MAX_NUM_ORDER_AMENDS" {
        max_num_order_amends: u32,
    }
    /// `MAX_NUM_ORDER_LISTS`.
    MaxNumOrderLists = "MAX_NUM_ORDER_LISTS" {
        max_num_order_lists: u32,
    }
}

#[cfg(test)]
mod tests {
    use polyoxide_test_support::{agreement, fixtures};

    use super::*;

    fn text(name: &str) -> String {
        fixtures!("spot/rest").text(name)
    }

    #[test]
    fn the_exchange_info_capture_agrees_with_its_types() {
        let text = text("exchange_info");
        let diff = agreement::compare::<ExchangeInfo>("exchange_info", &text);
        assert!(
            diff.unmodelled.is_empty(),
            "the server sent {:?}, which the types do not model",
            diff.unmodelled
        );
        assert!(
            diff.invented.is_empty(),
            "the types emit {:?}, which the server did not send",
            diff.invented
        );
    }

    #[test]
    fn every_symbol_and_filter_serializes_back_to_what_the_server_sent() {
        let text = text("exchange_info");
        let raw: Value = serde_json::from_str(&text).unwrap();
        let info: ExchangeInfo = serde_json::from_str(&text).unwrap();
        assert_eq!(
            serde_json::to_value(&info.rate_limits).unwrap(),
            raw["rateLimits"]
        );
        assert_eq!(
            serde_json::to_value(&info.exchange_filters).unwrap(),
            raw["exchangeFilters"]
        );
        let wire = raw["symbols"].as_array().unwrap();
        assert_eq!(info.symbols.len(), wire.len());
        for (symbol, sent) in info.symbols.iter().zip(wire) {
            assert_eq!(
                serde_json::to_value(symbol).unwrap(),
                *sent,
                "{}",
                symbol.symbol
            );
        }
    }

    #[test]
    fn the_capture_holds_the_cases_the_types_exist_for() {
        let info: ExchangeInfo = serde_json::from_str(&text("exchange_info")).unwrap();
        assert!(info.symbols.len() >= 2, "BTCUSDT and a non-trading symbol");
        assert!(
            info.symbols
                .iter()
                .any(|s| s.symbol == "BTCUSDT" && s.status == SymbolStatus::Trading),
            "BTCUSDT, trading"
        );
        assert!(
            info.symbols
                .iter()
                .any(|s| s.status != SymbolStatus::Trading),
            "a symbol that is not trading"
        );
        // An unknown value would land in `Other` and decode fine; failing here
        // is how a value Binance adds gets modelled instead of passing silently.
        let unnamed: Vec<_> = info
            .symbols
            .iter()
            .filter(|s| matches!(s.status, SymbolStatus::Other(_)))
            .map(|s| (&s.symbol, &s.status))
            .collect();
        assert!(
            unnamed.is_empty(),
            "statuses the enum does not name: {unnamed:?}"
        );
        // The same for filters: a misspelled tag would turn a PRICE_FILTER into
        // `Other`, and every other test would still pass.
        let unnamed: Vec<&Filter> = info
            .symbols
            .iter()
            .flat_map(|s| &s.filters)
            .chain(&info.exchange_filters)
            .filter(|f| matches!(f, Filter::Other { .. }))
            .collect();
        assert!(
            unnamed.is_empty(),
            "filter types Filter does not name, add them to `filters!`: {unnamed:?}"
        );
    }

    #[test]
    fn the_captured_btcusdt_filters_decode_to_their_values() {
        let text = text("exchange_info");
        let raw: Value = serde_json::from_str(&text).unwrap();
        let info: ExchangeInfo = serde_json::from_str(&text).unwrap();
        let btc = info.symbols.iter().find(|s| s.symbol == "BTCUSDT").unwrap();
        let wire = raw["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["symbol"] == "BTCUSDT")
            .unwrap();
        let sent = |filter_type: &str, key: &str| -> String {
            wire["filters"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["filterType"] == filter_type)
                .unwrap_or_else(|| panic!("no {filter_type}"))[key]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let price = btc
            .filters
            .iter()
            .find_map(|f| match f {
                Filter::PriceFilter { tick_size, .. } => Some(*tick_size),
                _ => None,
            })
            .expect("a PRICE_FILTER");
        assert_eq!(price.to_string(), sent("PRICE_FILTER", "tickSize"));
        let lot = btc
            .filters
            .iter()
            .find_map(|f| match f {
                Filter::LotSize { step_size, .. } => Some(*step_size),
                _ => None,
            })
            .expect("a LOT_SIZE");
        assert_eq!(lot.to_string(), sent("LOT_SIZE", "stepSize"));
        let market = btc
            .filters
            .iter()
            .find_map(|f| match f {
                Filter::MarketLotSize { step_size, .. } => Some(*step_size),
                _ => None,
            })
            .expect("a MARKET_LOT_SIZE");
        assert_eq!(market.to_string(), sent("MARKET_LOT_SIZE", "stepSize"));
        let notional = btc
            .filters
            .iter()
            .find_map(|f| match f {
                Filter::Notional { min_notional, .. } => Some(*min_notional),
                _ => None,
            })
            .expect("a NOTIONAL");
        assert_eq!(notional.to_string(), sent("NOTIONAL", "minNotional"));
    }

    #[test]
    fn the_time_capture_agrees_with_its_type() {
        let text = text("time");
        let diff = agreement::compare::<ServerTime>("time", &text);
        assert!(diff.unmodelled.is_empty(), "{:?}", diff.unmodelled);
        assert!(diff.invented.is_empty(), "{:?}", diff.invented);
        let time: ServerTime = serde_json::from_str(&text).unwrap();
        assert!(time.server_time > 1_790_000_000_000);
    }

    #[test]
    fn an_unknown_filter_type_is_kept_whole() {
        let raw = serde_json::json!({"filterType": "A_FILTER_FROM_THE_FUTURE", "limit": 7});
        let filter: Filter = serde_json::from_value(raw.clone()).unwrap();
        assert!(matches!(
            &filter,
            Filter::Other { filter_type, raw: kept }
                if filter_type == "A_FILTER_FROM_THE_FUTURE" && *kept == raw
        ));
        assert_eq!(serde_json::to_value(&filter).unwrap(), raw);
    }

    #[test]
    fn a_known_filter_that_loses_a_field_fails() {
        let raw = serde_json::json!({"filterType": "PRICE_FILTER", "minPrice": "0.01000000"});
        assert!(serde_json::from_value::<Filter>(raw).is_err());
    }

    #[test]
    fn a_filter_without_a_type_fails() {
        let raw = serde_json::json!({"minPrice": "0.01000000"});
        let err = serde_json::from_value::<Filter>(raw).unwrap_err();
        assert!(err.to_string().contains("filterType"), "{err}");
    }

    #[test]
    fn a_status_keeps_an_unknown_spelling() {
        assert_eq!(SymbolStatus::from_wire("TRADING"), SymbolStatus::Trading);
        assert_eq!(
            SymbolStatus::from_wire("PRE_TRADING"),
            SymbolStatus::PreTrading
        );
        let unknown = SymbolStatus::from_wire("A_NEW_STATE");
        assert_eq!(unknown, SymbolStatus::Other("A_NEW_STATE".into()));
        assert_eq!(unknown.as_str(), "A_NEW_STATE");
    }

    #[test]
    fn the_status_spellings_are_binance_s() {
        let table = [
            (SymbolStatus::Trading, "TRADING"),
            (SymbolStatus::Break, "BREAK"),
            (SymbolStatus::Halt, "HALT"),
            (SymbolStatus::PreTrading, "PRE_TRADING"),
            (SymbolStatus::PostTrading, "POST_TRADING"),
            (SymbolStatus::EndOfDay, "END_OF_DAY"),
            (SymbolStatus::AuctionMatch, "AUCTION_MATCH"),
        ];
        for (status, wire) in table {
            assert_eq!(status.as_str(), wire);
        }
    }
}
