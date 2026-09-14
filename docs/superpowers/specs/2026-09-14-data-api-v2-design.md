# Data API v2 Support — Design

**Status:** Approved (2026-09-14). Amended the same day during planning; see
[Amendments from planning](#amendments-from-planning) and the plan,
[`2026-09-14-data-api-v2.md`](../plans/2026-09-14-data-api-v2.md).
**Author:** aidanb
**Branch:** `aidanb/data-v2`
**Follows:** `b3849b7` (`ci(specs): mirror Data API v2 and watch it for drift`)

## Goal

Implement the 20-route Data API v2 contract (`/v2/*` on `data-api.polymarket.com`)
in `polyoxide-data`, then carry it through to the Python bindings and the CLI.
v1 stays fully supported; v2 becomes the recommended surface.

## Context

Upstream's migration guide
(<https://docs.polymarket.com/api-reference/data-api/migrating-from-v1>) says the
v1 routes keep working and that new integrations should start on v2. Upstream's
own SDKs (`@polymarket/client`, `polymarket-client`) moved to v2 in 0.10.0 with
breaking changes. The mirror, its conventions and the v1→v2 route mapping are in
[`docs/specs/data-v2/INDEX.md`](../../specs/data-v2/INDEX.md).

v2 is not v1 under new paths. It changes the contract itself:

- every response is `{"data": ...}`, and paged routes add `pagination`;
- pagination is cursor-only, and sending `offset` is a `400`;
- field names are snake_case (`asset` becomes `token_id`, `proxyWallet` becomes `proxy_wallet`);
- errors are structured: `{error, code, retryable, trace_id, parameter?}`.

None of the v1 types or builders can be reused as-is.

## Decisions

| # | Question | Decision |
|---|----------|----------|
| 1 | v1's fate | **Coexist, v2 preferred.** Nothing in v1 is removed or deprecated here. |
| 2 | Placement | **`polyoxide_data::v2` module**, reached via `data.v2()`, sharing v1's `HttpClient`. |
| 3 | Pagination | **`Page<T>` from `send()` plus an auto-walking `Stream` from `.pages()`.** Adds `futures-util`. |
| 4 | Request plumbing | **Typed envelopes on core's `Request`** (approach A). No dedicated v2 request type and no generic paged request in core. |
| 5 | 503 retry | **Not added.** The crate's own retry loop still retries only `429`; callers use `is_retriable()` and `retry_after()`. |
| 6 | Rate limits | **Borrow v1's buckets provisionally, then measure and pin before release.** |
| 7 | Scope | Rust v2, **plus** Python bindings, CLI port, and `pnl()`/`rankings()` documentation. Comparison against upstream's Python SDK is out of scope. |
| 8 | `pnl()` / `rankings()` | **Document the v2 alternative and how it differs. No deprecation.** The routes are not drop-in replacements (see Evidence). |

## Evidence gathered during design

All from read-only GETs against the live host on 2026-09-14.

**Errors match the spec, and v1 differs.** v2 returns
`{"error","code","retryable","trace_id"}`, plus `parameter` on some validation
failures. For example, `/v2/user-pnl` with no user names `"parameter":"user"`,
while `/v2/positions` with neither anchor omits `parameter`. v1 on the same host
still returns bare `{"error": "..."}`. The two shapes can be told apart without
knowing which route produced them.

**Required arguments are enforced even though the OpenAPI marks every
parameter optional and nullable.** Omitting them gives these 400s:

| Route | Message |
|-------|---------|
| `/v2/user-pnl` | `required query param 'user' not provided` |
| `/v2/positions` | `required query param 'user' or 'condition' not provided` |
| `/v2/resolutions` with two selectors | `provide exactly one of 'question_id', 'condition', or 'event_id'` |

**`outcome_index: 999` shows up in ordinary data.** The first row of the bare
`/v2/trades` feed had it.

**`/v2/trades` is CDN-cached.** Responses carry `cache-control: public,
max-age=300` via CloudFront. The first page of a walk can be up to five minutes
stale. Cursor URLs are unique, so later pages are not affected.

**`/v2/user-pnl` does not reproduce `data.pnl()`.** The spec describes
`trade_pnl` as "the compatibility chart series (`p` on the bare user-pnl
route)". For wallet `0x3048d65321be3497164cdfc2996f94f98a2e7537`, with
`interval=1w` and `fidelity=1d`:

| timestamp | v2 `trade_pnl` | `user-pnl-api` `p` |
|-----------|---------------:|-------------------:|
| 1788825600 | 380194.65 | 381974.50 |
| 1788912000 | 389154.70 | 391299.94 |
| 1788998400 | 394820.47 | 397242.62 |
| 1789084800 | 400876.18 | 403113.12 |
| 1789171200 | 414452.55 | 416448.44 |
| 1789257600 | 415628.19 | 417695.47 |
| 1789344000 | 416167.01 | 417942.70 |
| 1789369200 | — | 419601.47 |

On this wallet the v2 values were 0.42–0.55% lower, and the legacy host also
appends an off-grid "now" point.

*Amended during planning:* on three more wallets the gap was not a steady offset.
It ranged from +0.002% to +62.7%, the opposite sign from this wallet, and the
legacy host appended its extra point every time. The four-wallet table is in
`docs/specs/data-v2/OBSERVED.md` (plan Task 1).

**`/v2/leaderboard` does not reproduce `data.rankings()`.** v2 `volume` is
documented as both-sides **shares**, while lb-api's `amount` is USDC. The v2
windows are `day/week/month/all`; lb-api's are trailing `1d/7d/30d/all`.

## Component 1 — Module layout and public surface

```
polyoxide-data/src/
  v2/
    mod.rs          DataV2 handle and route methods
    envelope.rs     Envelope<T>, Page<T>, Pagination, PageStream<T>
    error.rs        V2Error, ErrorCode
    types/          wallet.rs · feeds.rs · markets.rs · boards.rs · service.rs
    api/            wallet.rs · feeds.rs · markets.rs · boards.rs · service.rs
```

The split into files follows the spec's five tags. Callers never see the tags:
every route is a flat method on `DataV2`.

```rust
impl DataApi {
    /// Data API v2 routes (`/v2/*`). Shares this client's connection pool,
    /// rate limiter, 429 cooldown and concurrency budget.
    pub fn v2(&self) -> DataV2;
}
```

### Route table

An argument the upstream docs call required is a **method argument**, never a
setter. Everything else is a chained setter.

| Route | Method | Arguments | `send()` returns |
|-------|--------|-----------|------------------|
| `GET /v2/approvals` | `approvals` | `user` | `Approvals` |
| `GET /v2/positions` | `positions` | `PositionAnchor` | `Page<Position>` |
| `GET /v2/positions/combos` | `combo_positions` | `user` | `Page<ComboPosition>` |
| `GET /v2/user-pnl` | `user_pnl` | `user` | `UserPnlSeries` |
| `GET /v2/user-stats` | `user_stats` | `user` | `Option<UserStats>` |
| `GET /v2/user-volume` | `user_volume` | `user` | `UserVolume` |
| `GET /v2/value` | `value` | `user` | `PortfolioValue` |
| `GET /v2/activity` | `activity` | `user` | `Page<Activity>` |
| `GET /v2/activity/combos` | `combo_activity` | `user` | `Page<ComboActivity>` |
| `GET /v2/trades` | `trades` | — | `Page<Trade>` |
| `GET /v2/holders` | `holders` | `conditions` | `Page<MetaHolder>` |
| `GET /v2/live-volume` | `live_volume` | `event_ids` | `LiveVolume` |
| `GET /v2/oi` | `open_interest` | — (omit `condition` for the global figure) | `Vec<OpenInterest>` |
| `GET /v2/prices-history` | `prices_history` | `token_id` | `Page<PricePoint>` |
| `GET /v2/resolutions` | `resolutions` | `ResolutionKey` | `Vec<Resolution>` |
| `GET /v2/biggest-winners` | `biggest_winners` | — | `Page<BiggestWinner>` |
| `GET /v2/builders/leaderboard` | `builders_leaderboard` | — | `Page<BuilderStanding>` |
| `GET /v2/builders/volume` | `builder_volume` | — | `Vec<BuilderVolumePoint>` |
| `GET /v2/leaderboard` | `leaderboard` | — | `Page<LeaderboardEntry>` |
| `GET /v2/leaderboard?user=` | `leaderboard_user` | `user` | `Option<LeaderboardUserEntry>` |
| `GET /v2/status` | `status` | — | `ServiceStatus` |

`/v2/leaderboard` is a `oneOf` that depends on `user`. Splitting it into two
methods means neither caller has to match on a shape it already chose.

`/v2/prices-history` is paged (its `limit` caps at 10,000, not 1,000) but has no
v1 counterpart. It is independent of CLOB `/prices-history` and of
`polyoxide clob prices download`, neither of which changes.

### Anchor enums

```rust
/// `/v2/positions` needs at least one of `user` and `condition`. Supplying both
/// narrows the user's positions to those markets.
pub enum PositionAnchor {
    User(String),
    Conditions(Vec<String>),
    UserInConditions { user: String, conditions: Vec<String> },
}

/// `/v2/resolutions` takes exactly one selector family.
pub enum ResolutionKey {
    Question(String),
    Conditions(Vec<String>),
    Events(Vec<String>),
}
```

`PositionAnchor` implements `From<&str>` and `From<String>`, both producing
`User`. `ResolutionKey` has no `From` impls: a UMA question id and a condition id
are both `0x` plus 64 hex characters, so a bare string does not say which it is.

### Changes outside `v2/`

- **`polyoxide-core/src/request.rs`:** a hand-written `impl<T, E> Clone for Request<T, E>`. A derived impl would require `T: Clone` and `E: Clone` because of `PhantomData<(T, E)>`.
- **`polyoxide-core/src/error.rs`:** `ApiError::from_status_and_body(status: u16, body: &str) -> ApiError`, with `from_response` delegating to it. Behaviour does not change.
- **`polyoxide-data/src/error.rs`:** `DataApiError` becomes `#[non_exhaustive]` and gains `V2(V2Error)` and `Pagination(String)`.
- **`polyoxide-core/src/rate_limit.rs`:** the provisional v2 rows in `data_default()` (Component 4), replaced by measured ones in Phase 3.
- **`polyoxide-data/Cargo.toml`:** add `futures-util = "0.3"` (already in the workspace via clob and rtds).

## Component 2 — Types

**Optionality follows the spec mechanically.** A property that is in `required`
and not nullable is a plain field. Every other property is `Option<T>`. Numeric
fields get no `#[serde(default)]`: the spec says a missing or `null` number means
*unavailable, never zero*.

**Every `Option` field serializes as `null`.** v2 types never use
`skip_serializing_if = "Option::is_none"`, because the key-set agreement test
(Component 5) depends on it.

**Numbers are `f64`; timestamps are `i64`.** v2 sends JSON numbers
(`format: double`), not decimal strings, and v1 data types already use `f64`.
The CLAUDE.md `Decimal` convention covers CLOB's string-encoded prices and does
not apply here. Fields ending in `_micros` stay `i64`. Every amount field's doc
comment states its unit, copied from the spec: bare `volume`/`size` are shares,
`_usdc` fields are USD, and `taker_` volumes count one side only.

**`OutcomeIndex` newtype.**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutcomeIndex(u32);

impl OutcomeIndex {
    pub const UNLABELED: u32 = 999;
    /// `None` when upstream could not label the outcome.
    pub fn get(self) -> Option<u32>;
    pub fn raw(self) -> u32;
}
```

A bare `u32` would let `outcomes[t.outcome_index]` compile, and on a `999` row
that panics.

Enum values come from the spec's parameter and field descriptions, because the
schema types every one of them as a bare `string`. All enums are
`#[non_exhaustive]` and implement `Display` with the wire spelling. There are
two kinds:

- **Enums that also appear in responses** — `TradeSide` (`Trade.side`,
  `Activity.side`), `ActivityType` (`Activity.type`) and `PositionStatus`
  (`Position.status`). These have an `Other(String)` arm holding the raw value,
  implemented with a custom `Deserialize`, following v1's `Allowance::Unknown`.
  A value upstream adds later therefore neither fails a page nor gets thrown
  away. `TIP` is already an opt-in activity type, and the CLI must be able to
  print a type it doesn't recognise. The same arm works for requests: sending
  `Other(raw)` passes the string through, so a new upstream value can be used
  before the SDK knows about it. This differs from v1's `TradeSide`, whose
  `#[serde(other)] Unknown` discards the value.
- **Request-only enums** — `ComboPositionStatus`, `PositionSortBy`,
  `ComboPositionSortBy`, `ActivitySortBy`, `FilterType`, `TimePeriod`,
  `LeaderboardBoard`, `PnlInterval`, `PnlFidelity`, `PricesInterval` and
  `BuilderInterval`. These are closed: the server rejects unknown values with a
  400, so an escape hatch would only move that error later.

**Reusing v1 types.** Allowed only where the wire format is byte-identical, and
each reuse gets a round-trip test:

- `ApprovalContract.amount` → `crate::types::Allowance` (`"max"`, an integer string, or the uint256-overflow `Unknown(String)`).
- `sort_direction` → `crate::types::SortDirection` (`ASC`/`DESC`).

Everything else is v2-local, even when a v1 type has the same name.

**Derives** match v1: `Debug, Clone, Serialize, Deserialize` and
`#[cfg_attr(feature = "specta", derive(specta::Type))]`. Wire names are already
snake_case, so there is no `rename_all`.

**Multi-value parameters** (`condition`, `event_id`, `type`) take
`impl IntoIterator<Item = impl Into<String>>` and are joined with commas. The
"at most 20 distinct values" cap is left to the server, whose 400 names the
`parameter`.

## Component 3 — Paging

```rust
pub struct Page<T> {
    pub data: Vec<T>,
    pub pagination: Pagination,
}

pub struct Pagination {
    /// Page size this page was served with.
    pub limit: u32,
    /// Cosmetic running offset; the cursor drives the seek. There is no total.
    pub offset: u32,
    /// Exact: `true` iff another page exists.
    pub has_more: bool,
    /// `None` on the last page.
    pub next_cursor: Option<String>,
}

pub type PageStream<T> =
    Pin<Box<dyn Stream<Item = Result<Page<T>, DataApiError>> + Send>>;
```

Every paged builder stores `request: Request<Page<T>, DataApiError>` and
`cursor: Option<String>` **as separate fields**. The cursor is added to the
query only at send time.

- `.cursor(c)` sets the starting cursor. Its docs state that upstream ignores `limit` whenever a cursor is supplied.
- `.send()` sends one page.
- `.pages()` consumes the builder and returns a `PageStream<T>` built on `futures_util::stream::try_unfold`. Each step clones `request`, adds the current cursor (none for the first page unless `.cursor()` was set), sends it, and keeps `next_cursor`.

Walk rules:

1. The walk ends when `next_cursor` is `None`. The cursor is trusted over `has_more`.
2. A page with empty `data` and a non-null cursor continues the walk.
3. If the server returns the same cursor it was just sent, the stream yields `DataApiError::Pagination("server returned the cursor it was sent")` and ends.
4. Any error is yielded and ends the stream.

Because every page clones the same `request`, **filters cannot change during a
walk**. That rules out the silent re-anchoring the spec warns about on
`trades` and `activity`. `.pages()` docs also note the CloudFront `max-age=300`
on the first page.

No row-level `.items()` stream is provided. Callers can flatten with
`try_flatten`, and a page-level stream keeps `next_cursor` visible so a walk can
be resumed.

## Component 4 — Errors and rate limits

### Errors

```rust
#[derive(Debug, Clone)]
pub struct V2Error {
    pub status: u16,
    pub code: ErrorCode,          // #[non_exhaustive], #[serde(other)] Unknown
    pub message: String,          // wire `error`
    pub retryable: bool,
    pub trace_id: String,
    pub parameter: Option<String>,
    pub retry_after: Option<Duration>,
}
```

`DataApiError::from_response`:

1. Reads the status and the `Retry-After` header before consuming the body.
2. Reads the body as text.
3. If the body deserializes as the v2 error shape (`error`, `code`, `retryable` and `trace_id` all present), returns `DataApiError::V2`.
4. Otherwise returns `DataApiError::Api(ApiError::from_status_and_body(status, &body))`. That covers v1 bodies and Cloudflare's non-JSON `1015` page.

The shape is detected **from the body, not the path**, so a v1 route that adopts
the v2 error body later gets structured errors with no code change.

New methods on `DataApiError`:

| Method | `V2` | `Api` | `Pagination` |
|--------|------|-------|--------------|
| `is_retriable()` | the server's `retryable` flag | `ApiError::is_retriable()` | `false` |
| `trace_id()` | `Some` | `None` | `None` |
| `retry_after()` | `retry_after` | `None` | `None` |

If `clippy::result_large_err` fires, the variant becomes `V2(Box<V2Error>)`.

### Rate limits

`RateLimiter::data_default()` gains three provisional rows, borrowed from v1:

| Prefix | Quota | Source |
|--------|-------|--------|
| `/v2/positions` | 150 / 10s | v1 `/positions` and `/closed-positions`, which v2 folds together |
| `/v2/trades` | 200 / 10s | v1 `/trades` |
| `/v2/user-pnl` | 200 / 10s | v1 `/user-pnl` |

Prefix matching makes `/v2/positions/combos` share the positions bucket. That is
deliberately conservative until measured. Each row carries a comment marking it
provisional, and the `documented_data_limits` test pins its effective quota.

Phase 3 replaces the borrowed figures with measured ones (see Component 7).

## Component 5 — Testing

Every agreement test is committed along with the mutation that was used to show
it fails, recorded in its doc comment or the commit message.

### 5.1 Spec agreement: `polyoxide-data/tests/v2_spec_agreement.rs`

Reads `docs/specs/data-v2/openapi.json` via `include_str!`. For v2 the mirror is
the host's own served schema (`/v2/openapi.json`), which is the strongest oracle
this API offers. A macro table maps schema names to Rust types.

For every mapped schema:

- **Required means non-`Option`.** Synthesize JSON containing only the required, non-nullable properties, using type-driven values and resolving `$ref`s recursively. It must deserialize. Removing any one of those properties must fail. Setting any other property to `null` must succeed.
- **No missing or misnamed fields.** Synthesize JSON with every property, deserialize, serialize again, and assert that the output key set equals the spec's property set. Serde ignores unknown keys, so without this check a struct that forgot a field would pass the first check.
- **Coverage.** Every non-envelope schema in `components.schemas` must be in the table, or in a short `NOT_MODELLED` list with a reason (for example `ErrorResponse`, which `V2Error` models by hand).

Mutations this must catch: turning a required field into `Option`, removing a
field, and misspelling a field.

### 5.2 Route and parameter agreement (same file)

This uses mockito. The table has one entry per `(path, builder)`: a closure
that builds the route with **every** argument and setter filled in and calls
`send()`. The mock records the query keys that actually went over the wire.

- For each path, the recorded query keys must equal the spec's parameter names.
- The table's paths must equal the spec's `paths`.

`leaderboard` and `leaderboard_user` cover `/v2/leaderboard` between them; their
combined key sets must equal the spec's list. Mutations this must catch: a
missing route, a camelCase key (`eventId`), and an added `offset` setter.

### 5.3 Wire agreement: `polyoxide-data/tests/v2_wire_agreement.rs`

The fixtures in `tests/fixtures/v2/` (Phase 0) are checked in both directions,
following `polyoxide-gamma/tests/wire_agreement.rs`:

1. Every key a type emits was sent by the server, or is listed in `EXPECTED_ABSENT` with a reason.
2. Every key the server sent is modelled, or is listed in `IGNORED` with a reason.

The fixtures must include a `999` trade, a `data: null` user-stats response, a
leaderboard-user response, and a `holders?include_pnl=true` response.

### 5.4 Mock behaviour: `polyoxide-data/tests/mock_api.rs`

- Plain routes unwrap `data`. `data: null` becomes `None` on `user_stats` and `leaderboard_user`.
- `.pages()` over three pages: **every request carries identical filter parameters**, and pages 2 and 3 carry the previous `next_cursor`. This is asserted on the recorded requests, not on the results.
- `.pages()` stops on a `null` cursor, continues past an empty page that still has a cursor, yields `Pagination` on a repeated cursor, and ends after yielding an error from the middle of the walk.
- Error mapping uses the **bodies captured on 2026-09-14**, not invented ones:

  | Case | Expected |
  |------|----------|
  | offset 400, bad-param 400, 404, `user-pnl` missing-user 400 (with `parameter`) | `V2` with every field checked |
  | v1 `{"error": "..."}` | `Api` |
  | non-JSON `1015` | `Api(ApiError::RateLimit)` |
  | 503 with `retryable: false` | `!is_retriable()` (the server's flag beats the status heuristic) |
  | 429 with `Retry-After: 7`, on a client built with `max_retries: 0` so core's loop hands the 429 to `from_response` | `retry_after() == Some(7s)` |

### 5.5 Other checks

- **Rate limits:** v2 rows in `documented_data_limits`, asserting effective quota.
- **Live** (`tests/live_api.rs`, `#[ignore]`): one test per method plus a two-page walk. Inputs are chosen live: take a wallet from the bare `/v2/trades` feed, then use it for activity, positions, pnl, stats, volume and value. No hardcoded wallets. `nightly-behavioral.yml` already runs this suite.
- **Docs:** the README gets a v2 example, which runs as a doctest. v2 `pub` items link only to `pub` items. Every phase runs `cargo test -p polyoxide-data --all-features --doc` and `RUSTDOCFLAGS="-D warnings" cargo doc` explicitly.

## Component 6 — Consumers

### 6.1 Python (`polyoxide-py`)

- **Surface:** `DataApi.v2` and `DataApiSync.v2` expose the 20 routes as flat keyword-argument methods, matching the existing py style. Required arguments are positional. `positions` takes `user=` and/or `conditions=`; `resolutions` takes exactly one of `question_id=`, `conditions=`, `event_ids=` and raises `ValueError` otherwise.
- **Pages:** one `Page` class (`data: list[T]`, `pagination: Pagination`). Its rows are already-wrapped `py_type!` objects. The `.pyi` declares `class Page(Generic[T])`, and methods return `Page[Trade]` and so on.
- **Walking:** each of the 10 paged routes also gets `v2.iter_<route>(...)`, which returns an async iterator of `Page` (a sync iterator on `DataApiSync`), backed by `PageStream` behind a mutex.
- **Errors:** `data_err` matches `DataApiError::V2` directly. `invalid_request` maps to `ValidationError`, `rate_limited` to `RateLimitError`, `request_timeout` to `TimeoutError`, and everything else to the base error. `code`, `retryable`, `trace_id` and `parameter` are set as attributes on the exception. v1 errors keep today's string-matching path. `DataApiError::Pagination` maps to the base error.
- **Tests:** `test_stub_consistency.py` covers the new classes. A Rust test builds each v2 `py_type!` from the shared v2 fixtures and asserts every getter returns a non-`None` value, which catches the silent `get_field` miss that stub consistency cannot. Plus pytest live tests mirroring 5.5.

### 6.2 CLI (`polyoxide data …`)

| Command | Change |
|---------|--------|
| `activity`, `builders`, `holders`, `trades`, `open-interest`, `live-volume` | moved to the matching v2 route |
| `positions` | open and closed become `--status OPEN\|REDEEMABLE\|CLOSED` on `/v2/positions`; `value` uses `/v2/value` |
| `traded` | `/v2/user-stats`, printing `trades` (the distinct-market count) |
| `health` | **stays on v1 `/`**, since `/v2/status` reports data freshness, not liveness |

- `--offset` is removed; using it gives a clap error that points to `--cursor`.
- `--condition` replaces `--market`, and `--market` stays as an alias.
- **Output:** a single call prints the `{data, pagination}` envelope as pretty JSON. `--all` walks with `.pages()` and **writes rows as JSONL**, flushing after each page. The last `next_cursor` goes to stderr when a walk stops early, whether from `--max-pages N` or an error.
- **Not included:** commands for routes that are new in v2.
- The changelog marks the camelCase → snake_case output change as breaking for scripts.

### 6.3 `pnl()` and `rankings()`

- `PnlApi`'s docs gain a *Documented alternative* section. It names `DataV2::user_pnl` and says `trade_pnl` is described upstream as this series, but on 2026-09-14 it differed by anywhere from −0.55% to +62.7% across four wallets and lacked the trailing live point, so it is **not a drop-in replacement**. It links to `OBSERVED.md`.
- `RankingsApi`'s docs gain the same section for `DataV2::leaderboard`: volume is in shares, not USDC, and windows are calendar-named, not trailing.
- `docs/specs/undocumented/INDEX.md` links both hosts to their v2 counterparts and to `OBSERVED.md`.

### 6.4 Documentation

- **CLAUDE.md:** the Data API v2 paragraph, the `data.*` namespace list (add `data.v2()`), the rate-limit table note, and the Python and CLI notes.
- **`docs/specs/INDEX.md` and `docs/specs/data-v2/INDEX.md`:** replace the "Not implemented" banner.
- **New `docs/specs/data-v2/OBSERVED.md`:** the pnl and rankings divergences, CDN caching, required parameters the schema does not mark as required, and the measured rate limits.

## Component 7 — Rate-limit measurement

`polyoxide-data/examples/v2_soak.rs` reuses `examples/common` (the throttle
observer, abort on the first 429, percentiles). It adds:

- `--route positions|trades|activity|user-pnl|holders-pnl`;
- `--rate` (omit it to use the shipped limiter), as in `closed_positions_soak.rs`.

**Cache busting is required.** Because of `cache-control: public, max-age=300`,
repeating one URL measures CloudFront, not the origin's per-client allowance, and
would report a clean run at any rate. Every request must have a distinct URL. The
harness gets this by rotating across wallets taken from the live feed, walking
real cursors, and varying `limit`, which is harmless to the measurement. It keeps
a set of every URL sent in the last 300 seconds and aborts if one would repeat.
The check lives in the harness because `DataApi` never exposes response headers,
so it cannot look at `x-cache` itself.

Procedure: start at the provisional rate, step up, and stop at the first 429. Pin
the highest clean sustained rate as the bucket (where `quota()` already reserves
`RESERVED_FRACTION`). Record the runs in `OBSERVED.md` with their parameters.
Routes without their own bucket stay on the 1000/10s default unless measurement
shows a lower cap.

## Phasing

Every phase ends with: `cargo fmt --all -- --check`, `cargo clippy --all-targets
--all-features -- -D warnings`, `cargo nextest run --all-features --workspace`,
`cargo test --all-features --doc --workspace`, `RUSTDOCFLAGS="-D warnings" cargo
doc --no-deps --all-features --workspace`, and `uv run pytest tests/` when
`polyoxide-py` changed.

| Phase | Contents | Exit criterion |
|-------|----------|----------------|
| **0 — Evidence** | Fixtures for all 20 routes plus the 2026-09-14 error bodies, with `tests/fixtures/v2/README.md` provenance. pnl/rankings comparison on ≥3 wallets. `OBSERVED.md` draft. | Fixtures committed and every divergence claim backed by a capture |
| **1 — Foundation** | Core `Clone`/`from_status_and_body`; `envelope.rs`, `error.rs`, `DataApiError` changes; 5.1 harness; `trades` and `user_stats` end to end with 5.4 tests | Both tracer routes pass 5.1–5.4; mutations recorded |
| **2 — Routes** | The remaining 18 methods and their types; 5.2 made exhaustive; 5.3 over every fixture; provisional rate-limit rows; README example; live tests | 5.2 table equals spec `paths` |
| **3 — Measure and document** | `v2_soak.rs`, measured limits pinned, 6.3 and 6.4 | Limits come from recorded runs, not borrowed figures |
| **Release** | PR from `aidanb/data-v2`; 0.x minor bump. Check `origin` before picking the version. | CI green on `main` and release tag published |
| **4 — Python** | 6.1 | pytest and stub consistency green; getter-fixture test in place |
| **5 — CLI** | 6.2 | CLI unit tests and live CLI tests green |

Phases 4 and 5 depend only on the released Rust surface and not on each other.
They run as separate loom sessions, each with its own branch and PR.

## Risks

- **Upstream drift during implementation.** 5.1 reads the vendored mirror, so adopting a drift makes it fail at exactly the types that need to change. That is intended.
- **IP throttling during Phase 3.** The harness stops on the first 429 and steps up from the provisional rates rather than starting high.
- **Local OOM.** earlyoom killing rustc (signal 15 / exit 254) during full workspace runs is environmental. Rerun with fewer jobs; it is not a test failure.
- **Enum values change upstream.** Request enums come from prose, which the drift check sees only as a description diff. The nightly drift issue's key-path summary names the parameter, and the change is then made by hand.

## Amendments from planning

Planning built and ran the Phase 0–2 code in a scratch workspace, against the
live host where needed. These changes came out of that, and the plan follows them:

| Area | Design said | Now | Why |
|------|-------------|-----|-----|
| Type delivery | Types for each route land with its builder (Phase 2) | All 33 schemas land in Phase 1 via `scripts/gen_data_v2_types.py` | The spec agreement test can check every schema from its first commit, with no "pending" list to maintain |
| `PositionAnchor` | `Conditions(Vec<String>)` arm | `Condition(String)` | Without `user`, upstream accepts exactly one condition and rejects a list |
| Response enums | `side` uses `#[serde(other)] Unknown` | `TradeSide`, `ActivityType`, `PositionStatus` keep unknown values in `Other(String)`; new `ActivitySide` | Activity `side` also carries `IN`/`OUT` (tips) and `""` |
| `BuilderInterval` | Separate enum | Dropped; `builder_volume().interval()` takes `TimePeriod` | The server validates it as `time_period must be one of day, week, month, all` |
| Leaderboard `sort_by` | — | `ListLeaderboard::board(LeaderboardBoard)` | The parameter selects a board rather than sorting one |
| Mock tests | `tests/mock_api.rs` | `tests/v2_mock_api.rs` | Kept apart from the 37 v1 mock tests |
| Anchor checks | Route table only | Plus mock tests pinning each `PositionAnchor`/`ResolutionKey` arm to its own keys | The route table unions keys per path, so a leaking arm would pass it |
| CDN caching | `/v2/trades` only | Per route: trades and oi 300s, activity 15s, positions 5s; a cache hit repeats the `trace_id` | Measured on 2026-09-14 |
| Unknown-wallet tests | — | Random addresses | `0x…0001` is a known wallet (zeros, not `null`) |
| Phase 3 | In this plan | **Its own plan**, written after Phase 2 lands | The soak harness has to be designed around which routes actually throttle and how CloudFront caching interacts with the origin limiter. Planning it now would be guessing. Phase 3 still precedes the release. |

## Out of scope

- Deprecating or removing any v1 route, `pnl()` or `rankings()`.
- In-crate retry of `503`.
- Comparing field names and optionality with upstream's `polymarket-client` 0.10.0.
- CLI commands for routes that are new in v2 (`user-pnl`, `user-volume`, `biggest-winners`, `prices-history`, `resolutions`, `status`, `approvals`, `leaderboard`).
- A generic paged-request abstraction in `polyoxide-core`.
