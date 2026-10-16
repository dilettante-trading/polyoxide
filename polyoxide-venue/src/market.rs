//! The traits a venue's products implement, so market data reads the same on
//! every venue.
//!
//! Every product implements [`MarketData`]. What only some products offer is a
//! separate trait, implemented only where offered: [`Instruments`] (the whole
//! listing, where one request or a few return it), [`Trades`], [`Candles`],
//! [`Funding`] and [`EventGroups`]. No method answers "unsupported": a product
//! that cannot do something does not implement its trait.
//!
//! Each method returns a [`ClassifiedError`], built with `?` from the module's
//! own error; the native clients keep their typed errors. A key of another
//! product is a [`KeyError`](crate::KeyError), an
//! [`InvalidRequest`](crate::Class::InvalidRequest).
//!
//! Each trait has a `Dyn` wrapper, so one value can be held as, for example,
//! `Arc<DynMarketData<'static>>` whichever venue it reads:
//!
//! ```
//! use std::sync::Arc;
//!
//! use polyoxide_venue::{
//!     Book, ClassifiedError, DynMarketData, Instrument, MarketData, MarketKey, Quote,
//! };
//!
//! struct Closed;
//!
//! impl MarketData for Closed {
//!     async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
//!         Err(polyoxide_venue::NotListed { key: key.clone() }.into())
//!     }
//!
//!     async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
//!         Ok(Quote::new(key.clone()))
//!     }
//!
//!     async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
//!         Ok(Book::new(key.clone()))
//!     }
//! }
//!
//! let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Closed);
//! # let _ = venue;
//! ```

use std::future::Future;

use crate::{
    Book, Candle, CandleQuery, ClassifiedError, EventGroup, FundingRate, Instrument, MarketKey,
    Quote, Trade, UnixMillis,
};

/// What every product implements: its instruments, quotes and books.
#[dynosaur::dynosaur(pub DynMarketData = dyn(box) MarketData)]
pub trait MarketData: Send + Sync {
    /// The instrument `key` names.
    ///
    /// For a key the venue does not list, the error's class is
    /// [`VenueRefusal`](crate::Class::VenueRefusal): a [`NotListed`](crate::NotListed)
    /// when the venue's answer identifies the instrument as unlisted, and
    /// otherwise the venue's own refusal. This is how to confirm a listing.
    /// Match on `class()`, not on the type.
    fn instrument(
        &self,
        key: &MarketKey,
    ) -> impl Future<Output = Result<Instrument, ClassifiedError>> + Send;

    /// The best bid and ask now.
    ///
    /// A key the venue does not list is a
    /// [`VenueRefusal`](crate::Class::VenueRefusal) by its class, and so is a
    /// listed instrument with no book, as the CLOB's `404` is. A venue that
    /// cannot tell an unlisted instrument from one with nothing quoted returns
    /// an empty quote instead. Use [`instrument`](MarketData::instrument) to
    /// confirm a listing.
    fn quote(&self, key: &MarketKey)
        -> impl Future<Output = Result<Quote, ClassifiedError>> + Send;

    /// The order book now, as deep as the venue serves it by default.
    ///
    /// A key the venue does not list is a
    /// [`VenueRefusal`](crate::Class::VenueRefusal) by its class, and so is a
    /// listed instrument with no book, as the CLOB's `404` is. A venue that
    /// cannot tell an unlisted instrument from one with nothing on its book
    /// returns an empty book instead. Use
    /// [`instrument`](MarketData::instrument) to confirm a listing.
    fn book(&self, key: &MarketKey) -> impl Future<Output = Result<Book, ClassifiedError>> + Send;
}

/// Every instrument a product lists, where one request or a few return them.
#[dynosaur::dynosaur(pub DynInstruments = dyn(box) Instruments)]
pub trait Instruments: Send + Sync {
    /// Every instrument the product lists.
    fn instruments(&self) -> impl Future<Output = Result<Vec<Instrument>, ClassifiedError>> + Send;
}

/// Public trades.
#[dynosaur::dynosaur(pub DynTrades = dyn(box) Trades)]
pub trait Trades: Send + Sync {
    /// Recent public trades, oldest first: as many as the venue returns in
    /// one request.
    fn trades(
        &self,
        key: &MarketKey,
    ) -> impl Future<Output = Result<Vec<Trade>, ClassifiedError>> + Send;
}

/// Candles.
#[dynosaur::dynosaur(pub DynCandles = dyn(box) Candles)]
pub trait Candles: Send + Sync {
    /// Candles opening between `query.start` and `query.end`, oldest first:
    /// as many as the venue returns in one request. Read on from the last
    /// candle's open time for more.
    ///
    /// A venue may also bound the span one request covers: the CLOB's is 14
    /// days from `query.start`. An empty answer before `query.end` then means
    /// no candles in that span, not none later, so read on from the end of the
    /// span the venue covered (`query.start` + 14 days on the CLOB).
    fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> impl Future<Output = Result<Vec<Candle>, ClassifiedError>> + Send;
}

/// A perpetual's funding.
#[dynosaur::dynosaur(pub DynFunding = dyn(box) Funding)]
pub trait Funding: Send + Sync {
    /// The rate now accruing, and when it next settles.
    fn funding_rate(
        &self,
        key: &MarketKey,
    ) -> impl Future<Output = Result<FundingRate, ClassifiedError>> + Send;

    /// Settled rates from `start`, oldest first: as many as the venue returns
    /// in one request.
    ///
    /// A venue may also bound the span one request covers: perps' is 100 hours
    /// less a minute from `start`. An empty answer before now then means no
    /// settlements in that span, not none later, so read on from the end of the
    /// span the venue covered (`start` + 100 hours less a minute on perps).
    fn funding_history(
        &self,
        key: &MarketKey,
        start: UnixMillis,
    ) -> impl Future<Output = Result<Vec<FundingRate>, ClassifiedError>> + Send;
}

/// Events that group an event-contract venue's outcomes.
#[dynosaur::dynosaur(pub DynEventGroups = dyn(box) EventGroups)]
pub trait EventGroups: Send + Sync {
    /// The event `key`'s outcome belongs to, with every outcome it groups.
    fn event_group(
        &self,
        key: &MarketKey,
    ) -> impl Future<Output = Result<EventGroup, ClassifiedError>> + Send;
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;

    use rust_decimal::Decimal;

    use super::*;
    use crate::{
        CandleInterval, Class, EventOutcome, KeySpace, Level, NotListed, ProductId, SizeUnit,
        VenueId,
    };

    struct Spot;

    impl KeySpace for Spot {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("spot");
        type Id = String;

        fn parse_id(text: &str) -> Result<String, String> {
            Ok(text.to_owned())
        }

        fn id_text(id: &String) -> Cow<'_, str> {
            Cow::Borrowed(id)
        }
    }

    /// One instrument, `acme.spot:ABC`, with a one-level book.
    struct Acme;

    impl Acme {
        fn check(key: &MarketKey) -> Result<String, ClassifiedError> {
            let id = Spot::id(key)?;
            if id != "ABC" {
                return Err(NotListed { key: key.clone() }.into());
            }
            Ok(id)
        }
    }

    impl MarketData for Acme {
        async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
            let id = Self::check(key)?;
            Ok(Instrument::new(key.clone(), id, SizeUnit::Base))
        }

        async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
            Self::check(key)?;
            let mut quote = Quote::new(key.clone());
            quote.bid = Some(Level::new(Decimal::ONE, Decimal::TWO));
            Ok(quote)
        }

        async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
            Self::check(key)?;
            let mut book = Book::new(key.clone());
            book.bids.push(Level::new(Decimal::ONE, Decimal::TWO));
            Ok(book)
        }
    }

    impl Instruments for Acme {
        async fn instruments(&self) -> Result<Vec<Instrument>, ClassifiedError> {
            let key = Spot::key(&"ABC".to_owned());
            Ok(vec![self.instrument(&key).await?])
        }
    }

    impl Trades for Acme {
        async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
            Self::check(key)?;
            Ok(vec![Trade::new(
                key.clone(),
                Decimal::ONE,
                Decimal::ONE,
                UnixMillis(1),
            )])
        }
    }

    impl Candles for Acme {
        async fn candles(
            &self,
            key: &MarketKey,
            query: &CandleQuery,
        ) -> Result<Vec<Candle>, ClassifiedError> {
            Self::check(key)?;
            Ok(vec![Candle::new(
                key.clone(),
                query.interval,
                query.start,
                Decimal::ONE,
            )])
        }
    }

    impl Funding for Acme {
        async fn funding_rate(&self, key: &MarketKey) -> Result<FundingRate, ClassifiedError> {
            Self::check(key)?;
            Ok(FundingRate::new(key.clone(), Decimal::ONE, UnixMillis(1)))
        }

        async fn funding_history(
            &self,
            key: &MarketKey,
            start: UnixMillis,
        ) -> Result<Vec<FundingRate>, ClassifiedError> {
            Self::check(key)?;
            Ok(vec![FundingRate::new(key.clone(), Decimal::ONE, start)])
        }
    }

    impl EventGroups for Acme {
        async fn event_group(&self, key: &MarketKey) -> Result<EventGroup, ClassifiedError> {
            Self::check(key)?;
            let mut group = EventGroup::new("E");
            group
                .outcomes
                .push(EventOutcome::new(key.clone(), "M", "Yes"));
            Ok(group)
        }
    }

    fn abc() -> MarketKey {
        Spot::key(&"ABC".to_owned())
    }

    /// Polls a future that never waits to completion.
    fn ready<F: Future>(future: F) -> F::Output {
        use std::pin::pin;
        use std::task::{Context, Poll, Waker};

        let mut future = pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("the test venue never waits"),
        }
    }

    #[test]
    fn every_trait_is_send_and_sync_and_held_through_its_dyn_wrapper() {
        fn send_sync<T: Send + Sync + ?Sized>() {}
        send_sync::<DynMarketData<'static>>();
        send_sync::<DynInstruments<'static>>();
        send_sync::<DynTrades<'static>>();
        send_sync::<DynCandles<'static>>();
        send_sync::<DynFunding<'static>>();
        send_sync::<DynEventGroups<'static>>();

        let market: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Acme);
        let listing: Arc<DynInstruments<'static>> = DynInstruments::new_arc(Acme);
        let trades: Arc<DynTrades<'static>> = DynTrades::new_arc(Acme);
        let candles: Arc<DynCandles<'static>> = DynCandles::new_arc(Acme);
        let funding: Arc<DynFunding<'static>> = DynFunding::new_arc(Acme);
        let groups: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(Acme);

        let key = abc();
        assert_eq!(
            ready(market.quote(&key)).unwrap().bid,
            Some(Level::new(Decimal::ONE, Decimal::TWO))
        );
        assert_eq!(ready(market.book(&key)).unwrap().bids.len(), 1);
        assert_eq!(ready(market.instrument(&key)).unwrap().key, key);
        assert_eq!(ready(listing.instruments()).unwrap().len(), 1);
        assert_eq!(ready(trades.trades(&key)).unwrap().len(), 1);
        let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(7));
        assert_eq!(
            ready(candles.candles(&key, &query)).unwrap()[0].open_time,
            UnixMillis(7)
        );
        assert!(ready(funding.funding_rate(&key)).is_ok());
        assert_eq!(
            ready(funding.funding_history(&key, UnixMillis(3))).unwrap()[0].time,
            UnixMillis(3)
        );
        assert_eq!(
            ready(groups.event_group(&key)).unwrap().outcomes[0].key,
            key
        );
    }

    #[test]
    fn a_key_of_another_product_is_an_invalid_request_and_an_unknown_one_a_refusal() {
        let market: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Acme);
        struct Other;
        impl KeySpace for Other {
            const VENUE: VenueId = VenueId::from_static("other");
            const PRODUCT: ProductId = ProductId::from_static("spot");
            type Id = String;
            fn parse_id(text: &str) -> Result<String, String> {
                Ok(text.to_owned())
            }
            fn id_text(id: &String) -> Cow<'_, str> {
                Cow::Borrowed(id)
            }
        }
        let foreign = Other::key(&"ABC".to_owned());
        let err = ready(market.quote(&foreign)).unwrap_err();
        assert_eq!(err.class, Class::InvalidRequest);

        let unknown = Spot::key(&"XYZ".to_owned());
        let err = ready(market.book(&unknown)).unwrap_err();
        assert_eq!(err.class, Class::VenueRefusal { code: None });
        assert!(err.source.downcast_ref::<NotListed>().is_some());
    }
}
