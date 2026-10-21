#![cfg(feature = "market")]
//! `CandleQuery::ensure_served` and `UnservedInterval`: a width a venue does
//! not serve is refused with the widths it does, as an `InvalidRequest` that
//! no wait cures.

use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use futures_util::FutureExt;
use polyoxide_venue::{
    Candle, CandleInterval, CandleQuery, Candles, Class, ClassifiedError, Classify, DynCandles,
    KeySpace, MarketKey, ProductId, UnixMillis, UnservedInterval, VenueId,
};
use rust_decimal::Decimal;

/// The widths the made-up venue serves.
const HOURLY_AND_DAILY: &[CandleInterval] = &[CandleInterval::Hour1, CandleInterval::Day1];

fn query(interval: CandleInterval) -> CandleQuery {
    CandleQuery::new(interval, UnixMillis(7))
}

#[test]
fn a_served_width_is_accepted() {
    for width in HOURLY_AND_DAILY {
        assert_eq!(query(*width).ensure_served(HOURLY_AND_DAILY), Ok(()));
    }
}

#[test]
fn every_other_width_is_refused_with_the_widths_that_are_served() {
    let unserved = [
        CandleInterval::Minute1,
        CandleInterval::Minute5,
        CandleInterval::Minute15,
        CandleInterval::Hour4,
        CandleInterval::Week1,
    ];
    for width in unserved {
        let refusal = query(width).ensure_served(HOURLY_AND_DAILY).unwrap_err();
        assert_eq!(refusal.interval, width);
        assert_eq!(refusal.served, HOURLY_AND_DAILY);
    }
    // The two lists together are every width, so none was left unchecked.
    assert_eq!(
        unserved.len() + HOURLY_AND_DAILY.len(),
        CandleInterval::ALL.len()
    );
}

#[test]
fn a_venue_that_serves_nothing_refuses_every_width() {
    for width in CandleInterval::ALL {
        let refusal = query(*width).ensure_served(&[]).unwrap_err();
        assert_eq!(refusal.interval, *width);
        assert!(refusal.served.is_empty());
    }
}

#[test]
fn the_message_names_the_width_and_the_widths_served() {
    let refusal = query(CandleInterval::Minute5)
        .ensure_served(HOURLY_AND_DAILY)
        .unwrap_err();
    assert_eq!(
        refusal.to_string(),
        "`Minute5` candles are not served; this venue serves Hour1, Day1"
    );

    let one = query(CandleInterval::Minute1)
        .ensure_served(&[CandleInterval::Day1])
        .unwrap_err();
    assert_eq!(
        one.to_string(),
        "`Minute1` candles are not served; this venue serves Day1"
    );

    let none = query(CandleInterval::Week1).ensure_served(&[]).unwrap_err();
    assert_eq!(
        none.to_string(),
        "`Week1` candles are not served; this venue serves none"
    );
}

#[test]
fn the_refusal_is_an_invalid_request_fault_that_no_wait_cures() {
    let refusal = query(CandleInterval::Minute5)
        .ensure_served(HOURLY_AND_DAILY)
        .unwrap_err();
    assert_eq!(refusal.class(), Class::InvalidRequest);
    assert!(refusal.is_fault());
    assert!(!refusal.is_retriable());
    assert_eq!(refusal.retry_after(), None);

    // `?` turns it into the one error a venue-neutral caller handles, with
    // the same class, fault and wait, and the refusal reachable by downcast.
    let classified: ClassifiedError = refusal.clone().into();
    assert_eq!(classified.class, Class::InvalidRequest);
    assert!(classified.is_fault());
    assert!(!classified.is_retriable());
    assert_eq!(classified.retry_after(), None);
    assert_eq!(
        classified.source.downcast_ref::<UnservedInterval>(),
        Some(&refusal)
    );
}

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

fn abc() -> MarketKey {
    Spot::key(&"ABC".to_owned())
}

/// Overrides nothing, so it serves every width: the trait's default.
struct Everything;

impl Candles for Everything {
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        query.ensure_served(self.intervals())?;
        Ok(vec![Candle::new(
            key.clone(),
            query.interval,
            query.start,
            Decimal::ONE,
        )])
    }
}

/// Serves hourly and daily candles only, and counts the reads it answered.
struct Sparse {
    reads: Arc<AtomicUsize>,
}

impl Candles for Sparse {
    fn intervals(&self) -> &'static [CandleInterval] {
        HOURLY_AND_DAILY
    }

    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        query.ensure_served(self.intervals())?;
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(vec![Candle::new(
            key.clone(),
            query.interval,
            query.start,
            Decimal::ONE,
        )])
    }
}

fn sparse_venue() -> (Arc<DynCandles<'static>>, Arc<AtomicUsize>) {
    let reads = Arc::new(AtomicUsize::new(0));
    let venue = DynCandles::new_arc(Sparse {
        reads: Arc::clone(&reads),
    });
    (venue, reads)
}

#[test]
fn a_venue_that_does_not_override_serves_every_width() {
    assert_eq!(Everything.intervals(), CandleInterval::ALL);
    assert_eq!(Everything.intervals().len(), 7);
}

#[test]
fn an_override_is_seen_through_the_dyn_wrapper_and_so_is_the_default() {
    let (sparse, _) = sparse_venue();
    let everything: Arc<DynCandles<'static>> = DynCandles::new_arc(Everything);
    assert_eq!(
        sparse.intervals(),
        &[CandleInterval::Hour1, CandleInterval::Day1]
    );
    assert_eq!(everything.intervals(), CandleInterval::ALL);
}

#[test]
fn an_unserved_width_is_refused_before_the_venue_is_asked_and_a_served_one_is_not() {
    let (sparse, reads) = sparse_venue();
    let refused = sparse
        .candles(&abc(), &query(CandleInterval::Minute5))
        .now_or_never()
        .expect("the venue answers at once")
        .unwrap_err();
    assert_eq!(refused.class, Class::InvalidRequest);
    assert!(refused.is_fault());
    assert!(!refused.is_retriable());
    let refusal = refused
        .source
        .downcast_ref::<UnservedInterval>()
        .expect("the source is the UnservedInterval");
    assert_eq!(refusal.interval, CandleInterval::Minute5);
    assert_eq!(refusal.served, HOURLY_AND_DAILY);
    assert_eq!(reads.load(Ordering::SeqCst), 0);

    let candles = sparse
        .candles(&abc(), &query(CandleInterval::Hour1))
        .now_or_never()
        .expect("the venue answers at once")
        .expect("a served width is answered");
    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].interval, CandleInterval::Hour1);
    assert_eq!(candles[0].open_time, UnixMillis(7));
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn the_default_refuses_no_width() {
    let everything: Arc<DynCandles<'static>> = DynCandles::new_arc(Everything);
    for width in CandleInterval::ALL {
        let candles = everything
            .candles(&abc(), &query(*width))
            .now_or_never()
            .expect("the venue answers at once")
            .expect("every width is served by default");
        assert_eq!(candles[0].interval, *width);
    }
}
