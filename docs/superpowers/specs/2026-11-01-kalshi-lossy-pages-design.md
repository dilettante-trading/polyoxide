# Kalshi lossy pages (issue #77) — design

Status: approved by the owner, 2026-11-01. This spec is the design record for issue #77. It builds
on the catalog-routes spec (`docs/superpowers/specs/2026-10-22-kalshi-catalog-routes-design.md`),
whose decisions stand. Read it with `.claude/rules/kalshi.md` and CLAUDE.md. Where this spec and
an earlier record disagree, this spec binds.

## 1. Scope, stage and sources

**1.1 Goal.** A page of a Kalshi listing with one row the types cannot decode fails today as a
whole, with `Class::Decode`, and the next page's `cursor` is lost with it: a walk stalls on that
page, a watermarked feed cannot move past it, and a batch of tickers fails together. This change
adds `send_lossy()`, which is strict about the page's envelope and decodes each row on its own,
returning the rows that decode, the ones that did not with their position and key, and the
envelope, `cursor` included. prader-rs's Kalshi sync tiers (its Spec A, §17 D5) switch to it and
drop their bisect and step-past workarounds.

No bad row has been seen live (4,400 markets, 200 events and 15,336 series decoded on
2026-10-22); the risk is schema drift.

**1.2 Stage.** The workspace stays in S3. The change adds API to `polyoxide-kalshi` and removes
none, so `docs/s3-removals.md` gains nothing. It adds no dependency (`serde_json` and `tracing` are
already in `predictions`), so `ci/dep-allowlists/` does not change. The release is the owner's,
after the branch lands.

**1.3 The owner's choices (2026-11-01).**

- Scope: the five builders #77 names and the three more that decode into the same page types
  (D1).
- Shape: a wrapper around the existing page type, `Lossy<P>`, rather than the issue's
  `LossyPage<T> { items, skipped, cursor }`, which would drop `EventsPage::milestones` and has no
  honest `cursor` for `orderbooks()` (D2).
- The `predictions::venue` trait impls stay strict (D7).

## 2. Decisions (binding)

- **D1. The builders.** Eight builders gain `send_lossy()`, beside their `send()`:

  | Builder | Route | `send_lossy` returns |
  |---|---|---|
  | `ListMarkets` (`markets().list()`, `tickers(..)` included) | `GET /markets` | `Lossy<MarketsPage>` |
  | `ListMarketWindow` (`markets().window(..)`) | `GET /markets` | `Lossy<MarketsPage>` |
  | `ListUpdatedMarkets` (`markets().updated(..)`) | `GET /markets` | `Lossy<MarketsPage>` |
  | `ListTrades` (`markets().trades()`) | `GET /markets/trades` | `Lossy<TradesPage>` |
  | `GetOrderbooks` (`markets().orderbooks(..)`) | `GET /markets/orderbooks` | `Lossy<OrderbooksResponse>` |
  | `ListEvents` (`events().list()`) | `GET /events` | `Lossy<EventsPage>` |
  | `ListHistoricalMarkets` (`historical().markets(..)`) | `GET /historical/markets` | `Lossy<MarketsPage>` |
  | `ListHistoricalTrades` (`historical().trades()`) | `GET /historical/trades` | `Lossy<TradesPage>` |

  Each is `pub async fn send_lossy(self) -> Result<Lossy<P>, PredictionsError>`, documented as
  waiting for the Read budget, sending, and decoding the response with each row that does not
  decode skipped, linking [`Lossy`].

- **D2. The types.** A new file, `src/predictions/types/lossy.rs`, re-exported from `types`
  beside the other types (`predictions::types::{Lossy, SkippedRow}`):

  ```rust
  /// A page decoded row by row: its envelope and the rows that decoded, and the rows that did not.
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
      pub row: serde_json::Value,
  }
  ```

  Neither derives `Serialize` or `Deserialize`: both are results, not wire types. `error` is a
  `String`, so the types stay `Clone` and `PartialEq` (`serde_json::Error` is neither).

- **D3. The rows trait.** A crate-private trait in `lossy.rs` names, for each page type, the field
  that holds its rows, the row type, the key field and a handle on the decoded rows:

  ```rust
  pub(crate) trait LossyRows: DeserializeOwned {
      type Row: DeserializeOwned;
      /// The field that holds the rows.
      const ROWS: &'static str;
      /// The row's field `SkippedRow::key` reads.
      const KEY: &'static str;
      fn rows_mut(&mut self) -> &mut Vec<Self::Row>;
  }
  ```

  | Page type | `Row` | `ROWS` | `KEY` |
  |---|---|---|---|
  | `MarketsPage` | `Market` | `markets` | `ticker` |
  | `TradesPage` | `Trade` | `trades` | `ticker` |
  | `OrderbooksResponse` | `MarketOrderbook` | `orderbooks` | `ticker` |
  | `EventsPage` | `Event` | `events` | `event_ticker` |

  The trait bounds only crate-private code (`Routed::send_lossy` and `decode_lossy`); every public
  `send_lossy` names its concrete page type, so the trait never appears in a public signature.

- **D4. The decode.** A crate-private function in `lossy.rs`,
  `decode_lossy<P: LossyRows>(path: &str, body: Value) -> Result<Lossy<P>, serde_json::Error>`,
  does no I/O:

  1. When `body` is an object whose `P::ROWS` field is an array, take the array out and leave `[]`
     in its place. Otherwise leave `body` as it is.
  2. Decode `P` from `body`. A failure is returned, so the envelope is exactly as strict as
     `send()`: a missing rows field, a `null` one, one that is not an array, a missing `cursor`,
     or a body that is not an object fails as the strict decode of the same body fails. Before
     returning it, log one ERROR in the shape of core's `decode_json`:
     `Failed to decode {path}: {err}: {envelope}`, the envelope (rows emptied by step 1)
     serialised and shortened by `polyoxide_core::truncate_for_log`.
  3. Decode each taken row, in order, with `P::Row::deserialize(&row)`. A row that decodes is
     pushed onto `page.rows_mut()`. A row that does not becomes a `SkippedRow`: `index` its
     position, `key` the row's `P::KEY` field when that is a string (`None` when absent or of
     another type), `error` the error's `Display`, `row` the row. Each skipped row logs one WARN,
     `tracing::warn!(path, index, key = ?key, error = %error, "skipping a row that does not decode")`, without the
     row's body, which the caller holds.

  A row is a top-level element of the rows list: an event whose nested market does not decode is
  skipped whole, its key the event's `event_ticker`.

- **D5. The send path.** `Routed<T>`'s `send` is split so its prelude is shared: a private
  `prepare::<U>(self) -> Result<Request<U, PredictionsError>, PredictionsError>` returns a refused
  route parameter, then the first key refusal, as `ApiError::Validation`; then waits for
  `before_send`, builds core's request with the route's cost, adds the authenticator and the query
  pairs, in today's order. `send` is `prepare::<T>()?.send().await`, its behaviour unchanged.
  `Routed<P: LossyRows>::send_lossy` keeps `path`, sends `prepare::<Value>()?`, and passes the
  `Value` to `decode_lossy`, mapping its error through `ApiError::from` (`Serialization`,
  `Class::Decode`, as the strict decode's). A body that is not JSON fails inside core's
  `Request::send`, logged by `decode_json`, as today. So `send_lossy` sends the same one request
  as `send`, charged the same, retried by the same policy, and a refusal or an unsuccessful
  response is the same error.

- **D6. What does not change.** `send()` on every builder; the page types, their fields and their
  `Deserialize`; every other route; the socket.

- **D7. Not in this change.** The `predictions::venue` trait impls (`trades`, `event_group` and the
  rest) keep reading strictly. `series().list()` (`SeriesList`, no cursor) gets no
  `send_lossy`. No other venue changes. The CLI, the Python bindings and the umbrella's prelude do
  not change; the new API is reachable as `polyoxide::kalshi::predictions::…`.

- **D8. Tests.**

  *Unit tests*, in `lossy.rs`, on `decode_lossy` over JSON literals:

  1. A `MarketsPage` of a good row, a bad row (a required field missing) and a good row: the page
     holds the two good rows in order and the cursor; one `SkippedRow` with `index` 1, the bad
     row's `ticker` as `key`, an `error` naming the missing field, and `row` equal to the row sent.
  2. Every row bad: the page's rows are empty, the cursor is kept, and every row is skipped, in
     order.
  3. `key` is `None` when the key field is absent, and when it is not a string.
  4. Envelope failures fail: the rows field absent, `null`, or an object; `cursor` absent; a body
     that is an array. For each, `serde_json::from_value::<P>` of the same body fails too.
  5. `EventsPage`: `milestones` is kept; an event whose nested market does not decode is skipped
     with its `event_ticker` as `key`.
  6. `OrderbooksResponse` (no cursor) and `TradesPage`: a bad row is skipped with its `ticker`.
  7. A page with no bad row gives a page equal to `serde_json::from_value::<P>` of the same body,
     and nothing skipped.

  *Mock suite*, a new `tests/mock_predictions_lossy.rs` registered as a `[[test]]` with
  `required-features = ["predictions"]`:

  1. For each of D1's eight builders: served a page of a good row, a bad row and a good row, on a
     mock that matches the path and the query `send()` would send, `send_lossy()` returns the two
     good rows, the envelope, and one skipped row with `index` 1 and its key; and `send()` on the
     same body fails with `Class::Decode`.
  2. Agreement: each captured page fixture served to its builder gives, through `send_lossy()`, a
     page equal to `send()`'s and nothing skipped: `markets_page`, `trades`,
     `catalog/events_page`, `catalog/events_page_nested`, `catalog/markets_tickers_page`,
     `catalog/markets_updated_page`, `catalog/markets_window_settled_page`, `catalog/orderbooks`,
     `historical/markets_page` and `historical/trades`. This shows the detour through `Value`
     loses no decimal or number.
  3. An empty `tickers` set on `ListMarkets` is refused through `send_lossy()` as
     `ApiError::Validation` with nothing sent (the mock's `expect(0)` with
     `match_query(Matcher::Any)`).
  4. A `404` with Kalshi's error body is `PredictionsError::Venue` through `send_lossy()`, as
     through `send()`.

  No existing test changes, so `ci/test-body-diff.toml` gains nothing.

- **D9. Docs.**
  - `.claude/rules/kalshi.md`, under Requests: a bullet saying the eight builders have
    `send_lossy()`, strict about the envelope and per row about the items (`Lossy`, `SkippedRow`,
    one WARN per skipped row), and that `send()` and the venue impls stay strict.
  - `src/predictions/README.md`: a sentence in the catalog paragraph that a walk or a feed which
    must not stall on a row it cannot read uses `send_lossy()`, which keeps the cursor and returns
    the rows it skipped.
  - The doc comments of D1, D2 and D3.

## 3. Gates

- fmt, clippy `-D warnings`, nextest and doctests over the workspace with all features, rustdoc
  `-D warnings` (no intra-doc link from a `pub` item to `LossyRows` or `decode_lossy`, which are
  `pub(crate)`), the msrv build, `test_body_diff.py --baseline v0.40.0`,
  `api_removals.py check --baseline v0.40.0`, and the CI scripts' suite (dependency allowlists,
  the venue gate, the rules files).
- Commits by pathspec, conventional, never `--no-verify`.

## 4. Owner actions

- None before the run: every route is unsigned on production and the suites are mocks.
- After the run, the release, and prader-rs's bump to it.
