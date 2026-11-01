//! Pages decoded row by row: strict about a page's envelope, and per row about its items, so a row
//! the types cannot read is skipped and the page, its `cursor` included, is still returned.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use super::events::{Event, EventsPage};
use super::markets::{Market, MarketOrderbook, MarketsPage, OrderbooksResponse, Trade, TradesPage};

/// A page decoded row by row: its envelope and the rows that decoded, and the rows that did not.
///
/// The `send_lossy` of a listing returns it. The envelope is as strict as `send`'s: a response
/// whose `cursor` or list is missing or of the wrong type fails as a whole.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Lossy<P> {
    /// The page, holding only the rows that decoded, in the order sent; every other field
    /// (`cursor`, `milestones`) as sent.
    pub page: P,
    /// The rows that did not decode, in the order sent.
    pub skipped: Vec<SkippedRow>,
}

/// A row of a page that did not decode into its type.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SkippedRow {
    /// Its position in the response's list, counting every row, from 0.
    pub index: usize,
    /// Its key, when the row carries the key field as a string: a market's, a trade's or a
    /// book's `ticker`, an event's `event_ticker`.
    pub key: Option<String>,
    /// Why it did not decode: serde's message.
    pub error: String,
    /// The row as sent.
    pub row: Value,
}

/// A page whose rows can be decoded one by one: the field that holds them, their type, and the
/// field a skipped row's key is read from.
pub(crate) trait LossyRows: DeserializeOwned {
    /// The type of one row.
    type Row: DeserializeOwned;
    /// The field that holds the rows.
    const ROWS: &'static str;
    /// The row's field [`SkippedRow::key`] reads.
    const KEY: &'static str;
    /// The decoded rows.
    fn rows_mut(&mut self) -> &mut Vec<Self::Row>;
}

impl LossyRows for MarketsPage {
    type Row = Market;
    const ROWS: &'static str = "markets";
    const KEY: &'static str = "ticker";
    fn rows_mut(&mut self) -> &mut Vec<Market> {
        &mut self.markets
    }
}

impl LossyRows for TradesPage {
    type Row = Trade;
    const ROWS: &'static str = "trades";
    const KEY: &'static str = "ticker";
    fn rows_mut(&mut self) -> &mut Vec<Trade> {
        &mut self.trades
    }
}

impl LossyRows for OrderbooksResponse {
    type Row = MarketOrderbook;
    const ROWS: &'static str = "orderbooks";
    const KEY: &'static str = "ticker";
    fn rows_mut(&mut self) -> &mut Vec<MarketOrderbook> {
        &mut self.orderbooks
    }
}

impl LossyRows for EventsPage {
    type Row = Event;
    const ROWS: &'static str = "events";
    const KEY: &'static str = "event_ticker";
    fn rows_mut(&mut self) -> &mut Vec<Event> {
        &mut self.events
    }
}

/// Decodes `body` as `P`, strict about the envelope and per row about the rows: a row that does
/// not decode is skipped, with one WARN, and the rest are kept in order. `path` names the request
/// in the log lines.
pub(crate) fn decode_lossy<P: LossyRows>(
    path: &str,
    mut body: Value,
) -> Result<Lossy<P>, serde_json::Error> {
    // Only a list is taken out; a missing, null or other field is left for the envelope's decode
    // to refuse, as `send`'s would.
    let rows = match body.get_mut(P::ROWS) {
        Some(Value::Array(rows)) => std::mem::take(rows),
        _ => Vec::new(),
    };
    let mut page = match P::deserialize(&body) {
        Ok(page) => page,
        Err(err) => {
            tracing::error!(
                "Failed to decode {path}: {err}: {}",
                polyoxide_core::truncate_for_log(&body.to_string())
            );
            return Err(err);
        }
    };
    let mut skipped = Vec::new();
    for (index, row) in rows.into_iter().enumerate() {
        match P::Row::deserialize(&row) {
            Ok(item) => page.rows_mut().push(item),
            Err(err) => {
                let key = row.get(P::KEY).and_then(Value::as_str).map(str::to_owned);
                let error = err.to_string();
                tracing::warn!(
                    path,
                    index,
                    key = ?key,
                    error = %error,
                    "skipping a row that does not decode"
                );
                skipped.push(SkippedRow {
                    index,
                    key,
                    error,
                    row,
                });
            }
        }
    }
    Ok(Lossy { page, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(ticker: &str) -> Value {
        json!({
            "event_ticker": ticker,
            "series_ticker": "KXS",
            "sub_title": "s",
            "title": "t",
            "collateral_return_type": "",
            "mutually_exclusive": false,
        })
    }

    fn events_page(rows: Vec<Value>) -> Value {
        json!({ "events": rows, "milestones": [{"id": "m"}], "cursor": "next" })
    }

    fn trade(ticker: &str) -> Value {
        json!({
            "trade_id": "t1",
            "ticker": ticker,
            "count_fp": "1.00",
            "yes_price_dollars": "0.4200",
            "no_price_dollars": "0.5800",
            "taker_outcome_side": "yes",
            "taker_book_side": "bid",
            "created_time": "2026-10-22T00:00:00Z",
            "is_block_trade": false,
        })
    }

    fn book(ticker: &str) -> Value {
        json!({
            "ticker": ticker,
            "orderbook_fp": { "yes_dollars": [["0.4200", "3.00"]], "no_dollars": [] },
        })
    }

    fn tickers(page: &EventsPage) -> Vec<&str> {
        page.events
            .iter()
            .map(|e| e.event_ticker.as_str())
            .collect()
    }

    #[test]
    fn a_bad_row_is_skipped_and_the_rows_around_it_and_the_cursor_are_kept() {
        let mut bad = event("KXB");
        bad.as_object_mut().unwrap().remove("title");
        let body = events_page(vec![event("KXA"), bad.clone(), event("KXC")]);

        let lossy = decode_lossy::<EventsPage>("/events", body).unwrap();

        assert_eq!(tickers(&lossy.page), ["KXA", "KXC"]);
        assert_eq!(lossy.page.cursor, "next");
        assert_eq!(lossy.skipped.len(), 1);
        let skipped = &lossy.skipped[0];
        assert_eq!(skipped.index, 1);
        assert_eq!(skipped.key.as_deref(), Some("KXB"));
        assert!(
            skipped.error.contains("missing field `title`"),
            "{}",
            skipped.error
        );
        assert_eq!(skipped.row, bad);
    }

    #[test]
    fn a_page_of_bad_rows_keeps_its_cursor_and_skips_every_row_in_order() {
        let rows: Vec<Value> = ["KXA", "KXB", "KXC"]
            .iter()
            .map(|t| json!({ "event_ticker": t }))
            .collect();
        let lossy = decode_lossy::<EventsPage>("/events", events_page(rows)).unwrap();

        assert!(lossy.page.events.is_empty());
        assert_eq!(lossy.page.cursor, "next");
        let skipped: Vec<(usize, Option<&str>)> = lossy
            .skipped
            .iter()
            .map(|s| (s.index, s.key.as_deref()))
            .collect();
        assert_eq!(
            skipped,
            [(0, Some("KXA")), (1, Some("KXB")), (2, Some("KXC"))]
        );
    }

    #[test]
    fn a_key_that_is_absent_or_not_a_string_is_none() {
        let body = events_page(vec![json!({ "title": "t" }), json!({ "event_ticker": 7 })]);
        let lossy = decode_lossy::<EventsPage>("/events", body).unwrap();

        assert_eq!(lossy.skipped.len(), 2);
        assert!(lossy.skipped.iter().all(|s| s.key.is_none()));
    }

    #[test]
    fn an_envelope_the_strict_decode_refuses_is_refused() {
        let good = event("KXA");
        let bodies = [
            json!({ "cursor": "" }),
            json!({ "events": null, "cursor": "" }),
            json!({ "events": { "0": good }, "cursor": "" }),
            json!({ "events": [good] }),
            json!([good]),
            json!("events"),
        ];
        for body in bodies {
            assert!(
                serde_json::from_value::<EventsPage>(body.clone()).is_err(),
                "the strict decode takes {body}"
            );
            assert!(
                decode_lossy::<EventsPage>("/events", body.clone()).is_err(),
                "the lossy decode takes {body}"
            );
        }
    }

    #[test]
    fn a_page_with_no_bad_row_is_the_strict_decode_with_nothing_skipped() {
        let body = events_page(vec![event("KXA"), event("KXB")]);
        let strict: EventsPage = serde_json::from_value(body.clone()).unwrap();

        let lossy = decode_lossy::<EventsPage>("/events", body).unwrap();

        assert_eq!(lossy.page, strict);
        assert!(lossy.skipped.is_empty());
        assert_eq!(lossy.page.milestones, [json!({"id": "m"})]);
    }

    #[test]
    fn an_event_whose_nested_market_does_not_decode_is_skipped_whole() {
        let mut nested = event("KXB");
        nested["markets"] = json!([{ "ticker": "KXB-1" }]);
        let body = events_page(vec![event("KXA"), nested]);

        let lossy = decode_lossy::<EventsPage>("/events", body).unwrap();

        assert_eq!(tickers(&lossy.page), ["KXA"]);
        assert_eq!(lossy.page.milestones, [json!({"id": "m"})]);
        assert_eq!(lossy.skipped.len(), 1);
        assert_eq!(lossy.skipped[0].key.as_deref(), Some("KXB"));
    }

    #[test]
    fn a_book_or_a_trade_that_does_not_decode_is_skipped_by_its_ticker() {
        let mut bad_book = book("KXB");
        bad_book["orderbook_fp"]["yes_dollars"] = json!([["0.42"]]);
        let books = json!({ "orderbooks": [book("KXA"), bad_book] });
        let lossy = decode_lossy::<OrderbooksResponse>("/markets/orderbooks", books).unwrap();
        assert_eq!(lossy.page.orderbooks.len(), 1);
        assert_eq!(lossy.page.orderbooks[0].ticker, "KXA");
        assert_eq!(lossy.skipped[0].index, 1);
        assert_eq!(lossy.skipped[0].key.as_deref(), Some("KXB"));

        let mut bad_trade = trade("KXB");
        bad_trade["count_fp"] = json!("lots");
        let trades = json!({ "trades": [bad_trade, trade("KXA")], "cursor": "c" });
        let lossy = decode_lossy::<TradesPage>("/markets/trades", trades).unwrap();
        assert_eq!(lossy.page.trades.len(), 1);
        assert_eq!(lossy.page.trades[0].ticker, "KXA");
        assert_eq!(lossy.page.cursor, "c");
        assert_eq!(lossy.skipped[0].index, 0);
        assert_eq!(lossy.skipped[0].key.as_deref(), Some("KXB"));
    }
}
