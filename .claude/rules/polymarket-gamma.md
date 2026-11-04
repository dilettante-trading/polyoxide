---
paths:
  - "polyoxide-polymarket/src/gamma/**"
  - "polyoxide-polymarket/tests/*gamma*.rs"
  - "polyoxide-polymarket/tests/fixtures/gamma/**"
  - "polyoxide-polymarket/examples/gamma_batch_ceiling.rs"
  - "polyoxide-cli/src/commands/polymarket/gamma/**"
  - "docs/specs/gamma/**"
---

# Polymarket Gamma

Namespaces: `gamma.markets()`, `events()`, `series()`, `tags()`, `comments()`, `sports()`,
`search()`, `user()`, `health()`. Tests build with `Gamma::builder().build()`. Its health route is
`/status`.

## `closed`

`closed` is `bool | "any"` upstream (#59). `closed(bool)` stays, and `closed_filter(ClosedFilter)`
on `ListEvents`, `ListKeysetEvents`, `ListMarkets` and `ListKeysetMarkets` is the only way to send
`any`, which `/markets` and `/markets/keyset` refuse with a `422` unless an identifier filter is
set (`docs/specs/gamma/OBSERVED.md`). A repeated `closed` is not last-wins (`any` beats a bool,
`false` beats `true`), so a caller sets it once. An identifier filter holds at most 100 values, so
`Markets::get_many` sends `closed=any` once for each 100 distinct ids, in turn, with a `limit` of
their number, because `/markets` pages at 20 rows otherwise (DRIFT R13).

## The mirror can match upstream and still be wrong

`docs/specs/gamma/OBSERVED.md` records where gamma's published spec disagrees with gamma's own
server: `parent_entity_type` accepts `PerpsAsset` and rejects the documented `market`; `limit`
counts top-level comments rather than rows; `GET /comments/{id}` returns a whole thread.

## Sports fields

Gamma's half of the sports join is `Event::game_id`, `teams` (each `Team::ordering` a `HomeAway`),
`sport` and `parent_event_id`, none of which `openapi.yaml` lists: they are modelled from the
served `Event.json`. One game id can return the game and its child events, so keep the one with no
`parent_event_id`. The sports feed reconciles through `events?game_id=` (`polymarket-sports.md`).

## Polymarket Protocol V2 is not the CLOB V2 migration

`Market::version` says which protocol a market trades on. A `v2` market takes its outcome ids from
`position_ids`, signs orders for ExchangeV3 (domain version `"3"`) and reads balances as
`CONDITIONAL-V2`; trading it is supported (#51), pending live verification, since no `v2` market
existed on 2026-10-07 and `live_v2_market_is_served` reports `environmental` until one does.

`Market::outcome_ids` is the one place that chooses the ids: `clob_token_ids` (JSON text) for `v1`,
`position_ids` for `v2`, the other field never read. Never choose by which field is present: 35% of
open `v1` markets also carry `position_ids`, and the CLOB has no book for them. It pairs the ids
with `outcomes` in order, and refuses with `OutcomeIdsError` (`gamma::OutcomeIdsError`, not a
`GammaError` variant, which decodes only through its `From<ApiError>`): `MissingVersion`,
`UnsupportedVersion` (any other `version`, which arrives as `ProtocolVersion::Other`), `MissingIds`,
`MalformedIds`, `MalformedOutcomes`, `CountMismatch` and `InvalidId` (an id that is empty or not all
ASCII digits). Each is `Class::Decode`, a fault, with no wait. The binary cases are pinned to
py-sdk's `normalize_market_position_context` by `tests/fixtures/gamma/protocol_v2/`
(`scripts/capture_polymarket_protocol_v2.py`); a three-outcome market and a malformed `outcomes`,
which py-sdk cannot express, are Rust-only cases in `tests/outcome_ids_gamma.rs`.

A `v2` market's `resolution_status` replaces `uma_resolution_status`: read the first for `v2` and
the second for `v1`. Upstream's guide is `docs.polymarket.com/migrate/polymarket-v2/`; the wire
facts are in `docs/specs/gamma/OBSERVED.md`.
