#![cfg(feature = "market")]
//! `CandleQuery::ensure_served` and `UnservedInterval`: a width a venue does
//! not serve is refused with the widths it does, as an `InvalidRequest` that
//! no wait cures.

use polyoxide_venue::{
    CandleInterval, CandleQuery, Class, ClassifiedError, Classify, UnixMillis, UnservedInterval,
};

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
