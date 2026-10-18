# Epic 8: the Kalshi walking skeleton — design

Status: approved by the owner, 2026-10-18. This spec is the design record for the plan
`docs/plans/2026-10-18-epic-8-kalshi-walking-skeleton.md`. It settles what the acceptance criteria
of `_bmad-output/planning-artifacts/epics.md` (Epic 8, Stories 8.1 to 8.5) leave open, and says
where it departs from them. Read it with those criteria and with the architecture spine,
`_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/ARCHITECTURE-SPINE.md`
(AD-1, AD-4, AD-5, AD-6, AD-7, AD-8, AD-10, AD-11, AD-13, AD-14, AD-15, AD-17, AD-19, AD-21,
AD-23), its amendments A6-1 (`spine-amendments/epic-6.md`) and A7-1 to A7-4
(`spine-amendments/epic-7.md`), the guide's recipe (`docs/ARCHITECTURE.md`, "Adding a venue"), and
CLAUDE.md. Where this spec and an acceptance criterion disagree, this spec binds.

## 1. Scope, stage and sources

**1.1 Stage.** The workspace is in stage S3 (`[workspace.metadata.polyoxide] stage = "S3"`, CI's
removals job `api_removals.py check --baseline v0.40.0`). The epic adds one crate,
`polyoxide-kalshi`, which is `publish = false`, and one doc-hidden recorder on `polyoxide-ws`'s
scripted test server (D17). It removes nothing, so `docs/s3-removals.md` gains nothing, and no
release ships the crate. The run makes no version bump and does not change the stage. It runs on
`aidanb/init`. Local `main` has moved on since this spec was written: it holds Epic 7,
`chore(release): v0.41.0` (`d1fa06a`) and a CLOB drift adoption (`9096489`), none of it pushed when
this was written. So the branch merges `main` before the run starts. The touch-set gate's base
(D20, §10) is `git merge-base main HEAD`, worked out each time the gate runs, so `main`'s own
commits never count, whether they are merged in or not. Edits to this spec and to the plan are in
the set.

**1.2 The plan's stories.**

| Plan story | Epic story | What |
|---|---|---|
| 1 | 8.1, and 8.2's exchange status | Register `polyoxide-kalshi`: the crate, its metadata, the mirror, the keys, a minimal client with `health().ping()` and `exchange().status()`, and the CI wiring |
| 2 | 8.2 | The public REST surface: routes, types, fixtures, and the mock, agreement and live suites, at the compiled costs |
| 3 | 8.3 | `MarketData`, `Trades` and `EventGroups` on `Predictions` |
| 4 | 8.2 | Signing: credentials, the authenticator, and the first-use confirmation of costs and limits |
| 5 | 8.4 (prerequisite) | The kit's scripted server records each handshake's headers |
| 6 | 8.4 | The supervised socket |
| 7 | 8.5 | Validation, amendments and records |

Stories 1, 2 and 3 run in order. Story 4 needs Story 2. Story 5 touches only `polyoxide-ws` and may
run at any time before Story 6. Story 6 needs Stories 4 and 5. Story 7 runs last.

**1.3 Sources**, read 2026-10-18.

- Kalshi's OpenAPI, `https://docs.kalshi.com/openapi.yaml`, `info.version` 3.34.0, and its
  AsyncAPI, `https://docs.kalshi.com/asyncapi.yaml`, `info.version` 2.0.0. These are the versions
  the trading mapping read on 2026-10-12. Kalshi publishes neither as JSON: `openapi.json` and
  `asyncapi.json` are 404.
- The prose pages `getting_started/api_environments`, `api_keys`, `rate_limits`,
  `quick_start_market_data`, `demo_env`, `quick_start_websockets`, and
  `websockets/websocket-connection`, `connection-keep-alive` and `orderbook-updates`; and the
  changelog, through its entries dated 2026-10-22.
- Read-only probes of the production host, 2026-10-18 at about 09:10 UTC, unsigned. Story 1
  records each in `docs/specs/kalshi/OBSERVED.md` (§3.6):
  1. `GET /trade-api/v2/account/endpoint_costs` answers anonymously. `default_cost` is 10, and the
     listed paths are full templates (`/trade-api/v2/portfolio/orders/:order_id`,
     `/trade-api/v2/cfbenchmarks/*endpoint`). None of this epic's routes is listed, so each costs 10.
  2. `GET /trade-api/v2/exchange/status` answers anonymously, with `exchange_index_statuses` for
     indexes 0 to 3. The demo host answers it with 200 too.
  3. For an unknown ticker, `/markets/{t}` and `/events/{e}` answer
     `404 {"error":{"code":"not_found","message":"not found"}}`, but `/markets/{t}/orderbook`
     answers `200 {"orderbook_fp":{"no_dollars":[],"yes_dollars":[]}}`.
  4. Tickers are case-sensitive: a lowercase ticker is a 404 on `/markets/{t}` and an empty book on
     `/orderbook`. Tickers contain `-` and `.` (`KXBTCD-26OCT1806-T82099.99`), though the
     AsyncAPI's `marketTicker` pattern is `^[A-Z0-9-]+$` and its own examples carry dots
     (`FED-23DEC-T3.00`). The longest of 1,000 open tickers was 51 characters.
  5. The REST orderbook lists each side's levels in ascending price, so worst first. `depth=n`
     keeps the n best levels, and `depth` 0 or negative keeps every level.
  6. A parameter-validation 400 is `{"msg":"Parameter validation failed for …"}`. An unsigned
     `/account/limits` is `401 {"error":{"code":"token_authentication_failure","message":…}}`. The
     docs give a 429's body as `{"error":"too many requests"}`, with no `Retry-After` and no
     `X-RateLimit-*` headers.
  7. Of 1,000 open markets, every one has `notional_value_dollars` `1.0000` and three
     `price_ranges` (steps `0.0001`, `0.0010` and `0.0001`, `price_level_structure`
     `center_deci_edge_centi_cent`). Trades come newest first, and `count_fp` can be fractional
     (`46.91`).
  8. `/events/{e}?with_nested_markets=true` returned all 188 markets of `KXBTCD-26OCT1806`, as many
     as `/markets?event_ticker=` paged. The OpenAPI says markets settled before the historical
     cutoff are left out.
- Rate limits (`rate_limits`):
  - every authenticated request costs tokens, 10 by default;
  - there are separate Read and Write buckets;
  - Basic's Read budget refills 200 tokens/s and holds 3 s (600), and its Write budget refills
    100/s and holds 1 s;
  - no tier's Read capacity or refill is below Basic's;
  - `GET /portfolio/balance` has its own bucket (2026-10-15), and Write buckets are per shard;
  - a 429 carries no penalty or cooldown, and the client should back off;
  - no limit for unauthenticated requests is published.
- Keys (`api_keys`):
  - Ed25519 is recommended, and the web app has generated it by default since 2026-10-01 (PKCS#8,
    `PRIVATE KEY`).
  - RSA 2048 is PKCS#1 (`RSA PRIVATE KEY`) when Kalshi generates it and PKCS#8 when `openssl
    genpkey` does. The PEM label does not tell PKCS#8 RSA from Ed25519, so the type is read from
    the parsed key.
  - The pre-sign text is `timestamp_ms + METHOD + path`, the path without its query, written from
    the API root (`/trade-api/v2/…`) whatever the host.
  - RSA signs with RSA-PSS over SHA-256, MGF1 with SHA-256 and a salt of the digest's length.
  - The signature is base64, sent with `KALSHI-ACCESS-KEY`, `KALSHI-ACCESS-TIMESTAMP` and
    `KALSHI-ACCESS-SIGNATURE`.
- Hosts (`api_environments`):
  - REST: production `https://external-api.kalshi.com` (`api.elections.kalshi.com` also),
    demo `https://external-api.demo.kalshi.co` (`demo-api.kalshi.co` also);
  - sockets: `wss://external-api-ws.kalshi.com/trade-api/ws/v2` and
    `wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2`;
  - keys do not cross environments.
- The socket (AsyncAPI):
  - the handshake needs the API-key headers, even for public channels;
  - Kalshi sends a protocol Ping every 10 s with the body `heartbeat`, and answers client Pings;
  - commands carry an integer `id`, where 0 means none;
  - `orderbook_delta` needs `market_ticker` or `market_tickers`, sends an `orderbook_snapshot`
    first and `orderbook_delta`s after, and takes `update_subscription` with `add_markets`,
    `delete_markets` and `get_snapshot`;
  - a `subscribed` answer carries `msg.sid`; `ok`, `unsubscribed`, snapshots, deltas and
    subscription-scoped errors carry `sid` and `seq`, `seq` counting from 1;
  - `use_yes_price` (orderbook channel only) reports the NO side's levels in YES pricing; its
    default is to flip to `true`, and the flag is then to be removed;
  - error codes run 1 to 28, with 6, 16 and 17 retired. Among them: 9 (authentication required),
    25 (subscription buffer overflow), 26 (per-subscription market limit) and 27 (command rate
    limit).
- `ring` 0.17.14 is in `Cargo.lock`, through rustls. The `rsa` crate's RUSTSEC-2023-0071 (a timing
  side channel) has no patched version; 0.9.10 is its latest stable release. No YAML crate is in
  `Cargo.lock`.

**1.4 Departures from the acceptance criteria.**

- **The product is `predictions`, not `events`** (D1).
- **Both key types are signed** (D2), where 8.2 defers Ed25519.
- **`Trades` and `EventGroups` are implemented beside `MarketData`** (D10), where 8.3 asks for
  `MarketData` alone.
- **The public REST live suite runs against production, unsigned and with no secrets** (D18),
  where 8.2 puts exchange status on the demo host, auth-gated.
- **The error types are `PredictionsError` and `PredictionsWsError`**, CLAUDE.md's
  `<Module>Error`, not `KalshiError`.
- **Kalshi has no `RetryPolicy` of its own** (D6).
- **The kit's test server gains `handshake_headers()`** (D17). That is a foundation edit the owner
  approved, so 8.5's "no edit to `polyoxide-{venue,core,ws,test-support}`" is met as amended.
- **The touch-list check is a plan gate, not a CI job** (D20).
- **The trading mapping stays a planning companion** (D21), and deferred-work's entry for 8.1 to
  move it moves to Kalshi's `Trading`.
- **8.5's spine merge, guide regeneration and success-signal record through `bmad-spec` are an
  owner session after the run**, as for Epics 6 and 7.

## 2. Decisions (binding)

Each settles what the criteria leave open or records the owner's choice. A developer who finds the
plan disagreeing with one of them stops and asks.

- **D1. The product is `predictions`** (the owner, 2026-10-18). Kalshi has split its API into a
  Predictions exchange (`/trade-api/v2/…`) and a Margin exchange (`/trade-api/v2/margin/…`,
  perpetual futures), and `events` collides with Kalshi's own `/events` group and with every
  `Event` type. So:
  - keys are `kalshi.predictions:<TICKER>:yes` and `kalshi.predictions:<TICKER>:no`;
  - the module is `polyoxide_kalshi::predictions`, its features `predictions` and
    `predictions-ws`, and its suites `<kind>_predictions`;
  - `margin` stays free for Kalshi's perps.

  This amends AD-4's and AD-7's examples (A8-1).
- **D2. Both key types, through `ring`** (the owner, 2026-10-18). The private `SigningKey` is an
  Ed25519 key (`ring::signature::Ed25519KeyPair`) or an RSA key (`ring::signature::RsaKeyPair`,
  signing with `RSA_PSS_SHA256` and `ring::rand::SystemRandom`). The PEM is decoded with the
  workspace's `base64`:
  - the label `RSA PRIVATE KEY` is PKCS#1 RSA (`RsaKeyPair::from_der`);
  - the label `PRIVATE KEY` is PKCS#8, read as Ed25519
    (`Ed25519KeyPair::from_pkcs8_maybe_unchecked`, which takes the v1 form `openssl genpkey`
    writes) and otherwise as RSA (`RsaKeyPair::from_pkcs8`);
  - any other label, an encrypted key included, and a body that parses as neither, is a
    `CredentialsError` (`InvalidRequest`) that names the label or the reason and never quotes key
    material;
  - a literal `\n` in the text reads as a line break, so a key kept on one line in an env file
    parses.

  No `rsa` crate (RUSTSEC-2023-0071). `ring = "0.17"` joins `[workspace.dependencies]`.
- **D3. A client with credentials signs every request; one without signs none.**
  `Credentials::from_pem(key_id, pem)` and `Credentials::from_pem_file(key_id, path)` hold the key
  as `polyoxide_venue::Secret<SigningKey>`, whose `Debug` is redacted. Signing a public read charges
  it to the account's Read bucket, whose size the client confirms (D5); an unsigned read falls under
  Kalshi's unpublished anonymous limit. A route that needs a key (`account().limits()`) fails before
  sending when the client has none: `InvalidRequest`, nothing sent, no token spent. The one request
  a key never signs is `health().ping()`: core's `HttpClient::health` takes no venue authenticator
  (it signs with a private stopwatch alone), and `/exchange/status` is public.
- **D4. The base URL is a scheme and host; route paths are written from the API root.** The consts
  are `PRODUCTION` (`https://external-api.kalshi.com`) and `DEMO`
  (`https://external-api.demo.kalshi.co`). `build` refuses a base URL with a path other than `/`
  (`InvalidRequest`). Every route path begins `/trade-api/v2/`, so the signed path is the request's
  `RequestParts::path` exactly, and the route table's keys are the method and the template as
  `endpoint_costs` writes them (`GET /trade-api/v2/markets/:ticker/orderbook`).
- **D5. The throttle is one Read layer, sized provisionally and confirmed at first use.**
  - `PredictionsThrottle` (private) holds one core `CapacityBucket` on the layer
    `predictions.read`, made with `CapacityBucket::provisional` at Basic's Read sizing: capacity
    600, refill 200 tokens/s. Its one `Hold` is the throttle's. Every request of this epic is a read
    and charges its route's cost to that layer.
  - The route table (`Route::cost`) is compiled in, at the default 10 for every route of this epic.
  - **The confirmation.** The first send of a client, and of every clone of it, waits on one
    confirmation that runs once (a `tokio::sync::OnceCell` behind an `Arc`). Every route builder's
    send waits on it, `health().ping()` included:
    - `GET /trade-api/v2/account/endpoint_costs` replaces the compiled cost of every route it
      lists, matched on method and template;
    - with credentials, `GET /trade-api/v2/account/limits` then resizes the bucket in place to its
      `read.bucket_capacity` and `read.refill_rate` (`CapacityBucket::resize`, which also confirms
      it);
    - the two reads are sent as core `Request`s directly, not through the route builders, so they
      do not wait on themselves, and both are charged to the Read layer at their own cost;
    - the confirmation always ends with the bucket confirmed: resized from `/account/limits` when
      that read succeeds, and otherwise confirmed at the provisional sizing (`confirm`), so a cost
      above the capacity is refused rather than left waiting for a resize that never comes;
    - a confirmation read that fails, and a limits answer with a zero capacity or refill (core's
      bucket panics on a zero refill), log one WARN under `polyoxide_kalshi` and keep the compiled
      costs or the Basic sizing for the client's life. Basic is the floor of every tier, so such a
      client under-sends, never over-sends.
  - A client with no credentials keeps Basic's sizing: no anonymous limit is published.
    `OBSERVED.md` records that sizing as provisional, and a soak to measure it is deferred.
  - Costs are therefore computed when a request is sent, after the confirmation, not when its
    builder is made. The throttle cannot run the confirmation itself: it is a hook inside core's
    send loop, and a hook that sent a request would re-enter the loop.
  - The Write, balance and per-shard Write buckets come with Kalshi's `Trading`. `OBSERVED.md`
    records them now.
- **D6. The retry policy is core's `DefaultRetryPolicy`.** It retries a 429 and holds every request
  for `RetryConfig::retry_delay(0)`, also with no retry left, and fails every other status, so no
  5xx, 408 or 425 is retried (AD-17). Kalshi sends no `Retry-After` and promises no cooldown. The
  recipe's step 3 ("implement the venue's … `RetryPolicy`") reads as optional (A8-2).
- **D7. `PredictionsError` has one decode.** Its `From<ApiError>` turns an `ApiError::Response`
  whose body is one of Kalshi's three error shapes into `Venue(VenueError)`, and keeps every other
  error as `Api(ApiError)`:
  - `{"error":{"code":…,"message":…,"details":…}}` gives `code`, `message` and `details`;
  - `{"error":"<text>"}` (a 429) gives `message` alone;
  - `{"msg":"<text>"}` (a parameter-validation 400) gives `message` alone.

  `VenueError` keeps the status, the headers' `Retry-After` as core's `ErrorResponse` surfaces it,
  `code`, `message` and `details`. Classification goes by status before body (AD-15), through
  `polyoxide-venue`'s status map, as core's `ApiError::Response` is classed. A key that does not
  parse is a `CredentialsError`, its own public type. `PredictionsError` and `VenueError` are
  `#[non_exhaustive]`.
- **D8. Keys.** `PredictionsKeys: KeySpace`, with `VENUE` `kalshi`, `PRODUCT` `predictions` and an
  id of a ticker and an outcome:
  - `parse_id` splits the id at its last `:`. The part before is the ticker, uppercased (ASCII),
    then checked against `[A-Z0-9.-]`, 1 to 200 characters (200 is the OpenAPI's `maxLength` for
    `GET /markets/orderbooks`' tickers), at least one of them a letter or digit, so `.` and `..`,
    which a URL resolves as dot segments, are refused (the owner, 2026-10-18). The part after is
    the outcome, lowercased, `yes` or `no`.
  - `id_text` writes `<TICKER>:<outcome>`.
  - Uppercasing is safe because every ticker the venue serves is uppercase and tickers are
    case-sensitive (§1.3, probe 4): a lowercase key would otherwise name nothing at the venue.

  Story 1 writes the key API (§3.4) and the test `the_ids_are_the_ones_the_crate_registers`.
- **D9. One YES/NO conversion, on every path.** A NO bid at price *q* and size *s* is a YES ask at
  *1 − q* and size *s*, and a YES bid at *p* is a NO ask at *1 − p*. A key's book is its outcome's
  bids, best (highest) first, and the other outcome's bids mapped to asks, best (lowest) first. A
  live test asserts that every converted book is not crossed (its best bid below its best ask),
  which also catches Kalshi switching REST to YES pricing for the NO side, as the socket's
  `use_yes_price` migration (D13) does there.
- **D10. `MarketData`, `Trades` and `EventGroups`** (the owner, 2026-10-18), in
  `src/predictions/venue.rs` (§5). `Candles` is not implemented, because `CandleInterval` promises
  every width and Kalshi serves 1, 60 and 1440 minutes (A8-3). `Instruments` is not implemented,
  since the open listing runs to many pages of 1,000, as the CLOB's does (A6-1). `Funding` does not
  apply.
- **D11. An unknown ticker.** `instrument` turns `/markets/{t}`'s 404 into `NotListed`. `quote` and
  `book` cannot tell (§1.3, probe 3), so they return an empty quote and an empty book for it, as
  perps' do, and `instrument` is how a caller confirms a listing.
- **D12. Instrument fields.**
  - `size_unit` is `SizeUnit::Shares`: each contract pays one dollar on its outcome (probe 7), as
    the CLOB's shares do.
  - `tick_size` is the step when a market has exactly one price range, and `None` otherwise.
  - `min_size` and `size_increment` are `0.01`, the `FixedPointCount` granularity the OpenAPI gives
    for orders. `OBSERVED.md` records them as provisional until Kalshi's `Trading` probes them.
  - `quote_asset` is `USD`, `base_asset` is `None`, and `tradable` means status `active`.
- **D13. The socket carries `orderbook_delta` only, and asks for YES pricing.**
  - The membership is a `BTreeSet` of tickers.
  - The replay sends one `subscribe` carrying every ticker and `use_yes_price: true`. Kalshi will
    flip the default and then drop the flag, so sending it explicitly means the flip changes
    nothing here.
  - A change goes on the live subscription's `sid` as `update_subscription` with `add_markets` or
    `delete_markets`. The first add with no subscription sends a `subscribe`, and a delete that
    empties the membership sends an `unsubscribe`.
  - A change is recorded before its frame is sent. One whose connection dies under it is answered
    `Ok` and replayed; one made during an outage is answered at once (`Offline::Recorded`) and
    replayed.
  - An empty membership is still wanted: the connection stays open and keeps pinging.
  - The `ticker` and `trade` channels are deferred.
- **D14. A gap in a subscription's `seq` is a lost message.** The protocol keeps the last `seq` of
  each `sid` from every frame that carries both (snapshots, deltas, `ok`, `unsubscribed` and
  subscription-scoped errors). The first `seq` of a `sid` is taken as it comes; after it, each must
  be the previous plus one. Any other value is
  `PredictionsWsError::SequenceGap { sid, expected, received }`, which recovers with
  `Recovery::Reconnect`, so the stream yields its `Disconnected`, reconnects, replays, yields
  `Reconnected`, and gets fresh snapshots. A local book and `get_snapshot` are deferred.
- **D15. Socket timing.** The client sends a protocol Ping every 10 s on the wall clock (D4 of
  `divergences.md`) and counts every inbound frame as liveness, Kalshi's `heartbeat` Pings included
  (D5). It is stale after 30 s, backs off 500 ms to 60 s, gives each attempt a 10 s connect timeout,
  and never rotates. Its queue is `Queue::Bounded(1024)`. Each outage yields one `Disconnected` and
  one `Reconnected` (AD-11).
- **D16. Socket recovery.**
  - A frame that does not decode is `PredictionsWsError::Json`: the stream yields it and keeps the
    connection (`Recovery::Skip`).
  - An error frame that answers a command fails that command, and the connection stays.
  - A subscription-scoped error frame is `PredictionsWsError::Server { code, message }`:
    - code 9 (authentication required) stops the stream;
    - code 10 (channel error) and code 25 (subscription buffer overflow) are `Unavailable` and
      reconnect, since Kalshi's AsyncAPI calls both terminal ("the user must resubscribe"; the
      owner, 2026-10-18), and the replay resubscribes;
    - code 27 (command rate limit) is `RateLimited` and reconnects;
    - every other code is `InvalidRequest` and is skipped.
  - `SequenceGap` reconnects (D14).
  - A connection used after it closed reconnects (D19 of `divergences.md`).
  - Any other error reconnects exactly when `Classify::is_retriable` says so and stops otherwise,
    so a refused handshake's 401 or 403 stops, and a 429 or 5xx retries after at least its
    `Retry-After` (DRIFT R11).
- **D17. The handshake is signed on every attempt, and the kit records it** (the owner,
  2026-10-18). `PredictionsProtocol::handshake` signs `timestamp_ms + "GET" + <the URL's path>`
  (`/trade-api/ws/v2` on Kalshi's hosts) afresh for each attempt, with the same signer as REST.
  `polyoxide-ws`'s scripted server gains `handshake_headers()`, which returns each accepted
  handshake's request headers in accept order (§7). It names no venue. It is a foundation edit
  outside AD-13's set, approved by the owner and recorded as A8-4.
- **D18. Live hosts split by authentication** (the owner, 2026-10-18).
  - `live_predictions` runs against production, unsigned and with no secrets, so every nightly run
    reads real markets.
  - `live_predictions_account` and `live_predictions_ws` run against the demo host with
    `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` (the PEM text), and are auth-gated
    without them.
  - Production credentials never reach CI. The owner adds the two repository secrets.
- **D19. Fixtures are captured, and the spec is read through a dev-only YAML parser.**
  - REST fixtures come from production, unsigned; `/account/limits` comes from the demo host,
    signed; socket frames come from the demo host.
  - The signing vectors come from throwaway keys the capture script generates, never the owner's.
  - Kalshi publishes no JSON spec, and no YAML crate is in the tree, so the spec-agreement suite
    reads `docs/specs/kalshi/openapi.yaml` through a YAML parser added as a dev-dependency only
    (§4.7). It never reaches a normal build, so `ci/dep-allowlists/` is unchanged by it.
- **D20. The touch set, and its gate.** The epic touches only AD-13's new-venue set and these
  pre-approved edits, each recorded as an amendment:
  - `.github/workflows/ci.yml`: Kalshi's line in the features job's two header-pin steps (A8-5);
  - `.github/scripts/tests/test_ci_workflow.py`: the "`full` turns each module on" assertion applies
    only to the venue crates the umbrella depends on, since a published umbrella cannot depend on a
    `publish = false` crate; Kalshi still runs its pin under `full` (A8-5);
  - `.github/scripts/tests/test_dep_allowlists.py`: a `PER_FEATURE` rule for `polyoxide-kalshi`,
    and a `MUST_BUILD` row for its socket file (D23, A8-6);
  - `CLAUDE.md`: the hand-written paragraphs that list the venue crates, the header pins or the
    allowlist rules, and the scripted server's list of recorders (AD-21, A8-4, A8-6);
  - `polyoxide-ws/src/test_server/`: `handshake_headers()` and its test (D17, A8-4).

  The planning records the run writes are in the set too: this spec, the plan,
  `_bmad-output/specs/**`, `spine-amendments/epic-8.md` and
  `_bmad-output/implementation-artifacts/deferred-work.md`. Every story's gates end with the
  touch-set gate (§10). An edit outside the set and this list stops the story and goes to the owner.
- **D21. The trading mapping stays a planning companion.** It stays at
  `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, since moving it would edit
  `docs/s3-migration.md`'s citation, outside the touch set. Story 7 renames its keys to
  `kalshi.predictions:…` and its size unit to `SizeUnit::Shares` (D12), and re-files deferred-work's
  move for Kalshi's `Trading`.
- **D22. No facade.** The umbrella, the CLI and the Python bindings do not change (non-goals).
  Story 6.6's generic `best_bid_ask` runs against Kalshi from Kalshi's own suite, which includes the
  umbrella's `examples/venue_neutral.rs` by `#[path]` (§5.6). That file imports Polymarket's and
  Binance's types and has a `main` that uses them, so Kalshi's path-only dev-dependency on
  `polyoxide` (`default-features = false`) turns on the example's `required-features`,
  `polymarket-clob`, `polymarket-perps` and `binance-usdm` (the owner, 2026-10-18), and its tokio
  dev-dependency has `macros` and `rt-multi-thread`. That builds the Polymarket and Binance stacks
  into Kalshi's test builds only: a dev-dependency is in no `cargo tree -e normal`, so
  `ci/dep-allowlists/` and Kalshi's normal build are unchanged. The umbrella's files are not edited.
- **D23. Kalshi's dependency allowlists have a rule of their own** (the owner, 2026-10-18).
  `test_dep_allowlists.py` checks a crate with no `PER_FEATURE` entry with its default features
  against the socket-only deny-list, which refuses `reqwest` and `polyoxide-core`, so a Kalshi file
  would fail. `PER_FEATURE` gains `polyoxide-kalshi`:
  - `alloy*`, `hmac` and `keyring` are never admitted;
  - `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with `predictions-ws`;
  - `reqwest`, `polyoxide-core`, `governor` and `sha2` with `predictions`, which every file names.

  The files are `ci/dep-allowlists/polyoxide-kalshi@predictions.txt` (Story 1) and
  `polyoxide-kalshi@predictions-ws.txt` (Story 6), whose `MUST_BUILD` row requires `polyoxide-ws`,
  so the socket rule cannot pass vacuously. The rule's exact shape follows the file's existing
  `Rule` entries for Binance. Recorded as A8-6.
- **D24. The flow profile runs Kalshi's header pins** (the owner, 2026-10-18; done in this design
  session, flow repo commit `de066a4`). The final `features` gate of
  `~/.claude/flow/repos/-tb-Source-DilettanteTrading-polyoxide.toml` runs
  `cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`
  after Binance's minimal pin, and
  `cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions`
  after Binance's full pin. Each is guarded by `[ ! -d polyoxide-kalshi ] ||`, since every worktree
  shares the profile and those without the crate must still pass.
- **D25. Times.** Kalshi's REST times are RFC 3339 (`created_time`) and its socket times epoch
  milliseconds (`ts_ms`, `sending_ts_ms`). `polyoxide_venue::UnixMillis` has no RFC 3339 constructor,
  so the traits parse them with the workspace's `chrono` pin, behind `predictions`. A time that does
  not parse is a `Decode` error naming the field, never a silent zero.

## 3. Story 1: register the crate

**3.1 Workspace.** In the root `Cargo.toml`:

- `members` gains `"polyoxide-kalshi"`, after `"polyoxide-core"`;
- `[workspace.dependencies]` gains `polyoxide-kalshi = { path = "polyoxide-kalshi" }`, with no
  version since it is never published, and `ring = "0.17"`;
- `[workspace.metadata.polyoxide.mirrors.kalshi]` (§3.6).

**3.2 The manifest**, `polyoxide-kalshi/Cargo.toml`:

- `[package]`: the workspace's `version`, `edition`, `license`, `authors`, `repository` and
  `rust-version`; a `description`; `publish = false`.
- `[features]`: `default = ["predictions"]`. `predictions` turns on `dep:polyoxide-core`,
  `polyoxide-venue/decimal`, `polyoxide-venue/market` and, `dep:`-prefixed as Binance's `usdm`
  does, the optional dependencies the module uses: `base64`, `chrono`, `ring`, `rust_decimal`,
  `serde`, `serde_json`, `thiserror`, `tokio` (`time`, `sync`), `tracing` and `url`. Story 6 adds
  `predictions-ws`. A story adds each dependency when its code first uses it.
- `[dependencies]`: `polyoxide-venue` (workspace, not optional), `polyoxide-core` (workspace,
  optional), and the optional crates above from their workspace pins.
- `[dev-dependencies]`: a path-only `polyoxide-test-support` with `features = ["query"]`,
  `mockito` (workspace), and `tokio` with `macros`, `rt-multi-thread`, `time` and `test-util`.
  Story 2 adds the YAML parser (§4.7), Story 3 the umbrella (D22) and Story 6 the kit's test server.
- One `[[test]]` stanza per target, with its `required-features`.
- `[package.metadata.docs.rs] features = ["predictions"]`, with `"predictions-ws"` added by Story 6.
  `test_docs_rs_builds_every_venue_feature_but_test_server` requires it to equal every feature but
  `default` and `test-server`, for a `publish = false` crate too.
- `[package.metadata.polyoxide]`:
  - `readme`, which `gen_registry.py` requires: "Client library for Kalshi's APIs, one feature per
    module: `predictions` (event-contract market data, with its order-book stream under
    `predictions-ws`); a walking skeleton, not published";
  - `venue = "kalshi"`, `products = ["predictions"]` and `mirrors = ["kalshi"]`;
  - no `identifiers`: every Kalshi host and header contains the venue id, which the gate already
    looks for;
  - Stories 2, 4 and 6 add the `live.<target>` entries.

**3.3 Layout.**

- `src/lib.rs` holds:
  - the crate docs and `#[cfg(feature = "predictions")] pub mod predictions;`;
  - the crate README's doctest struct, as Binance's `lib.rs:12-14`;
  - the module README's, in the form `test_module_readmes.py` requires:
    `#[cfg(all(doctest, feature = "predictions"))]`,
    `#[doc = include_str!("predictions/README.md")]`, `struct PredictionsReadmeDoctests;`;
  - the `Classify` assertion (§3.8).
- `src/predictions/` holds `mod.rs`, `README.md`, `api/{mod,health,exchange}.rs`, `error.rs`,
  `key.rs`, `request.rs`, `throttle.rs` and `types/{mod,exchange}.rs`. Later stories add
  `api/{markets,events,account}.rs` and `types/{markets,events,account}.rs` (Story 2), `venue.rs`
  (Story 3), `auth.rs`, `api/limits.rs` and `types/limits.rs` (Story 4), and `ws/` (Story 6).
- The crate `README.md` and `src/predictions/README.md` are prose with no code fence, since both
  are doctests. They say the crate is a walking skeleton and not published.

**3.4 Keys** (`key.rs`, D8):

- `pub const VENUE: VenueId = VenueId::from_static("kalshi")` and
  `pub const PRODUCT: ProductId = ProductId::from_static("predictions")`.
- `Ticker`, a newtype over `String`. `Ticker::new(&str) -> Result<Ticker, InvalidTicker>`
  uppercases ASCII and checks `[A-Z0-9.-]`, 1 to 200 characters, at least one of them a letter or
  digit (D8); `as_str` reads it. Response rows
  keep tickers as `String`, as Binance's rows keep symbols: `Ticker` is what a caller sends.
- `Outcome { Yes, No }`, `Copy` and `Eq` and `Hash`, with `as_str` (`yes`, `no`) and `other()`.
- `OutcomeId { pub ticker: Ticker, pub outcome: Outcome }`.
- `PredictionsKeys: KeySpace`, with `Id = OutcomeId`.
- Free functions as Binance's (`usdm/key.rs:50-71`): `key(&Ticker, Outcome) -> MarketKey`,
  `parse(&RawKey) -> Result<MarketKey, KeyError>` and `outcome(&MarketKey) -> Result<OutcomeId, KeyError>`.
- `InvalidTicker`: `thiserror`, `Classify` as `InvalidRequest`, and in the assertion.
- Inline tests:
  - `the_ids_are_the_ones_the_crate_registers`, through `polyoxide_test_support::registration!()`,
    as Binance's `key.rs:81-92`;
  - a lowercase ticker and outcome canonicalize, and a dotted ticker parses;
  - a ticker with another character, an empty one, one of 201 characters, and `.` and `..` are
    refused;
  - `kalshi.predictions:KXBTCD-26OCT1806-T82099.99:yes` round-trips through `RawKey::parse`,
    `parse` and `Display`;
  - a key of another venue or product is a `KeyError`.

**3.5 The minimal client** (`mod.rs`, `request.rs`, `throttle.rs`, `api/`):

- `PRODUCTION` and `DEMO` (D4).
- `PredictionsBuilder` holds a `ClientConfig::new(PRODUCTION, 4)` and invokes
  `polyoxide_core::client_config_setters!(config);`. Its `build`:
  - refuses a base URL with a path other than `/` (D4), as
    `PredictionsError::Api(ApiError::Validation(..))`;
  - builds the throttle, then the client with
    `config.http_builder().with_throttle(throttle.clone()).build()?`. That leaves core's
    `DefaultRetryPolicy` installed (D6).
- `Predictions`, `Clone`, holds one `Arc` of its shared state: the HTTP client, the throttle, the
  route costs, the confirmation cell, and the authenticator (`None` until Story 4's `credentials`
  sets it). It has `Predictions::new()`, `Predictions::builder()` and its accessors from
  `polyoxide_core::namespaces!`.
- **The route table is whole from Story 1**, so no later story edits it. Each route is a method and
  a template in `endpoint_costs`' spelling (`:param`), and costs the default 10. The table holds
  `exchange/status` and §4.1's routes, among them `account/endpoint_costs` and
  `account/limits`.
  - A doc-hidden public accessor lists every route's method and template, and two doc-hidden
    items give the default cost and the segment-by-segment match (§4.1).
  - The drift test (§4.6), an integration suite, reads the table through them.
- `Routed<T>` (private) is the route builder, after Binance's `usdm/request.rs:21-71`. It holds the
  shared state, its route and its query, and its inherent `query` replaces a key. Its `send`:
  1. awaits the state's `before_send`, which Story 1 writes as a method that returns at once
     and Story 4 replaces with the confirmation (D5);
  2. builds `Request::<T, PredictionsError>::new(http, path)` with
     `Cost { layer: READ, units, exact: true }`, `units` read at this point (D5);
  3. adds the authenticator when the state holds one (Story 4) and the query pairs;
  4. sends.
- `PredictionsThrottle` (private) implements `Throttle` after
  `polyoxide-test-support/tests/token_cost_throttle.rs`:
  - `acquire` waits out the hold, charges each cost on its own layer (a cost on another layer is
    ignored), and waits out the hold again (AD-23);
  - `observe` does nothing, and `hold` extends the hold;
  - the hold is `Hold::unbounded()`: every hold Kalshi's policy sets is a `retry_delay(0)`, which
    `RetryConfig::max_backoff_ms` bounds;
  - Story 1 makes the provisional bucket, and Story 4 adds the confirmation.
- `health().ping()` awaits `before_send`, then calls
  `HttpClient::health::<PredictionsError>("/trade-api/v2/exchange/status", &[cost])`.
- `exchange().status()` is `GetExchangeStatus`, which returns `ExchangeStatus { exchange_active,
  trading_active, intra_exchange_transfers_active, exchange_estimated_resume_time,
  exchange_index_statuses }`, with `ExchangeIndexStatus { description, exchange_active,
  exchange_index, intra_exchange_transfers_active, trading_active }`. Fields the schema makes
  optional are `Option`, and the list defaults to empty.
- `types` is a directory module, `types/mod.rs` with one file per group (Story 1's `exchange.rs`,
  then Story 2's `markets.rs`, `events.rs` and `account.rs`, and Story 4's `limits.rs`), so each
  story adds files rather than editing another's.

**3.6 The mirror.**

- `docs/specs/kalshi/openapi.yaml` and `docs/specs/kalshi/asyncapi.yaml` are byte for byte as
  served (`curl -sS -o`), and are never edited.
- `docs/specs/kalshi/OBSERVED.md` records:
  - §1.3's probes 1 to 8;
  - the AsyncAPI's ticker pattern and Kalshi's three error shapes;
  - the rate limits and what is provisional: Basic sizing, anonymous limits, `min_size` and
    `size_increment`;
  - the per-shard Write and balance buckets, recorded but not built, and that a 429 carries no
    `Retry-After`.

  The file ends with the line `<!-- capture findings go above this line -->`, and each later
  story inserts its findings as a section directly above it, so stories that run in any order
  never edit each other's text.
- The root manifest's `[workspace.metadata.polyoxide.mirrors.kalshi]`:
  - `name = "Kalshi"`, `section = "other-venue"`;
  - `base_urls`: the production and demo REST hosts and socket URLs, with a `description`;
  - two `specs`: `{ id = "kalshi", kind = "openapi", url = "https://docs.kalshi.com/openapi.yaml",
    vendored = "docs/specs/kalshi/openapi.yaml" }` and `{ id = "kalshi-ws", kind = "asyncapi",
    url = "https://docs.kalshi.com/asyncapi.yaml", vendored = "docs/specs/kalshi/asyncapi.yaml",
    covers = "Trade API WebSocket (13 channels); `orderbook_delta` is implemented" }`, since an
    asyncapi spec requires `covers`;
  - no `exclude`, so nightly-schema watches both. Its diff reads YAML for both kinds
    (`.github/scripts/diff_openapi.py:175-188`) and creates its own labels.

  Kalshi announced schema corrections dated 2026-10-22, so a drift issue will follow; adopting it
  is that issue's `curl`, outside this epic.
- `python3 scripts/gen_registry.py --write` writes the README's crate table,
  `docs/specs/INDEX.md`, CLAUDE.md's crate list and count, SELF-HEALING.md and the nightly regions.

**3.7 The CI wiring** (D20, D23):

- `.github/workflows/ci.yml`, the features job:
  - "Header pins, minimal builds" gains
    `cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`;
  - "Header pins, umbrella full build" gains
    `cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions`.
- `.github/scripts/tests/test_ci_workflow.py`:
  - in `test_the_features_job_runs_each_header_pin_under_the_umbrellas_full`, the loop that asserts
    `f"{crate}/{product}" in enabled` runs only over modules whose crate the umbrella depends on,
    read from the umbrella package's `dependencies` in the same metadata;
  - the assertion on the list of commands is unchanged, so Kalshi's full pin is still required;
  - a new test pins the rule both ways with stand-in metadata, as
    `test_http_modules_are_venue_crates_on_core` does: a venue crate outside the umbrella needs no
    umbrella feature, and one inside it still does.
- `.github/scripts/tests/test_dep_allowlists.py` gains D23's rule.
  `ci/dep-allowlists/polyoxide-kalshi@predictions.txt` is written by
  `uv run python -m tests.test_dep_allowlists --write`, run in `.github/scripts`.
- `CLAUDE.md`: these hand-written paragraphs name `polyoxide-kalshi`, its `predictions` module and
  that the crate is `publish = false`. Story 1 edits:
  - the overview;
  - the CI paragraph, for its header pins and allowlist rules;
  - "Each of those six HTTP modules has a `tests/headers_<module>.rs` …";
  - the mock-suites paragraph;
  - Module Organization's first paragraph.

  Story 5 edits the scripted-server paragraph. Story 6 edits the live-suites paragraph, naming all
  four Kalshi live targets at once, and Module Organization's WebSocket paragraph. No two stories
  edit one paragraph, with one exception: in the CI paragraph, Story 6 replaces the text that ends
  Story 1's Kalshi allowlist clause, which the plan's names contract gives, to name the
  `@predictions-ws` file and its `MUST_BUILD` row.

  The paragraphs that describe the umbrella and the CLI stay as they are, since Kalshi is in
  neither. `README.md`'s hand-written intro also stays: it describes what is published.

**3.8 Errors** (`error.rs`, D7; Story 1 writes the whole decode):

- `#[non_exhaustive] pub enum PredictionsError { Api(ApiError), Venue(VenueError) }`.
- `#[non_exhaustive] pub struct VenueError` holds `status`, `code: Option<String>`,
  `message: String`, `details: Option<String>`, and the `Retry-After` that core's `ErrorResponse`
  read.
- `From<ApiError>` decodes `ApiError::Response`'s `body`, never its `message`, since core's
  `message` keeps only a string `error` field. Every other variant stays `Api`. A signing failure
  stays `Api(ApiError::Sign(..))`, which is `InvalidRequest`.
- `Classify`:
  - `Api` is classed as core classes it;
  - `Venue` is `class_for_status(status)`, falling back to `Decode` outside 400 to 599, with
    Kalshi's `code` as the class's code where the class carries one;
  - `retry_after` is the response's.
- The lib.rs assertion lists `PredictionsError`, `VenueError` and `InvalidTicker`. Story 4 adds
  `CredentialsError`, and Story 6 `PredictionsWsError` under `predictions-ws`.

**3.9 Story 1's tests.**

- `tests/headers_predictions.rs`: one unauthenticated `exchange().status()` GET, through
  `polyoxide_test_support::query::headers_sent`, sends exactly `accept: */*`,
  `accept-encoding: gzip` and `host`.
- `tests/mock_predictions.rs`:
  - `exchange().status()` and `health().ping()` read the probe's body;
  - `build` refuses a base URL with a path;
  - each error shape decodes into its `VenueError` and class: 401 `Unauthorized`; 404
    `VenueRefusal` with code `not_found`; the validation 400 `VenueRefusal`; 429 `RateLimited`;
  - a 503 whose body is an `ExchangeStatus` stays `Api`, and is `Unavailable`.
- `tests/error_classes_predictions.rs`: each error variant's class, fault and `retry_after`.
- The key tests (§3.4).
- `[[test]]` stanzas: `headers_predictions`, `mock_predictions` and `error_classes_predictions`,
  each `required-features = ["predictions"]`.

## 4. Story 2: the public REST surface

**4.1 Routes.**

| Builder | Route | Template |
|---|---|---|
| `markets().list()` | `GET /trade-api/v2/markets` | the path |
| `markets().get(&Ticker)` | `GET /trade-api/v2/markets/{ticker}` | `/trade-api/v2/markets/:ticker` |
| `markets().orderbook(&Ticker)` | `GET /trade-api/v2/markets/{ticker}/orderbook` | `/trade-api/v2/markets/:ticker/orderbook` |
| `markets().trades()` | `GET /trade-api/v2/markets/trades` | the path |
| `events().get(&str)` | `GET /trade-api/v2/events/{event_ticker}` | `/trade-api/v2/events/:event_ticker` |
| `account().endpoint_costs()` | `GET /trade-api/v2/account/endpoint_costs` | the path |

A route matches an `endpoint_costs` entry by method and template, segment by segment, where a
`:name` segment matches any `:name` segment. So a parameter Kalshi spells differently still finds
its cost.

Query setters come from `polyoxide_core::query_setters!`:

- `list`: `status` (a `wire_enum!` of the filter's values: `unopened`, `open`, `paused`, `closed`
  and `settled`, the parameter's enum in the OpenAPI, whose prose leaves out `paused`),
  `event_ticker`, `series_ticker`, `mve_filter` (a `wire_enum!` `MveFilter` of `only` and
  `exclude`; the owner, 2026-10-18), `limit` and `cursor`;
- `orderbook`: `depth`;
- `trades`: `ticker`, `limit`, `cursor`, `min_ts` and `max_ts`;
- `events().get`: `with_nested_markets`.

**4.2 Types** (`types.rs`) model every key the wire carries, from the OpenAPI and the captured
fixtures:

- **Markets.** `Market`:
  - its money (`*_dollars`) and counts (`*_fp`) are `Decimal` through `rust_decimal::serde::str`;
  - its times are `String`;
  - `price_ranges` is `Vec<PriceRange { start, end, step }>`;
  - deprecated fields the wire still carries (`title`) are modelled, and fields only the wire
    carries are excused through an `OBSERVED_EXTRA` list, recorded in `OBSERVED.md` as perps'
    are.

  `MarketResponse { market }` and `MarketsPage { markets, cursor }` wrap it.
- **The orderbook.** `OrderbookResponse { orderbook_fp: OrderbookFp { yes_dollars, no_dollars } }`.
  Each level is the positional `[price, count]` pair, a tuple over
  `polyoxide_venue::positional::DecimalStr`, as perps' book levels are.
- **Trades.** `Trade` and `TradesPage { trades, cursor }`.
- **Events.** `Event`, and `EventResponse { event, markets }`. `markets` is the deprecated
  top-level list, which is empty when the markets are nested.
- **Endpoint costs.** `EndpointCosts { default_cost, endpoint_costs: Vec<EndpointCost { method, path, cost }> }`.
- **Enums.** `MarketStatus` is an `open_enum!`, since Kalshi extends it (today `initialized`,
  `inactive`, `active`, `closed`, `determined`, `disputed`, `amended` and `finalized`), and so are
  the trade's `taker_outcome_side` (`yes`, `no`) and `taker_book_side` (`bid`, `ask`).

**4.3 Costs.** Story 2 sends every route at the compiled cost. Reading `/account/endpoint_costs`
at first use belongs to Story 4's confirmation (§6.3), which replaces Story 1's `before_send` as a
whole, so no story edits another's code.

**4.4 The capture**, `scripts/capture_kalshi_predictions.py`: a PEP 723 script run with `uv run`, as
a file and never with `python3 -I`, on `scripts/capture_common.py`.

- **Production, unsigned.** It picks an `active` market with a two-sided book. It pages
  `markets?status=open&mve_filter=exclude&limit=1000`, at most 10 pages, to a row with a YES bid
  above 0 and a YES ask below 1, then confirms both sides on that market's orderbook. With none, it
  fails. The filter is needed: on 2026-10-18 the unfiltered open listing's first ten pages held
  9,996 multivariate combinations of 10,000 rows and no quoted single market, while the filtered
  listing's first page held 579 quoted rows (the owner, 2026-10-18). Story 2 records this in
  `OBSERVED.md`, in a section of its own above the anchor. It then writes
  `tests/fixtures/predictions/`:
  - `exchange_status.json` and `endpoint_costs.json`;
  - `market.json`, `markets_page.json` (5 rows), `orderbook.json` and `orderbook_depth1.json`;
  - `trades.json` (100 rows) and `event.json` (nested);
  - `error_not_found.json`, `error_validation.json` and `error_unauthenticated.json`.
- `error_rate_limited.json` is written from Kalshi's docs, and `PROVENANCE.md` (through
  `capture_common`'s `stamp`, `provenance_table` and `write_provenance`) says so.
- Story 4's own script, `scripts/capture_kalshi_account.py`, captures the signing vectors and the
  demo `/account/limits` (§6.4, §6.7).

**4.5 Story 2's suites.**

- `mock_predictions_routes` (a new file, so Story 1's `mock_predictions` is not edited):
  - every route's path and query;
  - each type decoding its fixture;
  - a 429 retried, then holding every request under core's policy.
- `query_setters_predictions`: every setter's exact pairs, through `query::{Case, assert_cases}`.
- `wire_agreement_predictions`: each fixture against its type, both ways (`agreement::compare`),
  with the excuses in a `Ledger`.
- `spec_agreement_predictions`: each type's field names and optionality against the mirror's
  schemas (`openapi::check`), and each builder's query keys against its route's documented
  parameters (`query::documented_parameters`). The YAML is read into `serde_json::Value` by the
  dev-only parser (§4.7).
- Each has a `[[test]]` stanza with `required-features = ["predictions"]`.

**4.6 `live_predictions`** runs against production, unsigned. Its entry,
`[package.metadata.polyoxide.live.live_predictions]`, has suite `predictions`, timeout 15, features
`["predictions"]` and `secrets = []`, and its `[[test]]` has `required-features = ["predictions"]`.
Every test is `#[ignore]`d.

- `exchange().status()` and `health().ping()` succeed.
- A market is chosen by precondition: open, with a two-sided book. Candidates come from at most 5
  pages of `markets().list()` with `status(Open)` and `mve_filter(Exclude)` (§4.4 says why), and the
  book is confirmed on the orderbook route its assertions read, since a CDN caches per URL. With
  none it is an `environmental(..)` skip.
- Each REST type of the chosen market carries no unmodelled keys. This is the drift detector,
  through the agreement helpers.
- The route table, read through Story 1's doc-hidden accessors, agrees with
  `/account/endpoint_costs`: `default_cost` is the crate's default, and every route the table
  holds is either unlisted or listed at the cost the crate charges, matched segment by segment
  (D5's drift test).
- Story 3's trait checks are a target of their own (§5.7), so this file is not edited.

**4.7 The YAML dev-dependency.** One maintained crate that reads YAML into `serde_json::Value`,
pinned in `[workspace.dependencies]` and used only as a dev-dependency. `serde-saphyr` is the first
choice. If it does not build on the workspace's toolchain, or its licence is not MIT- or
Apache-compatible, `yaml-rust2` with a small conversion to `Value` replaces it.

## 5. Story 3: the traits (`venue.rs`, D9 to D12)

**5.1 The conversion.** One function gives an outcome's book from an `OrderbookFp`, by D9:

- *1 − q* is exact `Decimal` arithmetic;
- bids are sorted highest first, and asks lowest first;
- a missing or empty side is an empty list.

**5.2 `MarketData` on `Predictions`.**

- `instrument`:
  - reads `markets().get(&ticker)`, and its 404 is `NotListed::new(key.clone())` (D11);
  - otherwise it is `Instrument::new(key, name, SizeUnit::Shares)` with D12's fields;
  - `name` is `yes_sub_title` or `no_sub_title`, or `"<TICKER> yes"` or `"<TICKER> no"` when empty;
  - its `InstrumentExt` holds `event_ticker`, `status`, `price_ranges`, `price_level_structure`,
    `close_time` and the expiry times, `notional_value`, `settlement_bounds_type` and
    `exchange_index`.
- `quote` reads `orderbook(&ticker).depth(1)`, and its sides are the converted best levels. `time`
  is `None`.
- `book` reads `orderbook(&ticker)` with every level.

**5.3 `Trades`.** It reads `markets().trades().ticker(&ticker).limit(1000)` and reverses it to
oldest first. Each row becomes a `Trade::new(key, price, count_fp, time)`:

- `price` is `yes_price_dollars` or `no_price_dollars`, by the key's outcome;
- `time` is `created_time` (D25);
- `id` is `trade_id`;
- `taker_side` is `Buy` when `taker_outcome_side` is the key's outcome, else `Sell`;
- `TradeExt` holds `is_block_trade`.

**5.4 `EventGroups`.**

1. `markets().get(&ticker)` gives the event ticker, and an unlisted key is `NotListed`.
2. `events().get(&event).with_nested_markets(true)` gives `EventGroup::new(event)` and its `title`.
3. Each nested market, in the order Kalshi sends them, gives two `EventOutcome`s: `:yes` labelled
   `Yes`, then `:no` labelled `No`. Each has `market` = the ticker and `market_title` = its
   `yes_sub_title`.
4. `EventGroupExt` holds `series_ticker`, `sub_title`, `mutually_exclusive` and
   `collateral_return_type`.

**5.5 Extension types.** `InstrumentExt`, `TradeExt` and `EventGroupExt`, at `predictions::venue`,
each `#[derive(Debug, Clone, PartialEq, Default)]`, `#[non_exhaustive]`, every field an `Option`.

**5.6 `venue_predictions`**, on a mock server that serves Story 2's fixtures:

- each method equals the native client's values;
- a `:no` key's book mirrors its `:yes` key's, and every converted book is not crossed;
- for an unknown ticker, `instrument` is `NotListed`, and `quote` and `book` are empty;
- a key of another venue or product is `InvalidRequest`, with nothing sent;
- `best_bid_ask` runs on `Arc<DynMarketData<'static>>` through the umbrella's example, included by
  `#[allow(dead_code)] #[path = "../../polyoxide/examples/venue_neutral.rs"] mod example;` (D22).

The umbrella dev-dependency is
`polyoxide = { path = "../polyoxide", default-features = false, features = ["polymarket-clob", "polymarket-perps", "binance-usdm"] }`.

**5.7 Live trait checks.** `live_predictions_venue`, a target of its own (production, unsigned,
suite `predictions`, timeout 15, features `["predictions"]`, no secrets), chooses its market as
§4.6 does and checks that:

- `instrument` lists it as `tradable`;
- the `quote` and `book` of both outcomes are each not crossed, and mirror each other;
- `trades` is oldest first;
- `event_group` holds both of the market's outcomes.

## 6. Story 4: signing

**6.1 Credentials** (`auth.rs`, D2, D3):

- `Credentials`:
  - `from_pem(key_id: impl Into<String>, pem: &str) -> Result<Self, CredentialsError>`;
  - `from_pem_file(key_id, path: impl AsRef<Path>) -> Result<Self, CredentialsError>`;
  - `key_id()`;
  - a `Debug` that prints the key id and `Secret`'s redaction.
- `SigningKey` is private.
- `CredentialsError` is `#[non_exhaustive]`. It covers an unreadable PEM, an unsupported label, a
  key ring refuses, and a file that cannot be read. Each is `InvalidRequest`, names the label,
  reason or path, and never quotes key material.

**6.2 The authenticator.** A private `KalshiAuth { key_id, key }` implements `Authenticator`:

1. `ts` is `UnixMillis::now()`.
2. The message is `ts + parts.method + parts.path` (D4).
3. It signs the message: Ed25519, or RSA-PSS-SHA256 with one `ring::rand::SystemRandom` per key.
4. It base64-encodes the signature (`STANDARD`, padded).
5. It inserts the three headers.

A failure is `ApiError::Sign(Box::new(..))`.

**6.3 Wiring.**

- `PredictionsBuilder::credentials(Credentials)`.
- Every route builder attaches the authenticator when the client holds one (D3), and
  `account().limits()` refuses without one.
- `AccountLimits { usage_tier, read: BucketLimit { refill_rate, bucket_capacity }, write, grants }`.
- **The confirmation (D5)** replaces Story 1's `before_send` as a whole:
  - `GET /trade-api/v2/account/endpoint_costs` (unsigned is enough, signed when the client holds
    a key) replaces the cost of every route it lists;
  - with a key, `GET /trade-api/v2/account/limits` then resizes the bucket;
  - the bucket always ends confirmed, as D5 says.

  Its inline tests, on a mock server:
  - one read of each across clones and concurrent first sends (mockito `expect(1)`, asserted);
  - a listed route's cost replaces the compiled one;
  - a failed read logs the WARN and keeps the compiled costs or Basic's sizing;
  - an anonymous client's bucket ends confirmed at Basic, so an exact cost above 600 is refused
    rather than left waiting.

**6.4 Signing vectors.** `scripts/capture_kalshi_account.py`, a PEP 723 script with
`cryptography`, on `capture_common.py`, has a `signing` step that needs no key of the owner's:

- it generates throwaway Ed25519 (PKCS#8 v1) and RSA-2048 (PKCS#1 and PKCS#8) keys with Python's
  `cryptography`;
- it signs `1700000000000GET/trade-api/v2/portfolio/balance`;
- it writes `tests/fixtures/predictions/signing/vectors.json`: each key's PEM body, the public
  keys, the message, the Ed25519 signature and an RSA-PSS signature;
- `PROVENANCE.md` says the keys exist only for this fixture.

The fixture always stores each key's body without its armour lines (its base64 lines, joined by
`\n`), and every test that needs the PEM armours it, so no push protection setting can refuse the
file (the owner, 2026-10-18; no PEM private key had been pushed to the repository before).

**6.5 `signing_predictions`.**

- The Ed25519 signature equals the vector's.
- RSA-PSS:
  - Rust verifies Python's signature with ring's `RSA_PSS_2048_8192_SHA256` and the public key,
    which proves MGF1 and the salt length agree;
  - ring verifies Rust's own signature.
- PEM detection reads PKCS#1 RSA, PKCS#8 RSA, PKCS#8 Ed25519 (v1) and a one-line key with literal
  `\n`s.
- Refusals: an EC key, an `ENCRYPTED PRIVATE KEY`, and a garbage body. Each message names the label
  or reason and holds no base64 from the key.
- `Debug` of `Credentials` prints no key material.

**6.6 `mock_predictions_signed`**, a new file (with credentials unless said otherwise):

- every route sends the three headers;
- the timestamp is milliseconds within a second of the clock;
- the signature verifies over `ts + METHOD + path`, query excluded, with the vector key's public
  key;
- a retried attempt (a 429, then a 200) carries a new timestamp;
- without credentials, no `KALSHI-` header is sent, and `account().limits()` sends nothing;
- the confirmation resizes from a mocked `/account/limits` (Premier, 1200 and 1200), once across
  clones;
- a zero refill keeps Basic, with the WARN.

**6.7 The demo capture.** `capture_kalshi_account.py`'s `demo` step reads `/account/limits` from
the demo host, signed through its `signed_headers(method, path)`, which reads
`KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` and returns the three headers, and writes it to
`tests/fixtures/predictions/account/limits.json`, with a `PROVENANCE.md` of its own. It needs the owner's demo key in `KALSHI_DEMO_API_KEY_ID` and
`KALSHI_DEMO_PRIVATE_KEY`. Without them, the story stops at the capture and asks the owner.

**6.8 `live_predictions_account`** runs against the demo host. Its entry has suite `predictions`,
timeout 15, features `["predictions"]` and secrets `["KALSHI_DEMO_API_KEY_ID",
"KALSHI_DEMO_PRIVATE_KEY"]`, exactly the names its loader reads.

- `account().limits()` succeeds, and its Read bucket is at least Basic's.
- A signed `markets().list()` succeeds, which proves a public route takes a signature.
- Without the secrets it is auth-gated.

## 7. Story 5: the kit records handshake headers (D17)

**7.1 The recorder.** In `polyoxide-ws/src/test_server/mod.rs`:

- `Recorder` gains `handshake_headers: Mutex<Vec<Vec<(String, String)>>>`, after `paths`.
- `handshake()` collects `request.headers()` in its `accept_hdr_async` callback, next to the path.
  Names are lowercase and values lossy UTF-8.
- `serve()` pushes them just after `paths.push(path)`.
- `ScriptedServer::handshake_headers(&self) -> Vec<Vec<(String, String)>>` sits beside `paths()`.
  Its documentation says it returns each accepted handshake's request headers, in accept order. A
  refused upgrade records nothing, as for `paths`.

**7.2 The test.** It is appended after the last test in `test_server/tests.rs`, so no cited line
moves. A client connects twice with a header of different values, and both values are recorded in
order. Its names are made-up words, with no venue.

**7.3 Docs.** CLAUDE.md's scripted-server paragraph names the recorder among the others.

**7.4 Additive.** `test_server` is `#[doc(hidden)]` and behind `test-server`, so
`docs/s3-removals.md` gains nothing.

## 8. Story 6: the supervised socket (D13 to D17)

**8.1 Feature and wiring.**

- `predictions-ws = ["predictions", "dep:polyoxide-ws", "dep:futures-util", "tokio/rt", "tokio/macros"]`.
- A path-only dev-dependency on `polyoxide-ws` with `features = ["test-server"]`.
- docs.rs's list gains `predictions-ws`.
- `ci/dep-allowlists/polyoxide-kalshi@predictions-ws.txt`, and its `MUST_BUILD` row (D23).
- The lib.rs assertion gains `#[cfg(feature = "predictions-ws")] is::<predictions::ws::PredictionsWsError>();`.

**8.2 Layout,** `src/predictions/ws/`:

- `mod.rs`: the docs, the re-exports and `#[cfg(test)] mod scripted;`;
- `protocol.rs`: `PredictionsProtocol`, `Change` and `PredictionsMembership`;
- `event.rs`: `Event`, `OrderbookSnapshot`, `OrderbookDelta`, `DisconnectReason` and the frames;
- `error.rs`: `PredictionsWsError`, and `polyoxide_venue::impl_ws_classification!();` at module
  scope, with `ws_class`, `ws_is_fault` and `ws_retry_after` all called;
- `builder.rs`: `PredictionsWsBuilder` and `SupervisedPredictionsWs`;
- `scripted.rs`: Kalshi's answer for the kit's scripted server. It is `#[cfg(test)]` and also
  `#[path]`-included by the integration suites, so it names nothing of the crate's own, as
  Binance's does.

**8.3 The builder.**

- `PredictionsWsBuilder::new()` defaults to `PRODUCTION_WS`. `DEMO_WS` is also a const.
- `url(&str)` takes `wss://`, or `ws://` to a loopback host. Anything else is `InvalidUrl`.
- `polyoxide_ws::timing_setters!` with D15's values. The kit's default ping is 20 s, so the builder
  sets 10 s.
- `connect(credentials: Credentials, tickers: impl IntoIterator<Item = Ticker>)` returns
  `Result<SupervisedPredictionsWs, PredictionsWsError>` through `Supervisor::connect`, so a failed
  first attempt is returned.

**8.4 The handshake.** `url.into_client_request()`, then the three headers over the URL's path
(D17). A header that cannot be built is `InvalidRequest`.

**8.5 Commands and answers.**

- **Frames.**
  - `subscribe`: `{"id":…,"cmd":"subscribe","params":{"channels":["orderbook_delta"],"market_tickers":[…],"use_yes_price":true}}`;
  - `update_subscription`: `{"params":{"sid":…,"action":"add_markets"|"delete_markets","market_tickers":[…]}}`;
  - `unsubscribe`: `{"params":{"sids":[…]}}`.

  `id` is the kit's request id.
- **Correlation.**
  - The same id with type `subscribed`, `ok` or `unsubscribed` is the answer.
  - The same id with type `error` answers the change with `Server`.
  - Another id is `Uncorrelated`, and a frame with no id is a `Push`.
  - An answer that carries `sid` and `seq` counts in that subscription's sequence (D14): the
    correlation closure records it, because the kit's `request` consumes answers before `decode`
    sees them.

**8.6 Membership and changes** (D13):

- The membership is `BTreeSet<Ticker>`.
- `Change::{Subscribe(Vec<Ticker>), Unsubscribe(Vec<Ticker>)}`, and `Ack = ()`.
- `PredictionsMembership`, implemented for `MembershipHandle<PredictionsProtocol>` (perps'
  `protocol.rs:351-399` is the model), gives `subscribe` and `unsubscribe`.
- A change is recorded before it is sent. During an outage it is `Offline::Recorded`.
- `wanted` is `true`.
- The replay is one `subscribe`, when the membership is not empty.
- The connection's `sid` and its sequence state are cleared in `reset()`.

**8.7 Decode.**

- `orderbook_snapshot` is `Event::Snapshot(OrderbookSnapshot { market_ticker, market_id, bids, asks })`:
  `yes_dollars_fp` gives the bids and `no_dollars_fp`, in YES pricing, the asks, each best first. A
  missing key is an empty side.
- `orderbook_delta` is `Event::Delta(OrderbookDelta { market_ticker, side, price, delta, ts,
  client_order_id })`, where `side` `yes` is a bid and `no` an ask, in YES pricing, and `delta` is
  signed.
- Each frame with `sid` and `seq` passes D14's check.
- An `error` with no `id` is `Server`.
- Any other `type`, and Ping, Pong and Close, are `Nothing`.
- Text that is not JSON is `Json`.

**8.8 Hooks.**

- `is_live`: every frame. `delivered`: `Event` or `Events`. `ping`: an empty protocol Ping.
- `disconnected` yields `Event::Disconnected { reason: DisconnectReason }`, the kit's `Outage` as
  `Closed`, `Error`, `Stale` or `Rotation`. `reconnected` yields `Event::Reconnected`.
- `queue` is `Bounded(1024)`.
- `recover` follows D16. `is_retriable` and `retry_after` read `Classify`.

**8.9 `PredictionsWsError`** is `#[non_exhaustive]`:

- `Connection(WsError)`, classed through the macro's table;
- `Server { code, message }`: 9 is `Unauthorized`, 27 `RateLimited`, 10 and 25 `Unavailable`, and
  every other code `InvalidRequest`;
- `SequenceGap { sid, expected, received }`, classed `Network` and a fault;
- `Json { text, source }`, built explicitly, since no `From<serde_json::Error>` is allowed;
- `InvalidUrl { url, reason }`.

**8.10 The scripted answer.**

- `subscribe` is answered with `subscribed` (sid 1, then 2, and so on) and one snapshot per ticker,
  with `seq` counting from 1.
- `update_subscription` is answered with `ok`, carrying `seq`, and `unsubscribe` with
  `unsubscribed`.
- Options inject a `seq` gap and an error frame (code 9, 10, 25). The kit's own scripts supply refusals
  (`refuse_upgrade(401)`, `refuse_upgrade(503)`), silence and cuts.
- Its frames follow the captured ones.

**8.11 `supervision_predictions`** (integration, with the answer included by `#[path]`):

1. **A fresh signature on every attempt.** A cut forces a second connection. `handshake_headers()`
   then holds two entries with different `kalshi-access-timestamp`s. Each
   `kalshi-access-signature` verifies over `ts + "GET" + "/trade-api/ws/v2"` with the vector key's
   public key, and `paths()` records `/trade-api/ws/v2`.
2. **Replay.** The second connection's first frame is a `subscribe` carrying every ticker and
   `use_yes_price: true`.
3. **Membership.** Changes go out as `update_subscription` on the live `sid`. The first add sends a
   `subscribe`, and emptying the membership sends an `unsubscribe`. A change made during an outage
   is replayed.
4. **A gap.** A `seq` gap yields `Disconnected(Error(SequenceGap))`, then `Reconnected`, then a
   fresh snapshot.
5. **Liveness.** The server's Pings alone keep a quiet connection live past `stale_after`.
6. **Error frames.** Code 9 ends the stream with its error, and codes 10 and 25 reconnect.
7. **Refusals.** A 401 refusal ends the stream, and a 503 is retried.
8. **Markers.** Each outage yields one `Disconnected` and one `Reconnected`.

Inline tests in `protocol.rs` cover the recovery map, the frames, the correlation (with `seq`
counted from answers), and decoding each fixture.

**8.12 The demo capture**, `scripts/capture_kalshi_predictions_ws.py`: a PEP 723 script with
`cryptography` and `websockets`, through `capture_common.ws_session`, with its handshake headers
from `capture_kalshi_account.py`'s `signed_headers` (§6.7).

- It subscribes one open demo market with `use_yes_price: true`.
- It records into `tests/fixtures/predictions/ws/`, with `PROVENANCE.md`:
  - the `subscribed` answer and the snapshot;
  - the deltas of up to 60 s;
  - an `update_subscription` `ok` and an `unsubscribed`;
  - one error frame, from a `subscribe` that names no market.
- It records in `OBSERVED.md`:
  - each subscription's first `seq`;
  - whether `ok` and `unsubscribed` advance `seq`;
  - whether the snapshot's book is not crossed, which shows `use_yes_price` is honoured;
  - any bound on tickers per subscription it meets.

It needs the demo key, as §6.7 does.

**8.13 `wire_agreement_predictions_ws`.** Each captured frame against its type, both ways.

**8.14 `live_predictions_ws`** runs against the demo host. Its entry has suite `predictions`,
timeout 15, features `["predictions-ws"]` and the secrets of §6.8.

- It connects with the demo key and subscribes an open demo market, which it finds over unsigned
  REST against the demo host. With none, it is an `environmental(..)` skip.
- The market's snapshot arrives within 30 s, and its book is not crossed.
- For 30 s, the frames carry no unmodelled keys. A feed that goes quiet after the snapshot passes.
- A stream that ends with no close code is `transient(..)`.
- Without the secrets it is auth-gated.

## 9. Story 7: validation, amendments and records

**9.1 The audit** (8.5), run against the range from the run's base (§1.1) to `HEAD`:

- **The touch set.** The touch-set gate (§10) prints nothing.
- **Generated regions.** Every changed hunk of `README.md`, `docs/specs/INDEX.md`,
  `SELF-HEALING.md` and the two nightly workflows lies inside a generated region.
- **The foundation.** The only foundation files changed are
  `polyoxide-ws/src/test_server/{mod.rs,tests.rs}` (Story 5), shown with
  `git diff --stat <base>..HEAD -- polyoxide-venue polyoxide-core polyoxide-ws polyoxide-test-support`.
- **No copies.** `polyoxide-kalshi` defines none of the helpers named in
  `_bmad-output/specs/spec-venue-extensibility/duplication-inventory.md`, checked with a grep for
  each name defined locally (`fn <name>`, `struct <name>`).

Each result goes into the amendments file.

**9.2 Amendments.** `spine-amendments/epic-8.md`, in `epic-6.md`'s format, records each gap the
recipe or the spine left:

| Id | Gap |
|---|---|
| A8-1 | The product is `predictions`: AD-4's and AD-7's examples (D1) |
| A8-2 | Recipe step 3's `RetryPolicy` is optional when core's default fits (D6) |
| A8-3 | `CandleInterval` promises every width, and a venue that serves some cannot implement `Candles`; proposal: per-venue widths, or a refusal of a width the venue lacks (D10) |
| A8-4 | The scripted server could not show a handshake's headers; it now records them (D17) |
| A8-5 | CI derives a header pin for every venue crate on core, and its full-pin rule must leave out crates the umbrella cannot depend on, a `publish = false` crate among them; the flow profile's gate needed the same (D20, D24) |
| A8-6 | Hand-written registration points the recipe does not name: `test_dep_allowlists.py`'s `PER_FEATURE` and `MUST_BUILD`, and CLAUDE.md's paragraphs that list venues and pins (D20, D23) |
| A8-7 | Step 3 says to read limits and costs "on first use" without saying where; a throttle cannot send, so the read belongs before the first send (D5) |
| A8-8 | The umbrella's generic example cannot be included without every venue's features; proposal: move its generic half into its own file (D22) |
| A8-9 | test-support's agreement and OpenAPI helpers read JSON only, so a venue that publishes YAML needs a parser (D19) |
| A8-10 | cargo-hack's `--ignore-private` leaves a `publish = false` venue's features unchecked |

The run appends any gap it meets, each one numbered. Merging them into the spine and regenerating
`docs/ARCHITECTURE.md` is an owner session after the run.

**9.3 `OBSERVED.md`** holds every probe and capture finding of §§3, 4, 6 and 8.

**9.4 The trading mapping** (D21). Its keys become `kalshi.predictions:…` and its size unit
`SizeUnit::Shares`. Its move is re-filed for Kalshi's `Trading`.

**9.5 `deferred-work.md`** gains entries for:

- a soak of the anonymous limits;
- a local book and `get_snapshot`, and the `ticker` and `trade` channels;
- cargo-hack's coverage of Kalshi's features (A8-10) and `Candles` (A8-3);
- the mapping's move with Kalshi's `Trading`;
- adopting the 2026-10-22 schema corrections.

The entry for the eight trading probes stays as it is.

## 10. Gates and repo rules every story keeps

- **The story gates.** These are the profile's story gates, run with `nix develop --command`:
  - fmt, clippy `-D warnings`, the tests, and rustdoc `-D warnings`;
  - `gen_registry.py --check`, `venue_gate.py` and `live_unwraps.py`;
  - `test_body_diff.py --baseline v0.40.0`.
- **The touch-set gate** follows them:
  `bash -c '{ git diff --name-only "$(git merge-base main HEAD)"; git ls-files --others --exclude-standard; } | sort -u | grep -v -E -f _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt; test $? -eq 1'`.
  It covers committed, uncommitted and untracked paths, and it passes when every one matches the
  allowlist. It prints any path that does not match. A missing allowlist makes `grep` exit 2, so
  the gate fails closed. Every story's **Verify** line ends with it, since the profile's story
  gates do not run it. Story 1 writes the allowlist, one extended regex per line, matching
  only D20's paths:
  - the root `Cargo.toml` and `Cargo.lock`;
  - `polyoxide-kalshi/`, `docs/specs/kalshi/`, `scripts/capture_kalshi_*.py` and
    `ci/dep-allowlists/polyoxide-kalshi*.txt`;
  - the files whose regions `gen_registry.py` writes, and `CLAUDE.md`;
  - `.github/workflows/ci.yml`, `.github/scripts/tests/test_ci_workflow.py` and
    `.github/scripts/tests/test_dep_allowlists.py`;
  - `polyoxide-ws/src/test_server/mod.rs` and `polyoxide-ws/src/test_server/tests.rs`;
  - this spec, the plan, `_bmad-output/specs/`, `spine-amendments/epic-8.md` and
    `_bmad-output/implementation-artifacts/deferred-work.md`.
- **The final gates** are the profile's, with D24's Kalshi pins. They add the removals check
  (`api_removals.py check --baseline v0.40.0`), which excludes a crate absent from the baseline,
  with a warning.
- **The foundation crates name no venue.** Story 5's test uses made-up words.
- **Error types.**
  - Every new public error type is in its crate's `Classify` assertion.
  - Every new error enum is `#[non_exhaustive]`.
  - No module implements `From<reqwest::Error>`, `From<serde_json::Error>` or
    `From<url::ParseError>`.
- **Add tests; never change an existing Rust test's body** (AD-12). Story 5 appends. The one
  existing test this epic changes is `test_ci_workflow.py`'s full-pin test, which is Python and
  outside the test-body diff (D20).
- **Generated regions are regenerated, never edited:** `gen_registry.py --write` after each change
  to metadata (Stories 1, 2, 4 and 6). `docs/ARCHITECTURE.md` is not edited.
- **Mirrors stay byte for byte.** Observations go in `OBSERVED.md`.
- **Live suites.**
  - Credentials come only through test-support's loaders, and failures through `or_fail`,
    `environmental` and `transient`.
  - No `.unwrap()` or `.expect(` (`live_unwraps.py`), and every test is `#[ignore]`d.
  - A new live file has no `live_unwraps.baseline.json` entry while its count is zero.
- **No landmines:** no `connect_async` outside the kit, no `.gzip(`, no `allow_burst(`.
- **Commits** go by pathspec, in conventional form naming the story, with `Cargo.lock` when cargo
  changes it, and never `--no-verify`.

## 11. Limits

- **HTTP**, requests per call:

  | Call | Requests |
  |---|---|
  | `instrument`, `quote`, `book`, `trades` | 1 each (`trades` asks for 1000 rows) |
  | `event_group` | 2 |
  | a client's first send | adds 1 confirmation read, or 2 with credentials |

  - Each request costs its route's tokens on the Read layer: 10 today, at 600 capacity and
    200/s until confirmed.
  - A 429 is retried (3 times by default), and every 429 holds the client for `retry_delay(0)`.
- **The socket.**
  - Connect: 10 s per attempt.
  - Ping, staleness and backoff: a 10 s ping, staleness after 30 s, and a 500 ms to 60 s backoff
    with the `Retry-After` floor.
  - No rotation, and a queue of 1024.
  - Commands go unpaced: Kalshi publishes no command rate, and code 27 reconnects.
  - No client cap on tickers per subscription: code 26 is the venue's bound, and the capture
    records any it meets.
- **Bounds on what a caller supplies.**
  - A ticker is 1 to 200 characters of `[A-Z0-9.-]`, at least one a letter or digit.
  - The native `trades().limit` sends what it is given, and Kalshi refuses more than 1000 as a
    `VenueRefusal` (probe 6).

## 12. Not in this epic

- Kalshi's `Trading`, its order routes and probes, the Write, balance and per-shard buckets, and
  subaccounts.
- Keychain storage of Kalshi credentials, a `polyoxide kalshi …` CLI, the umbrella, Python, and
  publishing `polyoxide-kalshi`.
- The Margin exchange.
- The `ticker`, `trade` and user channels, a local book, and `get_snapshot`.
- `Candles` and `Instruments`, and a soak of the anonymous limits.
- Merging the amendments into the spine, regenerating `docs/ARCHITECTURE.md`, and `bmad-spec`'s
  success-signal record: an owner session.
- Updating `sprint-status.yaml`, a release, and adopting the 2026-10-22 schema drift.

## 13. Owner actions

- Add the repository secrets `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` (the PEM text).
- Put the same two in the environment of the session that runs Stories 4 and 6, whose demo
  captures need them.
- The flow profile's Kalshi pins are done (D24).
