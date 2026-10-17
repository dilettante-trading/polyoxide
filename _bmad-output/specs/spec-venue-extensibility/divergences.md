# Divergences

There are two kinds of divergence here. **DFR** rows carry behaviour and stay per-venue,
however alike the surrounding code looks. **DRIFT** rows have no recorded reason; each needs a
recorded decision before its copies merge. CLAUDE.md, an adopted companion, holds the full
rationale for the rate-limit and socket rules cited below.

The sports design (2026-10-01, `docs/superpowers/specs/2026-10-01-polyoxide-sports-design.md`
lines 30-31 and 45-49) put a shared socket crate out of scope. Extracting one first would have
rewritten "two shipped, mutation-tested supervisors whose differences are load-bearing". That
was a deferral, not a prohibition. The DFR table is what makes the merge safe now.

## DFR: stays per-venue

| # | Divergence | Why it stays |
|---|---|---|
| D1 | Core's `quota()` omits `allow_burst`; `signer_limit.rs` keeps it | Cloudflare publishes a window quota with no capacity term, so depth borrows from the rate. Polymarket publishes burst as the bucket's capacity for the per-signer layer. "Consistent" would reintroduce the 2x over-permit. |
| D2 | A tenth of each published quota is reserved (`RESERVED_FRACTION`; binance 2160 of 2400) | Measured: 100% and 95% of the published rate were refused; 90% ran clean for 180 s. The published count is reachable as a burst, not as a rate. |
| D3 | Binance weight table, next-minute hold, 418 ban path, weightless funding bucket | Binance charges weight per IP per UTC minute, and continuing after a 429 earns a 418 ban. The table is measured, not copied. |
| D4 | Ping scheduling: rtds on a quiet tick; perps and binance on the wall clock; sports sends none; clob a text `PING` every 10 s, which its supervised sockets send on the kit's wall clock, the first one an interval after the connection opens, where the bare tier pings at once (Epic 7) | Each host's idle-close and ping rules differ. The rtds pump shape caused the perps idle-close bug: "Do not copy the rtds pump shape". |
| D5 | What counts as liveness for staleness | Sports counts pings (data is absent in quiet hours). Rtds counts only decoded events. Binance counts any frame. Perps counts frames plus an ok pong. |
| D6 | What counts as a "delivered" connection for resetting backoff | It follows D5. The shared `Backoff` takes it as a `bool` from the call site. |
| D7 | Supervision shape: sports is a `Stream` state machine with no task; perps and binance spawn a task with a command queue; rtds is a `run(handler)` future | Nothing is ever sent to the sports host, so there is nothing to schedule, which also avoids the close-deadlock class. The task shell (inventory W10) is for venues that send. |
| D8 | Event enums; binance's `Disconnected{path}` → `Reconnected{path}` invariant | prader folds outages on the invariant. Perps has no `Disconnected`. Rtds signals reconnects by a new `Snapshot`. |
| D9 | Binance rotation at 23 h 50 min; paced replay (200 names per request, a request every 200 ms, 1024 streams per connection) | Binance closes the connection when its limits are broken, so the client enforces them first. |
| D10 | Perps retries a `message_rate_limited` subscribe inside `PerpsProtocol::apply` (since Story 4.6); `SequenceRegressed` | Perps signals `message_rate_limited` and a server-wide sequence stamp. |
| D11 | Rtds snapshot and backfill semantics; correcting the mislabelled spot snapshot | These are behaviours of the rtds host, recorded in `docs/specs/rtds/OBSERVED.md`. |
| D12 | Clob decodes Binary frames as text; others skip them | Protocol-specific. The supervised clob sockets keep it: their decode reads a Binary frame as text, as the bare tier does (Epic 7). |
| D13 | Core's HTTP `RetryConfig::backoff` (jittered) vs the socket `Backoff` | Different algorithms for different failure modes. Do not unify. |
| D14 | Venue error-body formats: Polymarket `error`/`message`; data v2 envelope; perps `{status, error, ref}`; Binance `{code, msg}` with 403 meaning WAF and 451 meaning location | These are venue facts. The shared interface (CAP-4) classifies them; it does not replace them. |
| D15 | FAK/FOK kill outcomes classified from the 400 body (`classify_order_kill`) | Polymarket ships no error code for them, so the message body is the only signal. |
| D16 | Perps `Interval` includes `1s`; Binance's refuses it | The hosts accept different sets. |
| D17 | `425 Too Early` retried by the send loop | It is Polymarket's matching-engine signal, so the loop's retry of 425 belongs to Polymarket's one retry policy, shared by every Polymarket module (spine AD-17). The caller-facing retriable rule keeps 425 as standard HTTP (RFC 8470; class `Unavailable`), so it is not a Polymarket identifier in the foundation. |
| D18 | Rtds and sports depend on nothing heavy | They keep credential-free feeds free of `alloy` (clob with `ws` builds 352 crates; core builds 161). The shared socket crate must keep this, by the dependency list in SPEC Constraints. |
| D19 | After a use-after-close (`AlreadyClosed`), perps and rtds stop while Binance, sports and the supervised clob sockets reconnect | The class is `InvalidRequest` everywhere (spine AD-15). Each venue keeps its behaviour in its recovery hook: perps' and Binance's `Protocol::recover`, rtds' public `RtdsError::recovery`, and sports' private `retrying_can_fix` in `supervised.rs`, since D7 keeps rtds and sports off the Supervisor. Each is pinned by its suites (rtds `error.rs`). sports reconnects after any lost connection, a read failing with `AlreadyClosed` included; its recovery row also retries a connect that fails so (`supervised.rs`, since Story 4.10). The supervised clob sockets reconnect after one as well: their `recover` reconnects exactly when `Classify::is_retriable` says so, and a use after close (`Kind::AlreadyClosed`) reconnects too, though its class stays `InvalidRequest` (Epic 7). |
| D20 | Clob's per-address connect: it resolves every address, interleaves the families (RFC 8305) and dials each in turn | `polyoxide_ws::connect` dials only the first address, so a blackholed route (broken IPv6) would spend the whole timeout there. Each step, the TCP connect and then the TLS and upgrade, is bounded by `polyoxide_ws::with_timeout`, so `connect_timeout` keeps meaning per address and per step; the upgrade goes through `polyoxide_ws::connect_over` (Story 4.9). Under supervision the same dial runs on every attempt: `ClobMarketProtocol` and `ClobUserProtocol` override `Protocol::connect` with it (Epic 7), so each address is tried within `connect_timeout` per step there too. |

## DRIFT: decided 2026-10-08 (R11 2026-10-12, R12 2026-10-16)

These rows had no recorded reason. Each now has a decision, applied when its copies merge.
R1, R2, R4, R11 and R12 change behaviour users can see; record them in the release notes.

| # | Drift | Copies | Decision |
|---|---|---|---|
| R1 | Reconnect handshake statuses: perps retries 429/5xx only; rtds retries none (a 503 or 429 at reconnect ends `run()`); sports and binance retry 408/425/429/5xx | perps `ws/error.rs:108`; rtds `error.rs:140`; sports `error.rs:80` (in `supervised.rs` since Story 4.10); binance `error.rs:116` | Retry 408/425/429/5xx everywhere, matching core's HTTP rule. rtds stops treating 503/429 at reconnect as fatal; perps adds 408/425. |
| R2 | No connect timeout in rtds and perps | rtds `client.rs:101`; perps `ws/client.rs:87` | Every socket connect has a timeout, 10 s by default, through the shared connect-with-timeout. |
| R3 | Only sports sends the RFC 6455 close reply; four crates return without flushing it (observed, not yet verified on the wire) | rtds `client.rs:165`; perps `ws/client.rs:259`; binance `usdm/ws/client.rs:404`; clob `ws/client.rs:504` (fixed by Story 4.9) | Confirm on the wire that the reply is missing, then send it from the shared bare tier for every venue. |
| R4 | Four `Retry-After` acceptance rules (f64 > 0; f64 > 0 clamped to 3 days; f64 ≥ 0; u64 only) | inventory H13 | One parser. `Retry-After` may only lengthen the client's own backoff (CLAUDE.md), which settles zero. The upper clamp is a parameter. |
| R5 | Clob's `rustls` lacks `std` and relies on transitive enabling | `polyoxide-clob/Cargo.toml:52` | Declare `rustls` once in `[workspace.dependencies]` with `ring` and `std`. |
| R6 | Feature name `test-fixtures` (rtds) vs `test-server` (others) | inventory W14 | `test-server` everywhere; rtds renames `test-fixtures`. |
| R7 | Relay errors do not wrap `ApiError`; relay's `Core` and `RateLimit` variants are never constructed; relay has no 429 tests | `polyoxide-relay/src/error.rs`; `client.rs:285,368,1785` | Relay moves onto the shared send path and the CAP-4 error interface, and gains 429 tests. |
| R8 | Gamma `post_json` takes the limiter before the permit, with no retry and no 429 feedback; clob ping is ungated; gamma and data pings skip 429 feedback | inventory H2 | All go through the shared send path. |
| R9 | `ensure_crypto_provider` is bypassed by three live tests that call `install_default` themselves | sports `tests/live_api.rs:63`; binance `tests/live_ws.rs:133`; clob `tests/live_ws.rs:88` (fixed by Story 4.9) | Live tests call the shared function. |
| R10 | Retry log text and target differ by crate, so soak harnesses see only core's loop | inventory H1 | Settled by the SPEC constraint on the retry log line. |
| R11 | A reconnect refused with a `Retry-After` retries on the backoff step, inside the server's window: perps, Binance, sports, and rtds once R1 made it retry | the kit's `Supervisor` (perps, Binance); rtds `supervisor.rs`; sports `supervised.rs` | Wait the longer of the step and the `Retry-After`, clamped to the venue's backoff ceiling (`Backoff::take_at_least`), as core's HTTP loop lets `Retry-After` only extend its wait. perps and Binance answer `Protocol::retry_after` from their class; rtds and sports apply it at their reconnect waits. The supervised clob sockets (Epic 7) answer `Protocol::retry_after` from their class, as perps and Binance do, and the kit waits it out; the bare tier does not reconnect. |
| R12 | Polymarket retired `user-pnl-api` and `lb-api` on 2026-10-16: each answers `410 Gone` and names `/v2/user-pnl` or `/v2/leaderboard` | data's `pnl()` and `rankings()`; `live_data`'s `live_user_pnl_series` and `live_rankings_volume_and_profit` | Deprecate both accessors in 0.40.0, pointing at their v2 counterparts, which are not the same series (`docs/specs/data-v2/OBSERVED.md`); the two live tests hold the `410`. Removal is S3's. |
