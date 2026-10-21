//! Kalshi's predictions exchange through the venue-neutral market-data traits.
//!
//! [`Predictions`] implements `MarketData`, `Trades`, `Candles` and `EventGroups` over its
//! own routes, keyed
//! `kalshi.predictions:<TICKER>:yes` and `kalshi.predictions:<TICKER>:no` by the
//! constructors in `predictions::key`: a key names one outcome of one market,
//! never the market. What Kalshi alone publishes rides in this module's
//! `<Record>Ext` types, one per record, in each record's
//! [`Extensions`].
//!
//! Kalshi lists each market's resting bids twice, once for YES and once for NO,
//! each side in ascending price. A NO bid at price *q* is a YES ask at *1 - q*,
//! and a YES bid at *p* is a NO ask at *1 - p*. So a key's book is its own
//! outcome's bids, best first, and the other outcome's bids turned into asks,
//! best first, on every path: `quote` and `book` alike, and the two outcomes of
//! one market are one book read from its two sides.
//!
//! An unknown ticker is a `404` on the market route but a `200` with two empty
//! lists on the orderbook route, so `instrument` is `NotListed` for it while
//! `quote` and `book` return empty ones: use `instrument` to confirm a listing.
//! A time Kalshi sends is RFC 3339 and is read into `UnixMillis`; one that does
//! not parse is a `Decode` error naming the field, never a zero.
//!
//! `candles` reads one ticker from the batch candlestick route at the three widths
//! Kalshi serves, one minute, one hour and one day (`intervals()`). Any other width
//! is refused as `InvalidRequest` with nothing sent, since the route itself answers
//! one it does not serve with `200` and the daily candles, which would silently read
//! as daily: the refusal is the only guard. Kalshi selects a candlestick by
//! the time it ends and the trait by the time it opens, so the request's span is
//! the query's moved forward by one width: `start` rounded up to the second, and
//! `end` (or now) rounded down to the second, each plus the width. A candle opens
//! at its end less one width, and one opening outside the query is dropped. Its
//! prices are the trade prices, and a `no` key's are each turned into *1 - price*
//! with its high and low swapped. A period with no trade closes at the last price
//! before it (`previous`) and has no open, high or low, and a period with no
//! `previous` either is left out. Its volume is the contracts traded in the period,
//! in shares. `CandleExt` carries the YES bid and ask distributions and the open
//! interest in YES terms whatever the key's outcome, and the mean trade price,
//! turned over for a `no` key like the other prices. A market that settled before
//! Kalshi's historical cutoff has candlesticks only on its historical route, which
//! this client does not call: the live route's answer for it, an empty list or a
//! refusal, is returned as the venue gave it.
//!
//! | Method | Route | Requests |
//! | --- | --- | --- |
//! | `instrument` | `GET /trade-api/v2/markets/{ticker}` | 1 |
//! | `quote` | `GET /trade-api/v2/markets/{ticker}/orderbook`, `depth=1` | 1 |
//! | `book` | `GET /trade-api/v2/markets/{ticker}/orderbook`, every level | 1 |
//! | `trades` | `GET /trade-api/v2/markets/trades` for the ticker, 1000 rows | 1 |
//! | `candles` | `GET /trade-api/v2/markets/candlesticks` for the ticker | 1 |
//! | `event_group` | `GET /trade-api/v2/markets/{ticker}`, then `GET /trade-api/v2/events/{event}` with `with_nested_markets=true` | 2 |
//!
//! Each request costs its route's tokens on the client's Read layer, 10 today.

use polyoxide_core::ApiError;
use polyoxide_venue::{
    Book, Class, ClassifiedError, Extensions, Instrument, Level, MarketData, MarketKey, NotListed,
    Quote, SizeUnit, UnixMillis,
};
use polyoxide_venue::{Candle, CandleInterval, CandleQuery, Candles};
use polyoxide_venue::{EventGroup, EventGroups, EventOutcome};
use polyoxide_venue::{Side, Trade, TradeId, Trades};
use rust_decimal::Decimal;

use super::key::{self, Outcome, OutcomeId, Ticker};
use super::types::{
    self, BidAskDistribution, MarketCandlestick, MarketStatus, OrderbookFp, OutcomeSide,
    PeriodInterval, PriceRange,
};
use super::{Predictions, PredictionsError};

/// The smallest size step and the smallest order, in contracts: the
/// granularity of the `FixedPointCount` the OpenAPI gives for orders.
/// Provisional until Kalshi's `Trading` probes them (`OBSERVED.md`).
const GRANULARITY: Decimal = Decimal::from_parts(1, 0, 0, false, 2);

/// Kalshi's facts about a market, beside [`Instrument`]'s fields.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct InstrumentExt {
    /// The event the market belongs to.
    pub event_ticker: Option<String>,
    /// Where the market is in its life: active, closed, finalized and so on.
    pub status: Option<MarketStatus>,
    /// The price ranges the market trades in, each with its own step.
    pub price_ranges: Option<Vec<PriceRange>>,
    /// The name of the structure those ranges follow.
    pub price_level_structure: Option<String>,
    /// When trading closes.
    pub close_time: Option<UnixMillis>,
    /// When the market is expected to be determined, when Kalshi says.
    pub expected_expiration_time: Option<UnixMillis>,
    /// The latest time the market can be determined.
    pub latest_expiration_time: Option<UnixMillis>,
    /// What one contract pays on its outcome, in dollars.
    pub notional_value: Option<Decimal>,
    /// How the settlement value is bounded.
    pub settlement_bounds_type: Option<String>,
    /// The index the market settles against, when it has one.
    pub exchange_index: Option<u32>,
}

/// A time Kalshi sent as RFC 3339, in milliseconds since the Unix epoch.
///
/// A time that does not parse, or is before 1970, is a `Decode` error naming
/// `field`.
fn millis(field: &str, text: &str) -> Result<UnixMillis, ClassifiedError> {
    let parsed = chrono::DateTime::parse_from_rfc3339(text).map_err(|err| {
        ClassifiedError::new(
            Class::Decode,
            format!("{field} {text:?} is not an RFC 3339 time: {err}"),
        )
    })?;
    u64::try_from(parsed.timestamp_millis())
        .map(UnixMillis)
        .map_err(|_| {
            ClassifiedError::new(Class::Decode, format!("{field} {text:?} is before 1970"))
        })
}

/// Whether the venue answered `404`, in either of the error's two shapes.
fn is_not_found(err: &PredictionsError) -> bool {
    match err {
        PredictionsError::Venue(venue) => venue.status == 404,
        PredictionsError::Api(ApiError::Response(response)) => response.status.as_u16() == 404,
        _ => false,
    }
}

/// An outcome's bids and asks, each best first (D9): its own bids from the
/// highest, and the other outcome's bids at `1 - price` from the lowest.
fn sides(book: &OrderbookFp, outcome: Outcome) -> (Vec<Level>, Vec<Level>) {
    let (own, other) = match outcome {
        Outcome::Yes => (&book.yes_dollars, &book.no_dollars),
        Outcome::No => (&book.no_dollars, &book.yes_dollars),
    };
    let mut bids: Vec<Level> = own
        .iter()
        .map(|level| Level::new(level.price, level.count))
        .collect();
    let mut asks: Vec<Level> = other
        .iter()
        .map(|level| Level::new(Decimal::ONE - level.price, level.count))
        .collect();
    bids.sort_by_key(|level| std::cmp::Reverse(level.price));
    asks.sort_by_key(|level| level.price);
    (bids, asks)
}

/// The instrument a market's row describes, for one of its two outcomes.
///
/// Its name is `"<yes_sub_title> — Yes"` or `"<yes_sub_title> — No"`, by the
/// key's outcome, since Kalshi sends the same subtitle for both and a name
/// says its market and its outcome; a subtitle that is empty or only
/// whitespace gives `"<TICKER> yes"` or `"<TICKER> no"` instead.
fn instrument(
    key: &MarketKey,
    id: &OutcomeId,
    market: &types::Market,
) -> Result<Instrument, ClassifiedError> {
    let name = if market.yes_sub_title.trim().is_empty() {
        format!("{} {}", id.ticker, id.outcome.as_str())
    } else {
        let label = match id.outcome {
            Outcome::Yes => "Yes",
            Outcome::No => "No",
        };
        format!("{} — {label}", market.yes_sub_title)
    };
    let mut instrument = Instrument::new(key.clone(), name, SizeUnit::Shares);
    instrument.quote_asset = Some("USD".to_owned());
    // A fixed step exists only when the market has one price range.
    instrument.tick_size = match market.price_ranges.as_slice() {
        [only] => Some(only.step),
        _ => None,
    };
    instrument.min_size = Some(GRANULARITY);
    instrument.size_increment = Some(GRANULARITY);
    instrument.tradable = Some(market.status == MarketStatus::Active);
    instrument.extensions = Extensions::of(InstrumentExt {
        event_ticker: Some(market.event_ticker.clone()),
        status: Some(market.status.clone()),
        price_ranges: Some(market.price_ranges.clone()),
        price_level_structure: Some(market.price_level_structure.clone()),
        close_time: Some(millis("close_time", &market.close_time)?),
        expected_expiration_time: market
            .expected_expiration_time
            .as_deref()
            .map(|text| millis("expected_expiration_time", text))
            .transpose()?,
        latest_expiration_time: Some(millis(
            "latest_expiration_time",
            &market.latest_expiration_time,
        )?),
        notional_value: Some(market.notional_value_dollars),
        settlement_bounds_type: Some(market.settlement_bounds_type.clone()),
        exchange_index: market.exchange_index,
    });
    Ok(instrument)
}

impl Predictions {
    /// The outcome a key names and its market's row; `NotListed` when Kalshi
    /// answers `404` for the ticker (D11).
    async fn listed(&self, key: &MarketKey) -> Result<(OutcomeId, types::Market), ClassifiedError> {
        let id = key::outcome(key)?;
        match self.markets().get(&id.ticker).send().await {
            Ok(response) => Ok((id, response.market)),
            Err(err) if is_not_found(&err) => Err(NotListed::new(key.clone()).into()),
            Err(err) => Err(err.into()),
        }
    }
}

impl MarketData for Predictions {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let (id, market) = self.listed(key).await?;
        instrument(key, &id, &market)
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        let id = key::outcome(key)?;
        let response = self.markets().orderbook(&id.ticker).depth(1).send().await?;
        let (bids, asks) = sides(&response.orderbook_fp, id.outcome);
        let mut quote = Quote::new(key.clone());
        quote.bid = bids.first().copied();
        quote.ask = asks.first().copied();
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        let id = key::outcome(key)?;
        let response = self.markets().orderbook(&id.ticker).send().await?;
        let (bids, asks) = sides(&response.orderbook_fp, id.outcome);
        let mut book = Book::new(key.clone());
        book.bids = bids;
        book.asks = asks;
        Ok(book)
    }
}

/// Kalshi's facts about a trade, beside [`Trade`]'s fields.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct TradeExt {
    /// Whether it was a block trade.
    pub is_block_trade: Option<bool>,
}

/// Whether the taker bought `own`, the outcome a key names: when its outcome
/// side is `own` it took that outcome, and when it is the other it sold it. A
/// side this client does not know is none.
fn taker_side(taker: &OutcomeSide, own: Outcome) -> Option<Side> {
    let taker = match taker {
        OutcomeSide::Yes => Outcome::Yes,
        OutcomeSide::No => Outcome::No,
        OutcomeSide::Other(_) => return None,
    };
    Some(if taker == own { Side::Buy } else { Side::Sell })
}

/// The trade a row describes, priced for the key's outcome.
fn trade(key: &MarketKey, outcome: Outcome, row: &types::Trade) -> Result<Trade, ClassifiedError> {
    let price = match outcome {
        Outcome::Yes => row.yes_price_dollars,
        Outcome::No => row.no_price_dollars,
    };
    let time = millis("created_time", &row.created_time)?;
    let mut trade = Trade::new(key.clone(), price, row.count_fp, time);
    trade.id = Some(TradeId::new(row.trade_id.clone()));
    trade.taker_side = taker_side(&row.taker_outcome_side, outcome);
    trade.extensions = Extensions::of(TradeExt {
        is_block_trade: Some(row.is_block_trade),
    });
    Ok(trade)
}

impl Trades for Predictions {
    /// The last 1000 trades of the market, oldest first. Kalshi serves them
    /// newest first, with a cursor this method does not follow.
    async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
        let id = key::outcome(key)?;
        let page = self
            .markets()
            .trades()
            .ticker(&id.ticker)
            .limit(1000)
            .send()
            .await?;
        page.trades
            .iter()
            .rev()
            .map(|row| trade(key, id.outcome, row))
            .collect()
    }
}

/// The widths Kalshi's candlestick routes serve, shortest first: a minute, an hour and a day.
const SERVED: &[CandleInterval] = &[
    CandleInterval::Minute1,
    CandleInterval::Hour1,
    CandleInterval::Day1,
];

/// The first, highest, lowest and last price of one side's offers over a candle's period, in
/// dollars.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Ohlc {
    /// The price at the start of the period.
    pub open: Decimal,
    /// The highest price in the period.
    pub high: Decimal,
    /// The lowest price in the period.
    pub low: Decimal,
    /// The price at the end of the period.
    pub close: Decimal,
}

/// Kalshi's facts about a candle, beside [`Candle`]'s fields.
///
/// The bid and ask distributions and the open interest are in YES terms as the venue sends them,
/// whatever outcome the key names; the mean is turned over for a `no` key like the candle's
/// prices.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CandleExt {
    /// Offers to buy YES over the period.
    pub yes_bid: Ohlc,
    /// Offers to sell YES over the period.
    pub yes_ask: Ohlc,
    /// Contracts bought, without netting, by the end of the period.
    pub open_interest: Decimal,
    /// The mean trade price of the period; `None` for a period with no trade.
    pub mean: Option<Decimal>,
}

/// The wire width of a served interval.
fn wire_period(interval: CandleInterval) -> Result<PeriodInterval, ClassifiedError> {
    match interval {
        CandleInterval::Minute1 => Ok(PeriodInterval::Minute1),
        CandleInterval::Hour1 => Ok(PeriodInterval::Hour1),
        CandleInterval::Day1 => Ok(PeriodInterval::Day1),
        other => Err(ClassifiedError::new(
            Class::InvalidRequest,
            format!("{other:?} candles are not served"),
        )),
    }
}

/// A YES price as `outcome` sees it: itself for YES, `1 - price` for NO.
fn for_outcome(outcome: Outcome, price: Decimal) -> Decimal {
    match outcome {
        Outcome::Yes => price,
        Outcome::No => Decimal::ONE - price,
    }
}

/// Unix seconds, rounded up.
fn seconds_up(time: UnixMillis) -> i64 {
    i64::try_from(time.0.div_ceil(1000)).unwrap_or(i64::MAX)
}

/// Unix seconds, rounded down.
fn seconds_down(time: UnixMillis) -> i64 {
    i64::try_from(time.0 / 1000).unwrap_or(i64::MAX)
}

fn ohlc(offers: &BidAskDistribution) -> Ohlc {
    Ohlc {
        open: offers.open_dollars,
        high: offers.high_dollars,
        low: offers.low_dollars,
        close: offers.close_dollars,
    }
}

/// The candle a row describes, for `outcome`, or `None` for a period with neither a trade nor a
/// price before it.
fn to_candle(
    key: &MarketKey,
    outcome: Outcome,
    interval: CandleInterval,
    width: i64,
    row: &MarketCandlestick,
) -> Result<Option<Candle>, ClassifiedError> {
    let open_time = row
        .end_period_ts
        .checked_sub(width)
        .and_then(|seconds| u64::try_from(seconds).ok())
        .and_then(|seconds| seconds.checked_mul(1_000))
        .map(UnixMillis)
        .ok_or_else(|| {
            ClassifiedError::new(
                Class::Decode,
                format!(
                    "end_period_ts {} is less than one width after 1970",
                    row.end_period_ts
                ),
            )
        })?;
    let shown = |price: Decimal| for_outcome(outcome, price);
    let price = &row.price;
    let (open, high, low, close) = match (price.close_dollars, price.previous_dollars) {
        (Some(close), _) => {
            // A NO key's high is the YES low turned over, and its low the YES high.
            let (high, low) = match outcome {
                Outcome::Yes => (price.high_dollars, price.low_dollars),
                Outcome::No => (price.low_dollars, price.high_dollars),
            };
            (
                price.open_dollars.map(shown),
                high.map(shown),
                low.map(shown),
                shown(close),
            )
        }
        (None, Some(previous)) => (None, None, None, shown(previous)),
        (None, None) => return Ok(None),
    };
    let mut candle = Candle::new(key.clone(), interval, open_time, close);
    candle.open = open;
    candle.high = high;
    candle.low = low;
    candle.volume = Some(row.volume_fp);
    candle.extensions = Extensions::of(CandleExt {
        yes_bid: ohlc(&row.yes_bid),
        yes_ask: ohlc(&row.yes_ask),
        open_interest: row.open_interest_fp,
        mean: price.mean_dollars.map(shown),
    });
    Ok(Some(candle))
}

impl Candles for Predictions {
    fn intervals(&self) -> &'static [CandleInterval] {
        SERVED
    }

    /// The market's candles of one served width opening in the query's span, oldest first, in
    /// one request: up to 10,000 of them, which a minute query spans in a week.
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        let id = key::outcome(key)?;
        query.ensure_served(self.intervals())?;
        let period = wire_period(query.interval)?;
        let width = period.seconds();
        let end = query.end.unwrap_or_else(UnixMillis::now);
        let response = self
            .markets()
            .candlesticks(
                &id.ticker,
                period,
                seconds_up(query.start).saturating_add(width),
                seconds_down(end).saturating_add(width),
            )
            .send()
            .await?;
        let Some(market) = response
            .markets
            .into_iter()
            .find(|market| market.market_ticker == id.ticker.as_str())
        else {
            return Ok(Vec::new());
        };
        let mut candles = Vec::new();
        for row in &market.candlesticks {
            if let Some(candle) = to_candle(key, id.outcome, query.interval, width, row)? {
                if candle.open_time >= query.start && candle.open_time <= end {
                    candles.push(candle);
                }
            }
        }
        candles.sort_by_key(|candle| candle.open_time);
        Ok(candles)
    }
}

/// Kalshi's facts about an event, beside [`EventGroup`]'s fields.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct EventGroupExt {
    /// The series the event belongs to.
    pub series_ticker: Option<String>,
    /// The event's subtitle.
    pub sub_title: Option<String>,
    /// Whether exactly one of the event's markets can resolve YES.
    pub mutually_exclusive: Option<bool>,
    /// What a contract returns as collateral when the market is settled.
    pub collateral_return_type: Option<String>,
}

impl EventGroups for Predictions {
    /// The event of the key's market, with both outcomes of each of its
    /// markets. The event is read with its markets nested, so a settled market
    /// Kalshi leaves out of an event is left out here.
    async fn event_group(&self, key: &MarketKey) -> Result<EventGroup, ClassifiedError> {
        let (_, market) = self.listed(key).await?;
        let response = self
            .events()
            .get(&market.event_ticker)
            .with_nested_markets(true)
            .send()
            .await?;
        let event = response.event;
        let mut group = EventGroup::new(event.event_ticker.clone());
        group.title = Some(event.title.clone());
        for nested in &event.markets {
            // A ticker no key can carry is a response this client cannot read.
            let ticker = Ticker::new(&nested.ticker)
                .map_err(|err| ClassifiedError::new(Class::Decode, err))?;
            for (outcome, label) in [(Outcome::Yes, "Yes"), (Outcome::No, "No")] {
                let mut each =
                    EventOutcome::new(key::key(&ticker, outcome), nested.ticker.clone(), label);
                each.market_title = Some(nested.yes_sub_title.clone());
                group.outcomes.push(each);
            }
        }
        group.extensions = Extensions::of(EventGroupExt {
            series_ticker: Some(event.series_ticker.clone()),
            sub_title: Some(event.sub_title.clone()),
            mutually_exclusive: Some(event.mutually_exclusive),
            collateral_return_type: Some(event.collateral_return_type.clone()),
        });
        Ok(group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(json: &str) -> OrderbookFp {
        serde_json::from_str(json).unwrap()
    }

    fn lv(price: &str, size: &str) -> Level {
        Level::new(
            price.parse::<Decimal>().unwrap(),
            size.parse::<Decimal>().unwrap(),
        )
    }

    #[test]
    fn the_other_outcomes_bids_are_asks_at_one_minus_their_price() {
        let book = book(
            r#"{"yes_dollars":[["0.4000","10.00"],["0.4500","5.00"]],"no_dollars":[["0.5000","7.00"],["0.5200","3.00"]]}"#,
        );
        let (bids, asks) = sides(&book, Outcome::Yes);
        assert_eq!(bids, [lv("0.45", "5"), lv("0.40", "10")]);
        assert_eq!(asks, [lv("0.48", "3"), lv("0.50", "7")]);
        let (bids, asks) = sides(&book, Outcome::No);
        assert_eq!(bids, [lv("0.52", "3"), lv("0.50", "7")]);
        assert_eq!(asks, [lv("0.55", "5"), lv("0.60", "10")]);
    }

    #[test]
    fn a_missing_side_is_an_empty_list_and_the_complement_is_exact() {
        let book = book(r#"{"yes_dollars":[["0.0001","1.00"]]}"#);
        let (bids, asks) = sides(&book, Outcome::Yes);
        assert_eq!(bids, [lv("0.0001", "1")]);
        assert!(asks.is_empty());
        let (bids, asks) = sides(&book, Outcome::No);
        assert!(bids.is_empty());
        assert_eq!(asks, [lv("0.9999", "1")]);
    }

    #[test]
    fn a_time_is_read_with_any_offset_and_before_1970_is_refused() {
        assert_eq!(
            millis("t", "2026-10-18T09:10:11.250Z").unwrap(),
            UnixMillis(1_792_314_611_250)
        );
        assert_eq!(
            millis("t", "2026-10-18T11:10:11.250+02:00").unwrap(),
            UnixMillis(1_792_314_611_250)
        );
        let err = millis("created_time", "yesterday").unwrap_err();
        assert_eq!(err.class, Class::Decode);
        assert!(err.to_string().contains("created_time"), "{err}");
        let err = millis("created_time", "1960-01-01T00:00:00Z").unwrap_err();
        assert_eq!(err.class, Class::Decode);
    }
}
