//! Markets, their order books, their public trades and their candlesticks.
//!
//! Money (`*_dollars`) and counts (`*_fp`) are [`Decimal`], decoded from the fixed-point
//! strings Kalshi sends, so no value passes through an `f64` and every digit survives a round
//! trip. Times are the RFC 3339 strings as sent. A field Kalshi leaves out of a response is an
//! `Option`, and an absent one is left out again when the type is written back. Free-form
//! parts (`custom_strike`, a combination's legs) are kept as raw JSON.

use polyoxide_venue::positional::DecimalStr;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Number, Value};

polyoxide_venue::open_enum! {
    /// A market's place in its lifecycle. Kalshi extends the set, so a value this version does
    /// not know is kept as `Other`.
    pub enum MarketStatus {
        /// Wire value `initialized`.
        Initialized => "initialized",
        /// Wire value `inactive`.
        Inactive => "inactive",
        /// Wire value `active`: the market trades.
        Active => "active",
        /// Wire value `closed`.
        Closed => "closed",
        /// Wire value `determined`.
        Determined => "determined",
        /// Wire value `disputed`.
        Disputed => "disputed",
        /// Wire value `amended`.
        Amended => "amended",
        /// Wire value `finalized`.
        Finalized => "finalized",
    }
}

polyoxide_venue::open_enum! {
    /// An outcome: the side of a binary market a trade took or a level rests on.
    pub enum OutcomeSide {
        /// Wire value `yes`.
        Yes => "yes",
        /// Wire value `no`.
        No => "no",
    }
}

polyoxide_venue::open_enum! {
    /// A side of the book, in the YES leg's vocabulary: `bid` buys YES and `ask` sells it.
    pub enum BookSide {
        /// Wire value `bid`.
        Bid => "bid",
        /// Wire value `ask`.
        Ask => "ask",
    }
}

polyoxide_venue::wire_enum! {
    /// The lifecycle states `GET /markets` filters by. The OpenAPI's parameter enum lists all
    /// five; its prose leaves out `paused`.
    #[non_exhaustive]
    pub enum MarketStatusFilter {
        /// Not yet open.
        Unopened => "unopened",
        /// Open for trading.
        Open => "open",
        /// Paused.
        Paused => "paused",
        /// Closed.
        Closed => "closed",
        /// Settled.
        Settled => "settled",
    }
}

polyoxide_venue::wire_enum! {
    /// The `mve_filter` of `GET /markets`: whether the listing holds multivariate combinations.
    /// The unfiltered open listing serves them first, 9,996 of its first 10,000 rows on
    /// 2026-10-18, so a walk for a single quoted market asks to exclude them.
    #[non_exhaustive]
    pub enum MveFilter {
        /// Only the multivariate combinations.
        Only => "only",
        /// Everything but the multivariate combinations.
        Exclude => "exclude",
    }
}

/// Reads `null` as the type's default, for a list the host may send as `null` or leave out.
pub(crate) fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// One range of valid order prices on a market, and the step within it, in dollars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceRange {
    /// Where the range starts.
    #[serde(with = "rust_decimal::serde::str")]
    pub start: Decimal,
    /// Where the range ends.
    #[serde(with = "rust_decimal::serde::str")]
    pub end: Decimal,
    /// The tick size within the range.
    #[serde(with = "rust_decimal::serde::str")]
    pub step: Decimal,
}

/// A market: one binary outcome of an event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Market {
    /// The market's ticker.
    pub ticker: String,
    /// The event it belongs to.
    pub event_ticker: String,
    /// `binary` or `scalar`.
    pub market_type: String,
    /// Deprecated: the market's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Deprecated: the market's subtitle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// A short title for the YES side.
    pub yes_sub_title: String,
    /// A short title for the NO side.
    pub no_sub_title: String,
    /// When the market was created.
    pub created_time: String,
    /// When its non-trading metadata last changed.
    pub updated_time: String,
    /// When it opens.
    pub open_time: String,
    /// When it closes.
    pub close_time: String,
    /// When it is expected to expire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_expiration_time: Option<String>,
    /// Deprecated: when it expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiration_time: Option<String>,
    /// The latest it can expire.
    pub latest_expiration_time: String,
    /// Seconds between determination and settlement.
    pub settlement_timer_seconds: u32,
    /// The market's lifecycle state.
    pub status: MarketStatus,
    /// The best YES bid, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub yes_bid_dollars: Decimal,
    /// Contracts resting at the best YES bid.
    #[serde(with = "rust_decimal::serde::str")]
    pub yes_bid_size_fp: Decimal,
    /// The best YES ask, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub yes_ask_dollars: Decimal,
    /// Contracts resting at the best YES ask.
    #[serde(with = "rust_decimal::serde::str")]
    pub yes_ask_size_fp: Decimal,
    /// The best NO bid, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub no_bid_dollars: Decimal,
    /// The best NO ask, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub no_ask_dollars: Decimal,
    /// The last YES trade's price, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub last_price_dollars: Decimal,
    /// Contracts traded.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume_fp: Decimal,
    /// Contracts traded in the last 24 hours.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume_24h_fp: Decimal,
    /// `yes`, `no`, `scalar`, or empty before a result.
    pub result: String,
    /// Whether the market can close early.
    pub can_close_early: bool,
    /// Contracts bought, without netting.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest_fp: Decimal,
    /// What one contract pays at settlement, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub notional_value_dollars: Decimal,
    /// The best YES bid a day ago, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub previous_yes_bid_dollars: Decimal,
    /// The best YES ask a day ago, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub previous_yes_ask_dollars: Decimal,
    /// The last YES trade's price a day ago, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub previous_price_dollars: Decimal,
    /// What the YES side settled at, in dollars; set after determination.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub settlement_value_dollars: Option<Decimal>,
    /// When the market settled; set for a settled market.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_ts: Option<String>,
    /// Which settlement bounds apply: `default` (none) or `floor`.
    pub settlement_bounds_type: String,
    /// The lowest the YES side can settle at, in dollars; set with a `floor` bound.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub settlement_floor_dollars: Option<Decimal>,
    /// The value that was considered for the settlement.
    pub expiration_value: String,
    /// When the underlying event occurred, if recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_datetime: Option<String>,
    /// When the market's fee waiver expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_waiver_expiration_time: Option<String>,
    /// The condition under which the market can close early.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub early_close_condition: Option<String>,
    /// How the strike is defined and evaluated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike_type: Option<String>,
    /// The lowest expiration value that settles YES.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_strike: Option<Number>,
    /// The highest expiration value that settles YES.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_strike: Option<Number>,
    /// A mapping from expiration values to settlement values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub functional_strike: Option<String>,
    /// The expiration value of each target that settles YES, as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_strike: Option<Value>,
    /// A plain-language statement of the main terms.
    pub rules_primary: String,
    /// A plain-language statement of the secondary terms.
    pub rules_secondary: String,
    /// A combination's collection ticker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mve_collection_ticker: Option<String>,
    /// A combination's selected legs, as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mve_selected_legs: Option<Vec<Value>>,
    /// The market's primary participant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_participant_key: Option<String>,
    /// The market's price level structure, which defines `price_ranges`.
    pub price_level_structure: String,
    /// The valid price ranges for orders.
    pub price_ranges: Vec<PriceRange>,
    /// Whether the market may be removed after determination if nothing traded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_provisional: Option<bool>,
    /// The exchange shard the market trades on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange_index: Option<u32>,
}

/// `GET /markets/{ticker}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketResponse {
    /// The market.
    pub market: Market,
}

/// `GET /markets`: one page. An empty `cursor` ends a walk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketsPage {
    /// The page's markets.
    pub markets: Vec<Market>,
    /// The cursor of the next page, empty on the last.
    pub cursor: String,
}

/// One price level of an order book side, `[price, count]` on the wire.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceLevel {
    /// The price, in dollars.
    pub price: Decimal,
    /// Contracts resting at the price.
    pub count: Decimal,
}

/// A level's wire form: a tuple of exactly two, so a longer or shorter array is refused.
#[derive(Serialize, Deserialize)]
struct PriceLevelWire(DecimalStr, DecimalStr);

impl<'de> Deserialize<'de> for PriceLevel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let PriceLevelWire(price, count) = PriceLevelWire::deserialize(deserializer)?;
        Ok(Self {
            price: price.0,
            count: count.0,
        })
    }
}

impl Serialize for PriceLevel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        PriceLevelWire(DecimalStr(self.price), DecimalStr(self.count)).serialize(serializer)
    }
}

/// The two sides of an order book. Each lists its bids worst (lowest) first, and a NO bid at
/// `q` is a YES ask at `1 - q`. A side with no levels is an empty list, whether the host sends
/// `[]`, `null` or nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderbookFp {
    /// YES bids.
    #[serde(default, deserialize_with = "null_as_default")]
    pub yes_dollars: Vec<PriceLevel>,
    /// NO bids.
    #[serde(default, deserialize_with = "null_as_default")]
    pub no_dollars: Vec<PriceLevel>,
}

/// `GET /markets/{ticker}/orderbook`. An unlisted ticker answers `200` with both sides empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderbookResponse {
    /// The book.
    pub orderbook_fp: OrderbookFp,
}

/// One public trade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    /// The trade's id.
    pub trade_id: String,
    /// The market's ticker.
    pub ticker: String,
    /// Contracts traded; a trade can be fractional.
    #[serde(with = "rust_decimal::serde::str")]
    pub count_fp: Decimal,
    /// The YES price, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub yes_price_dollars: Decimal,
    /// The NO price, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub no_price_dollars: Decimal,
    /// Deprecated: the taker's side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taker_side: Option<OutcomeSide>,
    /// The outcome the taker is positioned for.
    pub taker_outcome_side: OutcomeSide,
    /// The same direction in book vocabulary.
    pub taker_book_side: BookSide,
    /// When the trade executed.
    pub created_time: String,
    /// Whether it matched off the book as a block trade.
    pub is_block_trade: bool,
}

/// `GET /markets/trades`: one page, newest first. An empty `cursor` ends a walk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradesPage {
    /// The page's trades.
    pub trades: Vec<Trade>,
    /// The cursor of the next page, empty on the last.
    pub cursor: String,
}

polyoxide_venue::wire_enum! {
    /// The `period_interval` of the candlestick routes: a candlestick's width in minutes. Kalshi
    /// serves these three. The batch route answers any other number with `200` and the daily
    /// candles, so an unserved width would silently read as daily, and the series route with a
    /// `400`; the closed set is what keeps the native client from sending one.
    #[non_exhaustive]
    pub enum PeriodInterval {
        /// One minute: wire value `1`.
        Minute1 => "1",
        /// One hour: wire value `60`.
        Hour1 => "60",
        /// One day: wire value `1440`.
        Day1 => "1440",
    }
}

impl PeriodInterval {
    /// The width in seconds.
    pub(crate) fn seconds(self) -> i64 {
        match self {
            Self::Minute1 => 60,
            Self::Hour1 => 3_600,
            Self::Day1 => 86_400,
        }
    }
}

/// Offers on one side of the market over a candlestick's period (`BidAskDistribution`): the
/// first, lowest, highest and last price, in dollars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BidAskDistribution {
    /// The price at the start of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_dollars: Decimal,
    /// The lowest price in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub low_dollars: Decimal,
    /// The highest price in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub high_dollars: Decimal,
    /// The price at the end of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub close_dollars: Decimal,
}

/// Trade prices over a candlestick's period (`PriceDistribution`), in dollars. Kalshi leaves a
/// field out when it has no value: a period with no trade has only `previous_dollars`, and a
/// period before the market's first trade has none of them (`{}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceDistribution {
    /// The first YES trade of the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub open_dollars: Option<Decimal>,
    /// The lowest YES trade of the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub low_dollars: Option<Decimal>,
    /// The highest YES trade of the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub high_dollars: Option<Decimal>,
    /// The last YES trade of the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub close_dollars: Option<Decimal>,
    /// The mean YES trade price of the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub mean_dollars: Option<Decimal>,
    /// The last YES trade before the period.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub previous_dollars: Option<Decimal>,
    /// The mirror's `min_dollars`, which Kalshi has not been seen to send.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub min_dollars: Option<Decimal>,
    /// The mirror's `max_dollars`, which Kalshi has not been seen to send.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub max_dollars: Option<Decimal>,
}

/// One candlestick (`MarketCandlestick`). Kalshi names it by when its period ends, inclusive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketCandlestick {
    /// The end of the period, in Unix seconds.
    pub end_period_ts: i64,
    /// Offers to buy YES.
    pub yes_bid: BidAskDistribution,
    /// Offers to sell YES.
    pub yes_ask: BidAskDistribution,
    /// Trades.
    pub price: PriceDistribution,
    /// Contracts traded in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume_fp: Decimal,
    /// Contracts bought, without netting, by the end of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest_fp: Decimal,
}

/// One market's candlesticks in the batch answer (`MarketCandlesticksResponse`), oldest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketCandlesticks {
    /// The market's ticker.
    pub market_ticker: String,
    /// Its candlesticks.
    pub candlesticks: Vec<MarketCandlestick>,
}

/// `GET /markets/candlesticks` (`BatchGetMarketCandlesticksResponse`): one entry per market.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchCandlesticksResponse {
    /// The markets asked for.
    pub markets: Vec<MarketCandlesticks>,
}

/// `GET /series/{series_ticker}/markets/{ticker}/candlesticks`
/// (`GetMarketCandlesticksResponse`): one market's candlesticks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesCandlesticksResponse {
    /// The market's ticker.
    pub ticker: String,
    /// Its candlesticks.
    pub candlesticks: Vec<MarketCandlestick>,
}

polyoxide_venue::wire_enum! {
    /// The states a created-time window of `GET /markets` may name
    /// ([`MarketWindow::Created`]); Kalshi refuses any other beside `min_created_ts`.
    #[non_exhaustive]
    pub enum CreatedStatus {
        /// Not yet open.
        Unopened => "unopened",
        /// Open for trading.
        Open => "open",
    }
}

/// One family of `GET /markets`' timestamp filters, with the states the host allows beside it.
///
/// Kalshi refuses two families at once, and a state the family does not allow, with a `400`
/// (`docs/specs/kalshi/OBSERVED.md`), so a window names one family and its states and nothing
/// else; the fourth family, `min_updated_ts`, is `markets().updated`. Times are Unix seconds, and
/// a bound left `None` is not sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MarketWindow {
    /// Markets created in the window (`min_created_ts`, `max_created_ts`), optionally only the
    /// unopened or only the open ones.
    Created {
        /// Only markets in this state.
        status: Option<CreatedStatus>,
        /// The earliest creation time.
        min_ts: Option<i64>,
        /// The latest creation time.
        max_ts: Option<i64>,
    },
    /// Markets that close in the window (`min_close_ts`, `max_close_ts`), optionally only the
    /// closed ones (`status=closed`).
    Closed {
        /// Send `status=closed`.
        closed_only: bool,
        /// The earliest close time.
        min_ts: Option<i64>,
        /// The latest close time.
        max_ts: Option<i64>,
    },
    /// Markets that settled in the window (`min_settled_ts`, `max_settled_ts`), optionally only
    /// the settled ones (`status=settled`).
    Settled {
        /// Send `status=settled`.
        settled_only: bool,
        /// The earliest settlement time.
        min_ts: Option<i64>,
        /// The latest settlement time.
        max_ts: Option<i64>,
    },
}

impl MarketWindow {
    /// The query pairs the window sends, in order: its state, then its lower and upper bound.
    pub(crate) fn pairs(&self) -> Vec<(&'static str, String)> {
        let (status, (min_key, min_ts), (max_key, max_ts)) = match *self {
            Self::Created {
                status,
                min_ts,
                max_ts,
            } => (
                status.map(CreatedStatus::as_str),
                ("min_created_ts", min_ts),
                ("max_created_ts", max_ts),
            ),
            Self::Closed {
                closed_only,
                min_ts,
                max_ts,
            } => (
                closed_only.then_some("closed"),
                ("min_close_ts", min_ts),
                ("max_close_ts", max_ts),
            ),
            Self::Settled {
                settled_only,
                min_ts,
                max_ts,
            } => (
                settled_only.then_some("settled"),
                ("min_settled_ts", min_ts),
                ("max_settled_ts", max_ts),
            ),
        };
        let mut pairs = Vec::new();
        if let Some(status) = status {
            pairs.push(("status", status.to_owned()));
        }
        if let Some(min_ts) = min_ts {
            pairs.push((min_key, min_ts.to_string()));
        }
        if let Some(max_ts) = max_ts {
            pairs.push((max_key, max_ts.to_string()));
        }
        pairs
    }
}

/// One market's book in `GET /markets/orderbooks` (`MarketOrderbookFp`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketOrderbook {
    /// The market's ticker.
    pub ticker: String,
    /// Its book; an unlisted ticker's sides are empty.
    pub orderbook_fp: OrderbookFp,
}

/// `GET /markets/orderbooks` (`GetMarketOrderbooksResponse`): one book per ticker asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderbooksResponse {
    /// The books.
    pub orderbooks: Vec<MarketOrderbook>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_is_a_positional_pair_of_strings_that_keeps_every_digit() {
        let level: PriceLevel = serde_json::from_str(r#"["0.0100","402.00"]"#).unwrap();
        assert_eq!(level.price.to_string(), "0.0100");
        assert_eq!(level.count.to_string(), "402.00");
        assert_eq!(
            serde_json::to_string(&level).unwrap(),
            r#"["0.0100","402.00"]"#
        );
    }

    #[test]
    fn a_level_of_another_arity_or_with_a_number_is_refused() {
        assert!(serde_json::from_str::<PriceLevel>(r#"["0.01"]"#).is_err());
        assert!(serde_json::from_str::<PriceLevel>(r#"["0.01","1.00","x"]"#).is_err());
        assert!(serde_json::from_str::<PriceLevel>(r#"["0.01",1]"#).is_err());
    }

    #[test]
    fn a_side_that_is_null_or_missing_is_empty() {
        for body in [
            r#"{"yes_dollars":null,"no_dollars":[]}"#,
            r#"{"no_dollars":[]}"#,
            r#"{}"#,
        ] {
            let book: OrderbookFp = serde_json::from_str(body).unwrap();
            assert!(
                book.yes_dollars.is_empty() && book.no_dollars.is_empty(),
                "{body}"
            );
        }
    }

    #[test]
    fn a_state_the_enum_does_not_name_is_kept_verbatim() {
        let status: MarketStatus = serde_json::from_str(r#""suspended""#).unwrap();
        assert_eq!(status, MarketStatus::Other("suspended".into()));
        assert_eq!(serde_json::to_string(&status).unwrap(), r#""suspended""#);
        assert_eq!(MarketStatus::from_wire("active"), MarketStatus::Active);
    }

    #[test]
    fn a_price_distribution_keeps_every_digit_and_leaves_out_what_the_host_leaves_out() {
        let price: PriceDistribution =
            serde_json::from_str(r#"{"previous_dollars":"0.4200"}"#).unwrap();
        assert_eq!(
            price.previous_dollars.map(|p| p.to_string()),
            Some("0.4200".to_owned())
        );
        assert!(price.open_dollars.is_none() && price.close_dollars.is_none());
        assert_eq!(
            serde_json::to_string(&price).unwrap(),
            r#"{"previous_dollars":"0.4200"}"#
        );

        let none: PriceDistribution = serde_json::from_str("{}").unwrap();
        assert_eq!(serde_json::to_string(&none).unwrap(), "{}");
    }

    #[test]
    fn a_period_interval_is_its_width_in_minutes_and_nothing_else_parses() {
        assert_eq!(PeriodInterval::Minute1.to_string(), "1");
        assert_eq!(PeriodInterval::Hour1.to_string(), "60");
        assert_eq!(PeriodInterval::Day1.to_string(), "1440");
        assert_eq!(
            "1440".parse::<PeriodInterval>().ok(),
            Some(PeriodInterval::Day1)
        );
        for other in ["0", "5", "15", "240", "10080", "hour"] {
            assert!(other.parse::<PeriodInterval>().is_err(), "{other}");
        }
    }

    #[test]
    fn a_period_interval_is_as_many_seconds_as_its_minutes() {
        for width in PeriodInterval::ALL {
            let minutes: i64 = width.as_str().parse().unwrap();
            assert_eq!(width.seconds(), minutes * 60);
        }
    }

    #[test]
    fn a_window_sends_its_status_then_its_bounds_and_leaves_out_what_is_unset() {
        let owned = |pairs: &[(&'static str, &str)]| -> Vec<(&'static str, String)> {
            pairs.iter().map(|(k, v)| (*k, (*v).to_owned())).collect()
        };
        let created = MarketWindow::Created {
            status: Some(CreatedStatus::Unopened),
            min_ts: Some(1),
            max_ts: Some(2),
        };
        assert_eq!(
            created.pairs(),
            owned(&[
                ("status", "unopened"),
                ("min_created_ts", "1"),
                ("max_created_ts", "2")
            ])
        );
        let closed = MarketWindow::Closed {
            closed_only: true,
            min_ts: None,
            max_ts: Some(4),
        };
        assert_eq!(
            closed.pairs(),
            owned(&[("status", "closed"), ("max_close_ts", "4")])
        );
        let settled = MarketWindow::Settled {
            settled_only: false,
            min_ts: Some(5),
            max_ts: None,
        };
        assert_eq!(settled.pairs(), owned(&[("min_settled_ts", "5")]));
        let open = MarketWindow::Created {
            status: Some(CreatedStatus::Open),
            min_ts: None,
            max_ts: None,
        };
        assert_eq!(open.pairs(), owned(&[("status", "open")]));
    }

    #[test]
    fn a_created_status_is_one_of_the_two_the_created_family_allows() {
        assert_eq!(
            CreatedStatus::ALL,
            [CreatedStatus::Unopened, CreatedStatus::Open]
        );
        assert_eq!(CreatedStatus::Open.to_string(), "open");
        assert!("closed".parse::<CreatedStatus>().is_err());
    }
}
