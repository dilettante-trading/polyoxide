//! The captured stream frames, one combined-stream envelope per stream kind,
//! for tests here and downstream. Provenance is in `tests/fixtures/spot/PROVENANCE.md`.

/// `btcusdt@aggTrade`.
pub const AGG_TRADE: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_aggTrade.json");
/// `btcusdt@trade`.
pub const TRADE: &str = include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_trade.json");
/// `btcusdt@kline_1m`.
pub const KLINE: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_kline_1m.json");
/// `btcusdt@miniTicker`.
pub const MINI_TICKER: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_miniTicker.json");
/// `!miniTicker@arr`, trimmed to two rows.
pub const ALL_MINI_TICKERS: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_all_miniTicker_arr.json");
/// `btcusdt@ticker`.
pub const TICKER: &str = include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_ticker.json");
/// `btcusdt@bookTicker`.
pub const BOOK_TICKER: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_bookTicker.json");
/// `btcusdt@depth5`, the bare (1000 ms) spelling.
pub const PARTIAL_DEPTH: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_depth5.json");
/// `btcusdt@depth20@100ms`, sides trimmed to three levels.
pub const PARTIAL_DEPTH_100MS: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_depth20_100ms.json");
/// `btcusdt@avgPrice`.
pub const AVG_PRICE: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_btcusdt_avgPrice.json");
/// `<s>@miniTicker` for a listed symbol with a non-ASCII character in it.
pub const NON_ASCII_MINI_TICKER: &str =
    include_str!("../../../tests/fixtures/spot/ws/stream_non_ascii_miniTicker.json");

/// Every frame, with its fixture's file stem.
pub const ALL: &[(&str, &str)] = &[
    ("stream_btcusdt_aggTrade", AGG_TRADE),
    ("stream_btcusdt_trade", TRADE),
    ("stream_btcusdt_kline_1m", KLINE),
    ("stream_btcusdt_miniTicker", MINI_TICKER),
    ("stream_all_miniTicker_arr", ALL_MINI_TICKERS),
    ("stream_btcusdt_ticker", TICKER),
    ("stream_btcusdt_bookTicker", BOOK_TICKER),
    ("stream_btcusdt_depth5", PARTIAL_DEPTH),
    ("stream_btcusdt_depth20_100ms", PARTIAL_DEPTH_100MS),
    ("stream_btcusdt_avgPrice", AVG_PRICE),
    ("stream_non_ascii_miniTicker", NON_ASCII_MINI_TICKER),
];
