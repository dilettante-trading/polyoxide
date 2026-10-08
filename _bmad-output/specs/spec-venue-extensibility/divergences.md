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
| D4 | Ping scheduling: rtds on a quiet tick; perps and binance on the wall clock; sports sends none; clob every 10 s | Each host's idle-close and ping rules differ. The rtds pump shape caused the perps idle-close bug: "Do not copy the rtds pump shape". |
| D5 | What counts as liveness for staleness | Sports counts pings (data is absent in quiet hours). Rtds counts only decoded events. Binance counts any frame. Perps counts frames plus an ok pong. |
| D6 | What counts as a "delivered" connection for resetting backoff | It follows D5. The shared `Backoff` takes it as a `bool` from the call site. |
| D7 | Supervision shape: sports is a `Stream` state machine with no task; perps and binance spawn a task with a command queue; rtds is a `run(handler)` future | Nothing is ever sent to the sports host, so there is nothing to schedule, which also avoids the close-deadlock class. The task shell (inventory W10) is for venues that send. |
| D8 | Event enums; binance's `Disconnected{path}` → `Reconnected{path}` invariant | prader folds outages on the invariant. Perps has no `Disconnected`. Rtds signals reconnects by a new `Snapshot`. |
| D9 | Binance rotation at 23 h 50 min; paced replay (200 names per request, a request every 200 ms, 1024 streams per connection) | Binance closes the connection when its limits are broken, so the client enforces them first. |
| D10 | Perps `Recovery::Retry` and `SequenceRegressed` | Perps signals `message_rate_limited` and a server-wide sequence stamp. |
| D11 | Rtds snapshot and backfill semantics; correcting the mislabelled spot snapshot | These are behaviours of the rtds host, recorded in `docs/specs/rtds/OBSERVED.md`. |
| D12 | Clob decodes Binary frames as text; others skip them | Protocol-specific. |
| D13 | Core's HTTP `RetryConfig::backoff` (jittered) vs the socket `Backoff` | Different algorithms for different failure modes. Do not unify. |
| D14 | Venue error-body formats: Polymarket `error`/`message`; data v2 envelope; perps `{status, error, ref}`; Binance `{code, msg}` with 403 meaning WAF and 451 meaning location | These are venue facts. The shared interface (CAP-4) classifies them; it does not replace them. |
| D15 | FAK/FOK kill outcomes classified from the 400 body (`classify_order_kill`) | Polymarket ships no error code for them, so the message body is the only signal. |
| D16 | Perps `Interval` includes `1s`; Binance's refuses it | The hosts accept different sets. |
| D17 | `425 Too Early` retried by the send loop | It is Polymarket's matching-engine signal, so the loop's retry of 425 belongs to Polymarket's one retry policy, shared by every Polymarket module (spine AD-17). The caller-facing retriable rule keeps 425 as standard HTTP (RFC 8470; class `Unavailable`), so it is not a Polymarket identifier in the foundation. |
| D18 | Rtds and sports depend on nothing heavy | They keep credential-free feeds free of `alloy` (clob with `ws` builds 352 crates; core builds 161). The shared socket crate must keep this, by the dependency list in SPEC Constraints. |
| D19 | After a use-after-close (`AlreadyClosed`), perps and rtds stop while Binance and sports reconnect | Each venue's behaviour is pinned by its suites (rtds `error.rs` test). The class is `InvalidRequest` everywhere, and each venue keeps its behaviour through its `Protocol` recovery hook (spine AD-15). |

## DRIFT: decided 2026-10-08

These rows had no recorded reason. Each now has a decision, applied when its copies merge.
R1, R2 and R4 change behaviour users can see; record them in the release notes.

| # | Drift | Copies | Decision |
|---|---|---|---|
| R1 | Reconnect handshake statuses: perps retries 429/5xx only; rtds retries none (a 503 or 429 at reconnect ends `run()`); sports and binance retry 408/425/429/5xx | perps `ws/error.rs:108`; rtds `error.rs:140`; sports `error.rs:80`; binance `error.rs:116` | Retry 408/425/429/5xx everywhere, matching core's HTTP rule. rtds stops treating 503/429 at reconnect as fatal; perps adds 408/425. |
| R2 | No connect timeout in rtds and perps | rtds `client.rs:101`; perps `ws/client.rs:87` | Every socket connect has a timeout, 10 s by default, through the shared connect-with-timeout. |
| R3 | Only sports sends the RFC 6455 close reply; four crates return without flushing it (observed, not yet verified on the wire) | rtds `client.rs:165`; perps `ws/client.rs:259`; binance `usdm/ws/client.rs:404`; clob `ws/client.rs:504` | Confirm on the wire that the reply is missing, then send it from the shared bare tier for every venue. |
| R4 | Four `Retry-After` acceptance rules (f64 > 0; f64 > 0 clamped to 3 days; f64 ≥ 0; u64 only) | inventory H13 | One parser. `Retry-After` may only lengthen the client's own backoff (CLAUDE.md), which settles zero. The upper clamp is a parameter. |
| R5 | Clob's `rustls` lacks `std` and relies on transitive enabling | `polyoxide-clob/Cargo.toml:52` | Declare `rustls` once in `[workspace.dependencies]` with `ring` and `std`. |
| R6 | Feature name `test-fixtures` (rtds) vs `test-server` (others) | inventory W14 | `test-server` everywhere; rtds renames `test-fixtures`. |
| R7 | Relay errors do not wrap `ApiError`; relay's `Core` and `RateLimit` variants are never constructed; relay has no 429 tests | `polyoxide-relay/src/error.rs`; `client.rs:285,368,1785` | Relay moves onto the shared send path and the CAP-4 error interface, and gains 429 tests. |
| R8 | Gamma `post_json` takes the limiter before the permit, with no retry and no 429 feedback; clob ping is ungated; gamma and data pings skip 429 feedback | inventory H2 | All go through the shared send path. |
| R9 | `ensure_crypto_provider` is bypassed by three live tests that call `install_default` themselves | sports `tests/live_api.rs:63`; binance `tests/live_ws.rs:133`; clob `tests/live_ws.rs:88` | Live tests call the shared function. |
| R10 | Retry log text and target differ by crate, so soak harnesses see only core's loop | inventory H1 | Settled by the SPEC constraint on the retry log line. |
