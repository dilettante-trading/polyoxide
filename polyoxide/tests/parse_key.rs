//! `polyoxide::parse_key` reads a key through the constructors of the product
//! it names, among the enabled venues.

use polyoxide::binance::usdm::{self, types::Symbol};
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
