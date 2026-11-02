//! Live integration tests against `api.binance.com`.
//!
//! These hit the real host and need network access, so they are `#[ignore]`d.
//! No credentials are needed. Run with:
//! ```sh
//! cargo test -p polyoxide-binance --no-default-features --features spot --test live_spot -- --ignored
//! ```
//!
//! `live_responses_carry_no_unmodelled_keys` is the host's drift detector:
//! the published spot OpenAPI is stale and lacks fields the wire sends, so
//! nothing else notices a new field. It sees keys, and the length of the
//! positional kline and book rows. It does not see a new value of an enum,
//! which decodes as that enum's `Other` variant.

use polyoxide_test_support::agreement as common;

use std::collections::BTreeSet;
use std::time::Duration;

use polyoxide_binance::spot::{
    self,
    types::{
        AggTrade, AvgPrice, BookTicker, Depth, ExchangeInfo, Interval, Kline, PriceTicker,
        ServerTime, Symbol, SymbolStatus, Ticker24h, Trade,
    },
    Spot, SpotError,
};
use polyoxide_core::ApiError;
use polyoxide_test_support::{environmental, fail, transient, ClassifiedResultExt, ResultExt};
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Instruments, MarketData, Trades, UnixMillis,
};
use serde::{de::DeserializeOwned, Serialize};

fn client() -> Spot {
    Spot::new().or_fail("binance spot client")
}

/// BTCUSDT and, when one is listed and trading, a pair whose symbol has a
/// non-ASCII character, chosen from today's `exchangeInfo` so a delisting
/// cannot break the suite.
async fn symbols(spot: &Spot) -> Vec<Symbol> {
    let info = spot
        .exchange()
        .exchange_info()
        .send()
        .await
        .or_fail("exchangeInfo");
    let mut chosen = vec![Symbol::new("BTCUSDT").or_fail("BTCUSDT")];
    if let Some(row) = info
        .symbols
        .iter()
        .find(|row| row.status == SymbolStatus::Trading && !row.symbol.is_ascii())
    {
        chosen.push(Symbol::new(row.symbol.as_str()).or_fail(&row.symbol));
    }
    chosen
}

fn unix_minute() -> u64 {
    UnixMillis::now().0 / 60_000
}

#[tokio::test]
#[ignore]
async fn live_ping_time_and_the_weight_header() {
    let spot = client();
    let latency = spot.health().ping().await.or_fail("ping");
    assert!(latency < Duration::from_secs(10), "latency {latency:?}");

    // Two clients, each with its own budget: the second has charged only its
    // own request, so a count of 2 or more can only be the server's header,
    // which counts the first client's request too. Both counts reset on the
    // minute, so a pair that straddles one is sent again.
    for _ in 0..3 {
        let before = unix_minute();
        let (first, second) = (client(), client());
        let a = first.health().time().send().await.or_fail("time");
        let b = second.health().time().send().await.or_fail("time");
        assert!(b.server_time > 1_790_000_000_000);
        if a.server_time / 60_000 != b.server_time / 60_000 || unix_minute() != before {
            continue;
        }
        let used = second.weight_budget().used();
        assert!(
            used >= 2,
            "X-MBX-USED-WEIGHT-1M was not recorded: the budget counts {used}, and its own charge is 1"
        );
        return;
    }
    // Each pair takes a fraction of a second, so three straddles in a row is a
    // stalled host rather than bad luck, and a retry runs on fresh minutes.
    transient("three pairs of requests each straddled a minute boundary");
}

#[tokio::test]
#[ignore]
async fn live_every_market_route_answers() {
    let spot = client();
    let market = spot.market();
    for symbol in symbols(&spot).await {
        let name = symbol.as_str();
        let ticker = market
            .ticker_24h(&symbol)
            .send()
            .await
            .or_fail("ticker/24hr");
        // The record serializes to the wire's own keys.
        let sent = serde_json::to_value(&ticker).expect("a serializable record"); // live-unwraps: serializing a decoded record
        assert_eq!(sent["symbol"], name);
        let avg = market.avg_price(&symbol).send().await.or_fail("avgPrice");
        assert!(avg.price > 0.into(), "{symbol} average {}", avg.price);
        let price = market.price(&symbol).send().await.or_fail("ticker/price");
        assert_eq!(price.symbol, name);
        let book = market
            .book_ticker(&symbol)
            .send()
            .await
            .or_fail("ticker/bookTicker");
        assert_eq!(book.symbol, name);
        let depth = market.depth(&symbol).limit(5).send().await.or_fail("depth");
        assert!(depth.bids.len() <= 5 && depth.asks.len() <= 5, "{symbol}");
        let trades = market
            .trades(&symbol)
            .limit(2)
            .send()
            .await
            .or_fail("trades");
        assert!(trades.len() <= 2, "{symbol}");
        let agg = market
            .agg_trades(&symbol)
            .limit(2)
            .send()
            .await
            .or_fail("aggTrades");
        assert!(agg.len() <= 2, "{symbol}");
        for interval in [Interval::Minute1, Interval::Second1] {
            let candles = market
                .klines(&symbol, interval)
                .limit(2)
                .send()
                .await
                .or_fail("klines");
            assert!(candles.len() <= 2, "{symbol} {}", interval.as_str());
            if name == "BTCUSDT" {
                assert_eq!(candles.len(), 2, "{}", interval.as_str());
            }
        }
        let info = spot
            .exchange()
            .exchange_info()
            .symbol(&symbol)
            .send()
            .await
            .or_fail("exchangeInfo?symbol=");
        assert!(
            info.symbols.iter().any(|row| row.symbol == name),
            "{symbol}"
        );
    }

    // The routes that answer for every pair at once.
    let all = market
        .tickers_24h()
        .send()
        .await
        .or_fail("ticker/24hr, all");
    assert!(!all.is_empty());
    let prices = market.prices().send().await.or_fail("ticker/price, all");
    assert!(prices.iter().any(|row| row.symbol == "BTCUSDT"));
    let books = market
        .book_tickers()
        .send()
        .await
        .or_fail("ticker/bookTicker, all");
    assert!(books.iter().any(|row| row.symbol == "BTCUSDT"));
}

#[tokio::test]
#[ignore]
async fn live_an_unknown_symbol_is_venue_error_1121() {
    let err = client()
        .exchange()
        .exchange_info()
        .symbol(&Symbol::new("NOTASYMBOLUSDT").or_fail("NOTASYMBOLUSDT"))
        .send()
        .await
        .unwrap_err(); // live-unwraps: the venue must refuse an unknown symbol

    // Any other error fails by its class, so a 5xx or a dropped connection is
    // retried rather than filed.
    if !matches!(
        err,
        SpotError::Venue {
            status: 400,
            code: -1121,
            ..
        }
    ) {
        fail("expected venue error -1121", &err);
    }
}

/// Fetches a route's raw JSON, outside the client, for the key comparison.
async fn raw(http: &polyoxide_core::reqwest::Client, path: &str) -> String {
    let response = http
        .get(format!("https://api.binance.com{path}"))
        .send()
        .await
        .map_err(ApiError::Network)
        .or_fail(path);
    let status = response.status();
    // A refusal fails as core classifies its status, so its tag says what the
    // nightly does with it: a 451 is environmental, a 429 or a 5xx transient,
    // and a 403 from the firewall or a 418 ban real.
    if !status.is_success() {
        fail(path, &ApiError::from_response(response).await);
    }
    response
        .text()
        .await
        .map_err(ApiError::Network)
        .or_fail(path)
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("a JSON body") // live-unwraps: an assertion on the response
}

/// Positional rows of any length but `len`. The key comparison cannot see a
/// value appended to a kline or a book level, since neither has keys.
fn rows_not_of_length(path: &str, rows: &serde_json::Value, len: usize) -> Vec<String> {
    rows.as_array()
        .expect("an array of rows") // live-unwraps: an assertion on the response
        .iter()
        .map(|row| row.as_array().expect("a positional row").len()) // live-unwraps: an assertion on the response
        .filter(|&n| n != len)
        .map(|n| format!("{path}: a row of {n} values, not {len}"))
        .collect()
}

fn agrees<T: DeserializeOwned + Serialize>(path: &str, text: &str) -> Vec<String> {
    let diff = common::compare::<T>(path, text);
    // A field the type models but this response omits is not drift: it may
    // be sent only sometimes. A key the type does not model is.
    if !diff.invented.is_empty() {
        eprintln!(
            "{path}: modelled but not sent this time: {:?}",
            diff.invented
        );
    }
    diff.unmodelled
        .into_iter()
        .map(|key| format!("{path}: {key}"))
        .collect()
}

#[tokio::test]
#[ignore]
async fn live_responses_carry_no_unmodelled_keys() {
    let http = polyoxide_core::reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("a reqwest client"); // live-unwraps: builds a local client
    let mut unmodelled = Vec::new();
    let mut check = |found: Vec<String>| unmodelled.extend(found);

    let path = "/api/v3/time";
    check(agrees::<ServerTime>(path, &raw(&http, path).await));
    let path = "/api/v3/exchangeInfo";
    check(agrees::<ExchangeInfo>(path, &raw(&http, path).await));
    let path = "/api/v3/trades?symbol=BTCUSDT&limit=3";
    check(agrees::<Vec<Trade>>(path, &raw(&http, path).await));
    let path = "/api/v3/aggTrades?symbol=BTCUSDT&limit=3";
    check(agrees::<Vec<AggTrade>>(path, &raw(&http, path).await));
    for interval in ["1m", "1s"] {
        let path = format!("/api/v3/klines?symbol=BTCUSDT&interval={interval}&limit=3");
        let text = raw(&http, &path).await;
        check(agrees::<Vec<Kline>>(&path, &text));
        check(rows_not_of_length(&path, &json(&text), 12));
    }
    let path = "/api/v3/avgPrice?symbol=BTCUSDT";
    check(agrees::<AvgPrice>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/24hr?symbol=BTCUSDT";
    check(agrees::<Ticker24h>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/24hr";
    check(agrees::<Vec<Ticker24h>>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/price?symbol=BTCUSDT";
    check(agrees::<PriceTicker>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/price";
    check(agrees::<Vec<PriceTicker>>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/bookTicker?symbol=BTCUSDT";
    check(agrees::<BookTicker>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/bookTicker";
    check(agrees::<Vec<BookTicker>>(path, &raw(&http, path).await));
    let path = "/api/v3/depth?symbol=BTCUSDT&limit=5";
    let text = raw(&http, path).await;
    check(agrees::<Depth>(path, &text));
    let book = json(&text);
    check(rows_not_of_length(path, &book["bids"], 2));
    check(rows_not_of_length(path, &book["asks"], 2));

    assert!(
        unmodelled.is_empty(),
        "the host sent what the types do not model; model it and record it in \
         docs/specs/binance/OBSERVED.md: {unmodelled:#?}"
    );
}

/// Every venue method answers for BTCUSDT, and returns what its trait says:
/// the best side first, the oldest row first.
#[tokio::test]
#[ignore]
async fn live_every_venue_method_answers_for_btcusdt() {
    let spot = client();
    let key = spot::key(&Symbol::new("BTCUSDT").or_fail("BTCUSDT"));

    let instrument = ClassifiedResultExt::or_fail(spot.instrument(&key).await, "instrument");
    assert_eq!(instrument.name, "BTCUSDT");
    assert!(instrument.tick_size.is_some() && instrument.min_size.is_some());

    let quote = ClassifiedResultExt::or_fail(spot.quote(&key).await, "quote");
    assert!(
        quote.bid.is_some() && quote.ask.is_some(),
        "BTCUSDT quotes both sides"
    );
    if let (Some(bid), Some(ask)) = (&quote.bid, &quote.ask) {
        assert!(bid.price < ask.price, "{bid:?} crosses {ask:?}");
    }

    let book = ClassifiedResultExt::or_fail(spot.book(&key).await, "book");
    assert!(!book.bids.is_empty() && !book.asks.is_empty());
    assert!(
        book.bids.windows(2).all(|w| w[0].price > w[1].price),
        "bids best first"
    );
    assert!(
        book.asks.windows(2).all(|w| w[0].price < w[1].price),
        "asks best first"
    );

    let trades = ClassifiedResultExt::or_fail(spot.trades(&key).await, "trades");
    assert!(!trades.is_empty());
    assert!(
        trades.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );

    let day_ago = UnixMillis(UnixMillis::now().0 - 24 * 60 * 60 * 1000);
    let query = CandleQuery::new(CandleInterval::Hour1, day_ago);
    let candles = ClassifiedResultExt::or_fail(spot.candles(&key, &query).await, "candles");
    assert!(!candles.is_empty());
    assert!(
        candles.windows(2).all(|w| w[0].open_time < w[1].open_time),
        "oldest first"
    );
}

/// The trait's `instruments()` skips a row that does not convert, with a WARN;
/// here a skipped row fails the nightly. `instruments()` reads `exchangeInfo`
/// by itself, so a pair listed or delisted between the test's read and its own
/// makes the two disagree: the test reads the listing once more, and if that
/// one is what `instruments()` returned, the listing moved.
#[tokio::test]
#[ignore]
async fn live_instruments_returns_one_instrument_per_exchange_info_symbol() {
    let spot = client();
    let listed = spot
        .exchange()
        .exchange_info()
        .send()
        .await
        .or_fail("exchangeInfo");
    let instruments = ClassifiedResultExt::or_fail(spot.instruments().await, "instruments");

    let have: BTreeSet<&str> = instruments.iter().map(|i| i.name.as_str()).collect();
    let first: BTreeSet<&str> = listed.symbols.iter().map(|r| r.symbol.as_str()).collect();
    if have == first {
        return;
    }
    let again = spot
        .exchange()
        .exchange_info()
        .send()
        .await
        .or_fail("exchangeInfo, again");
    let second: BTreeSet<&str> = again.symbols.iter().map(|r| r.symbol.as_str()).collect();
    if have == second {
        environmental(
            "exchangeInfo changed while the test ran: instruments() read the second listing",
        );
    }
    let missing: Vec<&&str> = second.difference(&have).collect();
    let unlisted: Vec<&&str> = have.difference(&second).collect();
    assert!(
        missing.is_empty() && unlisted.is_empty(),
        "instruments() skipped these exchangeInfo symbols: {missing:?}; \
         it returned these that exchangeInfo does not list: {unlisted:?}"
    );
}
