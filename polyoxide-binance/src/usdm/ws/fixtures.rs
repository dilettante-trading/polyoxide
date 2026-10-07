//! The captured stream frames, one combined-stream envelope per stream kind,
//! for tests here and downstream. Provenance is in `tests/fixtures/PROVENANCE.md`.

/// `!ticker@arr`, trimmed to two rows.
pub const ALL_TICKERS: &str = include_str!("../../../tests/fixtures/ws/stream_all_ticker_arr.json");
/// `!markPrice@arr@1s`, trimmed to two rows.
pub const ALL_MARK_PRICES: &str =
    include_str!("../../../tests/fixtures/ws/stream_all_markPrice_arr_1s.json");
/// `btcusdt@aggTrade`.
pub const AGG_TRADE: &str = include_str!("../../../tests/fixtures/ws/stream_btcusdt_aggTrade.json");
/// `btcusdt@kline_1m`.
pub const KLINE: &str = include_str!("../../../tests/fixtures/ws/stream_btcusdt_kline_1m.json");
/// `btcusdt@markPrice@1s`.
pub const MARK_PRICE: &str =
    include_str!("../../../tests/fixtures/ws/stream_btcusdt_markPrice_1s.json");
/// `btcusdt@ticker`.
pub const TICKER: &str = include_str!("../../../tests/fixtures/ws/stream_btcusdt_ticker.json");
/// `btcusdt@depth20@100ms`, sides trimmed to three levels.
pub const PARTIAL_DEPTH: &str =
    include_str!("../../../tests/fixtures/ws/stream_btcusdt_depth20_100ms.json");
/// `btcusdt@bookTicker`.
pub const BOOK_TICKER: &str =
    include_str!("../../../tests/fixtures/ws/stream_btcusdt_bookTicker.json");

/// Every frame, with its fixture's file stem.
pub const ALL: &[(&str, &str)] = &[
    ("stream_all_ticker_arr", ALL_TICKERS),
    ("stream_all_markPrice_arr_1s", ALL_MARK_PRICES),
    ("stream_btcusdt_aggTrade", AGG_TRADE),
    ("stream_btcusdt_kline_1m", KLINE),
    ("stream_btcusdt_markPrice_1s", MARK_PRICE),
    ("stream_btcusdt_ticker", TICKER),
    ("stream_btcusdt_depth20_100ms", PARTIAL_DEPTH),
    ("stream_btcusdt_bookTicker", BOOK_TICKER),
];
