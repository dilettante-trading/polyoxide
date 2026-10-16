//! Perps through the venue-neutral market-data traits.
//!
//! [`Perps`] implements [`MarketData`], [`Instruments`], [`Trades`],
//! [`Candles`] and [`Funding`] over its public `/v1/info/*` routes, keyed
//! `polymarket.perps:<instrument id>` by the constructors in
//! `shared::keys`. What only perps publishes (fee tiers, risk tiers, price
//! bounds and the rest) rides in this module's `<Record>Ext` types, one per
//! record, in each record's [`Extensions`].
//!
//! | Method | Routes |
//! | --- | --- |
//! | `instrument`, `instruments` | `/v1/info/instruments`, then `/v1/info/fees` |
//! | `quote` | `/v1/info/bbo` |
//! | `book` | `/v1/info/book` (100 levels, the host's default) |
//! | `trades` | `/v1/info/trades` |
//! | `candles` | `/v1/info/klines`, from `start` (up to 1000) |
//! | `funding_rate` | `/v1/info/tickers`, every instrument, filtered here |
//! | `funding_history` | `/v1/info/funding`, the 100 hourly settlements from `start` (`end_timestamp` is `start` + 100 h); an empty answer means none in that span |
//!
//! Perps prices have no fixed tick: a non-integer price keeps at most five
//! significant figures, so [`Instrument::tick_size`] is `None` and the
//! decimal places are in [`InstrumentExt`]. Sizes are contracts.
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use polyoxide_polymarket::perps::{types::InstrumentId, Perps};
//! use polyoxide_polymarket::shared::keys;
//! use polyoxide_venue::{DynMarketData, MarketData};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Perps::new()?);
//! let quote = venue.quote(&keys::perps(InstrumentId(1))).await?;
//! println!("bid {:?} ask {:?}", quote.bid, quote.ask);
//! # Ok(())
//! # }
//! ```

use polyoxide_venue::{
    Book, Candle, CandleInterval, CandleQuery, Candles, Class, ClassifiedError, Extensions,
    Funding, FundingRate, Instrument, Instruments, Level, MarketData, MarketKey, NotListed, Quote,
    Side, SizeUnit, Trade, TradeId, Trades, UnixMillis,
};
use rust_decimal::Decimal;

use crate::perps::{
    api::exchange::{FeeTier, FeesInfo, Instrument as PerpsInstrument, RiskTier},
    types::{self, InstrumentCategory, InstrumentType, Interval},
    Perps,
};
use crate::shared::keys;

/// Perps' facts about an instrument, beside [`Instrument`]'s fields.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct InstrumentExt {
    /// Instrument type.
    pub instrument_type: Option<InstrumentType>,
    /// Category.
    pub category: Option<InstrumentCategory>,
    /// The symbol first-party interfaces show, when it differs.
    pub display_symbol: Option<String>,
    /// Whether new positions are refused.
    pub close_only: Option<bool>,
    /// Whether only isolated margin is supported.
    pub isolated_only: Option<bool>,
    /// Decimal places of a price. A non-integer price also keeps at most five
    /// significant figures.
    pub price_decimals: Option<u32>,
    /// Decimal places of a quantity.
    pub quantity_decimals: Option<u32>,
    /// How far an order's price may stray from the mark, as a fraction.
    pub price_bounds: Option<Decimal>,
    /// Liquidation fee rate.
    pub liquidation_fee: Option<Decimal>,
    /// Most open orders.
    pub max_order_count: Option<u32>,
    /// Largest market-order notional, in USD.
    pub max_market_notional: Option<Decimal>,
    /// Largest limit-order notional, in USD.
    pub max_limit_notional: Option<Decimal>,
    /// Highest leverage.
    pub max_leverage: Option<u32>,
    /// Leverage caps by position size.
    pub risk_tiers: Option<Vec<RiskTier>>,
    /// Funding interval, such as `1h`.
    pub funding_interval: Option<String>,
    /// Base taker fee rate of the instrument's type and category, when the
    /// fee schedule has one.
    pub taker_fee_rate: Option<Decimal>,
    /// Base maker fee rate, likewise.
    pub maker_fee_rate: Option<Decimal>,
    /// The fee schedule's volume tiers for the instrument's type and
    /// category.
    pub fee_tiers: Option<Vec<FeeTier>>,
}

/// Perps' facts about a book, beside [`Book`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BookExt {
    /// The book's sequence number.
    pub sequence: Option<u64>,
}

/// Perps' facts about a trade, beside [`Trade`]'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct TradeExt {
    /// The transaction hash.
    pub hash: Option<String>,
    /// Whether it was a settlement trade. Undocumented.
    pub settlement: Option<bool>,
}

/// Perps' facts about the funding rate now accruing, beside
/// [`FundingRate`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct FundingExt {
    /// Mark price.
    pub mark_price: Option<Decimal>,
    /// Index price.
    pub index_price: Option<Decimal>,
    /// Last traded price.
    pub last_price: Option<Decimal>,
    /// Mid price.
    pub mid_price: Option<Decimal>,
    /// Open interest, in contracts.
    pub open_interest: Option<Decimal>,
}

/// The width of a funding-history window, in milliseconds: the host's cap of
/// 100 rows at its hourly settlement interval.
///
/// With only `start_timestamp` the host keeps the newest 100 rows of everything
/// since `start`, so for an old `start` the rates right after it are skipped.
/// A window of at most 100 settlements has no newer rows to push them out.
const FUNDING_WINDOW_MS: u64 = 100 * 3_600_000;

/// The instrument a row describes, with its type and category's fees.
fn instrument(row: &PerpsInstrument, fees: &FeesInfo) -> Instrument {
    let mut instrument = Instrument::new(
        keys::perps(row.instrument_id),
        row.symbol.clone(),
        SizeUnit::Contracts,
    );
    instrument.base_asset = Some(row.base_asset.clone());
    instrument.quote_asset = Some(row.quote_asset.clone());
    // `None` past the 28 places a decimal holds, which no instrument uses.
    instrument.size_increment = Decimal::try_new(1, row.quantity_decimals).ok();
    instrument.min_notional = Some(row.min_notional);
    instrument.tradable = row.close_only.map(|close_only| !close_only);
    let schedule = fees.fee_schedule.iter().find(|entry| {
        entry.instrument_type == row.instrument_type && entry.category == row.category
    });
    instrument.extensions = Extensions::of(InstrumentExt {
        instrument_type: Some(row.instrument_type),
        category: Some(row.category),
        display_symbol: row.display_symbol.clone(),
        close_only: row.close_only,
        isolated_only: Some(row.isolated_only),
        price_decimals: Some(row.price_decimals),
        quantity_decimals: Some(row.quantity_decimals),
        price_bounds: Some(row.price_bounds),
        liquidation_fee: Some(row.liquidation_fee),
        max_order_count: Some(row.max_order_count),
        max_market_notional: Some(row.max_market_notional),
        max_limit_notional: Some(row.max_limit_notional),
        max_leverage: Some(row.max_leverage),
        risk_tiers: Some(row.risk_tiers.clone()),
        funding_interval: Some(row.funding_interval.clone()),
        taker_fee_rate: schedule.map(|entry| entry.taker_fee_rate),
        maker_fee_rate: schedule.map(|entry| entry.maker_fee_rate),
        fee_tiers: schedule.map(|entry| entry.tiers.clone()),
    });
    instrument
}

/// A best price and its quantity, or no level when the quantity is zero.
fn side(price: Decimal, quantity: Decimal) -> Option<Level> {
    (!quantity.is_zero()).then_some(Level::new(price, quantity))
}

fn level(level: &types::Level) -> Level {
    Level::new(level.price, level.quantity)
}

/// The kline width a candle interval is.
fn interval(interval: CandleInterval) -> Result<Interval, ClassifiedError> {
    Ok(match interval {
        CandleInterval::Minute1 => Interval::M1,
        CandleInterval::Minute5 => Interval::M5,
        CandleInterval::Minute15 => Interval::M15,
        CandleInterval::Hour1 => Interval::H1,
        CandleInterval::Hour4 => Interval::H4,
        CandleInterval::Day1 => Interval::D1,
        CandleInterval::Week1 => Interval::W1,
        other => {
            return Err(ClassifiedError::new(
                Class::InvalidRequest,
                format!("perps serves no {other:?} klines"),
            ))
        }
    })
}

impl MarketData for Perps {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let rows = self
            .exchange()
            .instruments()
            .instrument_id(id)
            .send()
            .await?;
        let Some(row) = rows.iter().find(|row| row.instrument_id == id) else {
            return Err(NotListed { key: key.clone() }.into());
        };
        let fees = self.exchange().fees().send().await?;
        Ok(instrument(row, &fees))
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let rows = self.market().bbo().instrument_id(id).send().await?;
        let mut quote = Quote::new(key.clone());
        // An instrument the host does not know has no row, as its book is empty.
        if let Some(row) = rows.iter().find(|row| row.instrument_id == id) {
            quote.bid = side(row.bid_price, row.bid_quantity);
            quote.ask = side(row.ask_price, row.ask_quantity);
            quote.time = Some(UnixMillis(row.timestamp));
        }
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let native = self.market().book(id).send().await?;
        let mut book = Book::new(key.clone());
        book.bids = native.bids.iter().map(level).collect();
        book.asks = native.asks.iter().map(level).collect();
        book.time = Some(UnixMillis(native.timestamp));
        book.extensions = Extensions::of(BookExt {
            sequence: Some(native.sequence),
        });
        Ok(book)
    }
}

impl Instruments for Perps {
    async fn instruments(&self) -> Result<Vec<Instrument>, ClassifiedError> {
        let rows = self.exchange().instruments().send().await?;
        let fees = self.exchange().fees().send().await?;
        Ok(rows.iter().map(|row| instrument(row, &fees)).collect())
    }
}

impl Trades for Perps {
    async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let page = self.market().trades(id).send().await?;
        // The host sends the newest first.
        Ok(page
            .data
            .iter()
            .rev()
            .map(|row| {
                let mut trade = Trade::new(
                    key.clone(),
                    row.price,
                    row.quantity,
                    UnixMillis(row.timestamp),
                );
                trade.id = Some(TradeId::new(row.trade_id.to_string()));
                trade.taker_side = Some(match row.side {
                    types::Side::Long => Side::Buy,
                    types::Side::Short => Side::Sell,
                });
                trade.extensions = Extensions::of(TradeExt {
                    hash: Some(row.hash.clone()),
                    settlement: row.settlement,
                });
                trade
            })
            .collect())
    }
}

impl Candles for Perps {
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let mut request = self
            .market()
            .klines(id, interval(query.interval)?, query.start.0);
        if let Some(end) = query.end {
            request = request.end(end.0);
        }
        let page = request.send().await?;
        Ok(page
            .data
            .iter()
            .map(|row| {
                let mut candle = Candle::new(
                    key.clone(),
                    query.interval,
                    UnixMillis(row.open_time),
                    row.close,
                );
                candle.open = Some(row.open);
                candle.high = Some(row.high);
                candle.low = Some(row.low);
                candle.volume = Some(row.volume);
                candle.trades = Some(row.trades);
                candle
            })
            .collect())
    }
}

impl Funding for Perps {
    async fn funding_rate(&self, key: &MarketKey) -> Result<FundingRate, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        // The host ignores the filter and sends every instrument.
        let rows = self.market().tickers().instrument_id(id).send().await?;
        let Some(row) = rows.iter().find(|row| row.instrument_id == id) else {
            return Err(NotListed { key: key.clone() }.into());
        };
        let mut rate = FundingRate::new(key.clone(), row.funding_rate, UnixMillis(row.timestamp));
        rate.next_funding = Some(UnixMillis(row.next_funding));
        rate.extensions = Extensions::of(FundingExt {
            mark_price: Some(row.mark_price),
            index_price: Some(row.index_price),
            last_price: Some(row.last_price),
            mid_price: Some(row.mid_price),
            open_interest: Some(row.open_interest),
        });
        Ok(rate)
    }

    async fn funding_history(
        &self,
        key: &MarketKey,
        start: UnixMillis,
    ) -> Result<Vec<FundingRate>, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let page = self
            .market()
            .funding(id)
            .start(start.0)
            .end(start.0.saturating_add(FUNDING_WINDOW_MS))
            .send()
            .await?;
        // The host sends the newest first.
        Ok(page
            .data
            .iter()
            .rev()
            .map(|row| FundingRate::new(key.clone(), row.funding_rate, UnixMillis(row.timestamp)))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_candle_interval_is_a_kline_width() {
        let widths: Vec<&str> = CandleInterval::ALL
            .iter()
            .map(|each| interval(*each).unwrap().as_str())
            .collect();
        assert_eq!(widths, ["1m", "5m", "15m", "1h", "4h", "1d", "1w"]);
    }

    #[test]
    fn a_side_with_no_quantity_is_no_level() {
        assert_eq!(side(Decimal::ONE, Decimal::ZERO), None);
        assert_eq!(
            side(Decimal::ONE, Decimal::TWO),
            Some(Level::new(Decimal::ONE, Decimal::TWO))
        );
    }
}
