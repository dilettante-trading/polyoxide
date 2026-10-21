//! `polyoxide::parse_key` reads a key through the constructors of the product
//! it names, among the enabled venues.

use polyoxide::binance::usdm::{self, types::Symbol};
use polyoxide::kalshi::predictions::key::{self as kalshi_key, Outcome, Ticker};
use polyoxide::polymarket::{perps::types::InstrumentId, shared::keys};
use polyoxide::venue::{KeyError, RawKey};

fn parse(text: &str) -> Result<polyoxide::venue::MarketKey, KeyError> {
    polyoxide::parse_key(&RawKey::parse(text).unwrap())
}

#[test]
fn each_venues_keys_come_from_its_own_constructors() {
    assert_eq!(
        parse("binance.usdm:btcusdt").unwrap(),
        usdm::key(&Symbol::new("BTCUSDT").unwrap())
    );
    assert_eq!(
        parse("polymarket.perps:007").unwrap(),
        keys::perps(InstrumentId(7))
    );
    let token = "71321045679252212594626385532706912750332728571942532289631379312455583992563";
    assert_eq!(
        parse(&format!("polymarket.clob:{token}")).unwrap(),
        keys::clob(&keys::TokenId::new(token).unwrap())
    );
}

#[test]
fn a_venue_or_product_the_build_does_not_enable_is_unknown() {
    for text in [
        "acme.spot:ABC",
        "polymarket.rtds:btcusdt",
        "binance.coinm:BTCUSD_PERP",
    ] {
        let parsed = parse(text);
        assert!(
            matches!(parsed, Err(KeyError::UnknownProduct { .. })),
            "{text}: {parsed:?}"
        );
    }
}

#[test]
fn a_product_refuses_an_id_it_cannot_read() {
    assert!(matches!(
        parse("binance.usdm:BTC/USDT"),
        Err(KeyError::InvalidId { .. })
    ));
    assert!(matches!(
        parse("polymarket.clob:0x1234"),
        Err(KeyError::InvalidId { .. })
    ));
}

#[test]
fn kalshis_keys_come_from_its_own_constructors() {
    let ticker = Ticker::new("KXTEST-26OCT18-T50.5").unwrap();
    // The product canonicalizes: the ticker is uppercased and the outcome lowercased.
    let raw_yes = "kalshi.predictions:kxtest-26oct18-t50.5:YES";
    let yes = parse(raw_yes).unwrap();
    assert_eq!(yes, kalshi_key::key(&ticker, Outcome::Yes));
    let text = yes.to_string();
    assert_eq!(text, "kalshi.predictions:KXTEST-26OCT18-T50.5:yes");
    let raw_no = "kalshi.predictions:KXTEST-26OCT18-T50.5:no";
    let no = parse(raw_no).unwrap();
    assert_eq!(no, kalshi_key::key(&ticker, Outcome::No));
    let text = no.to_string();
    assert_eq!(text, "kalshi.predictions:KXTEST-26OCT18-T50.5:no");
}

#[test]
fn kalshis_other_products_and_unreadable_ids_are_refused() {
    // Kalshi's other exchange is not covered, and its name is left free.
    let parsed = parse("kalshi.margin:KXTEST-26OCT18-T50.5:yes");
    assert!(
        matches!(parsed, Err(KeyError::UnknownProduct { .. })),
        "{parsed:?}"
    );
    for text in [
        "kalshi.predictions:KXTEST-26OCT18-T50.5",
        "kalshi.predictions:KXTEST-26OCT18-T50.5:maybe",
        "kalshi.predictions:BAD/TICKER:yes",
    ] {
        let parsed = parse(text);
        assert!(
            matches!(parsed, Err(KeyError::InvalidId { .. })),
            "{text}: {parsed:?}"
        );
    }
}
