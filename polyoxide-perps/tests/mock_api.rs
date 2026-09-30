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
            r#"[{"instrument_id":1,"instrument_type":"perpetual","category":"index","isolated_only":false,"symbol":"SP500-USD","display_symbol":"USA500-USD","close_only":false,"base_asset":"SP500","quote_asset":"pUSD","funding_interval":"1h","quantity_decimals":5,"price_decimals":1,"price_bounds":"0.02","liquidation_fee":"0.005","max_order_count":200,"min_notional":"10","max_market_notional":"1000000","max_limit_notional":"5000000","max_leverage":50,"risk_tiers":[{"lower_bound":"0","max_leverage":50}],"ui_live_time":1790000000000,"logo":"https://example/x.png"}]"#,
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
        .match_query(Matcher::Any)
        .with_body(r#"[{"instrument_id":1,"symbol":"SP500-USD","volume":"1255218.444899","open_price":"7682.7","klines":[[1790668800000,"7682.7","7683","7682.7","7682.9","0.06996",4]]}]"#)
        .create_async()
        .await;
    let bbo = server
        .mock("GET", "/v1/info/bbo")
        .match_query(Matcher::Any)
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
    // 32 significant digits on the wire; Decimal keeps 28 and rounds.
    assert!(x.open_interest > Decimal::new(75_573_217, 0));
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
    for m in [stats, marks, trades, funding] {
        m.assert_async().await;
    }
}
