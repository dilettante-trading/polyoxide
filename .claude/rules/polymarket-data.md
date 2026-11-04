---
paths:
  - "polyoxide-polymarket/src/data/**"
  - "polyoxide-polymarket/tests/*data*.rs"
  - "polyoxide-polymarket/tests/fixtures/data/**"
  - "polyoxide-polymarket/examples/v2_soak/**"
  - "polyoxide-cli/src/commands/polymarket/data/**"
  - "polyoxide-cli/tests/data.rs"
  - "scripts/{capture_polymarket_data,gen_data_v2_types}.py"
  - "docs/specs/{data,data-v2,undocumented}/**"
---

# Polymarket Data API

Polymarket retired Data API v1 on 2026-11-02, and its `user-pnl-api` and `lb-api` hosts with it:
every v1 route answers `410 {"error":"Data API v1 has been retired"}`, and the bare `/` too. The
client keeps one host, `data-api.polymarket.com`, and every route is v2's, a method of `DataApi`
(`data.trades()`, `data.positions(user)`).
Tests build with `DataApi::builder().build()`. `tests/mock_data.rs` holds the send-loop proofs
`docs/MUTANTS.md` cites beside the routes' mock tests, and the other `*_data.rs` suites hold the
rest.

## The contract

A `data` envelope, cursor-only pagination and snake_case fields.

- The record structs under `data::types`, and `Page` and `Pagination`, are not
  `#[non_exhaustive]`, so a consumer builds its own rows and pages (prader-rs #179): a field
  added, removed or renamed is a breaking change, and raises the 0.x minor.
  `tests/records_data.rs` builds each by literal, field by field, so the build fails on any of
  them. The enums stay `#[non_exhaustive]` beside their `Other(raw)`.
- Paged builders return `Page<T>` from `send()` and a `Stream` from `.pages()`, which clones one
  request per page so a walk cannot change its filters: on `trades`/`activity` a changed filter
  re-anchors the walk silently, and on `positions` the `title`, `condition` and window filters are
  not in the cursor at all.
- An error with the structured body becomes `DataError::Venue` (a stable `code`, the server's `retryable`
  flag, surfaced and not obeyed, and `trace_id`), recognised by body shape rather than path.
- Its spec is the one mirror served by the API host (`/v2/openapi.json`) rather than
  docs.polymarket.com, so a parity audit that only walks `/api-spec/` misses it.
- `/v2/tokens` (#52) is `data.tokens(TokenKey)`: like `resolutions`, it takes exactly one selector
  family per request, `TokenKey::TokenIds` (at most 50) or `TokenKey::Conditions` (at most 10), and
  an unknown id is absent from the list rather than a `404`. Its `module` is the open
  `TokenModule`, since only `v1_ctf` and `neg_risk` rows have been seen on the wire.
- `/v2/user-pnl` is **not** the retired `user-pnl-api` series, and `/v2/leaderboard` ranks volume
  in shares, not USDC, so neither replaced the retired hosts like for like.
- `tests/spec_agreement_data.rs` checks every type's optionality and field names and every
  builder's query keys against the schema. `tests/wire_agreement_data.rs` checks the types
  against live captures in `tests/fixtures/data/`, in both directions
  (`scripts/capture_polymarket_data.py` refreshes them). `docs/specs/data-v2/OBSERVED.md` records
  where the server and the schema part ways.
- The v2 rate rows are the published table, and the two it does not list were measured
  (`rate-limits.md`). The CLI's `data` group and
  the Python bindings' v2 are in `cli.md` and `py.md`.
