//! Spot through the venue-neutral market-data traits.
//!
//! [`Spot`] implements [`MarketData`], [`Instruments`], [`Trades`] and
//! [`Candles`] over its own routes, keyed `binance.spot:<SYMBOL>` by the
//! constructors in `spot::key`. Spot has no funding, so it implements no
//! `Funding`. What Binance alone publishes rides in this module's
//! `<Record>Ext` types, one per record, in each record's
//! [`Extensions`](polyoxide_venue::Extensions); a quote has none, since
//! `ticker/bookTicker` carries nothing beyond the record's fields.
//!
//! `instruments` skips a pair whose `exchangeInfo` row does not convert, with
//! a WARN naming the symbol and the error, rather than failing the whole
//! listing.
//!
//! | Method | Route | Weight |
//! | --- | --- | --- |
//! | `instrument` | `exchangeInfo?symbol=` | 20 |
//! | `instruments` | `exchangeInfo` | 20 |
//! | `quote` | `ticker/bookTicker` | 2 |
//! | `book` | `depth`, no limit (100 levels) | 5 |
//! | `trades` | `aggTrades` (the last 1000) | 4 |
//! | `candles` | `klines`, from `start` (up to 1000) | 2 |
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use polyoxide_binance::spot::{self, types::Symbol, Spot};
//! use polyoxide_venue::{DynMarketData, MarketData};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Spot::new()?);
//! let quote = venue.quote(&spot::key(&Symbol::new("BTCUSDT")?)).await?;
//! println!("bid {:?} ask {:?}", quote.bid, quote.ask);
//! # Ok(())
//! # }
//! ```

use polyoxide_venue::{
    Book, Candle, CandleInterval, CandleQuery, Candles, Class, ClassifiedError, Instrument,
    Instruments, Level, MarketData, MarketKey, NotListed, Quote, Side, SizeUnit, Trade, TradeId,
    Trades, UnixMillis,
};
use rust_decimal::Decimal;

use crate::spot::{
    key,
    types::{self, Filter, Interval, Symbol, SymbolInfo, SymbolStatus},
    Spot,
};

/// Binance's facts about a trading pair, beside [`Instrument`]'s fields.
///
/// It is not `Eq`: [`Filter`] is `PartialEq` alone.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct InstrumentExt {
    /// Trading, halted and so on.
    pub status: Option<SymbolStatus>,
    /// The permission groups that may trade the pair.
    pub permission_sets: Option<Vec<Vec<String>>>,
    /// The order types the pair accepts.
    pub order_types: Option<Vec<String>>,
    /// Whether spot trading is allowed.
    pub is_spot_trading_allowed: Option<bool>,
    /// Whether margin trading is allowed.
    pub is_margin_trading_allowed: Option<bool>,
    /// Every order filter, the ones [`Instrument`]'s fields come from
    /// included.
    pub filters: Option<Vec<Filter>>,
}

/// Binance's facts about a book, beside [`Book`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BookExt {
    /// The book update id of the snapshot, to align it with the depth stream.
    pub last_update_id: Option<u64>,
}

/// Binance's facts about an aggregate trade, beside [`Trade`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct TradeExt {
    /// The first trade merged into it.
    pub first_trade_id: Option<u64>,
    /// The last trade merged into it.
    pub last_trade_id: Option<u64>,
    /// Whether the trade was the best price match.
    pub is_best_match: Option<bool>,
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

fn level(level: &types::Level) -> Level {
    Level::new(level.price, level.quantity)
}

/// The instrument a pair's row describes.
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
            Filter::Notional { min_notional, .. } => instrument.min_notional = Some(*min_notional),
            _ => {}
        }
    }
    instrument.extensions = polyoxide_venue::Extensions::of(InstrumentExt {
        status: Some(info.status.clone()),
        permission_sets: Some(info.permission_sets.clone()),
        order_types: Some(info.order_types.clone()),
        is_spot_trading_allowed: Some(info.is_spot_trading_allowed),
        is_margin_trading_allowed: Some(info.is_margin_trading_allowed),
        filters: Some(info.filters.clone()),
    });
    Ok(instrument)
}

/// The kline width a candle interval is.
fn interval(interval: CandleInterval) -> Interval {
    match interval {
        CandleInterval::Minute1 => Interval::Minute1,
        CandleInterval::Minute5 => Interval::Minute5,
        CandleInterval::Minute15 => Interval::Minute15,
        CandleInterval::Hour1 => Interval::Hour1,
        CandleInterval::Hour4 => Interval::Hour4,
        CandleInterval::Day1 => Interval::Day1,
        CandleInterval::Week1 => Interval::Week1,
    }
}

impl MarketData for Spot {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let info = self
            .exchange()
            .exchange_info()
            .symbol(&symbol)
            .send()
            .await?;
        match info
            .symbols
            .iter()
            .find(|row| row.symbol == symbol.as_str())
        {
            Some(row) => instrument(row),
            None => Err(NotListed::new(key.clone()).into()),
        }
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let ticker = self.market().book_ticker(&symbol).send().await?;
        let mut quote = Quote::new(key.clone());
        quote.bid = Some(Level::new(ticker.bid_price, ticker.bid_qty));
        quote.ask = Some(Level::new(ticker.ask_price, ticker.ask_qty));
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let depth = self.market().depth(&symbol).send().await?;
        let mut book = Book::new(key.clone());
        book.bids = depth.bids.iter().map(level).collect();
        book.asks = depth.asks.iter().map(level).collect();
        book.extensions = polyoxide_venue::Extensions::of(BookExt {
            last_update_id: Some(depth.last_update_id),
        });
        Ok(book)
    }
}

impl Instruments for Spot {
    /// Every pair `exchangeInfo` lists. A row that does not convert (a symbol
    /// no key can carry) is logged at WARN with its symbol and the error, and
    /// skipped: the rest are returned.
    async fn instruments(&self) -> Result<Vec<Instrument>, ClassifiedError> {
        let info = self.exchange().exchange_info().send().await?;
        Ok(info
            .symbols
            .iter()
            .filter_map(|row| match instrument(row) {
                Ok(instrument) => Some(instrument),
                Err(err) => {
                    tracing::warn!(symbol = %row.symbol, error = %err, "skipping an exchangeInfo row that does not convert");
                    None
                }
            })
            .collect())
    }
}

impl Trades for Spot {
    async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let rows = self.market().agg_trades(&symbol).limit(1000).send().await?;
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
                    is_best_match: Some(row.is_best_match),
                });
                trade
            })
            .collect())
    }
}

impl Candles for Spot {
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let mut request = self
            .market()
            .klines(&symbol, interval(query.interval))
            .start_time(query.start.0);
        if let Some(end) = query.end {
            request = request.end_time(end.0);
        }
        // 1000 candles cost weight 2, as the default 500 does.
        let rows = request.limit(1000).send().await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_candle_interval_is_a_kline_width() {
        let widths: Vec<&str> = CandleInterval::ALL
            .iter()
            .map(|each| interval(*each).as_str())
            .collect();
        assert_eq!(widths, ["1m", "5m", "15m", "1h", "4h", "1d", "1w"]);
    }

    #[test]
    fn a_row_whose_symbol_no_key_carries_is_a_decode_failure() {
        let info: types::ExchangeInfo = serde_json::from_str(include_str!(
            "../../tests/fixtures/spot/rest/exchange_info.json"
        ))
        .unwrap();
        let mut row = info.symbols[0].clone();
        row.symbol = "BTC/USDT".to_owned();
        let err = instrument(&row).unwrap_err();
        assert_eq!(err.class, Class::Decode);
    }
}
