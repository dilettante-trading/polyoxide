//! The records every venue's market data is read into.
//!
//! A record holds what venues share, as fields, and a venue's own facts in its
//! [`Extensions`]. A field a venue does not publish is an `Option`. Prices and
//! sizes are [`Decimal`]; a size is in the instrument's native quantity, which
//! [`Instrument::size_unit`] names, and an amount in the quote currency is a
//! separate `notional`.

use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use rust_decimal::Decimal;

use crate::{Class, Classify, Extensions, MarketKey, UnixMillis};

/// Which side of the book an order, or the taker of a trade, is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// Buying, against the asks.
    Buy,
    /// Selling, against the bids.
    Sell,
}

/// One price level of a book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Level {
    /// The price, in the instrument's quote currency.
    pub price: Decimal,
    /// The size resting at the price, in the instrument's native quantity.
    pub size: Decimal,
}

impl Level {
    /// A level of `size` at `price`.
    pub fn new(price: Decimal, size: Decimal) -> Self {
        Self { price, size }
    }
}

/// What an instrument's sizes count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SizeUnit {
    /// Units of the base asset, such as the coins a coin-quoted contract is
    /// written in.
    Base,
    /// Contracts, each a fixed amount of the underlying the venue defines.
    Contracts,
    /// Outcome shares of an event contract, each paying one unit of the
    /// settlement currency if its outcome happens.
    Shares,
}

/// An instrument a venue lists, and the one record that carries its trading
/// rules: tick size, minimum size and [`SizeUnit`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Instrument {
    /// The instrument's key.
    pub key: MarketKey,
    /// A name for people: the venue's symbol, or for an outcome of an event
    /// contract, its market and outcome.
    pub name: String,
    /// The asset the instrument's sizes are written in, when the venue names
    /// one.
    pub base_asset: Option<String>,
    /// The currency its prices are quoted in, when the venue names one.
    pub quote_asset: Option<String>,
    /// What its sizes count.
    pub size_unit: SizeUnit,
    /// The smallest price step, when the venue publishes a fixed one.
    pub tick_size: Option<Decimal>,
    /// The smallest order size, in native quantity.
    pub min_size: Option<Decimal>,
    /// The smallest size step, in native quantity.
    pub size_increment: Option<Decimal>,
    /// The smallest order value, in the quote currency.
    pub min_notional: Option<Decimal>,
    /// Whether the venue now takes orders on it that open or add to a
    /// position. A close-only instrument is not tradable.
    pub tradable: Option<bool>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Instrument {
    /// An instrument with its key, name and size unit, and nothing else known.
    pub fn new(key: MarketKey, name: impl Into<String>, size_unit: SizeUnit) -> Self {
        Self {
            key,
            name: name.into(),
            base_asset: None,
            quote_asset: None,
            size_unit,
            tick_size: None,
            min_size: None,
            size_increment: None,
            min_notional: None,
            tradable: None,
            extensions: Extensions::new(),
        }
    }
}

/// An instrument's best bid and ask at one moment.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Quote {
    /// The instrument's key.
    pub key: MarketKey,
    /// The highest bid, when there is one.
    pub bid: Option<Level>,
    /// The lowest ask, when there is one.
    pub ask: Option<Level>,
    /// When the venue took it.
    pub time: Option<UnixMillis>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Quote {
    /// A quote for `key` with neither side known.
    pub fn new(key: MarketKey) -> Self {
        Self {
            key,
            bid: None,
            ask: None,
            time: None,
            extensions: Extensions::new(),
        }
    }
}

/// An instrument's order book at one moment.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Book {
    /// The instrument's key.
    pub key: MarketKey,
    /// Bids, best (highest price) first.
    pub bids: Vec<Level>,
    /// Asks, best (lowest price) first.
    pub asks: Vec<Level>,
    /// When the venue took it.
    pub time: Option<UnixMillis>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Book {
    /// An empty book for `key`.
    pub fn new(key: MarketKey) -> Self {
        Self {
            key,
            bids: Vec::new(),
            asks: Vec::new(),
            time: None,
            extensions: Extensions::new(),
        }
    }
}

/// A trade's id at its venue. Opaque: compare it, never parse it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TradeId(Arc<str>);

impl TradeId {
    /// The id `id`.
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }

    /// The id, as the venue wrote it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TradeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One public trade.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Trade {
    /// The instrument's key.
    pub key: MarketKey,
    /// The venue's id for the trade, when it gives one.
    pub id: Option<TradeId>,
    /// The price.
    pub price: Decimal,
    /// The size, in native quantity.
    pub size: Decimal,
    /// The amount, in the quote currency, when the venue publishes it.
    pub notional: Option<Decimal>,
    /// The taker's side, when the venue says.
    pub taker_side: Option<Side>,
    /// When it traded.
    pub time: UnixMillis,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Trade {
    /// A trade of `size` at `price`, at `time`.
    pub fn new(key: MarketKey, price: Decimal, size: Decimal, time: UnixMillis) -> Self {
        Self {
            key,
            id: None,
            price,
            size,
            notional: None,
            taker_side: None,
            time,
            extensions: Extensions::new(),
        }
    }
}

/// How long a candle spans. A venue serves some or all of these widths, the
/// ones its [`Candles::intervals`](crate::Candles::intervals) lists, and
/// handles every variant, since the enum is exhaustive: a new width is a
/// breaking change that fails to compile at a venue that does not handle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CandleInterval {
    /// One minute.
    Minute1,
    /// Five minutes.
    Minute5,
    /// Fifteen minutes.
    Minute15,
    /// One hour.
    Hour1,
    /// Four hours.
    Hour4,
    /// One day.
    Day1,
    /// One week.
    Week1,
}

impl CandleInterval {
    /// Every width, shortest first.
    pub const ALL: &'static [CandleInterval] = &[
        Self::Minute1,
        Self::Minute5,
        Self::Minute15,
        Self::Hour1,
        Self::Hour4,
        Self::Day1,
        Self::Week1,
    ];

    /// How long the width is.
    pub fn duration(self) -> Duration {
        let minutes = match self {
            Self::Minute1 => 1,
            Self::Minute5 => 5,
            Self::Minute15 => 15,
            Self::Hour1 => 60,
            Self::Hour4 => 240,
            Self::Day1 => 1_440,
            Self::Week1 => 10_080,
        };
        Duration::from_secs(minutes * 60)
    }
}

/// Which candles to read: their width and the open times they span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CandleQuery {
    /// The candles' width.
    pub interval: CandleInterval,
    /// The earliest open time.
    pub start: UnixMillis,
    /// The latest open time; now, when `None`.
    pub end: Option<UnixMillis>,
}

impl CandleQuery {
    /// Candles of `interval` from `start` to now.
    pub fn new(interval: CandleInterval, start: UnixMillis) -> Self {
        Self {
            interval,
            start,
            end: None,
        }
    }

    /// Ends the span at `end`.
    pub fn end(mut self, end: UnixMillis) -> Self {
        self.end = Some(end);
        self
    }

    /// Checks that the query's width is one of `served`, which a venue takes
    /// from its [`Candles::intervals`](crate::Candles::intervals). Call it
    /// before the first request, and convert the refusal with `?`.
    pub fn ensure_served(&self, served: &'static [CandleInterval]) -> Result<(), UnservedInterval> {
        if served.contains(&self.interval) {
            Ok(())
        } else {
            Err(UnservedInterval {
                interval: self.interval,
                served,
            })
        }
    }
}

/// A query asked for a candle width the venue does not serve
/// ([`CandleQuery::ensure_served`]). An
/// [`InvalidRequest`](Class::InvalidRequest): the caller's misuse, refused
/// before anything is sent, and no wait or retry cures it.
///
/// Only [`CandleQuery::ensure_served`] builds one, since a field may be added
/// later and a struct literal is refused when the program is built:
///
/// ```compile_fail
/// use polyoxide_venue::{CandleInterval, UnservedInterval};
///
/// fn refuse() -> UnservedInterval {
///     UnservedInterval {
///         interval: CandleInterval::Minute5,
///         served: &[],
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UnservedInterval {
    /// The width asked for.
    pub interval: CandleInterval,
    /// The widths the venue serves, as its
    /// [`Candles::intervals`](crate::Candles::intervals) lists them.
    pub served: &'static [CandleInterval],
}

impl fmt::Display for UnservedInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{:?}` candles are not served; this venue serves ",
            self.interval
        )?;
        if self.served.is_empty() {
            return f.write_str("none");
        }
        for (index, width) in self.served.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{width:?}")?;
        }
        Ok(())
    }
}

impl Error for UnservedInterval {}

impl Classify for UnservedInterval {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

/// One candle. A venue that samples one price per interval gives only
/// [`close`](Self::close).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Candle {
    /// The instrument's key.
    pub key: MarketKey,
    /// The candle's width.
    pub interval: CandleInterval,
    /// When it opens.
    pub open_time: UnixMillis,
    /// The first price.
    pub open: Option<Decimal>,
    /// The highest price.
    pub high: Option<Decimal>,
    /// The lowest price.
    pub low: Option<Decimal>,
    /// The last price.
    pub close: Decimal,
    /// The size traded, in native quantity.
    pub volume: Option<Decimal>,
    /// The amount traded, in the quote currency.
    pub notional: Option<Decimal>,
    /// How many trades.
    pub trades: Option<u64>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Candle {
    /// A candle of `interval` opening at `open_time` that closed at `close`.
    pub fn new(
        key: MarketKey,
        interval: CandleInterval,
        open_time: UnixMillis,
        close: Decimal,
    ) -> Self {
        Self {
            key,
            interval,
            open_time,
            open: None,
            high: None,
            low: None,
            close,
            volume: None,
            notional: None,
            trades: None,
            extensions: Extensions::new(),
        }
    }
}

/// A perpetual's funding rate: the one now accruing, or one already settled.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FundingRate {
    /// The instrument's key.
    pub key: MarketKey,
    /// The rate, a fraction of position value per funding period, `interval`
    /// long.
    pub rate: Decimal,
    /// The length of the funding period `rate` is for; `None` when the venue
    /// does not say.
    pub interval: Option<Duration>,
    /// When the venue sampled the rate now accruing, or when a settled one
    /// settled.
    pub time: UnixMillis,
    /// When the rate now accruing next settles; `None` for a settled rate, or
    /// when none is scheduled.
    pub next_funding: Option<UnixMillis>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl FundingRate {
    /// A rate of `rate` at `time`.
    pub fn new(key: MarketKey, rate: Decimal, time: UnixMillis) -> Self {
        Self {
            key,
            rate,
            interval: None,
            time,
            next_funding: None,
            extensions: Extensions::new(),
        }
    }
}

/// An event and every outcome it groups, across its markets.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct EventGroup {
    /// The venue's id for the event.
    pub id: String,
    /// The event's title, when it has one.
    pub title: Option<String>,
    /// Its outcomes, market by market.
    pub outcomes: Vec<EventOutcome>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl EventGroup {
    /// The event `id`, with no outcomes yet.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: None,
            outcomes: Vec::new(),
            extensions: Extensions::new(),
        }
    }
}

/// One outcome of an event: a key of its own, in one of the event's markets.
///
/// Like [`Level`], it is a value inside a record, an [`EventGroup`], and not a
/// record of its own, so it carries no [`Extensions`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct EventOutcome {
    /// The outcome's key.
    pub key: MarketKey,
    /// The venue's id for the market the outcome is in.
    pub market: String,
    /// The market's title, when it has one.
    pub market_title: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub label: String,
}

impl EventOutcome {
    /// The outcome `label` of `market`, keyed `key`.
    pub fn new(key: MarketKey, market: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key,
            market: market.into(),
            market_title: None,
            label: label.into(),
        }
    }
}

/// The venue's answer identifies the key as an instrument it does not list.
/// A [`VenueRefusal`](Class::VenueRefusal), as the venue's own answer for an
/// unknown instrument is.
///
/// Not every venue says so: where its answer does not identify the instrument
/// as unlisted, the venue's own refusal comes through with the same class, and
/// some venues return an empty quote or book. The guarantee is the class, so
/// match on `class()`, not on this type.
///
/// A crate that depends on this one builds it with [`NotListed::new`], since a
/// field may be added later, and a struct literal is refused when the program
/// is built:
///
/// ```compile_fail
/// use polyoxide_venue::{MarketKey, NotListed};
///
/// fn refuse(key: MarketKey) -> NotListed {
///     NotListed { key }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct NotListed {
    /// The key asked for.
    pub key: MarketKey,
}

impl NotListed {
    /// The refusal for `key`, an instrument the venue does not list.
    pub fn new(key: MarketKey) -> Self {
        Self { key }
    }
}

impl fmt::Display for NotListed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not listed", self.key)
    }
}

impl Error for NotListed {}

impl Classify for NotListed {
    fn class(&self) -> Class {
        Class::VenueRefusal { code: None }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::{KeySpace, ProductClass, ProductId, VenueId};

    struct Spot;

    impl KeySpace for Spot {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("spot");
        const CLASS: ProductClass = ProductClass::PerpetualFuture;
        type Id = String;

        fn parse_id(text: &str) -> Result<String, String> {
            Ok(text.to_owned())
        }

        fn id_text(id: &String) -> Cow<'_, str> {
            Cow::Borrowed(id)
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct SpotInstrumentExt {
        lot: u32,
    }

    fn instrument(lot: u32) -> Instrument {
        let mut instrument = Instrument::new(Spot::key(&"ABC".to_owned()), "ABC", SizeUnit::Base);
        instrument.tick_size = Some(Decimal::new(1, 2));
        instrument
            .extensions
            .insert(SpotInstrumentExt { lot })
            .unwrap();
        instrument
    }

    #[test]
    fn records_are_clone_debug_and_partial_eq() {
        fn holds<T: Clone + std::fmt::Debug + PartialEq>() {}
        holds::<Instrument>();
        holds::<Level>();
        holds::<Side>();
        holds::<SizeUnit>();
        holds::<Quote>();
        holds::<Book>();
        holds::<Trade>();
        holds::<Candle>();
        holds::<FundingRate>();
        holds::<EventGroup>();
        holds::<EventOutcome>();
    }

    #[test]
    fn each_record_carries_extensions_into_its_equality() {
        #[derive(Debug, Clone, PartialEq)]
        struct Venue(u8);

        let key = Spot::key(&"ABC".to_owned());
        let time = UnixMillis(1);
        let mut quote = Quote::new(key.clone());
        quote.extensions.insert(Venue(1)).unwrap();
        let mut other = Quote::new(key.clone());
        other.extensions.insert(Venue(2)).unwrap();
        assert_ne!(quote, other);

        let mut trade = Trade::new(key.clone(), Decimal::ONE, Decimal::ONE, time);
        let plain = trade.clone();
        trade.extensions.insert(Venue(1)).unwrap();
        assert_ne!(trade, plain);

        let mut candle = Candle::new(key.clone(), CandleInterval::Hour1, time, Decimal::ONE);
        let plain = candle.clone();
        candle.extensions.insert(Venue(1)).unwrap();
        assert_ne!(candle, plain);

        let mut rate = FundingRate::new(key.clone(), Decimal::ONE, time);
        let plain = rate.clone();
        rate.extensions.insert(Venue(1)).unwrap();
        assert_ne!(rate, plain);

        let mut book = Book::new(key);
        let plain = book.clone();
        book.extensions.insert(Venue(1)).unwrap();
        assert_ne!(book, plain);

        let mut group = EventGroup::new("1");
        let plain = group.clone();
        group.extensions.insert(Venue(1)).unwrap();
        assert_ne!(group, plain);
    }

    #[test]
    fn each_candle_interval_has_its_duration_in_order() {
        let minutes: Vec<u64> = CandleInterval::ALL
            .iter()
            .map(|interval| interval.duration().as_secs() / 60)
            .collect();
        assert_eq!(minutes, [1, 5, 15, 60, 240, 1_440, 10_080]);
    }

    #[test]
    fn a_candle_query_ends_now_unless_told() {
        let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(5));
        assert_eq!(query.end, None);
        assert_eq!(query.end(UnixMillis(9)).end, Some(UnixMillis(9)));
    }

    #[test]
    fn a_trade_id_is_opaque_text() {
        let id = TradeId::new("8353866920580050");
        assert_eq!(id.as_str(), "8353866920580050");
        assert_eq!(id.to_string(), "8353866920580050");
        assert_eq!(id, TradeId::new(String::from("8353866920580050")));
    }

    #[test]
    fn an_unlisted_key_is_a_venue_refusal_that_names_it() {
        let err = NotListed::new(Spot::key(&"ABC".to_owned()));
        assert_eq!(err.to_string(), "acme.spot:ABC is not listed");
        assert_eq!(err.class(), Class::VenueRefusal { code: None });
        assert!(!err.is_retriable());
    }

    #[test]
    fn two_records_differing_only_in_one_extension_value_are_unequal() {
        assert_eq!(instrument(1), instrument(1));
        assert_ne!(instrument(1), instrument(2));
    }

    #[test]
    fn a_new_instrument_knows_only_what_it_was_given() {
        let key = Spot::key(&"ABC".to_owned());
        let instrument = Instrument::new(key.clone(), "ABC", SizeUnit::Shares);
        assert_eq!(instrument.key, key);
        assert_eq!(instrument.name, "ABC");
        assert_eq!(instrument.base_asset, None);
        assert_eq!(instrument.quote_asset, None);
        assert_eq!(instrument.size_unit, SizeUnit::Shares);
        assert_eq!(instrument.tick_size, None);
        assert_eq!(instrument.min_size, None);
        assert_eq!(instrument.size_increment, None);
        assert_eq!(instrument.min_notional, None);
        assert_eq!(instrument.tradable, None);
        assert!(instrument.extensions.is_empty());
    }

    #[test]
    fn a_level_holds_its_price_and_native_size() {
        let level = Level::new(Decimal::new(84_1255, 1), Decimal::new(14_663, 3));
        assert_eq!(level.price.to_string(), "84125.5");
        assert_eq!(level.size.to_string(), "14.663");
    }
}
