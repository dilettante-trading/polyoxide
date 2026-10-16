//! USDⓈ-M through the venue-neutral market-data traits.
//!
//! [`Usdm`] implements [`MarketData`], [`Instruments`], [`Trades`], [`Candles`]
//! and [`Funding`] over its own routes, keyed `binance.usdm:<SYMBOL>` by the
//! constructors in `usdm::key`.
//! What Binance alone publishes rides in this module's `<Record>Ext` types,
//! one per record, in each record's [`Extensions`](polyoxide_venue::Extensions).
//!
//! | Method | Route | Weight |
//! | --- | --- | --- |
//! | `instrument`, `instruments` | `exchangeInfo` | 1 |
//! | `quote` | `depth`, 5 levels | 2 |
//! | `book` | `depth`, no limit (500 levels) | 1 |
//! | `trades` | `aggTrades` (the last 500) | 20 |
//! | `candles` | `klines`, from `start` (up to 500) | 5 |
//! | `funding_rate` | `premiumIndex`, and `fundingInfo` for the interval | 1, and the funding bucket |
//! | `funding_history` | `fundingRate`, from `start` (up to 100), and `fundingInfo` for the interval | funding bucket, twice |
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use polyoxide_binance::usdm::{self, types::Symbol, Usdm};
//! use polyoxide_venue::{DynMarketData, MarketData};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Usdm::new()?);
//! let quote = venue.quote(&usdm::key(&Symbol::new("BTCUSDT")?)).await?;
//! println!("bid {:?} ask {:?}", quote.bid, quote.ask);
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use polyoxide_venue::{
    Book, Candle, CandleInterval, CandleQuery, Candles, Class, ClassifiedError, Funding,
    FundingRate, Instrument, Instruments, Level, MarketData, MarketKey, NotListed, Quote, Side,
    SizeUnit, Trade, TradeId, Trades, UnixMillis,
};
use rust_decimal::Decimal;

use crate::usdm::{
    key,
    types::{self, ContractType, DepthLimit, Filter, Interval, Symbol, SymbolInfo, SymbolStatus},
    Usdm,
};

/// Binance's facts about a contract, beside [`Instrument`]'s fields.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct InstrumentExt {
    /// Perpetual, TradFi perpetual or a delivery contract.
    pub contract_type: Option<ContractType>,
    /// Trading, settling (delisted) and so on.
    pub status: Option<SymbolStatus>,
    /// The underlying pair, `BTCUSDT` for `BTCUSDT_261225`.
    pub pair: Option<String>,
    /// The margin asset.
    pub margin_asset: Option<String>,
    /// Delivery time; far in the future for a perpetual.
    pub delivery_date: Option<UnixMillis>,
    /// Listing time.
    pub onboard_date: Option<UnixMillis>,
    /// Every order filter, the ones [`Instrument`]'s fields come from
    /// included.
    pub filters: Option<Vec<Filter>>,
}

/// Binance's facts about a quote, beside [`Quote`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct QuoteExt {
    /// The book update id of the snapshot.
    pub last_update_id: Option<u64>,
    /// When the message was produced.
    pub event_time: Option<UnixMillis>,
}

/// Binance's facts about a book, beside [`Book`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BookExt {
    /// The book update id of the snapshot, to align it with the depth stream.
    pub last_update_id: Option<u64>,
    /// When the message was produced.
    pub event_time: Option<UnixMillis>,
}

/// Binance's facts about an aggregate trade, beside [`Trade`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct TradeExt {
    /// The first trade merged into it.
    pub first_trade_id: Option<u64>,
    /// The last trade merged into it.
    pub last_trade_id: Option<u64>,
    /// The size without trades against RPI (Retail Price Improvement) orders.
    pub normal_size: Option<Decimal>,
}

/// Binance's facts about a candle, beside [`Candle`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct CandleExt {
    /// When the candle closes, inclusive.
    pub close_time: Option<UnixMillis>,
    /// The size takers bought.
    pub taker_buy_volume: Option<Decimal>,
    /// The quote-currency amount takers bought.
    pub taker_buy_notional: Option<Decimal>,
}

/// Binance's facts about a funding rate, beside [`FundingRate`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct FundingExt {
    /// The mark price: now, for the rate accruing, and at settlement for a
    /// settled one (absent before about 2022).
    pub mark_price: Option<Decimal>,
    /// The index price now, for the rate accruing.
    pub index_price: Option<Decimal>,
    /// The estimated settle price, meaningful only in the hour before a
    /// settlement.
    pub estimated_settle_price: Option<Decimal>,
    /// The interest rate, for the rate accruing.
    pub interest_rate: Option<Decimal>,
}

fn level(level: &types::Level) -> Level {
    Level::new(level.price, level.quantity)
}

/// The instrument a contract's row describes.
fn instrument(info: &SymbolInfo) -> Result<Instrument, ClassifiedError> {
    // A row whose symbol no key can carry is a response this client cannot read.
    let symbol = Symbol::new(info.symbol.as_str())
        .map_err(|err| ClassifiedError::new(Class::Decode, err))?;
    let mut instrument = Instrument::new(key::key(&symbol), info.symbol.clone(), SizeUnit::Base);
    instrument.base_asset = Some(info.base_asset.clone());
    instrument.quote_asset = Some(info.quote_asset.clone());
    instrument.tradable = Some(info.status == SymbolStatus::Trading);
    for filter in &info.filters {
        match filter {
            Filter::PriceFilter { tick_size, .. } => instrument.tick_size = Some(*tick_size),
            Filter::LotSize {
                min_qty, step_size, ..
            } => {
                instrument.min_size = Some(*min_qty);
                instrument.size_increment = Some(*step_size);
            }
            Filter::MinNotional { notional } => instrument.min_notional = Some(*notional),
            _ => {}
        }
    }
    instrument.extensions = polyoxide_venue::Extensions::of(InstrumentExt {
        contract_type: Some(info.contract_type.clone()),
        status: Some(info.status.clone()),
        pair: Some(info.pair.clone()),
        margin_asset: Some(info.margin_asset.clone()),
        delivery_date: Some(UnixMillis(info.delivery_date)),
        onboard_date: Some(UnixMillis(info.onboard_date)),
        filters: Some(info.filters.clone()),
    });
    Ok(instrument)
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
                format!("USDⓈ-M serves no {other:?} klines"),
            ))
        }
    })
}

impl MarketData for Usdm {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let info = self.exchange().exchange_info().send().await?;
        match info
            .symbols
            .iter()
            .find(|row| row.symbol == symbol.as_str())
        {
            Some(row) => instrument(row),
            None => Err(NotListed { key: key.clone() }.into()),
        }
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let depth = self
            .market()
            .depth(&symbol)
            .limit(DepthLimit::Five)
            .send()
            .await?;
        let mut quote = Quote::new(key.clone());
        quote.bid = depth.bids.first().map(level);
        quote.ask = depth.asks.first().map(level);
        quote.time = Some(UnixMillis(depth.transaction_time));
        quote.extensions = polyoxide_venue::Extensions::of(QuoteExt {
            last_update_id: Some(depth.last_update_id),
            event_time: Some(UnixMillis(depth.event_time)),
        });
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let depth = self.market().depth(&symbol).send().await?;
        let mut book = Book::new(key.clone());
        book.bids = depth.bids.iter().map(level).collect();
        book.asks = depth.asks.iter().map(level).collect();
        book.time = Some(UnixMillis(depth.transaction_time));
        book.extensions = polyoxide_venue::Extensions::of(BookExt {
            last_update_id: Some(depth.last_update_id),
            event_time: Some(UnixMillis(depth.event_time)),
        });
        Ok(book)
    }
}

impl Instruments for Usdm {
    async fn instruments(&self) -> Result<Vec<Instrument>, ClassifiedError> {
        let info = self.exchange().exchange_info().send().await?;
        info.symbols.iter().map(instrument).collect()
    }
}

impl Trades for Usdm {
    async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let rows = self.market().agg_trades(&symbol).send().await?;
        Ok(rows
            .iter()
            .map(|row| {
                let mut trade =
                    Trade::new(key.clone(), row.price, row.quantity, UnixMillis(row.time));
                trade.id = Some(TradeId::new(row.id.to_string()));
                // The buyer made the market, so the taker sold.
                trade.taker_side = Some(if row.is_buyer_maker {
                    Side::Sell
                } else {
                    Side::Buy
                });
                trade.extensions = polyoxide_venue::Extensions::of(TradeExt {
                    first_trade_id: Some(row.first_trade_id),
                    last_trade_id: Some(row.last_trade_id),
                    normal_size: Some(row.normal_quantity),
                });
                trade
            })
            .collect())
    }
}

impl Candles for Usdm {
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let mut request = self
            .market()
            .klines(&symbol, interval(query.interval)?)
            .start_time(query.start.0);
        if let Some(end) = query.end {
            request = request.end_time(end.0);
        }
        let rows = request.send().await?;
        Ok(rows
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
                candle.notional = Some(row.quote_volume);
                candle.trades = Some(row.trade_count);
                candle.extensions = polyoxide_venue::Extensions::of(CandleExt {
                    close_time: Some(UnixMillis(row.close_time)),
                    taker_buy_volume: Some(row.taker_buy_base_volume),
                    taker_buy_notional: Some(row.taker_buy_quote_volume),
                });
                candle
            })
            .collect())
    }
}

/// Binance's funding interval for a contract `fundingInfo` does not list.
const DEFAULT_FUNDING_INTERVAL: Duration = Duration::from_secs(8 * 60 * 60);

impl Usdm {
    /// The length of the funding period `symbol` settles on now: its
    /// `fundingIntervalHours` in `fundingInfo`, or Binance's default of 8
    /// hours for a contract the list does not carry.
    ///
    /// One request on the funding bucket, which `fundingInfo` shares with
    /// `fundingRate`.
    async fn funding_interval(&self, symbol: &Symbol) -> Result<Duration, ClassifiedError> {
        let rows = self.exchange().funding_info().send().await?;
        match rows.iter().find(|row| row.symbol == symbol.as_ref()) {
            None => Ok(DEFAULT_FUNDING_INTERVAL),
            Some(row) if row.funding_interval_hours == 0 => Err(ClassifiedError::new(
                Class::Decode,
                format!("fundingInfo gives {symbol} a funding interval of 0 hours"),
            )),
            Some(row) => Ok(Duration::from_secs(
                u64::from(row.funding_interval_hours) * 60 * 60,
            )),
        }
    }
}

impl Funding for Usdm {
    async fn funding_rate(&self, key: &MarketKey) -> Result<FundingRate, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let index = self.market().premium_index(&symbol).send().await?;
        let interval = self.funding_interval(&symbol).await?;
        let mut rate =
            FundingRate::new(key.clone(), index.last_funding_rate, UnixMillis(index.time));
        rate.interval = Some(interval);
        // `0` is a contract with no funding scheduled.
        rate.next_funding =
            (index.next_funding_time > 0).then_some(UnixMillis(index.next_funding_time));
        rate.extensions = polyoxide_venue::Extensions::of(FundingExt {
            mark_price: Some(index.mark_price),
            index_price: Some(index.index_price),
            estimated_settle_price: Some(index.estimated_settle_price),
            interest_rate: Some(index.interest_rate),
        });
        Ok(rate)
    }

    async fn funding_history(
        &self,
        key: &MarketKey,
        start: UnixMillis,
    ) -> Result<Vec<FundingRate>, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let rows = self
            .market()
            .funding_rate()
            .symbol(&symbol)
            .start_time(start.0)
            .send()
            .await?;
        // The symbol's current interval, for every row: `fundingInfo` keeps no
        // history of it.
        let interval = self.funding_interval(&symbol).await?;
        Ok(rows
            .iter()
            .map(|row| {
                let mut rate =
                    FundingRate::new(key.clone(), row.funding_rate, UnixMillis(row.funding_time));
                rate.interval = Some(interval);
                rate.extensions = polyoxide_venue::Extensions::of(FundingExt {
                    mark_price: row.mark_price,
                    ..FundingExt::default()
                });
                rate
            })
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
    fn a_row_whose_symbol_no_key_carries_is_a_decode_failure() {
        let info: types::ExchangeInfo = serde_json::from_str(include_str!(
            "../../tests/fixtures/usdm/rest/exchange_info.json"
        ))
        .unwrap();
        let mut row = info.symbols[0].clone();
        row.symbol = "BTC/USDT".to_owned();
        let err = instrument(&row).unwrap_err();
        assert_eq!(err.class, Class::Decode);
    }
}
