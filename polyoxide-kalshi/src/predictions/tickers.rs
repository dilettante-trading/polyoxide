//! The `tickers` budget: how many tickers one request may name, and the chunker that splits a set
//! into requests that fit.
//!
//! Kalshi answers a request target over about 16 KiB with `414 URI Too Long`
//! (`docs/specs/kalshi/OBSERVED.md`, issue #71), and a [`Ticker`] may be 200 characters long, so a
//! count alone cannot keep a set under it. A set is measured as if each ticker were sent as its own
//! `tickers=` pair, which the comma-joined form never exceeds, and refused past
//! [`TICKERS_QUERY_BUDGET`].

use crate::predictions::key::Ticker;

/// The most bytes of query one request's tickers may take: half the request target Kalshi was
/// measured to accept, so the path and the other parameters keep the rest. About 200 tickers of
/// today's length fit.
pub const TICKERS_QUERY_BUDGET: usize = 8_000;

/// The most tickers `GET /markets/orderbooks` takes in one request; Kalshi refuses 101 with a
/// `400`.
pub const MAX_ORDERBOOK_TICKERS: usize = 100;

/// The bytes one ticker costs besides its own: `tickers=` and a separator.
const PER_TICKER: usize = "tickers=".len() + 1;

/// What `ticker` costs the budget.
fn cost(ticker: &str) -> usize {
    ticker.len() + PER_TICKER
}

/// Why one request may not send `tickers`, or `None` when it may: an empty set, which would ask
/// for every market, a set of more than `max_count`, or one past [`TICKERS_QUERY_BUDGET`].
pub(crate) fn refusal(tickers: &[&str], max_count: usize) -> Option<String> {
    let size: usize = tickers.iter().map(|ticker| cost(ticker)).sum();
    if tickers.is_empty() {
        Some("the set is empty, and sending none would ask for every market".to_owned())
    } else if tickers.len() > max_count {
        Some(format!(
            "{} tickers are more than the {max_count} the route takes; split them with ticker_chunks",
            tickers.len()
        ))
    } else if size > TICKERS_QUERY_BUDGET {
        Some(format!(
            "{} tickers take {size} bytes of query, past the budget of {TICKERS_QUERY_BUDGET}; \
             split them with ticker_chunks",
            tickers.len()
        ))
    } else {
        None
    }
}

/// Splits `tickers` into consecutive slices, in order, each of at most `max_count` tickers (a
/// `max_count` of 0 is taken as 1) and within [`TICKERS_QUERY_BUDGET`], so each slice can be sent in
/// one request. Pass [`MAX_ORDERBOOK_TICKERS`] for `markets().orderbooks`, and `usize::MAX` for a
/// listing. An empty set yields nothing.
///
/// ```
/// use polyoxide_kalshi::predictions::{key::Ticker, ticker_chunks, MAX_ORDERBOOK_TICKERS};
///
/// let tickers: Vec<Ticker> = (0..250)
///     .map(|i| Ticker::new(&format!("KXT-{i}")).unwrap())
///     .collect();
/// let sizes: Vec<usize> = ticker_chunks(&tickers, MAX_ORDERBOOK_TICKERS)
///     .map(<[Ticker]>::len)
///     .collect();
/// assert_eq!(sizes, [100, 100, 50]);
/// ```
pub fn ticker_chunks(tickers: &[Ticker], max_count: usize) -> TickerChunks<'_> {
    TickerChunks {
        rest: tickers,
        max_count: max_count.max(1),
    }
}

/// The slices [`ticker_chunks`] yields.
#[derive(Debug, Clone)]
pub struct TickerChunks<'a> {
    rest: &'a [Ticker],
    max_count: usize,
}

impl<'a> Iterator for TickerChunks<'a> {
    type Item = &'a [Ticker];

    fn next(&mut self) -> Option<&'a [Ticker]> {
        if self.rest.is_empty() {
            return None;
        }
        let mut size = 0;
        let mut taken = 0;
        for ticker in self.rest {
            let next = size + cost(ticker.as_str());
            // A first ticker always fits: a ticker is at most 200 characters.
            if taken == self.max_count || (taken > 0 && next > TICKERS_QUERY_BUDGET) {
                break;
            }
            size = next;
            taken += 1;
        }
        let (chunk, rest) = self.rest.split_at(taken);
        self.rest = rest;
        Some(chunk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `count` distinct tickers of `len` characters each.
    fn tickers(count: usize, len: usize) -> Vec<Ticker> {
        (0..count)
            .map(|i| {
                let digits = i.to_string();
                Ticker::new(&format!("{}{digits}", "K".repeat(len - digits.len()))).unwrap()
            })
            .collect()
    }

    fn texts(tickers: &[Ticker]) -> Vec<&str> {
        tickers.iter().map(Ticker::as_str).collect()
    }

    fn sizes(chunks: TickerChunks<'_>) -> Vec<usize> {
        chunks.map(<[Ticker]>::len).collect()
    }

    #[test]
    fn a_ticker_costs_its_length_and_nine_bytes() {
        assert_eq!(PER_TICKER, 9);
        assert_eq!(cost("KXT-1"), 14);
    }

    #[test]
    fn an_empty_set_is_refused_because_it_would_ask_for_every_market() {
        let reason = refusal(&[], usize::MAX).unwrap();
        assert!(reason.contains("every market"), "{reason}");
    }

    #[test]
    fn a_set_over_the_count_is_refused_and_one_at_it_is_not() {
        let set = tickers(101, 10);
        let set = texts(&set);
        assert!(refusal(&set[..100], MAX_ORDERBOOK_TICKERS).is_none());
        let reason = refusal(&set, MAX_ORDERBOOK_TICKERS).unwrap();
        assert!(
            reason.contains("101 tickers") && reason.contains("100"),
            "{reason}"
        );
    }

    #[test]
    fn a_set_at_the_budget_is_sent_and_one_a_byte_over_is_refused() {
        // 200 tickers of 31 characters cost 200 × 40 = 8,000 bytes, the budget.
        let at = tickers(200, 31);
        assert_eq!(
            texts(&at).iter().map(|ticker| cost(ticker)).sum::<usize>(),
            TICKERS_QUERY_BUDGET
        );
        assert!(refusal(&texts(&at), usize::MAX).is_none());

        let mut over = tickers(199, 31);
        over.push(Ticker::new(&"K".repeat(32)).unwrap());
        let reason = refusal(&texts(&over), usize::MAX).unwrap();
        assert!(reason.contains("8001 bytes"), "{reason}");
    }

    #[test]
    fn chunks_keep_order_and_fill_up_to_the_count_or_the_budget() {
        let set = tickers(450, 31);
        // 200 tickers of 31 characters fill the budget.
        assert_eq!(sizes(ticker_chunks(&set, usize::MAX)), [200, 200, 50]);
        assert_eq!(
            sizes(ticker_chunks(&set, MAX_ORDERBOOK_TICKERS)),
            [100, 100, 100, 100, 50]
        );
        let joined: Vec<&Ticker> = ticker_chunks(&set, 7).flatten().collect();
        assert_eq!(joined, set.iter().collect::<Vec<_>>());
        for chunk in ticker_chunks(&set, usize::MAX) {
            assert!(refusal(&texts(chunk), usize::MAX).is_none());
        }
    }

    #[test]
    fn an_empty_set_yields_nothing_and_a_count_of_zero_is_one() {
        assert_eq!(ticker_chunks(&[], 5).count(), 0);
        assert_eq!(sizes(ticker_chunks(&tickers(3, 10), 0)), [1, 1, 1]);
    }

    #[test]
    fn the_longest_tickers_fit_38_to_a_chunk() {
        // 209 bytes each: 38 take 7,942 bytes, and a 39th would pass 8,000.
        let set = tickers(50, 200);
        assert_eq!(sizes(ticker_chunks(&set, usize::MAX)), [38, 12]);
    }
}
