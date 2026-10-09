//! Live integration tests against the Polymarket Perps API.
//!
//! These hit the real host and need network access, so they are `#[ignore]`d.
//! No credentials are needed. Run with:
//! ```sh
//! cargo test -p polyoxide-perps --test live_api -- --ignored
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use polyoxide_perps::{
    api::exchange::Instrument,
    types::{BookDepth, InstrumentId, Interval, LeaderboardWindow},
    Perps,
};
use polyoxide_test_support::{environmental, fail, ResultExt};

fn client() -> Perps {
    Perps::new().or_fail("perps client")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap() // live-unwraps: the clock is after the epoch
        .as_millis() as u64
}

/// An instrument that is currently quoting, so book and bbo assertions have
/// something to look at. Selects on the precondition it asserts on.
async fn a_quoting_instrument(perps: &Perps) -> Instrument {
    let instruments = perps
        .exchange()
        .instruments()
        .send()
        .await
        .or_fail("instruments");
    // A probe that errors is skipped, but its error is kept: when every probe
    // errored, the host is failing, and that is not "no suitable market".
    let mut answered = false;
    let mut last_error = None;
    for instrument in instruments {
        let book = perps
            .market()
            .book(instrument.instrument_id)
            .depth(BookDepth::Ten)
            .send()
            .await;
        match book {
            Ok(book) => {
                if !book.bids.is_empty() && !book.asks.is_empty() {
                    return instrument;
                }
                answered = true;
            }
            Err(err) => last_error = Some(err),
        }
    }
    if let (false, Some(err)) = (answered, &last_error) {
        fail("every book probe failed", err);
    }
    environmental("no suitable market: no instrument has a two-sided book right now");
}

#[tokio::test]
#[ignore]
async fn live_ping_and_time() {
    let perps = client();
    let latency = perps.health().ping().await.or_fail("ping");
    assert!(
        latency < Duration::from_secs(10),
        "latency too high: {latency:?}"
    );
    let time = perps.health().time().send().await.or_fail("time");
    let skew = time.time.abs_diff(now_ms());
    assert!(skew < 60_000, "server clock differs from ours by {skew} ms");
}

#[tokio::test]
#[ignore]
async fn live_reference_data() {
    let perps = client();
    let exchange = perps.exchange().exchange().send().await.or_fail("exchange");
    assert_eq!(exchange.chain_id, 137);
    let assets = perps.exchange().assets().send().await.or_fail("assets");
    assert!(assets.iter().any(|a| a.asset == "pUSD"));
    let fees = perps.exchange().fees().send().await.or_fail("fees");
    assert!(!fees.fee_schedule.is_empty());
    let tiers = perps
        .exchange()
        .limit_tiers()
        .send()
        .await
        .or_fail("limit tiers");
    assert!(!tiers.is_empty());
}

#[tokio::test]
#[ignore]
async fn live_market_data_for_a_quoting_instrument() {
    let perps = client();
    let instrument = a_quoting_instrument(&perps).await;
    let iid = instrument.instrument_id;

    // The host ignores `instrument_id` on these two routes and returns every
    // instrument (`docs/specs/perps/OBSERVED.md`). Asserting the observed
    // behaviour makes an upstream fix show up as a nightly failure.
    let tickers = perps
        .market()
        .tickers()
        .instrument_id(iid)
        .send()
        .await
        .or_fail("tickers");
    assert!(
        tickers.len() > 1,
        "the tickers instrument_id filter is now honoured; update OBSERVED.md and this assertion"
    );
    assert!(tickers.iter().any(|t| t.instrument_id == iid));

    let statistics = perps
        .market()
        .statistics()
        .instrument_id(iid)
        .send()
        .await
        .or_fail("statistics");
    assert!(
        statistics.len() > 1,
        "the statistics instrument_id filter is now honoured; update OBSERVED.md and this assertion"
    );
    assert!(statistics.iter().any(|s| s.instrument_id == iid));

    let bbo = perps
        .market()
        .bbo()
        .instrument_id(iid)
        .send()
        .await
        .or_fail("bbo");
    let best = bbo.first().unwrap_or_else(|| {
        environmental("no suitable market: bbo answered no rows for the selected instrument")
    });
    assert!(best.bid_price < best.ask_price);

    let start = now_ms() - 6 * 60 * 60 * 1000;
    let klines = perps
        .market()
        .klines(iid, Interval::H1, start)
        .send()
        .await
        .or_fail("klines");
    assert!(!klines.data.is_empty());

    let marks = perps
        .market()
        .mark_history(iid, Interval::H1, start)
        .send()
        .await
        .or_fail("mark history");
    assert!(!marks.data.is_empty());

    let trades = perps.market().trades(iid).send().await.or_fail("trades");
    assert!(trades.data.iter().all(|t| t.instrument_id == iid));

    let funding = perps.market().funding(iid).send().await.or_fail("funding");
    assert!(!funding.data.is_empty());

    let index = perps
        .market()
        .index(&instrument.base_asset)
        .send()
        .await
        .or_fail("index");
    assert_eq!(index.asset, instrument.base_asset);

    let stats = perps
        .market()
        .exchange_stats(start, now_ms())
        .send()
        .await
        .or_fail("exchange stats");
    assert!(stats.end_timestamp >= stats.start_timestamp);
}

#[tokio::test]
#[ignore]
async fn live_public_lookups() {
    let perps = client();
    let board = perps
        .public()
        .leaderboard()
        .window(LeaderboardWindow::Week)
        .limit(3)
        .send()
        .await
        .or_fail("leaderboard");
    assert!(!board.entries.is_empty());
    let address = &board.entries[0].account;

    let portfolio = perps
        .public()
        .portfolio(address)
        .send()
        .await
        .or_fail("portfolio");
    let iid = portfolio
        .positions
        .first()
        .map(|p| p.instrument_id)
        .unwrap_or(InstrumentId(1));
    let fills = perps
        .public()
        .position_fills(address, iid)
        .send()
        .await
        .or_fail("position fills");
    assert!(fills.data.iter().all(|f| f.instrument_id == iid));

    let invite = perps
        .public()
        .invite("polyoxide-live-test")
        .send()
        .await
        .or_fail("invite");
    assert!(!invite.valid);
}
