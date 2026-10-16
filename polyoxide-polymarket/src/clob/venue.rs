//! The CLOB through the venue-neutral market-data traits.
//!
//! Each key names one outcome token, `polymarket.clob:<token id>`, made by
//! the constructors in `shared::keys`, and an outcome's book is bids and asks
//! in that outcome's own price terms. [`Clob`] implements [`MarketData`] and
//! [`Candles`] with `clob` alone; [`Trades`](polyoxide_venue::Trades) with
//! `data` as well, since the CLOB serves no public trades and the Data API
//! does; and [`EventGroups`](polyoxide_venue::EventGroups) with `gamma` as
//! well, which groups outcomes into events. What only the CLOB publishes (neg
//! risk, the book's tick size and the rest) rides in this module's
//! `<Record>Ext` types, one per record, in each record's [`Extensions`].
//!
//! | Method | Routes |
//! | --- | --- |
//! | `instrument` | `/markets-by-token/{token}`, then `/markets/{condition}` |
//! | `quote`, `book` | `/book` |
//! | `candles` | `/prices-history`, one price per interval, at most 14 days from `start`; an empty answer means none in that span |
//! | `trades` (`data`) | `/markets-by-token/{token}`, then the Data API's `/trades` |
//! | `event_group` (`gamma`) | Gamma's `/markets?clob_token_ids=`, then `/events/{id}` |
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use polyoxide_polymarket::clob::Clob;
//! use polyoxide_polymarket::shared::keys::{self, TokenId};
//! use polyoxide_venue::{DynMarketData, MarketData};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Clob::public());
//! let token = TokenId::new("71321045679252212594626385532706912750332728571942532289631379312455583992563")?;
//! let quote = venue.quote(&keys::clob(&token)).await?;
//! println!("bid {:?} ask {:?}", quote.bid, quote.ask);
//! # Ok(())
//! # }
//! ```

use std::cmp::Reverse;

use polyoxide_venue::{
    Book, Candle, CandleInterval, CandleQuery, Candles, Class, ClassifiedError, Extensions,
    Instrument, Level, MarketData, MarketKey, NotListed, Quote, SizeUnit, UnixMillis,
};
use rust_decimal::Decimal;

use crate::clob::{
    api::markets::{Market, MarketToken, OrderBook, OrderLevel, PricesHistoryQuery},
    Clob,
};
use crate::shared::keys;

/// The CLOB's facts about an outcome, beside [`Instrument`]'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct InstrumentExt {
    /// The condition id of the market the outcome is in.
    pub condition_id: Option<String>,
    /// The market's question.
    pub question: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// Whether the market is in a negative-risk event.
    pub neg_risk: Option<bool>,
    /// The negative-risk event's id.
    pub neg_risk_market_id: Option<String>,
    /// Whether the market is active.
    pub active: Option<bool>,
    /// Whether the market is closed.
    pub closed: Option<bool>,
    /// When the market ends, as the CLOB writes it.
    pub end_date_iso: Option<String>,
    /// Whether the outcome won, once the market resolved.
    pub winner: Option<bool>,
}

/// The CLOB's facts about a quote, beside [`Quote`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct QuoteExt {
    /// The last traded price.
    pub last_trade_price: Option<Decimal>,
}

/// The CLOB's facts about a book, beside [`Book`]'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BookExt {
    /// The condition id of the market the outcome is in.
    pub condition_id: Option<String>,
    /// The book's hash.
    pub hash: Option<String>,
    /// The tick size now in force, which the CLOB changes as the price
    /// nears 0 or 1.
    pub tick_size: Option<Decimal>,
    /// The smallest order size, in shares.
    pub min_order_size: Option<Decimal>,
    /// Whether the market is in a negative-risk event.
    pub neg_risk: Option<bool>,
    /// The last traded price.
    pub last_trade_price: Option<Decimal>,
}

/// A decimal the CLOB sent as a JSON number, read through its shortest
/// decimal spelling, so `0.001` is `0.001` and not the nearest binary value.
fn decimal(value: f64) -> Option<Decimal> {
    value.to_string().parse().ok()
}

/// A decimal the CLOB sent as text.
fn text_decimal(value: Option<&str>) -> Option<Decimal> {
    value.and_then(|text| text.parse().ok())
}

fn decode(what: &str) -> ClassifiedError {
    ClassifiedError::new(Class::Decode, format!("the CLOB sent {what}"))
}

fn level(level: &OrderLevel) -> Level {
    Level::new(level.price, level.size)
}

/// A book's sides best first. The CLOB sends each side worst first.
fn sides(book: &OrderBook) -> (Vec<Level>, Vec<Level>) {
    let mut bids: Vec<Level> = book.bids.iter().map(level).collect();
    let mut asks: Vec<Level> = book.asks.iter().map(level).collect();
    bids.sort_by_key(|level| Reverse(level.price));
    asks.sort_by_key(|level| level.price);
    (bids, asks)
}

fn book_time(book: &OrderBook) -> Option<UnixMillis> {
    book.timestamp.parse().ok().map(UnixMillis)
}

/// The instrument an outcome of a market is.
fn instrument(key: &MarketKey, market: &Market, outcome: &MarketToken) -> Instrument {
    let name = match &market.question {
        Some(question) => format!("{question} — {}", outcome.outcome),
        None => outcome.outcome.clone(),
    };
    let mut instrument = Instrument::new(key.clone(), name, SizeUnit::Shares);
    instrument.tick_size = market.minimum_tick_size.and_then(decimal);
    instrument.min_size = market.minimum_order_size.and_then(decimal);
    instrument.tradable = market.accepting_orders;
    instrument.extensions = Extensions::of(InstrumentExt {
        condition_id: Some(market.condition_id.clone()),
        question: market.question.clone(),
        outcome: Some(outcome.outcome.clone()),
        neg_risk: market.neg_risk,
        neg_risk_market_id: market.neg_risk_market_id.clone(),
        active: Some(market.active),
        closed: Some(market.closed),
        end_date_iso: market.end_date_iso.clone(),
        winner: outcome.winner,
    });
    instrument
}

/// The `/prices-history` fidelity, in minutes, of a candle interval.
fn fidelity(interval: CandleInterval) -> Result<i32, ClassifiedError> {
    fidelity_of(interval, interval.duration())
}

/// The CLOB's `fidelity` for a width: whole minutes, refused otherwise.
fn fidelity_of(
    interval: impl std::fmt::Debug,
    width: std::time::Duration,
) -> Result<i32, ClassifiedError> {
    let secs = width.as_secs();
    let refuse = || {
        ClassifiedError::new(
            Class::InvalidRequest,
            format!("the CLOB serves no {interval:?} price history"),
        )
    };
    if secs == 0 || !secs.is_multiple_of(60) || width.subsec_nanos() != 0 {
        return Err(refuse());
    }
    i32::try_from(secs / 60).map_err(|_| refuse())
}

/// The longest window `/prices-history` serves, in seconds. A longer one is a
/// 400 (`'startTs' and 'endTs' interval is too long`), observed at 30, 90 and
/// 365 days; 3, 7 and 14 days are served. The limit between 14 and 30 days was
/// not bisected.
const PRICES_HISTORY_WINDOW_SECONDS: i64 = 14 * 24 * 3_600;

/// Unix seconds, from milliseconds, rounded down.
fn seconds(time: UnixMillis) -> i64 {
    i64::try_from(time.0 / 1_000).unwrap_or(i64::MAX)
}

/// Unix seconds, from milliseconds, rounded up, so a point in the sub-second
/// before the time is not read as at or after it.
fn seconds_up(time: UnixMillis) -> i64 {
    i64::try_from(time.0.div_ceil(1_000)).unwrap_or(i64::MAX)
}

impl MarketData for Clob {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let token = keys::token_id(key)?;
        let found = self
            .markets()
            .market_by_token(token.as_str())
            .send()
            .await?;
        let market = self.markets().get(found.condition_id).send().await?;
        let Some(outcome) = market
            .tokens
            .iter()
            .find(|outcome| outcome.token_id.as_deref() == Some(token.as_str()))
        else {
            return Err(NotListed::new(key.clone()).into());
        };
        Ok(instrument(key, &market, outcome))
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        let token = keys::token_id(key)?;
        let book = self.markets().order_book(token.as_str()).send().await?;
        let (bids, asks) = sides(&book);
        let mut quote = Quote::new(key.clone());
        quote.bid = bids.first().copied();
        quote.ask = asks.first().copied();
        quote.time = book_time(&book);
        quote.extensions = Extensions::of(QuoteExt {
            last_trade_price: text_decimal(book.last_trade_price.as_deref()),
        });
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        let token = keys::token_id(key)?;
        let native = self.markets().order_book(token.as_str()).send().await?;
        let (bids, asks) = sides(&native);
        let mut book = Book::new(key.clone());
        book.bids = bids;
        book.asks = asks;
        book.time = book_time(&native);
        book.extensions = Extensions::of(BookExt {
            condition_id: Some(native.market.clone()),
            hash: Some(native.hash.clone()),
            tick_size: text_decimal(native.tick_size.as_deref()),
            min_order_size: text_decimal(native.min_order_size.as_deref()),
            neg_risk: native.neg_risk,
            last_trade_price: text_decimal(native.last_trade_price.as_deref()),
        });
        Ok(book)
    }
}

impl Candles for Clob {
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        let token = keys::token_id(key)?;
        let start = seconds_up(query.start);
        // The window is cut to what one request serves; the caller reads on
        // from the last candle's open time.
        let end = seconds(query.end.unwrap_or_else(UnixMillis::now))
            .min(start.saturating_add(PRICES_HISTORY_WINDOW_SECONDS));
        let params = PricesHistoryQuery {
            interval: None,
            fidelity: Some(fidelity(query.interval)?),
            start_ts: Some(start),
            end_ts: Some(end),
        };
        let history = self
            .markets()
            .prices_history_with(token.as_str(), &params)
            .send()
            .await?;
        history
            .history
            .iter()
            .map(|point| {
                let open_time = u64::try_from(point.timestamp)
                    .ok()
                    .and_then(|seconds| seconds.checked_mul(1_000))
                    .ok_or_else(|| decode("a price point before 1970"))?;
                let close =
                    decimal(point.price).ok_or_else(|| decode("a price that is not a decimal"))?;
                Ok(Candle::new(
                    key.clone(),
                    query.interval,
                    UnixMillis(open_time),
                    close,
                ))
            })
            .collect()
    }
}

#[cfg(feature = "data")]
mod trades {
    use polyoxide_venue::{
        ClassifiedError, Extensions, MarketKey, Side, Trade, Trades, UnixMillis,
    };

    use super::{decimal, decode};
    use crate::clob::Clob;
    use crate::data::types::TradeSide;
    use crate::shared::keys;

    /// The CLOB's facts about a public trade, beside [`Trade`]'s fields.
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    #[non_exhaustive]
    pub struct TradeExt {
        /// The settling transaction's hash.
        pub transaction_hash: Option<String>,
        /// The taker's proxy wallet.
        pub proxy_wallet: Option<String>,
        /// The outcome's label, such as `Yes`.
        pub outcome: Option<String>,
    }

    impl Trades for Clob {
        async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
            let token = keys::token_id(key)?;
            let found = self
                .markets()
                .market_by_token(token.as_str())
                .send()
                .await?;
            let rows = self
                .data
                .trades()
                .list()
                .market([found.condition_id])
                .taker_only(true)
                .send()
                .await?;
            // The Data API sends the newest first, and both outcomes' trades.
            rows.iter()
                .rev()
                .filter(|row| row.asset == token.as_str())
                .map(|row| {
                    let price = decimal(row.price)
                        .ok_or_else(|| decode("a price that is not a decimal"))?;
                    let size =
                        decimal(row.size).ok_or_else(|| decode("a size that is not a decimal"))?;
                    let seconds =
                        u64::try_from(row.timestamp).map_err(|_| decode("a trade before 1970"))?;
                    let mut trade = Trade::new(
                        key.clone(),
                        price,
                        size,
                        UnixMillis(seconds.saturating_mul(1_000)),
                    );
                    // A taker-only row is the taker's own side.
                    trade.taker_side = match row.side {
                        TradeSide::Buy => Some(Side::Buy),
                        TradeSide::Sell => Some(Side::Sell),
                        TradeSide::Unknown => None,
                    };
                    trade.extensions = Extensions::of(TradeExt {
                        transaction_hash: row.transaction_hash.clone(),
                        proxy_wallet: Some(row.proxy_wallet.clone()),
                        outcome: Some(row.outcome.clone()),
                    });
                    Ok(trade)
                })
                .collect()
        }
    }
}

#[cfg(feature = "data")]
pub use trades::TradeExt;

#[cfg(feature = "gamma")]
mod events {
    use polyoxide_core::{QueryBuilder, Request};
    use polyoxide_venue::{
        Class, ClassifiedError, EventGroup, EventGroups, EventOutcome, Extensions, MarketKey,
        NotListed,
    };
    use serde::Deserialize;

    use super::decode;
    use crate::clob::Clob;
    use crate::gamma::{types::ProtocolVersion, GammaError};
    use crate::shared::keys::{self, TokenId};

    /// The CLOB's facts about an event, beside [`EventGroup`]'s fields.
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    #[non_exhaustive]
    pub struct EventGroupExt {
        /// The event's slug.
        pub slug: Option<String>,
        /// Whether the event is negative-risk: its markets' Yes outcomes are
        /// mutually exclusive.
        pub neg_risk: Option<bool>,
        /// The negative-risk event's id.
        pub neg_risk_market_id: Option<String>,
    }

    /// The events a Gamma market row names. Gamma's `Market` does not model
    /// them, so this reads them alone.
    #[derive(Deserialize)]
    struct MarketEvents {
        #[serde(default)]
        events: Vec<EventId>,
    }

    #[derive(Deserialize)]
    struct EventId {
        id: String,
    }

    /// A JSON-encoded array of strings, as Gamma sends a market's outcomes
    /// and token ids.
    fn strings(json: Option<&str>, what: &str) -> Result<Vec<String>, ClassifiedError> {
        json.map_or(Ok(Vec::new()), |json| {
            serde_json::from_str(json).map_err(|_| decode(what))
        })
    }

    impl Clob {
        /// The id of the first event Gamma lists for a token's market, among
        /// the open markets or, with `closed`, the closed ones.
        async fn event_id_of(
            &self,
            token: &TokenId,
            closed: bool,
        ) -> Result<Option<String>, ClassifiedError> {
            let mut request = Request::<Vec<MarketEvents>, GammaError>::new(
                self.gamma.http_client.clone(),
                "/markets",
            )
            .query("clob_token_ids", token.as_str());
            if closed {
                request = request.query("closed", "true");
            }
            let markets = request.send().await?;
            Ok(markets
                .iter()
                .flat_map(|market| &market.events)
                .map(|event| event.id.clone())
                .next())
        }
    }

    impl EventGroups for Clob {
        async fn event_group(&self, key: &MarketKey) -> Result<EventGroup, ClassifiedError> {
            let token = keys::token_id(key)?;
            // Gamma leaves a closed market out unless asked for it, so a
            // resolved market's token is found by the second request.
            let mut event_id = self.event_id_of(&token, false).await?;
            if event_id.is_none() {
                event_id = self.event_id_of(&token, true).await?;
            }
            let Some(event_id) = event_id else {
                return Err(NotListed::new(key.clone()).into());
            };
            let event = self.gamma.events().get(event_id).send().await?;
            let mut group = EventGroup::new(event.id.clone());
            group.title = event.title.clone();
            for market in &event.markets {
                // Only a v1 market's outcomes are the CLOB's tokens.
                if market.version != Some(ProtocolVersion::V1) {
                    continue;
                }
                let tokens = strings(
                    market.clob_token_ids.as_deref(),
                    "token ids that are not a JSON array of strings",
                )?;
                let labels = strings(
                    market.outcomes.as_deref(),
                    "outcomes that are not a JSON array of strings",
                )?;
                if tokens.len() != labels.len() {
                    return Err(decode(
                        "a market whose token ids and outcomes differ in number",
                    ));
                }
                for (token, label) in tokens.into_iter().zip(labels) {
                    // A token id no key can carry is a response this client
                    // cannot read, not a value the caller sent.
                    let token = TokenId::new(token)
                        .map_err(|err| ClassifiedError::new(Class::Decode, err))?;
                    let mut outcome =
                        EventOutcome::new(keys::clob(&token), market.condition_id.clone(), label);
                    // An empty title is none, as a missing one is.
                    outcome.market_title = market
                        .group_item_title
                        .clone()
                        .filter(|title| !title.is_empty())
                        .or_else(|| Some(market.question.clone()));
                    group.outcomes.push(outcome);
                }
            }
            group.extensions = Extensions::of(EventGroupExt {
                slug: event.slug.clone(),
                neg_risk: event.neg_risk,
                neg_risk_market_id: event.neg_risk_market_id.clone(),
            });
            Ok(group)
        }
    }
}

#[cfg(feature = "gamma")]
pub use events::EventGroupExt;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::keys::TokenId;

    #[test]
    fn every_candle_interval_is_a_fidelity_in_minutes() {
        let minutes: Vec<i32> = CandleInterval::ALL
            .iter()
            .map(|each| fidelity(*each).unwrap())
            .collect();
        assert_eq!(minutes, [1, 5, 15, 60, 240, 1_440, 10_080]);
    }

    #[test]
    fn fidelity_refuses_a_width_that_is_not_whole_minutes() {
        use std::time::Duration;
        for width in [
            Duration::ZERO,
            Duration::from_secs(30),
            Duration::from_secs(90),
            Duration::from_millis(60_500),
        ] {
            let err = fidelity_of("test width", width).unwrap_err();
            assert_eq!(err.class, Class::InvalidRequest, "width {width:?}");
        }
        assert_eq!(
            fidelity_of("test width", Duration::from_secs(60)).unwrap(),
            1
        );
    }

    #[test]
    fn a_number_is_read_through_its_shortest_spelling() {
        assert_eq!(decimal(0.001), Some(Decimal::new(1, 3)));
        assert_eq!(decimal(5.0), Some(Decimal::from(5)));
        assert_eq!(decimal(0.155), Some(Decimal::new(155, 3)));
        assert_eq!(decimal(f64::NAN), None);
    }

    #[test]
    fn a_book_is_sorted_best_first_whatever_order_it_came_in() {
        let book: OrderBook = serde_json::from_value(serde_json::json!({
            "market": "0x1", "asset_id": "1", "timestamp": "1792161583225", "hash": "h",
            "bids": [{"price": "0.13", "size": "1"}, {"price": "0.15", "size": "2"}, {"price": "0.14", "size": "3"}],
            "asks": [{"price": "0.18", "size": "1"}, {"price": "0.16", "size": "2"}, {"price": "0.17", "size": "3"}],
        }))
        .unwrap();
        let (bids, asks) = sides(&book);
        let bid_prices: Vec<String> = bids.iter().map(|l| l.price.to_string()).collect();
        let ask_prices: Vec<String> = asks.iter().map(|l| l.price.to_string()).collect();
        assert_eq!(bid_prices, ["0.15", "0.14", "0.13"]);
        assert_eq!(ask_prices, ["0.16", "0.17", "0.18"]);
        assert_eq!(book_time(&book), Some(UnixMillis(1_792_161_583_225)));
    }

    #[test]
    fn a_token_id_this_module_reads_is_a_clob_key() {
        let token = TokenId::new(
            "54533043819946592547517511176940999955633860128497669742211153063842200957669",
        )
        .unwrap();
        assert_eq!(keys::token_id(&keys::clob(&token)), Ok(token));
    }
}
