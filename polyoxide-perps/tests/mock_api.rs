//! Mock-server tests: one per builder, checking the path, the query keys the
//! builder sends, and the decoding of a representative body.

use mockito::{Matcher, Server, ServerGuard};
use polyoxide_perps::{Perps, PerpsError};

fn test_perps(server: &ServerGuard) -> Perps {
    Perps::builder().base_url(server.url()).build().unwrap()
}

// ── health ──────────────────────────────────────────────────────

#[tokio::test]
async fn ping_reports_latency_when_the_host_says_ok() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/ping")
        .with_status(200)
        .with_body(r#"{"status":"ok"}"#)
        .create_async()
        .await;

    let latency = test_perps(&server).health().ping().await.expect("ping");
    mock.assert_async().await;
    assert!(latency < std::time::Duration::from_secs(5));
}

#[tokio::test]
async fn time_returns_the_server_clock() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/time")
        .with_status(200)
        .with_body(r#"{"time":1790758431064}"#)
        .create_async()
        .await;

    let time = test_perps(&server)
        .health()
        .time()
        .send()
        .await
        .expect("time");
    mock.assert_async().await;
    assert_eq!(time.time, 1790758431064);
}

#[tokio::test]
async fn a_venue_error_body_maps_to_perps_error_venue() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/v1/info/time")
        .with_status(404)
        .with_body(r#"{"status":"err","error":"not_found"}"#)
        .create_async()
        .await;

    let err = test_perps(&server)
        .health()
        .time()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(&err, PerpsError::Venue(v) if v.code == "not_found" && v.status == 404));
}

#[tokio::test]
async fn a_non_venue_error_body_stays_an_api_error() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/v1/info/time")
        .with_status(502)
        .with_body("<html>bad gateway</html>")
        .create_async()
        .await;

    let err = test_perps(&server)
        .health()
        .time()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, PerpsError::Api(_)));
}

// ── exchange ────────────────────────────────────────────────────

use polyoxide_perps::types::{InstrumentCategory, InstrumentId, InstrumentType};

#[tokio::test]
async fn instruments_sends_every_filter_and_decodes_undocumented_fields() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/instruments")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("instrument_type".into(), "perpetual".into()),
            Matcher::UrlEncoded("category".into(), "index".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"[{"instrument_id":1,"instrument_type":"perpetual","category":"index","isolated_only":false,"symbol":"SP500-USD","display_symbol":"USA500-USD","close_only":false,"base_asset":"SP500","quote_asset":"pUSD","funding_interval":"1h","quantity_decimals":5,"price_decimals":1,"price_bounds":"0.02","liquidation_fee":"0.005","max_order_count":200,"min_notional":"10","max_market_notional":"1000000","max_limit_notional":"5000000","max_leverage":50,"risk_tiers":[{"lower_bound":"0","max_leverage":50}],"ui_live_time":1790000000000,"logo":{"light":"https://example/x-light.svg","dark":"https://example/x-dark.svg"}}]"#,
        )
        .create_async()
        .await;

    let rows = test_perps(&server)
        .exchange()
        .instruments()
        .instrument_id(InstrumentId(1))
        .instrument_type(InstrumentType::Perpetual)
        .category(InstrumentCategory::Index)
        .send()
        .await
        .expect("instruments");
    mock.assert_async().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].symbol, "SP500-USD");
    assert_eq!(rows[0].display_symbol.as_deref(), Some("USA500-USD"));
    assert_eq!(rows[0].close_only, Some(false));
    assert_eq!(rows[0].risk_tiers[0].max_leverage, 50);
    assert_eq!(rows[0].ui_live_time, Some(1790000000000));
    assert_eq!(
        rows[0].logo.as_ref().map(|l| l.dark.as_str()),
        Some("https://example/x-dark.svg")
    );
}

#[tokio::test]
async fn exchange_assets_fees_and_limit_tiers_decode() {
    let mut server = Server::new_async().await;
    let exchange = server
        .mock("GET", "/v1/info/exchange")
        .with_body(r#"{"name":"Polymarket","version":"1","chain_id":137,"contract":"0xDCa4af75705dbB50f62437045afF9921947917d2","cancel_only":false,"engine_version":"0.0.7"}"#)
        .create_async()
        .await;
    let assets = server
        .mock("GET", "/v1/info/assets")
        .with_body(r#"[{"asset":"pUSD","address":"0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB","decimals":6,"collateral_ratio":"1.00","withdrawal_fee":"0.000000"}]"#)
        .create_async()
        .await;
    let fees = server
        .mock("GET", "/v1/info/fees")
        .with_body(r#"{"fee_schedule":[{"instrument_type":"perpetual","category":"equity","taker_fee_rate":"0.0004","maker_fee_rate":"0.000125","tiers":[{"min_volume_30d":"0","taker_fee_rate":"0.0004","maker_fee_rate":"0.000125"}]}]}"#)
        .create_async()
        .await;
    let tiers = server
        .mock("GET", "/v1/info/limit-tiers")
        .with_body(r#"[{"min_volume_14d":"0","rate_per_minute_limit":750,"rate_burst_limit":500,"actions_per_minute_limit":2000,"actions_burst_limit":500,"open_orders_limit":200,"messages_per_minute":1000,"connects_per_minute_limit":60,"max_connections":10,"ws_messages_burst_limit":100,"ws_messages_per_minute_limit":1200}]"#)
        .create_async()
        .await;

    let perps = test_perps(&server);
    let x = perps.exchange().exchange().send().await.expect("exchange");
    assert_eq!(x.chain_id, 137);
    let a = perps.exchange().assets().send().await.expect("assets");
    assert_eq!(a[0].decimals, 6);
    let f = perps.exchange().fees().send().await.expect("fees");
    assert_eq!(f.fee_schedule[0].tiers.len(), 1);
    let t = perps
        .exchange()
        .limit_tiers()
        .send()
        .await
        .expect("limit tiers");
    assert_eq!(t[0].rate_per_minute_limit, 750);
    assert_eq!(t[0].ws_messages_per_minute_limit, Some(1200));
    for m in [exchange, assets, fees, tiers] {
        m.assert_async().await;
    }
}

// ── market ──────────────────────────────────────────────────────

use polyoxide_perps::types::{BookDepth, Interval, Side};
use rust_decimal::Decimal;

#[tokio::test]
async fn klines_sends_required_and_optional_params_and_decodes_rows() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/klines")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("interval".into(), "1m".into()),
            Matcher::UrlEncoded("start_timestamp".into(), "100".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "200".into()),
        ]))
        .with_body(r#"{"data":[[1790758080000,"7689.7","7689.7","7689.6","7689.6","1.47861",3]],"more":false}"#)
        .create_async()
        .await;

    let klines = test_perps(&server)
        .market()
        .klines(InstrumentId(1), Interval::M1, 100)
        .end(200)
        .send()
        .await
        .expect("klines");
    mock.assert_async().await;
    assert_eq!(klines.data[0].trades, 3);
    assert!(!klines.more);
}

#[tokio::test]
async fn book_sends_depth_and_decodes_levels() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/book")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("depth".into(), "10".into()),
        ]))
        .with_body(r#"{"instrument_id":1,"bids":[["7688.5","0.31605"]],"asks":[["7688.6","0.31358"]],"timestamp":1790758475031,"sequence":58737731190}"#)
        .create_async()
        .await;

    let book = test_perps(&server)
        .market()
        .book(InstrumentId(1))
        .depth(BookDepth::Ten)
        .send()
        .await
        .expect("book");
    mock.assert_async().await;
    assert_eq!(book.bids[0].price, Decimal::new(76885, 1));
    assert_eq!(book.sequence, 58737731190);
}

#[tokio::test]
async fn tickers_statistics_bbo_and_index_decode() {
    let mut server = Server::new_async().await;
    let tickers = server
        .mock("GET", "/v1/info/tickers")
        .match_query(Matcher::UrlEncoded("instrument_id".into(), "1".into()))
        .with_body(r#"[{"instrument_id":1,"symbol":"SP500-USD","index_price":"7686.8","mark_price":"7687.8","last_price":"7689.9","mid_price":"7687.8","open_interest":"1000","funding_rate":"0.00000625","next_funding":1790762400000,"timestamp":1790758485077}]"#)
        .create_async()
        .await;
    let statistics = server
        .mock("GET", "/v1/info/statistics")
        .match_query(Matcher::Missing)
        .with_body(r#"[{"instrument_id":1,"symbol":"SP500-USD","volume":"1255218.444899","open_price":"7682.7","klines":[[1790668800000,"7682.7","7683","7682.7","7682.9","0.06996",4]]}]"#)
        .create_async()
        .await;
    let bbo = server
        .mock("GET", "/v1/info/bbo")
        .match_query(Matcher::Missing)
        .with_body(r#"[{"instrument_id":1,"bid_price":"7687.8","bid_quantity":"6.85437","ask_price":"7687.9","ask_quantity":"0.31358","timestamp":1790758485077}]"#)
        .create_async()
        .await;
    let index = server
        .mock("GET", "/v1/info/index")
        .match_query(Matcher::UrlEncoded("asset".into(), "BTC".into()))
        .with_body(r#"{"asset":"BTC","index_price":"83019","constituents":[{"source":"binance","symbol":"BTCUSDT","weight":"0.5","price":"83020"}],"ts":1790758492229}"#)
        .create_async()
        .await;

    let perps = test_perps(&server);
    let t = perps
        .market()
        .tickers()
        .instrument_id(InstrumentId(1))
        .send()
        .await
        .expect("tickers");
    assert_eq!(t[0].next_funding, 1790762400000);
    let s = perps
        .market()
        .statistics()
        .send()
        .await
        .expect("statistics");
    assert_eq!(s[0].klines[0].trades, 4);
    let b = perps.market().bbo().send().await.expect("bbo");
    assert_eq!(b[0].ask_quantity, Decimal::new(31358, 5));
    let i = perps.market().index("BTC").send().await.expect("index");
    assert_eq!(i.constituents[0].source, "binance");
    for m in [tickers, statistics, bbo, index] {
        m.assert_async().await;
    }
}

#[tokio::test]
async fn exchange_stats_mark_history_trades_and_funding_decode() {
    let mut server = Server::new_async().await;
    let stats = server
        .mock("GET", "/v1/info/exchange-stats")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("start_timestamp".into(), "1".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "2".into()),
        ]))
        .with_body(r#"{"start_timestamp":1,"end_timestamp":2,"volume":"65715149.726359","open_interest":"75573217.100902647081712288","open_interest_timestamp":2,"fees":"1000.5"}"#)
        .create_async()
        .await;
    let empty_stats = server
        .mock("GET", "/v1/info/exchange-stats")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("start_timestamp".into(), "3".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "4".into()),
        ]))
        .with_body(r#"{"start_timestamp":3,"end_timestamp":4,"volume":"0","open_interest":null,"open_interest_timestamp":null,"fees":"0"}"#)
        .create_async()
        .await;
    let marks = server
        .mock("GET", "/v1/info/mark-history")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("interval".into(), "1h".into()),
            Matcher::UrlEncoded("start_timestamp".into(), "1".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "2".into()),
        ]))
        .with_body(r#"{"data":[[1790668800000,"7684.7"]],"more":false}"#)
        .create_async()
        .await;
    let trades = server
        .mock("GET", "/v1/info/trades")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("start_timestamp".into(), "1".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "2".into()),
        ]))
        .with_body(r#"{"data":[{"trade_id":1736331004042335,"instrument_id":1,"side":"long","price":"7689.9","quantity":"2.28875","settlement":false,"timestamp":1790758400000,"hash":"0xabc"}],"more":true}"#)
        .create_async()
        .await;
    let funding = server
        .mock("GET", "/v1/info/funding")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("start_timestamp".into(), "1".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "2".into()),
        ]))
        .with_body(
            r#"{"data":[{"funding_rate":"0.00000625","timestamp":1790755200032}],"more":false}"#,
        )
        .create_async()
        .await;

    let perps = test_perps(&server);
    let x = perps
        .market()
        .exchange_stats(1, 2)
        .send()
        .await
        .expect("exchange stats");
    // 26 significant digits on the wire, within Decimal's 28: decoded exactly.
    assert_eq!(
        x.open_interest.unwrap(),
        "75573217.100902647081712288".parse::<Decimal>().unwrap()
    );
    assert_eq!(x.open_interest_timestamp, Some(2));
    let empty = perps
        .market()
        .exchange_stats(3, 4)
        .send()
        .await
        .expect("empty exchange stats");
    assert_eq!(empty.open_interest, None);
    assert_eq!(empty.open_interest_timestamp, None);
    let m = perps
        .market()
        .mark_history(InstrumentId(1), Interval::H1, 1)
        .end(2)
        .send()
        .await
        .expect("mark history");
    assert_eq!(m.data[0].mark_price, Decimal::new(76847, 1));
    let t = perps
        .market()
        .trades(InstrumentId(1))
        .start(1)
        .end(2)
        .send()
        .await
        .expect("trades");
    assert_eq!(t.data[0].side, Side::Long);
    assert_eq!(t.data[0].settlement, Some(false));
    assert!(t.more);
    let f = perps
        .market()
        .funding(InstrumentId(1))
        .start(1)
        .end(2)
        .send()
        .await
        .expect("funding");
    assert_eq!(f.data[0].funding_rate, Decimal::new(625, 8));
    for m in [stats, empty_stats, marks, trades, funding] {
        m.assert_async().await;
    }
}

// ── public ──────────────────────────────────────────────────────

use polyoxide_perps::types::{LeaderboardSort, LeaderboardWindow, SortOrder};

#[tokio::test]
async fn leaderboard_sends_every_setter_and_decodes_the_optional_account() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/leaderboard")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("window".into(), "week".into()),
            Matcher::UrlEncoded("sort_by".into(), "account_value".into()),
            Matcher::UrlEncoded("limit".into(), "3".into()),
            Matcher::UrlEncoded("offset".into(), "6".into()),
            Matcher::UrlEncoded("address".into(), "0xabc".into()),
        ]))
        .with_body(r#"{"window":"week","sort_by":"account_value","timestamp":1790758440134,"total":7751,"entries":[{"rank":1,"account":"0x65c8","pnl":"1","notional":"2","account_value":"3"}],"account":{"account":"0xabc","pnl":"0","notional":"0","account_value":"0"}}"#)
        .create_async()
        .await;

    let board = test_perps(&server)
        .public()
        .leaderboard()
        .window(LeaderboardWindow::Week)
        .sort_by(LeaderboardSort::AccountValue)
        .limit(3)
        .offset(6)
        .address("0xabc")
        .send()
        .await
        .expect("leaderboard");
    mock.assert_async().await;
    assert_eq!(board.total, 7751);
    assert_eq!(board.entries[0].rank, 1);
    let account = board.account.expect("account echoed");
    assert_eq!(account.rank, None);
}

#[tokio::test]
async fn portfolio_position_fills_and_invite_decode() {
    let mut server = Server::new_async().await;
    let portfolio = server
        .mock("GET", "/v1/info/portfolio")
        .match_query(Matcher::UrlEncoded("address".into(), "0xabc".into()))
        .with_body(r#"{"positions":[{"instrument_id":32,"symbol":"ZEC-USD","size":"-253.4562","entry_price":"1446.1","unrealized_pnl":"10642.357770000000000000000018","return_on_equity":"0.1"}],"equity":"100","timestamp":1790758440134}"#)
        .create_async()
        .await;
    let fills = server
        .mock("GET", "/v1/info/position-fills")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), "0xabc".into()),
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("cursor".into(), "c1".into()),
            Matcher::UrlEncoded("sort".into(), "asc".into()),
        ]))
        .with_body(r#"{"data":[{"trade_id":1,"order_id":2,"instrument_id":1,"side":"short","price":"1","quantity":"2","taker":true,"fee":"0.1","fee_asset":"pUSD","previous_size":"0","previous_entry_price":"0","pnl":"0","liquidation":false,"adl":false,"timestamp":1,"hash":"0x"}],"more":true,"cursor":"c2"}"#)
        .create_async()
        .await;
    let invite = server
        .mock("GET", "/v1/info/invite")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("code".into(), "nope".into()),
            Matcher::UrlEncoded("address".into(), "0xabc".into()),
        ]))
        .with_body(r#"{"valid":false,"error":"code not found"}"#)
        .create_async()
        .await;

    let perps = test_perps(&server);
    let p = perps
        .public()
        .portfolio("0xabc")
        .send()
        .await
        .expect("portfolio");
    assert_eq!(p.positions[0].size, Decimal::new(-2534562, 4));
    // 29 significant digits on the wire. Decimal's limit is 28 fractional
    // places on a 96-bit mantissa, so this one is held exactly; a longer
    // fraction would be rounded by the same parser.
    let wire = "10642.357770000000000000000018";
    assert_eq!(
        p.positions[0].unrealized_pnl,
        wire.parse::<Decimal>().unwrap()
    );
    assert_eq!(p.positions[0].unrealized_pnl.to_string(), wire);
    assert!(p.positions[0].unrealized_pnl.to_string().len() <= 30);
    let f = perps
        .public()
        .position_fills("0xabc", InstrumentId(1))
        .cursor("c1")
        .sort(SortOrder::Asc)
        .send()
        .await
        .expect("position fills");
    assert_eq!(f.cursor.as_deref(), Some("c2"));
    assert!(f.data[0].taker);
    let i = perps
        .public()
        .invite("nope")
        .address("0xabc")
        .send()
        .await
        .expect("invite");
    assert!(!i.valid);
    assert_eq!(i.error.as_deref(), Some("code not found"));
    for m in [portfolio, fills, invite] {
        m.assert_async().await;
    }
}

// ── rate limiting ───────────────────────────────────────────────

#[tokio::test]
async fn the_default_client_paces_by_the_perps_table() {
    // `PerpsBuilder` installs `polymarket::perps_limits()` unless told
    // otherwise. Nothing else observes that: the limiter is a private field,
    // so without this test the line could be deleted and every offline test
    // would still pass. The trades row is 10 per 10 s, which `quota()` paces
    // at one request every 1.25 s (a tenth reserved, one token of depth).
    use std::time::{Duration, Instant};
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/trades")
        .match_query(Matcher::Any)
        .with_body(r#"{"data":[],"more":false}"#)
        .expect(2)
        .create_async()
        .await;

    let perps = test_perps(&server);
    let start = Instant::now();
    perps
        .market()
        .trades(InstrumentId(1))
        .send()
        .await
        .expect("first");
    perps
        .market()
        .trades(InstrumentId(1))
        .send()
        .await
        .expect("second");
    mock.assert_async().await;
    assert!(
        start.elapsed() >= Duration::from_secs(1),
        "two trades requests completed in {:?}: the default client is not paced by the perps table",
        start.elapsed()
    );
}

#[tokio::test]
async fn a_429_is_retried_and_retry_after_zero_does_not_shorten_the_backoff() {
    use std::time::{Duration, Instant};
    let mut server = Server::new_async().await;
    let throttled = server
        .mock("GET", "/v1/info/time")
        .with_status(429)
        .with_header("retry-after", "0")
        .with_body(r#"{"status":"err","error":"ip_rate_limited"}"#)
        .expect(1)
        .create_async()
        .await;
    let ok = server
        .mock("GET", "/v1/info/time")
        .with_status(200)
        .with_body(r#"{"time":1}"#)
        .expect(1)
        .create_async()
        .await;

    let start = Instant::now();
    let time = test_perps(&server)
        .health()
        .time()
        .send()
        .await
        .expect("retried to success");
    throttled.assert_async().await;
    ok.assert_async().await;
    assert_eq!(time.time, 1);
    // The client's own first backoff is the floor; a Retry-After of zero may
    // not pull the retry forward (the Cloudflare lesson in CLAUDE.md).
    // `RetryConfig::default()` starts at 500 ms and jitters down to 75% of
    // it, so the earliest the retry can legitimately land is 375 ms.
    assert!(
        start.elapsed() >= Duration::from_millis(375),
        "retry landed after {:?}: Retry-After: 0 shortened the backoff",
        start.elapsed()
    );
}
