//! Reference data: `/v1/info/{exchange,assets,instruments,fees,limit-tiers}`.

use polyoxide_core::{HttpClient, QueryBuilder, Request};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    api::{fetch, setter, Fetch},
    error::PerpsError,
    types::{InstrumentCategory, InstrumentId, InstrumentType},
};

/// Exchange namespace: static reference data.
#[derive(Clone)]
pub struct ExchangeApi {
    pub(crate) http_client: HttpClient,
}

impl ExchangeApi {
    /// `GET /v1/info/exchange`: the EIP-712 domain and maintenance state.
    pub fn exchange(&self) -> Fetch<Exchange> {
        fetch(&self.http_client, "/v1/info/exchange")
    }

    /// `GET /v1/info/assets`: collateral assets.
    pub fn assets(&self) -> Fetch<Vec<Asset>> {
        fetch(&self.http_client, "/v1/info/assets")
    }

    /// `GET /v1/info/instruments`: listed instruments, optionally filtered.
    pub fn instruments(&self) -> ListInstruments {
        ListInstruments {
            request: Request::new(self.http_client.clone(), "/v1/info/instruments"),
        }
    }

    /// `GET /v1/info/fees`: the tiered maker/taker schedule.
    pub fn fees(&self) -> Fetch<FeesInfo> {
        fetch(&self.http_client, "/v1/info/fees")
    }

    /// `GET /v1/info/limit-tiers`: volume-based rate-limit tiers.
    pub fn limit_tiers(&self) -> Fetch<Vec<LimitTier>> {
        fetch(&self.http_client, "/v1/info/limit-tiers")
    }
}

/// Request builder for `GET /v1/info/instruments`.
pub struct ListInstruments {
    request: Request<Vec<Instrument>, PerpsError>,
}

impl ListInstruments {
    setter! {
        /// Restrict to one instrument.
        instrument_id => "instrument_id"
    }
    setter! {
        /// Restrict to one instrument type.
        instrument_type => "instrument_type"
    }
    setter! {
        /// Restrict to one category.
        category => "category"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Instrument>, PerpsError> {
        self.request.send().await
    }
}

/// `GET /v1/info/exchange`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Exchange {
    /// Exchange name used in the EIP-712 domain.
    pub name: String,
    /// Exchange version used in the EIP-712 domain.
    pub version: String,
    /// Chain the exchange is deployed on.
    pub chain_id: u64,
    /// Verifying contract of the EIP-712 domain.
    pub contract: String,
    /// True while the exchange is in cancel-only (maintenance) mode.
    pub cancel_only: bool,
    /// Engine release serving this response.
    pub engine_version: String,
}

/// A collateral asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Asset {
    /// Asset name.
    pub asset: String,
    /// Token address.
    pub address: String,
    /// Token decimals.
    pub decimals: u32,
    /// Collateral ratio.
    #[serde(with = "rust_decimal::serde::str")]
    pub collateral_ratio: Decimal,
    /// Withdrawal fee in decimalised asset units.
    #[serde(with = "rust_decimal::serde::str")]
    pub withdrawal_fee: Decimal,
}

/// A listed instrument.
///
/// `display_symbol`, `close_only` and `logo` are on the wire and not in the
/// published schema (`docs/specs/perps/OBSERVED.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Instrument {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Instrument type.
    pub instrument_type: InstrumentType,
    /// Category.
    pub category: InstrumentCategory,
    /// Whether only isolated margin is supported.
    pub isolated_only: bool,
    /// Symbol, e.g. `SP500-USD`.
    pub symbol: String,
    /// Symbol first-party interfaces show, when it differs. Undocumented.
    pub display_symbol: Option<String>,
    /// Whether new positions are refused. Undocumented.
    pub close_only: Option<bool>,
    /// Base asset name.
    pub base_asset: String,
    /// Quote asset name.
    pub quote_asset: String,
    /// Funding interval, e.g. `1h`.
    pub funding_interval: String,
    /// Decimal places for quantity.
    pub quantity_decimals: u32,
    /// Decimal places for price.
    pub price_decimals: u32,
    /// Price bounds as a fraction.
    #[serde(with = "rust_decimal::serde::str")]
    pub price_bounds: Decimal,
    /// Liquidation fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub liquidation_fee: Decimal,
    /// Maximum open orders.
    pub max_order_count: u32,
    /// Minimum order notional in USD.
    #[serde(with = "rust_decimal::serde::str")]
    pub min_notional: Decimal,
    /// Maximum market-order notional in USD.
    #[serde(with = "rust_decimal::serde::str")]
    pub max_market_notional: Decimal,
    /// Maximum limit-order notional in USD.
    #[serde(with = "rust_decimal::serde::str")]
    pub max_limit_notional: Decimal,
    /// Maximum leverage.
    pub max_leverage: u32,
    /// Leverage caps by position size.
    pub risk_tiers: Vec<RiskTier>,
    /// When first-party interfaces may show the instrument, Unix ms.
    pub ui_live_time: u64,
    /// Logo URL. Undocumented.
    pub logo: Option<String>,
}

/// One leverage tier by position size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RiskTier {
    /// Position size lower bound.
    #[serde(with = "rust_decimal::serde::str")]
    pub lower_bound: Decimal,
    /// Maximum leverage at and above the bound.
    pub max_leverage: u32,
}

/// `GET /v1/info/fees`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FeesInfo {
    /// One entry per instrument type and category.
    pub fee_schedule: Vec<FeeScheduleEntry>,
}

/// Fees for one instrument type and category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FeeScheduleEntry {
    /// Instrument type.
    pub instrument_type: InstrumentType,
    /// Category.
    pub category: InstrumentCategory,
    /// Base taker fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub taker_fee_rate: Decimal,
    /// Base maker fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub maker_fee_rate: Decimal,
    /// Volume tiers.
    pub tiers: Vec<FeeTier>,
}

/// One volume tier of the fee schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FeeTier {
    /// 30-day volume at which the tier starts.
    #[serde(with = "rust_decimal::serde::str")]
    pub min_volume_30d: Decimal,
    /// Taker fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub taker_fee_rate: Decimal,
    /// Maker fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub maker_fee_rate: Decimal,
}

/// One volume-based rate-limit tier.
///
/// The four `Option` fields are on the wire and not in the published schema
/// (`docs/specs/perps/OBSERVED.md`); they describe the WebSocket budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LimitTier {
    /// 14-day volume at which the tier starts.
    #[serde(with = "rust_decimal::serde::str")]
    pub min_volume_14d: Decimal,
    /// Sustained request rate per minute.
    pub rate_per_minute_limit: u64,
    /// Request burst allowance.
    pub rate_burst_limit: u64,
    /// Sustained order actions per minute.
    pub actions_per_minute_limit: u64,
    /// Order-action burst allowance.
    pub actions_burst_limit: u64,
    /// Resting open-order cap.
    pub open_orders_limit: u64,
    /// Display-only messages-per-minute figure.
    pub messages_per_minute: u64,
    /// WebSocket connects per minute. Undocumented.
    pub connects_per_minute_limit: Option<u64>,
    /// Concurrent WebSocket connections. Undocumented.
    pub max_connections: Option<u64>,
    /// WebSocket inbound-message burst allowance. Undocumented.
    pub ws_messages_burst_limit: Option<u64>,
    /// WebSocket inbound messages per minute. Undocumented.
    pub ws_messages_per_minute_limit: Option<u64>,
}
