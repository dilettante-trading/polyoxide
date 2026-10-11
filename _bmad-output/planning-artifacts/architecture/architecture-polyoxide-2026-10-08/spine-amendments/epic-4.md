# Proposed spine amendments: Epic 4

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A4-1: venue suites switch to the shared test server in Stories 4.6–4.9 (amends Story 4.4's second criterion)

- **Proposed by:** Claude, as the user's delegate, 2026-10-10, during Epic 4's bundle K (Stories 4.1–4.4), as its decision K-D1.
- **Current rule (Story 4.4):** when the shared server lands, perps', Binance's, sports' and rtds' suites switch to it, each keeping only its protocol `answer` and its fixtures, and the `test_server` paths prader imports are removed and listed.
- **Proposed rule:** Story 4.4 ships `polyoxide_ws::test_server` (feature `test-server`) and its self-tests, and no venue switches to it. Each of Stories 4.6–4.9 switches its venue's suites onto it, under the venue's old supervision loop first, then moves the loop; that story removes the venue's `test_server` module and lists it in `docs/s1-removals.md` (marking its entry `**Removed**` on the doc-hidden paths list, with the story and the replacement).
- **Why:**
  - **One variable per change.** A suite that switches server and loop in one commit cannot say which of the two broke it. Switching the server under the old loop first proves the server is a faithful replacement before the loop moves.
  - **No public API leaves in bundle K.** The `test_server` modules are doc-hidden paths prader imports; removing them belongs with the story that replaces their callers.
  - **The server's fit is still proved now.** Its self-tests write perps' and Binance's answers as the closures their suites will pass, and drive one of each over the wire; rtds' eight server tests are ported by name.
- **Follow-up in the code:** Stories 4.6–4.9 each switch, remove and list their venue's server.

## A4-2: `WsError` is opaque and read through `Kind`; the table is two generated functions (amends AD-15's socket rule and W6)

- **Proposed by:** Claude, as the user's delegate, 2026-10-10, during Epic 4's bundle K (Stories 4.1 and 4.3), as its decisions K-D2 and K-D3.
- **Current rule (AD-15, W6):** socket enums "use `impl_ws_classification!`, the single `WsError` table", whose rows name tungstenite's variants (`Io`, `Protocol`, `ConnectionClosed`, `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity`, `AlreadyClosed`, a handshake `Http(status)`) and close codes; W6 is "the boxed `WsError` wrapper". Neither fixes the type's shape or what the macro expands to.
- **Proposed rule:**
  - **`WsError`** is an opaque `pub struct WsError(Box<Repr>)`, `Send + Sync + 'static`, implementing `std::error::Error` with the tungstenite error as `source()`. It is built by `From<tungstenite::Error>` and by kit constructors for the kit's own failures: `connect_timeout(after)`, `closed(code, reason)`, `from_close_frame(frame)` and `no_answer(after)`.
  - **`kind(&self) -> Kind<'_>`** returns a `#[non_exhaustive]` enum: `Io` (a TLS EOF included), `Protocol`, `ConnectionClosed`, `Closed { code: Option<u16>, reason: &str }`, `Refused { status: u16, retry_after: Option<&str> }` (the raw `Retry-After` header), `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity`, `AlreadyClosed`, `ConnectTimeout { after }`, `NoAnswer { after }`, and `Other` (`WriteBufferFull`, `Utf8`, any later tungstenite variant). No crate outside `polyoxide-ws` matches on tungstenite's error.
  - **`impl_ws_classification!()`** takes no arguments and expands, in the invoking module, to private `fn ws_class(&::polyoxide_ws::WsError) -> Class` and `fn ws_is_fault(&::polyoxide_ws::WsError) -> bool`, using only `$crate::` paths for venue items. A venue's `Classify` impl calls them on its `WsError` arm.
  - **Its rows:** `Io`, `Protocol`, `ConnectionClosed`, `Other`, `ConnectTimeout` and `NoAnswer` are `Network`, as is any kind added later; `Closed { code }` goes through `class_for_close_code`; `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity` and `AlreadyClosed` are `InvalidRequest`; `Refused { status, retry_after }` is `class_for_handshake_status(status).with_retry_after(parse_retry_after(v, Duration::MAX))`, so a 429 carries its wait, a zero none, and a 418 or 451 is `Restricted`. `ws_is_fault` is false only for a 451 refusal.
  - **`polyoxide-ws` re-exports `tokio_tungstenite::tungstenite`**, the reqwest precedent, so venue code names `Message` and `CloseFrame` through it.
- **Why:**
  - **Who can write the impl.** `polyoxide-venue` does not depend on `polyoxide-ws` (AD-2), so it cannot name `WsError`, and `polyoxide-ws` does not depend on venue, so it cannot see `Classify`. In a venue crate both the trait and the type are foreign, which the orphan rule forbids. So the table is a macro that expands in the venue crate, as functions the impl for the venue's own enum calls.
  - **Functions, not an impl.** A venue's enum has its own arms (`Stalled`, `Refused`, a decode failure); two functions slot into its existing `match`. An unused one fails `-D warnings`, so a venue cannot forget the 451 non-fault.
  - **`Kind` over tungstenite's enum.** A borrowed, non-exhaustive view keeps the kit's own failures (a timeout, an unanswered request, a captured close) in the same table, survives a tungstenite upgrade, and carries the refusal's `Retry-After`, which tungstenite's `Http` variant buries in a response.
- **Follow-up in the code:** Story 4.10 reshapes the five socket error enums onto it, keeping Story 2.2's class tables except R1's deliberate handshake changes. `polyoxide-venue/tests/ws_classification.rs` pins every row.

## A4-3: the reconnect rule takes venue's status rule as a function (amends W7)

- **Proposed by:** Claude, as the user's delegate, 2026-10-10, during Epic 4's bundle K (Story 4.2), as its decision K-D4.
- **Current rule (W7, AD-11):** "handshake-status and close classification take the retriable-status rule as an injected function"; refusals and closes are classified through the injected `polyoxide-venue` status rule.
- **Proposed rule:** `polyoxide_ws::retrying_can_fix(err: &WsError, retriable_status: impl Fn(u16) -> bool) -> bool` generalises sports' `retrying_can_fix`: a `Refused { status }` is the rule's answer; `Url`, `HttpFormat`, `Tls`, `AttackAttempt` and `Capacity` are `false`; everything else is `true`. D19's `AlreadyClosed` split and what a close code means stay with each venue's recovery (Story 4.5's hook, or rtds' and sports' call sites). Callers pass venue's rule, `|status| class_for_handshake_status(status).is_retriable()`.
- **Why:**
  - **Two questions, kept apart.** The class (venue's table) is what consumers and nightly runs read; whether a reconnect can help a failed connect is recovery's question. They differ on purpose for `AlreadyClosed` and close codes, and must agree on refused statuses.
  - **One status set.** With the rule injected from venue, the two cannot disagree on a status; `polyoxide-venue/tests/ws_classification.rs` asserts `retrying_can_fix(e, venue_rule) == ws_class(e).is_retriable()` for every status 100–599.
- **Follow-up in the code:** Story 4.8 calls it from rtds and sports (DRIFT R1 for rtds), and Story 4.5's default recovery hook reads the class.

## A4-4: a membership change is applied and answered, recovery may skip, and `is_retriable` is required (amends AD-11)

- **Proposed by:** Claude, as the user's delegate, 2026-10-10, during Epic 4's bundle L1 (Story 4.5), as its decision L1-D1.
- **Current rule (AD-11):** a venue implements a `Protocol` with "`P::Membership` with membership frames and paced `replay`", `wanted`, and "venue-specific recovery". The rule does not say how a caller changes the membership of a running connection or what the caller is told, what recovery may decide besides reconnecting, or what it decides when the venue says nothing.
- **Proposed rule:**
  - **Changes are applied and answered.** `Protocol::apply(&mut self, &mut Connection, &mut Membership, Change) -> Result<Ack, Error>` makes one change on the live connection, through its request correlation, and records it in the membership. How much of a partly refused change to record is the protocol's to decide: perps records none of it, as its old loop did, so the channels a refused request did change are not replayed (an accepted subscribe) or are replayed again (an accepted unsubscribe) after a reconnect.
    - **The caller.** `Supervised::membership()` gives a `Clone` `MembershipHandle<P>`, whose `apply(Change)` waits for that `Ack` or `Error`.
    - **Bounded.** The `Supervisor` bounds each `apply` and `replay` by `stale_after`, because a request is not cancel-safe. A change that outlives it is answered with `Protocol::stalled`, and its connection is replaced.
    - **During an outage.** A change goes to `Protocol::record`. That either records it for the replay and answers at once (`Offline::Recorded`), or holds it (`Offline::Held`, the default) for `apply` once the reconnect has replayed the membership.
    - **After the end.** Once the task has stopped, a change is answered with `Protocol::stopped`.
  - **Recovery has three answers.** `Protocol::recover(&Error) -> Recovery` answers:
    - `Reconnect`, to replace the connection;
    - `Skip`, to yield the error and keep the connection, as for one frame that did not decode (a failed attempt has no connection to keep, so there it reads as `Reconnect`);
    - `Stop`, to yield the error and end the stream.
  - **`is_retriable` is required.** `Protocol::is_retriable(&Error) -> bool` is the one recovery hook a venue must write. `recover` defaults to `Reconnect` exactly when it is true, and `Stop` otherwise. A venue whose rule is not its class overrides `recover`.
- **Why:**
  - **A change needs an answer.** perps tells its callers when a subscribe is refused (`Refused`), unanswered (`Stalled`) or too late (`Stopped`), and its D10 retry of a rate-limited subscribe happens inside the change. A membership frame sent and forgotten could report none of that. A membership the caller mutated directly would leave the caller to decide what a partly refused request recorded, where the protocol knows its venue's answer.
  - **Skipping is not reconnecting.** perps yields a frame that does not decode and keeps reading. With only `Reconnect` and `Stop`, one odd frame would cost a connection or the whole stream.
  - **No silent default.** The kit cannot read a venue's class, since it depends on neither venue nor core (AD-2), so the venue must say which of its errors a reconnect can fix. A default of "always reconnect" would loop on a refusal no retry fixes, and "never" would end a stream on every lost connection.
  - **perps' rule is not its class.** A server close of any code, a `Capacity` failure and a pong that is not `ok` reconnect; a frame that does not decode is skipped; `AlreadyClosed` stops (D19). So perps overrides `recover` and calls `retrying_can_fix` with venue's status rule itself for a refused handshake (DRIFT R1). A4-3's follow-up, that the default recovery reads the class, holds for a venue whose rule is its class.
- **Follow-up in the code:** Story 4.7 writes Binance's `Protocol` on these hooks, with `record` for the changes it records during the backoff and `Supervisor::start` to start in an outage. Epic 7's clob sockets and Epic 8's Kalshi socket follow.
