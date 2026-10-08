# Duplication inventory

This is the checklist behind the success signal. Every row must end with exactly one definition.

- Locations come from the 2026-10-08 audit of commit `e3d8c3e` (v0.37.1). Gamma and sports rows were re-audited at `v0.38.1` (`12e8316`); nothing else cited here changed between the two. Line counts are rough and include tests.
- "Class" is IDENTICAL or PARAMETRIC; see `glossary.md`.
- Rows whose copies also differ in ways that carry behaviour point to `divergences.md`, which governs what may merge.

## HTTP (CAP-1, CAP-2, CAP-4)

| # | Concern | Copies | Class | ~Lines |
|---|---|---|---|---|
| H1 | Send/retry loop (gate, send, 429 feedback, retry decision, sleep) | core `request.rs:123` and `client.rs:194` (`get_bytes`); clob `request.rs:195`; relay `client.rs:285`, `:368`, `:1785`; binance `usdm/request.rs:83` | PARAMETRIC (binance adds a 418 ban, a next-minute hold and weight charging; clob adds auth headers on every attempt and the signer limiter) | 465 + 250 overlapping tests |
| H2 | Unlooped, hand-gated sends | gamma `api/health.rs:35`, `api/markets.rs:143` (`post_json`, wrong gate order); data `api/health.rs:38`; clob `api/health.rs:34` (ungated) | PARAMETRIC, with holes | 60 |
| H3 | Decode body as JSON, log a truncated body on failure | core `request.rs:105`; clob `request.rs:178`; binance `request.rs:71`; relay `client.rs:1866` | IDENTICAL | 40 |
| H4 | Cooldown: extend-only deadline, re-check after waking, poison-tolerant lock | core `rate_limit.rs:121,360-396`; binance `weight.rs:189,348-400` | PARAMETRIC (binance clamps to 3 days) | 220 incl. mirrored tests |
| H5 | Reserve a tenth of the published quota; depth-one paced slot | core `rate_limit.rs:177`; binance `weight.rs:35,247,385`; data `examples/common/mod.rs:228`; perps `examples/info_soak.rs:303` | PARAMETRIC | 60 |
| H6 | Client builder knobs: `base_url`, `timeout_ms`, `pool_size`, `with_retry_config`, `max_concurrent`, limiter, gzip | gamma, data, perps, clob, relay, binance builders; core `HttpClientBuilder::new` vs `Default` | PARAMETRIC (default concurrency 2, 4 or 8) | 540 |
| H7 | Namespace accessors `X { http_client: clone }` and their structs | gamma 9, data 14, perps 4, clob 8, binance 3 | IDENTICAL shape | 430 |
| H8 | `ping` / health | gamma, data, clob, perps, relay, binance (6 hand-written versions) plus `ping_propagates_*` test pairs | PARAMETRIC | 285 |
| H9 | Query setters `fn x(mut self, v) -> Self` | gamma ~197 at `e3d8c3e`, +2 in v0.38 (`ListEvents::game_id` and `ListKeysetEvents::include_markets`, `api/events.rs`), data ~144, clob ~54, binance 12; perps has a `pub(crate)` `setter!` | IDENTICAL shape | 2,000 |
| H10 | Open/closed string-enum macros and `UnknownVariant` | `open_enum!` in data `v2/types/common.rs:5`, gamma `types.rs:10`, binance `usdm/types.rs:132`; relay `open_string_enum!`; `wire_enum!` in perps `types.rs:41`, binance `usdm/types.rs:94`; data `closed_enum!`; `UnknownVariant` in perps and binance | PARAMETRIC | 390 |
| H11 | Positional decimal-array serde (klines, mark points) | perps `types.rs:155-264`; binance `usdm/types.rs:828-1036` | PARAMETRIC (the `Interval` enums differ on purpose) | 300 |
| H12 | Retriable-status rule 408/425/429/5xx | core `error.rs:93`; perps `error.rs:77`; binance `error.rs:102` | IDENTICAL | 30 + pinning tests |
| H13 | `Retry-After` parser | core `client.rs:147`; binance `error.rs:128`; data `v2/error.rs:94`; perps `error.rs:64` | DRIFT (3 acceptance rules) | 40 |
| H14 | Venue error-body parse `(status, retry_after, body)` and `RequestError::from_response` | data `error.rs:56`; perps `error.rs:107`; binance `error.rs:75` | PARAMETRIC | 100 |
| H15 | `ApiError` wrapping and conversion macro | gamma, data, perps, binance use `impl_api_error_conversions!`; clob hand-writes it (`error.rs:166`); relay does not wrap at all (`Api(String)`) | PARAMETRIC, with holes | 60 |
| H16 | Unix-ms "now" | about 9 inline copies (clob `client.rs:367,465`, binance `weight.rs:217`, tests, examples); core's `current_timestamp` is seconds only | IDENTICAL | 30 |

## Sockets (CAP-3)

| # | Concern | Copies | Class | ~Lines |
|---|---|---|---|---|
| W1 | `ensure_crypto_provider` + its test | clob `ws/client.rs:47`; perps `ws/mod.rs:41`; binance `usdm/ws/mod.rs:92`; rtds `client.rs:32`; sports `client.rs:37`; three live tests call `install_default` directly | IDENTICAL | 150 |
| W2 | `rustls` dependency stanza | rtds, sports, perps, binance `["ring","std"]`; clob `["ring"]` only | DRIFT (clob) | 5 stanzas |
| W3 | `Backoff` (clamp, doubling, ceiling, reset hook, `MIN_BACKOFF`), `backoff()` setter, 500 ms–60 s defaults, unit tests | rtds `supervisor.rs:37`; sports `supervised.rs:53`; perps `ws/supervised.rs:39`; binance `usdm/ws/supervised.rs:48` | IDENTICAL (the reset **input** stays per venue) | 430 + 150 timing tests |
| W4 | Timing setters and defaults (`ping_interval`, `stale_after`, `connect_timeout`, `url`), `defaults_match_*` tests | rtds, sports, perps, binance | PARAMETRIC | 200 |
| W5 | Deadline arithmetic `min(ping_due, stale_due[, age_due])` with saturation | perps `ws/supervised.rs:433`; binance `usdm/ws/supervised.rs:902`; sports `supervised.rs:350` | PARAMETRIC | 30 |
| W6 | Boxed `tungstenite::Error` wrapper + `From` | rtds `error.rs:94`; perps `error.rs:100`; binance `error.rs:108`; clob `ws/error.rs:47` | IDENTICAL | 40 |
| W7 | Handshake-status classifier and `Recovery` enum | sports `error.rs:80`; binance `error.rs:116`; perps `error.rs:108`; rtds `error.rs:140`; `Recovery` in rtds, perps (adds `Retry`) and binance | PARAMETRIC + DRIFT (status sets differ) | 240 |
| W8 | Connect with timeout | sports `client.rs:47`; binance `usdm/ws/client.rs:160`; rtds and perps have none; clob has a per-address `connect_ws` (`ws/client.rs:84`) | PARAMETRIC + DRIFT | 120 |
| W9 | Close code/reason capture | sports `client.rs:97` (`closed()`); binance `usdm/ws/client.rs:279` (`record_close`) | PARAMETRIC | 30 |
| W10 | Supervised task shell: events mpsc(1024), commands mpsc(16), `membership()`, `close()`, `poll_recv`, `Debug`, `MembershipHandle::send` with oneshot reply | perps `ws/supervised.rs:208-300`; binance `usdm/ws/supervised.rs:346-446` | IDENTICAL shape (generic over event, error, command) | 140 |
| W11 | Bare-tier `poll_next` skeleton (skip Ping/Pong/Frame, `close()`, `connect_to(url)`, socket type alias) | rtds, perps, binance, sports, clob | PARAMETRIC | 60 |
| W12 | Request/answer correlation (`next_id`, buffered `VecDeque`) | perps `ws/client.rs:215`; binance `usdm/ws/client.rs:325` | PARAMETRIC (the envelope differs) | 60 |
| W13 | Scripted test server: accept loop, script per connection with the last repeated, `connection_count`, `wait_for`, reject/stall/refuse/close, recorders, timer idiom; pluggable `answer` | sports `test_server.rs` (271 lines); perps (354); binance (364); rtds (470); clob has two ad-hoc servers (`ws/client.rs:1070`) | PARAMETRIC | 320 |
| W14 | Test-only feature name | rtds `test-fixtures`; sports, perps, binance `test-server` | DRIFT | — |
| W15 | Test helpers `next_event`/`next_item`, `fast()`, ping counting by substring | perps, binance (`tests/supervision.rs`, `supervision_edges.rs`), sports | PARAMETRIC | 80 |

## Tests, examples, scripts (CAP-9)

| # | Concern | Copies | Class | ~Lines |
|---|---|---|---|---|
| T1 | `key_paths` | data `tests/v2_wire_agreement.rs:158`; binance `tests/common/mod.rs:14`; perps `tests/wire_agreement.rs:52`, `tests/ws_wire_agreement.rs:33` | IDENTICAL | 80 |
| T2 | `assert_values_agree` | binance `common/mod.rs:35`; perps `wire_agreement.rs:73`, `ws_wire_agreement.rs:49` | IDENTICAL | 60 |
| T3 | Allow-lists (IGNORED / EXPECTED_ABSENT / NEVER_ON_WIRE) with a stale-excuse check | perps `wire_agreement.rs:97-181`; data `v2_wire_agreement.rs:186-290`; gamma's dotted-path version differs. Its one `check` walker (`tests/wire_agreement.rs:261`) is fed by `round_trip` (`:311`, Comment only), six inline decode-and-re-emit test bodies (`:330`, `:339`, `:348`, `:357`, `:415`, `:424`; Profile, UserResponse, SearchProfile), a generic `agrees<T>` (`:462`) and `captured_event` (`:476`), whose `Event` feeds direct `check` calls on `teams` and `sport`. `sports_events_carry_no_unmodelled_top_level_keys` (`:499`) checks direction 2 only, at an event's top level, and never consults `IGNORED`; the shared helper must support that to keep the assertion. `agrees<T>`, `captured_event` and the top-level check arrived in v0.38.0 | PARAMETRIC | 180 |
| T4 | OpenAPI synthesiser (`spec`, `schemas`, `is_nullable`, `synth`, `fields`, `check`, `agreement!`) | data `v2_spec_agreement.rs:45-203`; perps `spec_agreement.rs:24-254` (a fork that adds `$ref`-nullable, enum and example handling, and `OBSERVED_EXTRA`) | PARAMETRIC | 380 |
| T5 | `query_keys_sent` / `Fire` | data `v2_spec_agreement.rs:345`; perps `spec_agreement.rs:399` | IDENTICAL apart from the client type | 70 |
| T6 | Fixture loader from `CARGO_MANIFEST_DIR/tests/fixtures` | 6 copies | IDENTICAL | 30 |
| T7 | Minute-boundary waits | binance `tests/mock_api.rs:58`, `examples/weight_probe.rs:110` | IDENTICAL | 30 |
| T8 | Soak harness: `Pacer`, `MessageVisitor`, `ThrottleLayer`, `classify`/`judge`/`pin`, stage parsing | data `examples/common/mod.rs:155-242` + `examples/v2_soak/`; perps `examples/info_soak.rs:167-356` | PARAMETRIC (the data harness is wired by `#[path]`) | 300 |
| T9 | Capture scripts: HTTP `get()`, PROVENANCE writer, WebSocket client | `scripts/capture_{perps,v2}_fixtures.py` share a near-identical `get()`; binance has its own `get()` and a hand-rolled RFC 6455 client; perps-ws and sports use `websockets` | PARAMETRIC | 150 |

## CLI and Python (CAP-7)

| # | Concern | Copies | Class |
|---|---|---|---|
| C1 | `OutputFormat` enum | `ws/{market,user,prices,sports,binance}.rs`, `clob/prices/types.rs` | PARAMETRIC (6 copies) |
| C2 | Streaming `run_with` loop (count, deadline, broken pipe, feed ended) and `excerpt()` | `ws/sports.rs:75-153,270`; `ws/binance.rs:174-243,330`; `market`/`user`/`prices` use an older `ctrlc` + `select!` loop that cannot be tested | PARAMETRIC |
| C3 | CLI stream tests (count stops, huge timeout, closed reader, quiet feed) | `tests/ws_sports.rs`, `tests/ws_binance.rs` | PARAMETRIC |
| C4 | Python error mapping | `polyoxide-py/src/error.rs:13-54`: per-crate functions, classified by substring of the Display text, `with_details` tied to data v2 | PARAMETRIC, replaced by CAP-4 |
