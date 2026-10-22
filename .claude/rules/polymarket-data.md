---
paths:
  - "polyoxide-polymarket/src/data/**"
  - "polyoxide-polymarket/tests/*data*.rs"
  - "polyoxide-polymarket/tests/common/**"
  - "polyoxide-polymarket/tests/fixtures/data/**"
  - "polyoxide-polymarket/examples/v2_soak/**"
  - "polyoxide-polymarket/examples/closed_positions_*.rs"
  - "polyoxide-cli/src/commands/polymarket/data/**"
  - "polyoxide-cli/tests/data_v2.rs"
  - "scripts/{capture_polymarket_data,gen_data_v2_types,compare_v2_overlaps}.py"
  - "docs/specs/{data,data-v2,undocumented}/**"
---

# Polymarket Data API

Namespaces: `data.user(addr)`, `trades()`, `holders()`, `leaderboard()`, `builders()`,
`live_volume()`, `open_interest()`, `market_positions()`, `combos()`, `misc()`, `pnl()`,
`rankings()`, `accounting()`, `health()`, `v2()`. Tests build with `DataApi::builder().build()`.
Its health route is `/`.

- `data.approvals()` is deprecated: upstream removed `/v1/approvals` and the host now returns
  `404`; `data.v2().approvals()` serves the same data.
- `data.pnl()` and `data.rankings()` target sibling hosts (`user-pnl-api` and `lb-api`) that have
  no published spec (`docs/specs/undocumented/INDEX.md`). Polymarket retired both on 2026-10-16:
  each answers `410 Gone`, so both accessors are deprecated (DRIFT R12) and their live tests hold
  the `410`. Their base URLs are configurable via `DataApiBuilder::pnl_base_url` /
  `rankings_base_url`, and all three hosts share one connection pool and concurrency budget via
  `HttpClient::with_base_url`.
- `tests/common/mod.rs` is shared by `live_data.rs` and `holders_shape_data.rs`, not by the mock
  suites.

## v2 (`data.v2()`)

Upstream says the v1 routes keep working. v2 uses a different contract: a `data` envelope,
cursor-only pagination and snake_case fields.

- Paged builders return `Page<T>` from `send()` and a `Stream` from `.pages()`, which clones one
  request per page so a walk cannot change its filters: on `trades`/`activity` a changed filter
  re-anchors the walk silently, and on `positions` the `title`, `condition` and window filters are
  not in the cursor at all.
- An error with the v2 body becomes `DataError::V2` (a stable `code`, the server's `retryable`
  flag, surfaced and not obeyed, and `trace_id`), recognised by body shape rather than path.
- Its spec is the one mirror served by the API host (`/v2/openapi.json`) rather than
  docs.polymarket.com, so a parity audit that only walks `/api-spec/` misses it.
- `/v2/tokens` (#52) is `v2().tokens(TokenKey)`: like `resolutions`, it takes exactly one selector
  family per request, `TokenKey::TokenIds` (at most 50) or `TokenKey::Conditions` (at most 10), and
  an unknown id is absent from the list rather than a `404`. Its `module` is the open
  `TokenModule`, since only `v1_ctf` and `neg_risk` rows have been seen on the wire.
- `/v2/user-pnl` is **not** the `data.pnl()` series, and `/v2/leaderboard` ranks volume in shares,
  not USDC, so neither replaces the undocumented hosts.
- `tests/spec_agreement_data_v2.rs` checks every type's optionality and field names and every
  builder's query keys against the schema. `tests/wire_agreement_data_v2.rs` checks the types
  against live captures in `tests/fixtures/data/v2/`, in both directions
  (`scripts/capture_polymarket_data.py` refreshes them). `docs/specs/data-v2/OBSERVED.md` records
  where the server and the schema part ways.
- The v2 rate rows were measured over distinct URLs (`rate-limits.md`). The CLI's `data` group and
  the Python bindings' v2 are in `cli.md` and `py.md`.
