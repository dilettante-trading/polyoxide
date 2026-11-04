# Retire Data API v1 — design

Status: approved by the owner, 2026-11-04. It is the design record for removing Polymarket's
Data API v1 from polyoxide and making v2 the `data` module's only API. Read it with CLAUDE.md and
`.claude/rules/{polymarket-data,polymarket-clob,errors,cli,py,ci-gates,rate-limits,testing,specs}.md`.
Where this spec and an earlier record disagree, this spec binds.

## 1. Scope, stage and sources

**1.1 Goal.** Every v1 route on `data-api.polymarket.com` and both sibling hosts the `data`
module reads (`user-pnl-api`, `lb-api`) answer `410 Gone`, so every v1 call polyoxide makes
fails. This work removes v1 from the Rust `data` module, the CLOB's market-data and `Trading`
impls, the CLI, the Python bindings, the tests, the gates' records, the spec mirrors and the
docs. It also flattens v2 into the module root and makes v2's row structs constructible
outside the crate.

**1.2 Stage.** The workspace stays in S3. The change removes and re-types public items, so the
release is a 0.x minor (0.47.0 unless the owner picks another), in a version-bump commit of its
own on `main`, cut when the owner says. Every removed or re-typed public item gets its line in
`docs/s3-removals.md`.

**1.3 The owner's choices (2026-11-04).**

- v1 is removed outright, not deprecated. A deprecated v1 method could only return 410.
- v2 is flattened: `data::v2::*` moves to `data::*`, and `data.v2().x()` becomes `data.x()`.
- The Python bindings flatten too.
- prader-rs #179's ask is folded in (D6), so prader migrates once, from 0.44.0 to this
  release, against the flattened paths. Its session was told on 2026-11-04 and acknowledged.
- A retired test cites its subject's entry in `docs/s3-removals.md` (D9), not a row of S2's
  final rename manifest.
- prader-rs #179's second ask is folded in too, widened to gamma's open enums by the owner
  (D17): the open enums export to TypeScript as named open unions.

**1.4 Sources (measured 2026-11-04).**

- `GET https://data-api.polymarket.com/trades?limit=1`, `/positions`, `/v1/leaderboard`,
  `/v1/market-positions`, `/v1/accounting/snapshot`, `/oi` and `/` all answer
  `410 {"error":"Data API v1 has been retired","migration":"https://data-api.polymarket.com/v2/","docs":"https://docs.polymarket.com/api-reference/data-api/overview"}`.
  The bare root answers 410 with no CORS headers (f29a4c5f).
- `https://user-pnl-api.polymarket.com/user-pnl` answers 410
  `{"error":"user-pnl-api.polymarket.com has been retired. Use https://data-api.polymarket.com/v2/user-pnl?user=<address> instead."}`.
- `https://lb-api.polymarket.com/rank` answers 410
  `{"error":"lb-api.polymarket.com has been retired. Use https://data-api.polymarket.com/v2/leaderboard instead."}`.
- `https://data-api.polymarket.com/v2/status` answers 200.
- The `data` mirror's upstream, `https://docs.polymarket.com/api-spec/data-openapi.yaml`,
  answers 404.

## 2. Decisions

**D1. v1 is deleted.** The following go from `polyoxide_polymarket::data`, with every type,
builder and test that serves only them:

- the v1 `api` and `types` modules (today's `src/data/api/` and `src/data/types.rs`);
- the namespaces `user(addr)`, `trades()`, `holders()`, `leaderboard()`, `builders()`,
  `live_volume()`, `open_interest()`, `market_positions()`, `combos()`, `misc()`,
  `accounting()`, `approvals()`, `health()`, `pnl()` and `rankings()`;
- `DataApiBuilder::pnl_base_url` and `rankings_base_url`. `DataApi` keeps one `HttpClient`
  where it held three.

**D2. v2 is the module root.**

- `data::v2::api::*` → `data::api::*`; `data::v2::types::*` → `data::types::*`;
  `data::v2::{Page, PageStream, Pagination}` → `data::{Page, PageStream, Pagination}`.
- `DataV2` and `DataApi::v2` go. Every v2 route is a method on `DataApi`: `data.trades()`,
  `data.positions(anchor)`, `data.status()` and the rest, with v2's names and signatures
  unchanged.
- The source files move with their modules: `src/data/v2/api/*` → `src/data/api/*`,
  `src/data/v2/types/*` → `src/data/types/*`, `src/data/v2/envelope.rs` →
  `src/data/envelope.rs`, and `src/data/v2/error.rs` into `src/data/error.rs`.
- `DataApi::new`, `DataApi::builder`, `DataApiBuilder`'s transport setters, the `Polymarket`
  unified client (`data: DataApi`, `data_base_url`) and the umbrella's
  `polymarket::data::{DataApi, DataError}` re-exports keep their names.

**D3. The error follows the venue convention.** `V2Error` → `data::VenueError`;
`DataError::V2` → `DataError::Venue`, as perps' `Venue(VenueError)` and Kalshi's `Venue`.
`ErrorCode` → `data::ErrorCode`. `DataError::Api` and `DataError::Pagination`, `trace_id()`,
the one `From<ApiError>` decode (by body shape) and the `Classify` impl are unchanged. Each
crate's `const _` assertion names `data::VenueError`.

**D4. The CLOB's public trades read v2.** `impl Trades for Clob` reads
`data.trades().conditions([condition_id]).taker_only(true).limit(100)`, one page (v1's default
limit was 100, and the v1 call sent none). It keeps the rows whose `token_id` is the key's
token and returns them oldest first. `TradeExt` keeps its fields (`transaction_hash`,
`proxy_wallet`, `outcome`), filled from v2's fields of the same meaning, and `taker_side` maps
from v2's `TradeSide`.

**D5. `Trading::positions` reads v2.**

- It reads `data.positions(maker).status(PositionStatus::Open).limit(500)` and walks
  `.pages()` to the end. v2's `OPEN` also returns `REDEEMABLE` rows, so the result covers what
  v1's `/positions` returned with `size_threshold(0)`.
- The cursor walk has no offset ceiling, so the refusal "the wallet holds more positions than
  /positions can list" goes, with `POSITIONS_PAGE` and `POSITIONS_LAST_OFFSET`.
- The position's size is `current_size` and its average price `avg_price`.
- `PositionExt` maps from the v2 row. `condition_id`, `outcome`, `outcome_index`,
  `current_price`, `current_value`, `realized_pnl`, `redeemable`, `mergeable`,
  `negative_risk`, `title` and `end_date` keep their names. Two fields take v2's names,
  because their source and meaning change: `initial_value` → `entry_cost` (from
  `entry_cost_usdc`, the fee-exclusive entry basis) and `cash_pnl` → `unrealized_pnl` (from
  `unrealized_pnl`). Each rename has its `s3-removals.md` entry.

**D6. v2's records are constructible (prader-rs #179).**

- `#[non_exhaustive]` is dropped from every struct under `src/data/types/` (after D2's move)
  and from `Page` and `Pagination`. The enums keep it (`TradeSide`, `ActivityType`,
  `PositionStatus` and the rest), since they already hold `Other(raw)`.
- `OutcomeIndex` gains `pub const fn new(raw: u32) -> Self`, which takes any value, 999
  (`UNLABELED`) included.
- Where dropping the attribute does not fit a struct, it gets a public constructor instead.
  The plan names each such struct and why.

**D7. The CLI.** `polyoxide polymarket data …` keeps every command and flag. Its calls drop
`.v2()`. `data health` reads `/v2/status` and prints the `ServiceStatus` object, since `/`
answers 410 and a 200 from `/v2/status` is the only liveness signal left. `tests/data_v2.rs`
becomes `tests/data.rs`, and `live_api`'s data check reads v2.

**D8. Python.**

- **Removed:** the v1 methods of `DataApi`/`DataApiSync` (`user()`, the v1 `trades()`,
  `holders()`, `open_interest()`, `live_volume()`, `leaderboard()` and `builders()`, and
  `health()`); their client classes (`DataApiUser*` and the namespace classes); the v1 row
  classes (`src/types/data.rs`); and the `@v1_retired` tests.
- **Flattened:**
  - Every v2 route is a method of `DataApi`/`DataApiSync` (`DataApi().trades()`), and
    `DataV2`, `DataV2Sync` and `.v2()` go.
  - The row classes, `Page`, `Pagination` and the page iterators move from `polyoxide.v2` to
    `polyoxide`. No v2 class name clashes with a CLOB or Gamma class (checked 2026-11-04).
  - The `v2` submodule, `python/polyoxide/v2.py` and `v2.pyi` go, and their stubs join
    `__init__.pyi`.
- **Errors:** map by `ErrorCode` as now. Their `trace_id`, `parameter` and `retry_after` are
  set when the error came from the Data API's structured body.
- **Tests:** `test_data_v2_offline.py` → `test_data_offline.py`, on the flattened calls;
  `test_stub_consistency.py` checks `__init__.pyi`; `test_live_api.py` keeps its v2 live
  tests, on the flattened calls.

**D9. A retired test cites its subject's removal.**

- `scripts/test_body_diff.py` accepts `[[retired]]` entries with `removal = "<key>"` in place
  of `row`, where the key is an entry under "Removed" in the listing for the workspace's stage
  (`docs/s3-removals.md` in S3, as `api_removals.py`'s `LISTINGS` names it, read with its
  parser).
- An entry gives exactly one of `row` and `removal`. A `removal` key the listing does not hold
  fails, and a stale entry fails as before.
- `.github/scripts/tests/test_test_body_diff.py` shows each of those failing first, then
  passing.
- `ci-gates.md` and the toml's header document the form.

**D10. DRIFT R27.** A new row of
`_bmad-output/specs/spec-venue-extensibility/divergences.md` records v1's retirement upstream
and this flatten. Every test body this work changes cites it in a `[[drift]]` entry with its
sha256.

- **Items that change:**
  - the CLOB mocks of D4 and D5 (`trading_clob`, `venue_clob`), which serve v2 rows;
  - `data::error`'s inline tests;
  - `headers_data`, `error_classes_data` and `query_setters_data`: v1 rows dropped, v2 calls
    without `.v2()`;
  - `live_data`'s v2 tests;
  - each v2 suite item whose body calls `.v2()`.
- **Normalised instead:** a change of path alone needs no entry. `ci/test-body-diff.toml`'s
  `[paths]` maps `polyoxide_polymarket::data::v2` → `polyoxide_polymarket::data`,
  `…::data::v2::V2Error` → `…::data::VenueError` and `DataError::V2` → `DataError::Venue` on
  the baseline side.
- **Retired** with `removal` entries (D9):
  - v1's mock tests;
  - `holders_shape_data` and the parts of `tests/common` only it and the v1 live tests used;
  - the v1 inline tests in `src/data/{api,types}`;
  - the v1 `live_data` tests;
  - `mock_data`'s `a_429_on_the_pnl_host_holds_the_data_host`;
  - any v1 example that is a test target.

**D11. The send-loop proofs leave v1.** `docs/MUTANTS.md`'s rows (a), (a, the policy) and (b)
cite three v1 `mock_data` tests.

- `a_429_makes_the_next_request_wait_even_though_it_never_saw_one` and
  `retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop` move onto a v2 route with
  their assertions unchanged (R27).
- `a_429_on_the_pnl_host_holds_the_data_host` goes with its host. The rows keep their
  `send_loop` tests.
- Each row's mutation is re-run after the move and must turn its cited tests red before
  `docs/MUTANTS.md` cites their new lines. Commits 4 and 5 move those tests' lines again; each
  updates the rows it moves, and the mutations are re-run once more after commit 5.

**D12. Renames.** These are git moves. `ci/test-body-diff.toml`'s `[files]` pairs each one
whose similarity falls under 30%, and `[fixtures]`/`[literals]` cover the fixture directory:

- `tests/mock_data_v2.rs` → `tests/mock_data.rs`: commit 4 leaves only D11's two send-loop
  tests in `mock_data.rs`, and commit 5 moves `mock_data_v2.rs`'s items in beside them
- `tests/spec_agreement_data_v2.rs` → `tests/spec_agreement_data.rs`
- `tests/wire_agreement_data_v2.rs` → `tests/wire_agreement_data.rs`
- `tests/enum_wire_data_v2.rs` → `tests/enum_wire_data.rs`
- `tests/fixtures/data/v2/` → `tests/fixtures/data/`
- `polyoxide-cli/tests/data_v2.rs` → `polyoxide-cli/tests/data.rs`

**D13. Rate limits.** `shared::limits::data_limits_on` drops the v1 rows (`/closed-positions`,
`/positions`, `/trades`) and the user-pnl host's `/user-pnl`. It keeps every `/v2/*` row and
the `/` catch-all, whose comment is re-justified, since the published v1 figure behind it is
gone. Its tests and `rate-limits.md`'s tables follow.

**D14. Spec mirrors and the nightly.**

- `docs/specs/data/` is deleted, with its `[workspace.metadata.polyoxide.mirrors.data]`
  entry.
- `docs/specs/undocumented/` is deleted, with its mirror entry and its nightly-schema
  exclusion.
- Any fact in either that still holds for v2 moves into `docs/specs/data-v2/OBSERVED.md`
  first.
- `data-v2` keeps its id and directory, since "v2" there is upstream's version
  (`/v2/openapi.json`).
- `python3 scripts/gen_registry.py --write` regenerates every generated region, which will
  list eight watched OpenAPI mirrors, not nine.
- The nightly's data job keeps `live_data`, now v2 only.

**D15. Scripts and examples.**

- `scripts/compare_v2_overlaps.py`, which compared v1 with v2, goes.
- `scripts/gen_data_v2_types.py` and `scripts/capture_polymarket_data.py` write to the
  flattened paths and the moved fixture directory.
- The `closed_positions_burst_probe` and `closed_positions_soak` examples (v1 probes) go.
- `examples/v2_soak` keeps its name and drops `.v2()`.

**D16. Docs and rules.**

- `polymarket-data.md` is rewritten for one API, its `paths:` globs following D12 and D15.
- The data lines of `errors.md` (D3), `cli.md` (D7), `py.md` (D8), `clients.md`, `testing.md`
  and `ci-gates.md` (D9, R27, D12) are updated.
- CLAUDE.md's "user data, v1 and v2" and its API-specs paragraph are updated.
- `polyoxide-polymarket/README.md` and `src/data/README.md` (a doctest) show the flattened
  calls.
- `docs/s3-migration.md` gains a v1 → v2 table, route by route, with `pnl` → `user_pnl` and
  `rankings` → `leaderboard` and their differences noted. The table also lists D2's path
  renames, D3's error rename, D5's two `PositionExt` renames and D6's constructible records.
  The release's CHANGELOG quotes it.

**D17. Open enums export to TypeScript as named open unions (prader-rs #179, the owner's
choice of 2026-11-04, which covers gamma's open enums too).**

- **Which enums.** The eight open enums that `polyoxide_venue::specta_as_string!` gives a
  `string` shape today: data's `TradeSide`, `ActivitySide`, `ActivityType`, `PositionStatus`
  and `TokenModule`, and gamma's `ProtocolVersion`, `ResolutionStatus` and `HomeAway`.
- **What each exports** under specta 2.0.0-rc.20 and specta-typescript 0.0.7, prader's
  pins: `export type TradeSide = "BUY" | "SELL" | OpenString` (every known wire spelling, in
  declaration order), beside one shared
  `export type OpenString = (string) & ({ [key in never]: never })`, which is
  `string & {}`.
- **TypeScript semantics** (checked with tsc 6.0.3, 2026-11-04):
  - any string is assignable, so the type stays open as `Other(raw)` does;
  - `Record<TradeSide, V>` must name every known spelling;
  - `Exclude<TradeSide, OpenString>` is `never`, since every literal is also a
    `string & {}`, so a `switch` over the type needs a `default` arm.
- **Mechanism.** A crate-private wrapper macro in `polyoxide-polymarket`'s `shared` takes an
  `open_enum!` body. It invokes `polyoxide_venue::open_enum!` with it unchanged, and, under
  the crate's `specta` feature, expands a private shape module from the same variant list.
  The module holds a unit enum of the wire spellings and an `#[serde(untagged)]` enum named
  as the open enum, whose variants are the known set (`#[specta(inline)]`) and
  `OpenString`. The open enum's `Type::inline` and `Type::reference` delegate to that
  untagged enum.
  - The variant list exists once.
  - `OpenString` is defined once, in `shared`, so specta sees one type of that name.
  - `polyoxide-venue` gains no specta dependency, and `specta_as_string!`, left with no
    caller, is removed (its `s3-removals.md` entry, and `wire-enums.md`).
- **Tests.** `polyoxide-polymarket` gains specta-typescript `=0.0.7` as a dev-dependency.
  A test under `specta`, `gamma` and `data` exports each of the eight enums. It asserts the
  enum's TypeScript is exactly the union of `ALL`'s `as_str()` spellings plus `OpenString`,
  and that `OpenString`'s export is the line above, with no duplicate type names. Shown red
  first against today's `string` shape.

## 3. Delivery

Seven commits on the branch. Each builds, and each runs the gates its stories touch. The
final gates are all ten CI jobs, wasm included.

1. **The gate (D9).** Full three-layer review: a defect could weaken a gate.
2. **The CLOB onto v2 (D4, D5).** This adds R27 (D10), the CLOB mocks' drift entries and the
   `PositionExt` removal entries. It fixes the live breakage on its own. Full review: it
   re-types `Trading`'s public records.
3. **The send-loop proofs (D11).** One review pass.
4. **v1 removed, workspace-wide (D1, D7's `data health`, D8's removals, D10's retirements,
   D13, D15's deletions).** Full review: it removes public API.
5. **The flatten, workspace-wide (D2, D3, D6, D7, D8's flatten, D10's `[paths]` and drift,
   D12, D15's updates).** Full review.
6. **The TypeScript open unions (D17).** One review pass. It sits after the flatten so it
   writes the final paths once.
7. **Docs and specs (D14, D16).** One review pass.

**After merge:** the version bump is a commit of its own on `main`, cut when the owner says.
After the release publishes, the prader-rs #179 session is told the version, D2's and D3's
names, and per type which records dropped `#[non_exhaustive]` and which gained a constructor.
