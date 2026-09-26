## [0.33.0] - 2026-09-26

Adds Deposit Wallets and session keys. `polyoxide-clob` signs signature-type-3
orders, an ERC-7739 `TypedDataSign` envelope pinned to py-sdk's golden digest,
for a `SigningTarget::DepositWallet { wallet, role }`. It also accepts any alloy
signer (`Account::with_signer`) or none (`Account::l2_only`), takes an externally
produced L1 signature and checks it locally, and lists a wallet's session signers.
`polyoxide-relay` speaks the relayer's `type: WALLET` dialect: CREATE2 wallet
derivation, `resolve_wallet`, Deposit Wallet batches, session-signer
authorization and revocation, and the redemption pair, which targets the
collateral adapter as py-sdk does. `RelayClientBuilder::with_auth` builds a
client that has auth but no account. The contract and the SDK behaviours the
upstream pages omit are recorded in `docs/specs/session-keys/`.

Breaking: `Wallet::signer()` returns `Result<&DynSigner, ClobError>` instead of
`&PrivateKeySigner`, and `Wallet::ethereum_wallet()` is removed.
`BuilderAccount::signer()` returns `&DynSigner`, so `.credential()` and
`.to_bytes()` on it no longer compile. `AuthMode` gains `L1Signed`. `WalletType`
gains `DepositWallet` (wire `"WALLET"`) and is now `#[non_exhaustive]`.
`ContractConfig` gains five fields, so an external struct literal breaks.
`Account::sign_order` and `Clob::sign_order` sign type-3 orders for a Deposit
Wallet target instead of refusing them, and `ClobBuilder::signature_type`
defaults from the account's target. Every existing loader yields an EOA target,
so no existing caller changes behaviour.

### 🚀 Features

- *(clob)* SigningTarget names the account an Account signs for
- *(clob)* Wallet holds any alloy signer, or none for L2-only use
- *(clob)* Account::with_signer, Account::l2_only and a signing target
- *(clob)* ERC-7739 Deposit Wallet order signing pinned to py-sdk vectors
- *(clob)* Account::sign_order honours the signing target
- *(clob)* Create_order builds Deposit Wallet orders from the signing target
- *(clob)* L1 auth typed data out, signature in, for external wallets
- *(clob)* L1 signature-in is checked locally and pinned to py-sdk's typed data
- *(clob)* List a Deposit Wallet's session signers; SessionSignerScope in core
- *(relay)* Pure CREATE2 derivations for Deposit Wallet, Safe and Proxy, pinned to py-sdk
- *(relay)* WalletType::DepositWallet, v1 params and transaction routes, resolve_wallet
- *(relay)* Deposit Wallet Batch typed data, calldata encoders and session envelope pinned to py-sdk
- *(relay)* Execute through a Deposit Wallet; auth without a key; typed-data-out, signature-in batches
- *(relay)* Session-signer authorization and revocation under Builder HMAC
- *(relay)* Deposit Wallet redemption pair; document the Deposit Wallet and session-key surface
- *(relay)* Resolve_wallet probes the Proxy with type=PROXY, as py-sdk does; authorize_session_signer refuses a relayer key before I/O

### 🐛 Bug Fixes

- *(clob)* Actionable session-signers mismatch error, verify L2-only POLY_ADDRESS
- *(relay)* Resolver tests assert their probe counts; encode transaction ids; WalletType is non_exhaustive
- *(relay)* Keep a Deposit Wallet call value above u64 exact in the typed data
- *(relay)* Pin the session-key batch signature; refuse a Deposit Wallet on an unsupported chain before I/O; export RelayClientBuilder
- *(relay)* Let revocations take a relayer API key; wait 300s on session-signer POSTs
- *(relay)* Request py-sdk's full trading-approval set; always send Deposit Wallet metadata
- *(relay)* Deposit Wallet redemption targets the collateral adapter, as py-sdk does

### 🚜 Refactor

- *(clob)* Share the exchange domain and 0x1901 digest; type 3 needs maker == signer
- *(clob)* Deposit Wallet primitives are crate-private; guards are validation errors
- *(core)* SessionSignerScope is non_exhaustive and parses via FromStr
- *(core)* DepositWalletRole lives in core for clob and relay to share
- *(relay)* WalletKind is non_exhaustive with an infallible address; Safe init hash lives in ContractConfig

### 📚 Documentation

- *(specs)* Adopt upstream data, data-v2, perps and perps-ws specs
- *(handoff)* Correct the ERC-7739 domain orientation for session keys
- Handoff amendments for two Deposit Wallet generations, relayer auth enum, tx state
- *(specs)* Design for the offline Deposit Wallet session-key surface
- *(specs)* Align the session-key spec with prader-rs [#126](https://github.com/dilettante-trading/prader-rs/issues/126) and its §8 contract
- *(plans)* Session keys plan 1, CLOB signing core and account model
- *(plans)* Vector() needs a mutable map
- *(plans)* Refuse non-type-3 overrides on a Deposit Wallet target before I/O; target() by value
- *(clob)* SigningTarget docs state what overrides can and cannot do
- *(clob)* [**breaking**] DynSigner needs sign_hash; export it from the crate root
- *(plans)* Task 9 relinks the DynSigner doc once clob_auth_typed_data exists
- *(plans)* Fold Task 4 review notes into Tasks 7 and 11
- *(plans)* Task 8 refuses a foreign funder on a Deposit Wallet target; Task 7 pins the full vector
- *(plans)* Task 8 guards market orders' funder too
- *(plans)* MarketOrderArgs lives under types
- *(plans)* Fold Task 7 review notes into Tasks 8 and 11
- Balance queries default to the target's signature type (Task 10); spec names the right params type
- L1 signature-in lives on Clob and is checked locally; Task 11 shows the onboarding round trip
- *(specs)* Scope validator belongs to the relay; session-key credentials on session-signers is an open item
- *(clob)* Document Deposit Wallet accounts and session keys
- *(specs)* Align section 1 and 2 text with the shipped code; record plan 1's breaking changes
- *(plans)* Session keys plan 2, relay Deposit Wallet dialect
- *(plans)* Calldata lengths count the 0x prefix
- *(plans)* Relay tests read the py-sdk-built bodies and the two-call batch
- *(plans)* Redemption submit sends the empty metadata py-sdk sends
- *(plans)* Resolve_wallet returns Option<WalletKind>
- *(plans)* Assert every expect(n) mock
- *(relay)* Batch_typed_data departs from py-sdk in two places, not one
- *(plans)* Revocation accepts a Relayer API key; session-signer routes use a 300 s timeout
- *(specs)* List plan 2's breaking changes for the 0.33.0 release notes
- *(relay)* Bring the README and crate docs up to the Deposit Wallet surface
- *(plans)* Plan 3, docs and the ignored live round trip for session keys
- *(specs)* Record the Deposit Wallet and session-key contract
- *(specs)* Record what py-sdk and ts-sdk do for session keys that the pages omit
- *(plans)* Plan 3 Task 6, resolve_wallet probes the Proxy as py-sdk does
- Point the spec index, the drift workflow and CLAUDE.md at docs/specs/session-keys
- *(specs)* Session-keys README, corrections from review
- *(plans)* Plan 3 Task 7, Deposit Wallet redemption goes through the collateral adapter
- *(specs)* Session-keys OBSERVED, corrections from review
- *(plans)* Plan 3 Task 6 also moves the relayer-key refusal ahead of the nonce fetch
- *(plans)* Plan 3 Task 7 also drops the unobserved venue claim in OBSERVED row 9
- The relay mirror does document /deployed?type=WALLET; no unobserved venue claims
- *(relay)* /deployed answers every non-WALLET type alike; revoke accepts a relayer key
- *(plans)* Seven signed batches after the adapter redemption vectors
- Status of every handoff item; plans 2 and 3 marked done in the design
- *(handoff)* Item 8 status line says seven batches, not five
- Handoff status wording; the redemption pair's real names
- OBSERVED intro admits the /deployed probe; redeem_submit_body is unconsumed; lifetime claim attributed

### 🧪 Testing

- *(clob)* Golden vectors for Deposit Wallet order signing from py-sdk
- *(clob)* Session-key vectors carry provenance and assert py-sdk's golden digest
- *(clob)* Session-key provenance names the right envelope
- *(clob)* Deposit Wallet market orders carry the wallet as maker and signer
- *(clob)* Expect(0) mocks can match; pin the proxy-target maker path
- *(clob)* L1 signature check names the real signer and normalises v
- *(relay)* Golden vectors for Deposit Wallet batches and derivations from py-sdk
- *(relay)* Session-signer bodies come from py-sdk; a two-call batch; one documented command
- *(relay)* Provenance flags the synthetic two-call batch
- *(clob)* Ignored live round trip for a Deposit Wallet session key
- *(clob)* Live session-key round trip selects its market first and self-heals; own nightly row
- *(clob)* Live session-key round trip never aborts on the owner listing; self-heal waits for the revocation
- *(relay)* Adapter batches in the digest and signature loops
- *(relay)* Pin the adapter redemption batches' typed data to py-sdk; digest test covers every batch
- *(relay)* Neg-risk submit target, estimate refusal, and no RPC before the client checks

## [0.32.2] - 2026-09-23

Adds the new v2 positions filters and sort key to `polyoxide-data` and the CLI,
and deprecates `data.approvals()`, whose upstream route now returns 404.

### 🚀 Features

- *(data)* `PositionStatus::RedeemableLost` (settled losing positions; rows come back labelled `REDEEMABLE`), `PositionStatus::Mergeable` (`OPEN` narrowed to conditions holding a complementary set of live outcome tokens; rows come back labelled `OPEN`) and `PositionSortBy::Price` (by `current_price`) on `/v2/positions`. Both enums are `#[non_exhaustive]` (#38)
- *(cli)* `data positions list --status` accepts `redeemable-lost` and `mergeable` (and the upstream `REDEEMABLE_LOST` spelling), and `--sort-by` accepts `price` (#38)

### ⚠️ Deprecated

- *(data)* `DataApi::approvals()` and `ApprovalsApi`: upstream removed `GET /v1/approvals` and the host answers `404` for it. Use `data.v2().approvals(user)`. Removal is deferred to a breaking release (#41)

### 📚 Documentation

- *(data, cli)* The `event_id` filters on trades, activity, positions and live-volume document upstream's cap of 20 distinct ids, which the server enforces with a non-retryable 400 (#38)
- *(gamma)* `ListKeysetEvents::limit` is capped at 100 (was documented as 500). The server clamps larger values rather than rejecting them, so a short page does not mean the end of the data. Page on `next_cursor` (#42)
- *(specs)* Adopt upstream perps and perps-ws specs: `GET /v1/account/backstops`, a required `fee_tier` on `Portfolio`, and 503 responses on the leverage, margin and internal-transfer routes (#39, #40)

## [0.32.1] - 2026-09-23

Adds `ListEvents::include_markets`. Everything else in the workspace is
byte-identical to 0.32.0.

### 🚀 Features

- *(gamma)* Add `include_markets(bool)` to `ListEvents`. The parameter is not in upstream's `openapi.yaml`, but the server applies it (verified 2026-09-23): omitting it is the same as `true`, and `false` removes the `markets` key from each event rather than sending `[]`. `Event::markets` is `#[serde(default)]`, so such a response parses with an empty `markets`

### 📚 Documentation

- *(gamma)* Record `include_markets` in `docs/specs/gamma/OBSERVED.md` as an undocumented parameter the server applies on both `GET /events` and `GET /events/keyset`

## [0.32.0] - 2026-09-16

Adds Data API v2 (20 routes under `/v2` on `data-api.polymarket.com`) to
`polyoxide-data` as `data.v2()`, alongside the v1 routes, which upstream says
keep working. v2 has its own contract: a `data` envelope, cursor-only
pagination and snake_case fields. Paged builders return `Page<T>` from
`send()` and a `Stream` from `.pages()`. `RateLimiter::data_default` carries
measured rows for six v2 routes. The Python bindings expose v2 as
`DataApi().v2()` / `DataApiSync().v2()`, with the row classes on
`polyoxide.v2`.

Breaking: `DataApiError` is now `#[non_exhaustive]` and gains `V2` (a v2 error
body's `code`, `retryable` flag and `trace_id`) and `Pagination` (for
`.pages()` walks), so exhaustive matches must change. `is_retriable` follows
the server's `retryable` flag for v2 errors.

Breaking for scripts: `polyoxide data` commands now read Data API v2. Output
is the v2 `{data, pagination}` envelope with snake_case fields
(`proxy_wallet`, not `proxyWallet`). `--offset` is replaced by `--cursor`,
`--all` and `--max-pages`, `positions closed` by
`positions list --status closed`, and `holders` requires `--condition`
(`--market` remains an alias). `data traded` prints the `/v2/user-stats`
object (its `trades` field is the distinct-market count), or `null` for a
wallet the API does not know, instead of `{user, traded}`. `data health` is
unchanged. Comma-separated `--market` and `--event-id` values, which panicked
in earlier releases, now work.

Also fixes `data.holders().list()`, which failed with a serialization error
when the venue answered a miss with `null`; it now returns no holders.

### 🚀 Features

- *(core)* Make Request cloneable without Clone markers
- *(data)* [**breaking**] Structured Data API v2 errors on DataApiError
- *(data)* Data API v2 response and parameter types
- *(data)* Data API v2 envelopes, cursor walks, trades and user-stats
- *(data)* V2 activity and combo activity
- *(data)* V2 wallet routes
- *(data)* V2 market routes
- *(data)* V2 board routes
- *(data)* V2 status, completing the v2 surface
- *(core)* Provisional rate limits for Data API v2
- *(data)* V2_soak harness for measuring Data API v2 rate limits
- *(core)* Pin measured Data API v2 rate limits
- *(cli)* Cursor paging and JSONL output helpers for data commands
- *(cli)* [**breaking**] Data trades on Data API v2 with cursor paging
- *(cli)* [**breaking**] Data activity and positions on Data API v2
- *(cli)* [**breaking**] Data holders, open-interest and live-volume on Data API v2
- *(cli)* [**breaking**] Data traded and builders on Data API v2
- *(py)* Data API v2 row classes on a polyoxide.v2 submodule
- *(py)* Polyoxide.v2 module and typed stub for the v2 rows
- *(py)* DataApi.v2() with every Data API v2 route
- *(py)* Map Data API v2 errors by code and keep their fields

### 🐛 Bug Fixes

- *(data)* Read a null /holders body as no holders
- *(data)* V2_soak builds holder probes from market condition ids only

### 🚜 Refactor

- *(core)* Split ApiError::from_status_and_body out of from_response
- *(data)* Share the soak harness Pacer via examples/common

### 📚 Documentation

- Handoff for Deposit Wallet and Session Key support
- *(specs)* Sync the data mirror
- *(clob)* Sync the clob mirror and document order status semantics
- *(gamma)* Sync the gamma mirror and pin the market-maker surface it dropped
- *(specs)* Sync the perps and perps-ws mirrors
- *(specs)* Design Data API v2 support
- *(plans)* Data API v2 implementation plan for Phases 0-2
- *(data)* Data API v2 in the README
- Mark Data API v2 implemented and document its overlaps
- *(plans)* Data API v2 rate-limit measurement plan (Phase 3)
- *(specs)* Record Data API v2 rate-limit ramps
- Document the measured Data API v2 rate limits
- *(plans)* Data API v2 CLI (Phase 5) implementation plan
- *(plans)* Data traded prints the v2 user-stats object
- *(plans)* Keep mutation-check copies out of the loom worktree
- *(cli)* Document the Data API v2 data commands
- *(plans)* Data API v2 Python bindings (Phase 4) implementation plan
- Document the v2 data CLI commands and Python bindings

### 🧪 Testing

- *(data)* Capture Data API v2 fixtures and record observed behaviour
- *(data)* Hold the v2 types to live captures in both directions
- *(data)* Live tests for every Data API v2 route
- *(data)* V2_soak probe space of distinct URLs per route
- *(data)* V2_soak verdict rules for stages and pinned counts
- *(data)* Validate the pinned Data API v2 rows per route
- *(data)* Validate the Data API v2 rows across all routes at once
- *(data)* Pin that empty v2 multi-value setters omit their parameter
- *(data)* Pin every v2 enum's wire values to what the server accepts
- *(data)* Decode a captured response through every v2 builder offline
- *(cli)* Live tests for the Data API v2 data commands
- *(py)* Live Data API v2 tests and README usage

### ⚙️ Miscellaneous Tasks

- *(specs)* Mirror Data API v2 and watch it for drift
- *(data)* Run the rate-limit harnesses' unit tests

## [0.31.0] - 2026-09-09

The CLOB market channel can change membership on a live socket. The venue
has always accepted `{"operation":"subscribe"|"unsubscribe","assets_ids":[…]}`
on the market channel (it is `SubscriptionRequestUpdate` in
`asyncapi-market.json`), but the crate gated subscription updates to the user
channel. `WebSocket::subscribe_assets` / `unsubscribe_assets` send the frame
on a plain socket, and a `MembershipHandle` (from
`WebSocketWithPing::membership`, taken before `run`) sends it while the ping
loop drives the connection. Verified live on 2026-09-09: an added asset gets a
fresh `book` in ~155 ms, a duplicate add gets nothing (unsubscribe then
subscribe to force a snapshot), and an empty-membership socket stays open
under the 10 s `PING`.

Breaking: `WebSocketError` is not `#[non_exhaustive]` and gains
`MembershipClosed`, so exhaustive matches must add an arm. Everything else in
the workspace is byte-identical to 0.30.0.

### 🚀 Features

- *(clob)* Add the market-channel MarketSubscriptionUpdate frame
- *(clob)* [**breaking**] Market-channel subscribe_assets/unsubscribe_assets on WebSocket
- *(clob)* MembershipHandle sends market subscription updates while run pumps

### 📚 Documentation

- *(clob)* The market channel changes membership on a live socket
- *(clob)* Membership() is market-only, name the duplicate-add remedy, README section for live membership

## [0.30.0] - 2026-09-07

Adds `polyoxide-rtds`, a client for Polymarket's Real-Time Data Service
(`wss://ws-live-data.polymarket.com`) crypto price streams. It depends on
nothing else in the workspace — not even `polyoxide-core` — so a
credential-free price feed does not pull in the signing stack. Reachable from
the unified crate under the `rtds` feature and from the CLI as
`polyoxide ws prices`.

Also carries two fixes that landed after the 0.29.0 tag was cut, and so appear
here rather than in that release's section.

### 🚀 Features

- *(rtds)* Add the polyoxide-rtds crate skeleton
- *(rtds)* Add Topic and TwapWindow with wire mappings
- *(rtds)* Build subscription filters as compact JSON
- *(rtds)* Add RtdsError with a recoverability classifier
- *(rtds)* Classify errors three ways instead of two
- *(rtds)* Decode E18 and plain decimal values without panicking
- *(rtds)* Add per-topic payload types
- *(rtds)* Dispatch frames into per-topic events
- *(rtds)* Add the Rtds stream client
- *(rtds)* Add reconnect, staleness detection, and keep-alive
- *(rtds)* Re-export through the unified crate and add an example
- *(cli)* Add ws prices for RTDS reference price streams

### 🐛 Bug Fixes

- *(rtds)* Compute the subscription struct length from its fields
- *(rtds)* Treat a malformed handshake request as fatal
- *(rtds)* Say what a decode failure actually was
- *(rtds)* Make dropped frames diagnosable
- *(rtds)* Reconnect with a fresh stream and back off a flapping host
- *(release)* Publish polyoxide-rtds to crates.io
- *(ci)* Match transient failures in both of a panic's error renderings
- *(data)* Select a holders market instead of trusting the newest trade

### 🚜 Refactor

- *(rtds)* Derive topic and window parsing from their wire mappings
- *(rtds)* Keep the test seam off the public API

### 📚 Documentation

- *(specs)* Design for the RTDS crypto price streams crate
- *(plans)* Implementation plan for polyoxide-rtds
- *(rtds)* Stop TwapWindow::ALL claiming a guarantee it lacks
- *(plans)* Fix a step ordering that made red-then-green unobservable
- *(plans)* Classify errors three ways, and keep the frame that failed
- *(plans)* Record what SkipFrame costs when a symbol stays broken
- *(rtds)* Correct what decode_plain actually accepts
- *(plans)* Share the captured frames instead of pasting one
- *(plans)* Correct the mislabelled spot snapshot in Task 8
- *(plans)* Fix a stale test count in Task 8
- *(rtds)* Record which field actually keeps the envelopes apart
- *(plans)* Stop Task 9 shipping a doc link to a module that does not exist
- *(plans)* Say why Task 11 reconnects with a fresh Rtds
- *(plans)* Match the harness the crate's features can actually build
- *(plans)* Finish the std Mutex swap in the harness
- *(rtds)* Record the harness ordering the supervision tests rely on
- *(rtds)* Say what the delivered flag does not fix
- *(specs)* Close the design's open question — RTDS does accept resubscribe
- *(specs)* Mirror the observed RTDS contract
- *(rtds)* Stop claiming a fixed backfill length
- *(rtds)* A backfill only arrives for a filtered subscription
- *(claude)* Document polyoxide-rtds and its gotchas

### 🧪 Testing

- *(rtds)* Tie the topic list to the window list
- *(rtds)* Add frames captured verbatim from the live host
- *(rtds)* Let integration tests share the captured frames
- *(rtds)* Assert the fixture cross-check exactly, not within 0.001
- *(rtds)* Pin a server bug that mislabels Chainlink-spot backfills
- *(rtds)* Add a scripted local WebSocket server
- *(rtds)* Give the scripted server its own smoke test
- *(rtds)* Stop the supervision tests waiting on timeouts
- *(rtds)* Establish that RTDS accepts a second subscribe frame
- *(rtds)* Add live tests with an unfiltered control subscription
- *(rtds)* Cover the supervision layer, which no test could see

## [0.29.0] - 2026-08-26

### 🚀 Features

- *(clob)* Per-address connect timeout and IPv6/IPv4 fallback for WebSocket connects. `connect_async` resolves DNS internally and connects to only the first address with no timeout, so one blackholed AAAA record hung the connect indefinitely. Addresses are now resolved up front and interleaved by family (RFC 8305 §4), and each address's TCP connect and TLS+handshake is bounded by the new `WebSocketBuilder::connect_timeout` (default 10s). The free `connect_*` functions delegate to the shared helper

## [0.28.1] - 2026-08-21

No library code changed in this release: every crate's shipped source is
identical to 0.28.0. The entries below cover the changelog itself, the live
test suite, and CI.

### 📚 Documentation

- *(changelog)* Backfill the missing 0.27.0 entry. The 0.27.0 bump rode along inside `chore(specs): sync the data spec and release 0.27.0` rather than a dedicated release commit, so the step that regenerates `CHANGELOG.md` was never run and the file jumped 0.28.0 -> 0.26.1 with nothing to signal it
- *(changelog)* Backfill six 0.28.0 entries. All six commits landed between the release commit and the tag, after the section had already been written — including a behavioural fix to `ParentEntityType`'s `Display` impl that shipped in 0.28.0 undocumented

### 🧪 Testing

- *(clob)* Select the order-test markets by price rather than asserting the precondition about an arbitrary one. `find_active_token_id` returns whichever open market gamma lists first, and gamma's ordering is stable, so a listing whose first market asked 0.042 failed `live_fak_unmatched_is_typed_error` on four consecutive runs with no way to retry out of it. The new `find_token_id_with_min_ask` filters on the `best_ask` gamma already returns in the listing, then confirms the candidate against the live CLOB book
- *(clob)* Read credentials from the OS keychain when the environment is unset. The live suites called `Account::from_env()` only, so credentials already held in the keyring did nothing for them, and bridging the gap by materialising a `.env` would put four secrets on disk to run a test suite. `load_account()` and `l1_account()` now try the environment, then `Account::from_keychain()` under the crate's non-default `keychain` feature, then panic as before — CI has no keychain and still falls through to the same auth-gated panics
- *(ci)* Assert that `CHANGELOG.md` and the workspace version agree. `release.yml` runs git-cliff only to compose the GitHub release body and never writes the file back, so the version bump and the changelog entry are two independent manual steps that nothing checked. The new `.github/scripts/tests/test_changelog.py` pins that the version in `Cargo.toml` has a dated section, that it heads the file, that sections stay strictly descending, and that every `[workspace.dependencies]` path pin moved with the workspace version

### ⚙️ Miscellaneous Tasks

- *(ci)* Teach the nightly failure classifier that a market-state refusal is environmental, not a defect. `ENVIRONMENTAL_RE` now also matches `no (?:qualifying|suitable) market`, the wording both the CLOB helper's give-up panic and the tests' own guards use — without it, wiring up the `POLYMARKET_*` secrets would file a tracking issue every night the listing happened to open cheap

## [0.28.0] - 2026-08-19

### 🚀 Features

- *(gamma)* Add `ParentEntityType` with the server's accepted values

### 🐛 Bug Fixes

- *(gamma)* [**breaking**] Model comments against the wire, not the fork's invention. `Comment`, `CommentReaction` and `CommentPosition` described a payload Polymarket has never sent; `CommentUser` had no upstream counterpart at all and is removed. Removed fields: `Comment::user`, `market_id`, `event_id`, `series_id`, `parent_id`, `positions`, `like_count`, `dislike_count`, `reply_count`; `CommentReaction::user_id`; `CommentPosition::outcome` and `shares`. Added: `Comment::parent_entity_type`, `parent_entity_id`, `parent_comment_id`, `user_address`, `reply_address`, `profile`, `report_count`, `reaction_count`; the new `CommentProfile`; and `CommentPosition::position_size`. Any code touching these types was already failing at runtime with a deserialization error
- *(gamma)* Comment endpoints no longer fail with `missing field 'userId'` on any response containing a reaction (#28)
- *(gamma)* [**breaking**] `GET /comments/{id}` returns a thread, not one comment — `Comments::get` now returns `Vec<Comment>`
- *(gamma)* [**breaking**] `ListComments::parent_entity_type` takes `ParentEntityType`, not a string — the server rejects `market` in either casing
- *(gamma)* `ParentEntityType`'s `Display` impl emitted `UNKNOWN` for the unknown variant while `Serialize` emitted `Unknown` for the same variant — `Display` now agrees with the wire vocabulary
- *(gamma)* [**breaking**] `Profile` described a payload `/profiles/user_address/{address}` has never sent — `id` was required and the server never sends it, so `Gamma::user().get_by_address` failed for every address. Rewritten against the endpoint's own published `PublicProfile.json` schema (linked from the response's `$schema` key) rather than `openapi.yaml`, whose `Profile` schema turns out to describe an unrelated object. Removed: `id`, `user`, `referral`, `created_by`, `updated_by`, `updated_at`, `utm_source`, `utm_medium`, `utm_campaign`, `utm_content`, `utm_term`, `wallet_activated`, `display_username_public`, `profile_image_optimized`, `is_close_only`, `is_cert_req`, `cert_req_date`. Added, now required: `taker_tier`, `taker_tier_name`, `weighted_volume` — sent on every response, and not in `openapi.yaml` at all
- *(cli)* [**breaking**] Drop the rejected `market` entity type, add `perps-asset`
- *(py)* [**breaking**] Align the comment bindings with the corrected types — `CommentUser` is removed and `CommentProfile` added, the comment getters renamed to match, and `GammaComments.get` now returns `list[Comment]`
- *(gamma)* [**breaking**] `UserResponse` and `UserInfo` described a payload `/public-profile` has never sent. Rewritten against the endpoint's own published `PublicProfileResponse.json` / `PublicProfileUser.json` schemas (linked from the response's `$schema` key) rather than invented fields. Removed: `UserResponse::address`, `UserResponse::id` — the wire never sends either; the account id lives at `users[].id` instead. Added, now required: `UserResponse::taker_tier`, `taker_tier_name`, `weighted_volume` — sent on every response. Added, optional: `UserResponse::discord_username`. `UserInfo::id` is now a required `String`, not `Option<String>` — the schema's only required property on that object. Added `UserInfo::community_mod`
- *(py)* [**breaking**] Align the `UserResponse` / `UserInfo` bindings with the corrected types — `address` and `id` are removed from `UserResponse`, `taker_tier`/`taker_tier_name`/`weighted_volume`/`discord_username` are added, and `UserInfo` gains `community_mod`
- *(gamma)* `SearchResponse::profiles` no longer fails the whole call with `invalid type: null, expected struct SearchProfile` — `/public-search` sends a JSON `null` for some entries in the array (reproducible on `q=sports&search_profiles=true&limit_per_type=20`, stable across 5 attempts), and the old `Vec<SearchProfile>` couldn't hold one
- *(gamma)* [**breaking**] `SearchProfile::address` described a payload `/public-search` has never sent (0 of 228 sampled profiles) — removed, along with `SearchResponse::profiles`'s element type changing to `Option<SearchProfile>` to tolerate the server's `null` entries. Added `SearchProfile::display_username_public`, sent on every sampled profile and previously unmodelled. Completes finding #10
- *(py)* [**breaking**] Align the `SearchProfile` bindings with the corrected type — `address` is removed, `display_username_public` is added

### 📚 Documentation

- *(gamma)* Update the README comments example for the typed filter
- Add `docs/specs/gamma/OBSERVED.md`, recording where gamma's published spec disagrees with gamma's server
- Record the gamma type parity sweep in `docs/plans/2026-08-19-gamma-type-parity-worklist.md`, cataloguing 15 further parity findings across the crate, including a second runtime failure of the same class (`Profile::id` is required and never sent)
- *(specs)* Sync perps, perps-ws, bridge and combos-rfq mirrors to upstream (#23, #20, #24, #21) — mirror-only, no client crate implements them
- Refresh the perps endpoint and channel counts in `CLAUDE.md` and both `INDEX.md` files after the mirror sync — it added 6 endpoints and 2 WebSocket channels the prose still under-counted
- *(gamma)* Record the wire-agreement guard's blind spot: an invented `Option<T>` field passes it, because the no-invented-fields direction must exempt null and cannot distinguish "optional and absent this time" from "does not exist"
- *(gamma)* Correct false and stale claims found by review — `/profiles/user_address/{address}` returns 200 rather than 404 for an address with no profile, `limit` bounds top-level comments rather than returned rows, and `comment_full.json`'s nested profile still lacks `pseudonym`
- *(gamma)* Record in `docs/specs/gamma/OBSERVED.md` that some endpoints publish a live, authoritative JSON Schema via a `$schema` response key — a better oracle than `openapi.yaml` where present
- *(gamma)* Record in `docs/plans/2026-08-19-gamma-type-parity-worklist.md` that finding #10 is partially fixed — `UserResponse`/`UserInfo` done, `SearchProfile::address` still open — and that `/public-search` serves no `$schema`
- *(gamma)* Record in `docs/specs/gamma/OBSERVED.md` and `docs/plans/2026-08-19-gamma-type-parity-worklist.md` that finding #10 is fully fixed, and add the `profiles[]` null-element failure as a new confirmed finding found while fixing it

### 🧪 Testing

- *(gamma)* Capture live comment payloads as fixtures
- *(gamma)* Add `tests/wire_agreement.rs`, asserting both directions of agreement between the comment types and captured live payloads
- *(gamma)* Fix the four live comment tests
- *(gamma)* Close the wire-agreement guard's null-exemption hole — every key a type emits must now be present on the wire or declared in `EXPECTED_ABSENT` with a reason, and array lengths must agree before zipping rather than comparing only `min(len)`
- *(py)* Guard the comment getters against silent `None` with a real instance — `hasattr` on the class object is true for every `py_type!` property regardless of which JSON key it resolves, so the guard moved to a Rust test that deserializes the shared fixture and asserts every getter is non-`None`
- *(gamma)* Capture live profile payloads as fixtures and extend `tests/wire_agreement.rs` to `Profile`
- *(gamma)* Stop `live_get_profile_by_address` from swallowing a deserialization error as if it were a 404
- *(gamma)* Capture live `/public-profile` payloads as fixtures and extend `tests/wire_agreement.rs` to `UserResponse`, including a hand-written case for the schema's explicit `null` on `users`
- *(gamma)* Capture live `/public-search` payloads as fixtures, including a response with a `null` `profiles[]` entry, and extend `tests/wire_agreement.rs` to `SearchProfile`/`SearchResponse`
- *(gamma)* Add a live test asserting `public_search(...).search_profiles(true)` succeeds — the regression that would have caught the `profiles[]` null failure

### 🎨 Styling

- Rustfmt the mock_api thread assertion

## [0.27.0] - 2026-08-15

### 🚀 Features

- *(ci)* Add canonical-tree walker producing JSON-pointer changes
- *(ci)* Report changed key paths in the drift summary
- *(ci)* Compose the issue body in Python under a tested size budget
- *(ci)* Fingerprint the canonical diff
- *(ci)* Exit 3 for drift we have accepted as permanent
- *(ci)* Put adopt and acknowledge commands in the issue body
- *(data)* [**breaking**] Add Position fee-basis fields, mark non_exhaustive
- *(py)* Expose Position fee-basis fields
- *(data)* Add typed Allowance for approval amounts
- *(data)* Add approval types for /v1/approvals
- *(data)* Add approvals namespace for /v1/approvals

### 🐛 Bug Fixes

- *(ci)* Treat integral floats as ints when canonicalizing specs
- *(ci)* Distinguish int from bool when walking canonical trees
- *(ci)* Identify drift issues by spec label, not tokenized title search
- *(ci)* Drop branch and PR machinery from the nightly drift check
- *(data)* Make deposit and withdrawal activity filters reachable

### 📚 Documentation

- *(specs)* Design for trustworthy schema drift findings
- *(plans)* Implementation plan for trustworthy schema drift findings
- Describe label-based drift issue identity in CLAUDE.md
- *(specs)* Design for drift findings that converge
- *(plans)* Implementation plan for drift convergence
- *(specs)* Design for absorbing the data spec drift
- *(plans)* Implementation plan for absorbing the data spec drift

### 🧪 Testing

- *(ci)* Make the bool/int branch-order guard mutation-sensitive

### ⚙️ Miscellaneous Tasks

- *(specs)* Acknowledge clob's upstream quoting regression
- *(specs)* Sync the data spec and release 0.27.0

## [0.26.1] - 2026-08-06

### 🐛 Bug Fixes

- *(core)* Stop rate limit buckets over-spending their published quota

### 🧪 Testing

- *(data)* Soak /closed-positions against the live rate limit
- *(data)* Bracket the real burst ceiling for /closed-positions

## [0.26.0] - 2026-08-06

### 🚀 Features

- *(core)* Make an observed 429 back off the whole client

### 🐛 Bug Fixes

- *(core)* Stop a zero Retry-After from collapsing the retry backoff

## [0.25.0] - 2026-08-05

### 🚀 Features

- *(clob)* Model the per-signer trading rate limits

### 🐛 Bug Fixes

- *(core)* Cap the health routes the hosts actually serve

## [0.24.2] - 2026-08-05

### 🐛 Bug Fixes

- *(clob)* Stop size truncation from shortening compliant amounts
- *(clob)* Apply the venue's per-tick decimal limits to limit orders
- *(clob)* Reject order quantities below the venue's 2-decimal floor

## [0.24.1] - 2026-08-03

### 🐛 Bug Fixes

- *(clob)* Apply the venue's per-tick decimal limits to market orders

### 📚 Documentation

- *(clob)* Stop linking public docs at pub(crate) precision helpers
- *(claude.md)* Document the rustdoc gate and correct the CI job count

## [0.24.0] - 2026-08-03

### 🚀 Features

- *(ci)* Classifier core for nextest failure categorization
- *(ci)* Nextest NDJSON parser in classify_failures.py
- *(ci)* CLI for classify_failures.py with classify and merge subcommands
- *(ci)* Canonicalize() for OpenAPI YAML drift comparison
- *(ci)* Structural drift detection for OpenAPI specs
- *(ci)* CLI for diff_openapi.py with check subcommand
- *(ci)* Channel-aware drift detection with per-entry vendored labels

### 🐛 Bug Fixes

- *(ci)* Classify live_ws panics and emit binary-aware retry filters
- *(ci)* Harden nightly workflows against review findings
- *(ci)* Disable rustdoc for the polyoxide-cli bin to stop the doc-build race
- *(ci)* Degrade gracefully when Actions may not open drift PRs
- *(data)* Pin holders `limit` clamping after upstream dropped the 400
- *(clob)* [**breaking**] Type Trade.owner as String, not Address
- *(clob)* [**breaking**] Type Notification.id as u64, not String

### 📚 Documentation

- Add design for nightly Polymarket API smoketest
- Add implementation plan for nightly API smoketest
- *(claude.md)* Document nightly API smoketest workflows
- Record nightly smoketest coverage extension
- Add SELF-HEALING.md describing the nightly drift machinery
- *(specs)* Refresh perps counts and guard them against future rot
- Use backticks for the org settings URL placeholder

### 🧪 Testing

- *(ci)* Add nextest output fixtures for classifier
- *(ci)* Add OpenAPI drift fixtures
- *(ci)* Cover diff_openapi exit code 2 on YAML parse error

### ⚙️ Miscellaneous Tasks

- *(ci)* Bootstrap python project for nightly helper scripts
- *(ci)* Commit scripts uv.lock for reproducibility
- *(ci)* Drop unused pytest import from classifier tests
- *(ci)* Drop unused TestOutcome import from classifier tests
- *(ci)* Consolidate diff_openapi imports at top of test file
- Run helper script unit tests on every PR
- Nightly behavioral smoketest workflow against live Polymarket APIs
- Nightly schema-drift workflow with auto-PR + tracking issue
- Cover clob live_ws and polyoxide-cli in the nightly behavioral matrix
- Extend nightly schema drift to all published Polymarket specs
- Bump pinned actions to their first Node 24-native majors
- *(specs)* Sync perps spec from upstream
- *(specs)* Sync perps-ws spec from upstream

## [0.23.0] - 2026-07-25

### 🚀 Features

- *(clob)* Document the TradeStatus lifecycle and add terminal-state helpers

### 🐛 Bug Fixes

- *(core)* Align the CLOB rate limiter with the published limits
- *(gamma)* [**breaking**] Return relationship rows, not tags, from related-tags
- *(data)* Add the missing ESPORTS leaderboard category
- *(clob)* Stop every WebSocket connect panicking on rustls provider ambiguity
- *(clob)* [**breaking**] Parse the sports frames the venue sends, and make the user channel usable
- *(gamma)* Tolerate the nulls the schema permits in RelatedTag

### 📚 Documentation

- *(plans)* Add the prader-audit upstream worklist
- *(data)* Correct the holders limit range to 0-500 default 20
- *(specs)* Record where upstream's sports contract diverges from the wire
- Fix broken intra-doc links across the workspace

### 🧪 Testing

- *(clob)* Add live WebSocket tests

### ⚙️ Miscellaneous Tasks

- Fail the build on rustdoc warnings

## [0.22.0] - 2026-07-25

### 🚀 Features

- *(clob)* Add ListOrders::send_raw as an escape hatch

### 🐛 Bug Fixes

- *(clob)* [**breaking**] Pin OpenOrder to the shape the venue actually returns
- *(clob)* Make live_v2_place_and_cancel rest instead of filling
- *(clob)* [**breaking**] Surface FAK/FOK kill outcomes as typed non-retriable errors
- *(core)* Retry 425 Too Early alongside 429

### 📚 Documentation

- Correct the inverted L1/L2 description in CLAUDE.md

## [0.21.0] - 2026-07-24

### 🐛 Bug Fixes

- *(clob)* [**breaking**] Correct the ClobAuth EIP-712 struct and auth domain
- *(clob)* Send a checksummed POLY_ADDRESS on L1 auth

### 🧪 Testing

- Close coverage gaps in the new WebSocket and sibling-host code
- *(clob)* Add a live L1 auth test

## [0.20.0] - 2026-07-24

### 🚀 Features

- *(clob)* [**breaking**] Add sports channel and the gated market WebSocket events
- *(data)* Add pnl and rankings namespaces on undocumented hosts

### ⚙️ Miscellaneous Tasks

- *(specs)* Mirror AsyncAPI specs and record undocumented hosts

## [0.19.0] - 2026-07-24

### 🚀 Features

- *(data)* Add combos and misc namespaces, close query param gaps
- *(clob)* [**breaking**] Add rewards multi/rebates, public rewards, cursor pagination
- *(gamma)* Expose missing keyset, search, and series query params

### 📚 Documentation

- Record new namespaces and the unimplemented upstream APIs

### ⚙️ Miscellaneous Tasks

- *(specs)* Refresh upstream specs and mirror perps, bridge, combos-rfq

## [0.18.2] - 2026-07-24

### 🐛 Bug Fixes

- *(clob)* Remove live_activity endpoint hitting nonexistent path
- *(data)* Tolerate unrecognized ActivityType/TradeSide values

## [0.18.1] - 2026-07-15

### 🚀 Features

- *(clob)* Add PricesHistoryQuery + prices_history_with for time-bounded history
- *(cli)* Scaffold clob prices download command group
- *(cli)* Prices download domain types (format, target, manifest)
- *(cli)* Prices download selection helpers (file parse, dedupe, token ids)
- *(cli)* Gamma-based market discovery for prices download
- *(cli)* Dataset writers (csv/jsonl) with atomic temp-then-rename
- *(cli)* Rate-limited, retrying single-market price fetch
- *(cli)* Write prices download run manifest as jsonl
- *(cli)* Orchestrate concurrent resumable prices download + manifest
- *(cli)* Feature-gated Parquet dataset writer

### 🐛 Bug Fixes

- *(cli)* Validate token ids for path safety; guard closed/open + fidelity

### 📚 Documentation

- Skip README doctests that require non-default features
- *(cli)* Design spec for clob prices download (bulk ML dataset)
- *(cli)* Implementation plan for clob prices download

### 🧪 Testing

- *(cli)* Live prices download test + document clob command group

## [0.18.0] - 2026-06-30

### 💥 Breaking Changes

- *(clob)* [**breaking**] **CLOB V2 migration.** Orders are now signed with the Polymarket CLOB V2 EIP-712 scheme (domain version "2", V2 exchange contracts, 11-field signed struct). V1-shaped orders are rejected by the live exchange as of 2026-04-28. `Order`/`SignedOrder` gained `timestamp`/`metadata`/`builder` and dropped `taker`/`nonce`/`feeRateBps` from the signed struct; fees are no longer signed (collected on-chain at match).
- *(clob)* [**breaking**] `SignatureType` gained `Poly1271` (EIP-1271; signing not yet implemented — rejected at the signing layer).
- *(clob)* [**breaking**] `create_builder_key` now uses L2 auth and returns `BuilderApiKeyResponse { key, secret, passphrase }`.

### 🚀 Features

- *(clob)* Builder-program attribution: `ClobBuilder::builder_code(B256)` stamps the signed `builder` field on every order.

### 🐛 Bug Fixes

- *(clob)* Mask the order `salt` to the JS-safe-integer range (2^53-1) so the live CLOB V2 exchange accepts orders. A raw 64-bit salt overflows the server's numeric parse and, since `salt` is part of the EIP-712 signed struct, corrupts the signature — orders were rejected with "Invalid order payload". Matches the official Polymarket clients; verified against the live exchange.

## [0.17.0] - 2026-06-28

### 🚀 Features

- *(clob)* [**breaking**] Remove dead RFQ trading API
- *(clob)* [**breaking**] Require builder_code arg for builder_trades

### 📚 Documentation

- *(specs)* Re-sync OpenAPI mirrors from upstream

### 🧪 Testing

- *(keychain)* Isolate tests in per-test keychain services

### ⚙️ Miscellaneous Tasks

- Add .env.example documenting auth credentials

## [0.16.0] - 2026-06-02

### 🐛 Bug Fixes

- *(gamma)* [**breaking**] Make Market.description optional for abridged markets
- *(clob)* [**breaking**] Align reads and schemas with the live API

### 📚 Documentation

- *(specs)* Refresh OpenAPI mirrors and endpoint docs from upstream

## [0.15.1] - 2026-05-22

### 🐛 Bug Fixes

- *(gamma)* Fix double-slash URL in health ping — `format!("{}/status", base_url)` produced `//status` because `Url::Display` normalizes to include a trailing `/`; switched to `base_url.join("status")?`

### 🧪 Testing

- *(clob, data, gamma)* Add per-crate ping mock tests covering the 200 happy path, unexpected 3xx as error, and 5xx as error

## [0.15.0] - 2026-04-23

### 💥 Breaking Changes

- *(gamma)* `markets().query_by_information()` and `markets().query_abridged()` now return request builders; add `.send().await` and pass the body by value instead of by reference

### 🚀 Features

- *(gamma)* Add `limit()` / `offset()` pagination to `query_by_information` and `query_abridged` builders, sent on the URL query string because the server ignores body-level pagination; defaults to `limit=1000` to prevent silent truncation at the server's 20-row default

### 🧪 Testing

- *(gamma)* Add mock coverage for explicit `limit` / `offset` on `query_by_information`

## [0.14.0] - 2026-04-22

### 🚀 Features

- *(core)* Add `HttpClient::get_bytes` helper for endpoints that return binary responses (e.g. ZIP archives)
- *(clob)* Add path-variant market metadata endpoints: `fee_rate_path`, `tick_size_path`, `neg_risk_path`
- *(clob)* Add `markets().clob_market_details(condition_id)` returning structured CLOB market metadata
- *(clob)* Add `markets().market_by_token(token_id)` for token→market lookup
- *(clob)* Add `markets().live_activity_bulk(ids)` and `live_activity_market(condition_id)` for real-time order/trade counters
- *(clob)* Add `markets().batch_prices_history(req)` for bulk historical price queries
- *(gamma)* Add events endpoints: `list_creators`, `get_creator`, `list_paginated`, `list_results`, `list_keyset`
- *(gamma)* Add markets endpoints: `get_description`, `query_by_information`, `query_abridged`, `list_keyset`
- *(gamma)* Add `series().get_summary`, `get_summary_by_slug`, and `comment_count`
- *(gamma)* Add `sports().get_team(id)` and `user().get_by_address(addr)`
- *(data)* Add `data.market_positions()` namespace with `ListMarketPositions` builder
- *(data)* Add `data.accounting().snapshot(user)` returning raw ZIP bytes
- *(relay)* Add `list_relayer_api_keys()` and `list_transactions()` methods with per-endpoint auth dispatch

### 💥 Breaking Changes

- *(clob)* `update_balance_allowance` now calls `PUT /balance-allowance` with query params; signature changed to `(asset_type, token_id: Option<_>, signature_type: Option<_>)`
- *(gamma)* `tags().get_related_detailed` now returns `Vec<Tag>` instead of a single `Tag`
- *(gamma)* `keep_closed_markets` is typed as an integer to match the upstream contract
- *(gamma)* Removed ghost `/events/slug/{slug}/related` endpoint that never existed upstream
- *(clob)* `GET /trades` now requires `maker_address` per upstream contract
- *(relay)* Response type fields aligned with upstream OpenAPI (some renames)

### 🐛 Bug Fixes

- *(clob)* Add `POST /heartbeats` endpoint for session keep-alive
- *(gamma)* Probe `/status` for health pings instead of a non-existent path
- *(data)* Add `MakerRebate` and `ReferralReward` activity variants

### 📚 Documentation

- *(specs)* Vendor upstream Polymarket OpenAPI YAMLs as the source of truth
- *(specs)* Sync per-endpoint markdown docs with upstream OpenAPI (CLOB, Gamma, Data, Relay)

### 🧪 Testing

- *(clob)* Add mock and live coverage for all new market endpoints and the PUT balance-allowance migration
- *(clob)* Add mock coverage for `heartbeat`
- *(gamma)* Add mock, live, and serde roundtrip coverage for all new events/markets/series/sports/user endpoints
- *(gamma)* Add mock and live coverage for `get_related_detailed` and `get_related_detailed_by_slug`
- *(data)* Add mock, live, and serde coverage for market-positions and accounting snapshot
- *(relay)* Add mock coverage for new endpoints including builder-HMAC vs static-key auth dispatch
- *(py)* Remove stale xfail markers on CLOB market tests

### 🎨 Styling

- *(gamma)* Apply rustfmt to example probes

## [0.13.1] - 2026-04-22

### 🚀 Features

- *(gamma)* Add `markets().get_many(ids)` for batch market lookup regardless of open/closed state

### 🐛 Bug Fixes

- *(gamma)* Work around the upstream `closed=false` default that silently dropped closed markets from `list().id()`, `.slug()`, and `.condition_ids()` lookups; new `get_many` helper fans out `closed=true` + `closed=false` requests in parallel and the trap is called out in the doc comments of the affected list builder methods

### 🚜 Refactor

- *(gamma)* Move Cloudflare probes from tests to examples

### 📚 Documentation

- *(gamma)* Document safe batch sizes on `query_many` methods

### 🧪 Testing

- *(gamma)* Add binary-search probe for batch-ID URL ceiling
- *(gamma)* Add burst probe for Cloudflare rate-limit responses

### ⚙️ Miscellaneous Tasks

- Ignore `.loom/` local data directory

## [0.13.0] - 2026-04-16

### 🚀 Features

- *(core)* Add `keychain` module for OS credential storage (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- *(core)* Add `keychain::delete` function for credential removal
- *(clob)* Add `Account::from_keychain()` and `Account::save_to_keychain()` for keychain-based credential loading
- *(clob)* Add `Account::delete_from_keychain()` to remove all stored credentials
- *(clob)* Add `ApiCredentials::from_keychain()` for WebSocket authentication
- *(relay)* Add `BuilderAccount::from_keychain()` and `from_keychain_relayer_api_key()` for keychain-based credential loading
- *(relay)* Add `BuilderAccount::delete_from_keychain()` to remove all stored credentials
- *(cli)* Add `credentials store` and `credentials show` subcommands for keychain management
- *(cli)* Add `credentials delete` subcommand to remove stored credentials
- *(cli)* Add `--credential-source keychain` flag to `ws user` command

### 🐛 Bug Fixes

- *(relay)* Clear stale passphrase from keychain when saving config with `passphrase = None`

### 🚜 Refactor

- Consolidate keychain service name strings into shared `KEYCHAIN_SERVICE` constants

### 🔧 Build

- Add `keyring` dependency with `apple-native`, `windows-native`, `async-secret-service`, and `crypto-rust` backends
- Add `keychain` feature flag to core, clob, relay, polyoxide, and cli crates

### 🧪 Testing

- *(clob)* Add keychain roundtrip and delete integration tests
- *(relay)* Add keychain roundtrip, no-config, and stale passphrase integration tests
- *(cli)* Add parsing tests for credentials delete and keychain credential source

## [0.12.5] - 2026-04-15

### 🚀 Features

- *(relay)* Add relayer API key authentication as alternative to HMAC builder credentials
- *(relay)* Add `AuthConfig` enum and `RelayerApiKeyConfig` for dual auth support
- *(relay)* Wire `AuthConfig` into request signing and add builder convenience methods

### 🚜 Refactor

- *(relay)* Encapsulate `RelayerApiKeyConfig` and validate inputs
- *(relay)* Remove deprecated `config()` and extract `parse_signer`

### 🧪 Testing

- *(relay)* Add relayer API key integration tests

### 📚 Documentation

- Rewrite all workspace READMEs with accurate code examples and full API coverage
- *(relay)* Add README with both auth methods, builder pattern, and gasless redemption examples
- Fix CLAUDE.md data API namespace, add MSRV, correct relay env vars

## [0.12.4] - 2026-04-14

### 🐛 Bug Fixes

- *(clob)* Limit order salt to u64 range to prevent serialization panic and API rejection

## [0.12.3] - 2026-04-14

### 🐛 Bug Fixes

- *(clob)* Serialize order salt as u128 number using serde_json `arbitrary_precision` instead of string encoding

## [0.12.2] - 2026-04-14

### 🐛 Bug Fixes

- *(clob)* Serialize order salt as string to avoid serde_json rejection of u128 values exceeding u64::MAX

## [0.12.1] - 2026-04-01

### 🐛 Bug Fixes

- *(clob)* Add missing `id` and `timestamp` fields to `Notification` struct
- *(clob)* Add `next_cursor` pagination support to `ListClobTrades` request builder

### 📚 Documentation

- Fix `transactionsHashes` field name in CLOB orders spec

### 💼 Other

- Add MIT/Apache-2.0 dual license and PyPI package metadata
- Skip already-published crates during crates.io release

## [0.12.0] - 2026-03-27

### ⚠️ Breaking Changes

- *(clob)* `BalanceAllowanceResponse.allowance: String` replaced with `allowances: HashMap<String, String>` to match upstream API change (#1)
- *(clob)* Several fields on `Market` changed to `Option<T>`: `question_id`, `minimum_order_size`, `minimum_tick_size`, `description`, `question` (#1)
- *(clob)* Several fields on `SpreadResponse` changed to `Option<T>`: `token_id`, `bid`, `ask` (#1)
- *(clob)* Several fields on `LastTradePriceResponse` changed to `Option<T>`: `token_id`, `last_trade_price`, `timestamp`; new optional fields `price`, `side` (#1)

### 🚀 Features

- *(core)* Add concurrency limiter to `HttpClient` for Cloudflare connection limits (#3)
- *(core)* Set default concurrency limits in all API client builders (#3)
- *(relay)* Hold concurrency permit across full request lifetime (#3)
- *(data)* Hold concurrency permit across full request lifetime (#3)
- *(py)* Scaffold `polyoxide-py` crate with PyO3 + maturin (#2)
- *(py)* Add Gamma, Data, and CLOB domain type wrappers via `py_type!` macro (#2)
- *(py)* Add Gamma, Data, and CLOB clients with sync + async variants (#2)
- *(py)* Add `.pyi` type stubs and `py.typed` marker (#4)
- *(py)* Export all type classes from polyoxide package (#4)

### 🐛 Bug Fixes

- *(clob)* Update `BalanceAllowanceResponse` for upstream API field change from `allowance` to `allowances` (#1)
- *(clob)* Make optional fields on `Market`, `SpreadResponse`, and `LastTradePriceResponse` to match upstream API (#1)
- *(clob)* Serialize order salt as string to avoid serde_json rejection of u128 values exceeding u64::MAX

### 🧪 Testing

- *(core)* Add concurrency limiter integration and default verification tests (#3)
- *(py)* Add live API tests and expand unit tests (#2)

### 📚 Documentation

- Add upstream Polymarket API specs for CLOB, Gamma, Data, and Relay
- Add design and implementation plans for PyPI publishing (#4)

### 🔧 CI

- Add Python bindings test job (#2)
- Add PyPI wheel build and publish to release workflow (#4)
- Filter release artifacts to exclude Python wheels (#4)

### ⚙️ Miscellaneous Tasks

- Add `*.so` and `.claude-squad` directory to `.gitignore`

## [0.11.0] - 2026-03-05

### 🚀 Features

- *(clob)* Add RFQ namespace for request-for-quote trading
- *(clob)* Add rewards namespace for liquidity reward tracking
- *(clob)* Add auth namespace for API key management
- *(clob)* Add batch order operations and single order lookup
- *(clob)* Add batch pricing endpoints for books, prices, midpoints, spreads, and last trades
- *(clob)* Add single pricing, live activity, calculate price, and server time endpoints
- *(clob)* Add heartbeat, notifications, order scoring, and ban status endpoints
- *(clob)* Add simplified/sampling market lists and builder trades endpoint
- *(clob)* Add `ListClobTrades` request builder with filter methods
- *(gamma)* Add missing endpoints and query params across all namespaces
- *(gamma)* Add public search endpoint for profiles, events, and tags
- *(gamma)* Complete `UserResponse` with profile, bio, and badge fields
- *(data)* Add trader leaderboard endpoint and move `TimePeriod` to types

### 🐛 Bug Fixes

- *(clob)* Fix 5 critical deserialization crashes against live API
- *(clob)* Add missing fields to `OpenOrder`, `OrderResponse`, and `Trade` types
- *(clob)* Add missing fields to `OrderBook` type
- *(gamma)* Correct 6 serde renames and expand `SeriesInfo` to match live API
- *(gamma)* Align SDK types with real Polymarket API responses
- *(data)* Add missing `verified` field to `Holder` type
- *(core)* Replace silent epoch fallback with explicit panic in `current_timestamp`
- *(cli)* Use `floor_char_boundary` for safe UTF-8 string truncation

### 🚜 Refactor

- *(clob)* Make gamma dependency optional behind `gamma` feature flag
- *(clob)* Extract WebSocket subscription validation helper
- *(clob)* Deduplicate EIP-712 order conversion and digest computation
- *(core)* Simplify rate limiter config with `endpoint_limit` helper
- *(relay)* Extract retry helper, named constants, and module-level types

### 🧪 Testing

- *(clob)* Add 73 new tests: WebSocket message types, utils, error, mock API (retry, errors, order creation), rejection and edge cases
- *(gamma)* Add mock tests for open() inversion, volume serde renames, and events namespace
- *(data)* Add 12 mock HTTP tests bootstrapping polyoxide-data coverage
- *(core)* Fix retry mock strictness and add 401/403/408 error tests
- *(cli)* Add multibyte and emoji edge case tests for truncate
- Add mockito HTTP mock tests for core, gamma, and clob

### 📚 Documentation

- Add docstrings across workspace, complete relay crate coverage
- Fix incorrect API examples and update project documentation

### 🔧 Build

- Add mockito workspace dev-dependency for HTTP mock tests
- Move futures-util from workspace deps to per-crate
- Specify per-crate tokio features instead of workspace-wide

### 🎨 Styling

- Apply rustfmt to gamma and data

## [0.10.0] - 2026-03-01

### ⚠️ Breaking Changes

- *(clob)* `WebSocketBuilder::market_url()` and `user_url()` now return `Result<Self, WebSocketError>` to enforce `wss://` scheme validation
- *(core)* MSRV raised from 1.75 to 1.91 (required by `str::floor_char_boundary`)
- *(core)* HTTP client now disables redirect following to prevent open redirect attacks

### 🐛 Bug Fixes

- *(relay)* Strip `0x` prefix from `PROXY_INIT_CODE_HASH` to prevent `hex::decode` panic at runtime
- *(core)* Truncate response bodies in error logs to 512 chars to prevent sensitive data leakage
- *(clob)* Truncate response bodies in error logs to 512 chars
- *(relay)* Truncate response bodies in error logs to 512 chars
- *(core)* Add 10-second connect timeout to HTTP client
- *(clob)* Enforce `wss://` scheme on WebSocket builder URLs to prevent plaintext connections

### 🛡️ Security

- *(clob)* Redact `private_key` in `AccountConfig` `Debug` impl to prevent secret leakage in logs
- *(relay)* Redact signer key in `BuilderAccount` `Debug` impl, showing only address
- Harden `.gitignore` to cover `.env.*`, `*.pem`, `*.key`, and `account.json`

### 🧪 Testing

- *(core)* Add tests for prefix collisions, concurrency, and retry edge cases
- *(core)* Add unit tests for `truncate_for_log` including multibyte boundary handling
- *(clob)* Add tests for WebSocket URL scheme validation
- *(clob)* Add test for `AccountConfig` Debug redaction
- *(relay)* Add test for `BuilderAccount` Debug redaction

### 🔧 CI

- Remove sccache and add lightweight ci profile
- Consolidate publish steps into retry loop
- Use cargo-nextest for parallel test execution
- Merge lint/test jobs and remove redundant release build

### 🎨 Styling

- Apply cargo fmt across workspace

## [0.9.2] - 2026-03-01

### 🚀 Features

- *(core)* Parse `Retry-After` header for server-guided backoff delays
- *(core)* Expose `RetryConfig` through all high-level client builders (`Clob`, `Gamma`, `DataApi`, `RelayClient`)

### 🐛 Bug Fixes

- *(core)* Add segment-boundary-aware endpoint matching to prevent `/price` from matching `/prices-history`
- *(core)* Replace `SystemTime` nanos with `fastrand` for uniform backoff jitter
- *(clob)* Generate fresh L1 auth timestamp on each retry to avoid staleness
- *(relay)* Add retry loops with 429 handling to all relay endpoints

## [0.9.1] - 2026-02-28

### ⚙️ Miscellaneous Tasks

- Prune unused deps, tokio/alloy features, and fix TLS duplication
- Apply rustfmt formatting across workspace

### 📚 Documentation

- Add testing conventions and module organization to CLAUDE.md

### 🔧 CI

- Replace rust-cache with sccache for shared compilation caching

## [0.9.0] - 2026-02-28

### 🚀 Features

- *(core)* Add per-endpoint rate limiting with configurable quotas, retry-on-429 backoff with jitter, and governor-based throttling

### 🐛 Bug Fixes

- *(core)* Fix rate limit quota precision, backoff jitter range, and add missing endpoint quota
- *(core)* Carry message context in RateLimit error variant and downgrade retry log level
- *(core)* Redact secrets from Debug impls to prevent log leakage
- *(clob)* Use BUY/SELL strings for price endpoint side parameter
- *(clob)* Use typed request for `get_fee_rate` with correct field and token_id
- *(clob)* Fix tautological assertion in salt test
- *(clob)* Reject NaN and infinity in order parameter validation
- *(clob)* Classify service errors as Api instead of Validation
- *(clob)* Return None on insufficient liquidity and increase salt entropy
- *(data)* Route all HTTP calls through Request<T> for rate limiting and 429 retries
- *(data)* Align Display impls with serde SCREAMING_SNAKE_CASE for sort enums
- *(relay)* Replace unwraps with error propagation and compile-time address validation
- *(cli)* Replace `process::exit` with Result-based error handling in WS credentials and completions
- *(cli)* Reject invalid activity types with error instead of silently dropping

### 🚜 Refactor

- *(core)* Make `Signer::new` infallible

### 🧪 Testing

- *(core)* Add unit tests for Request query builder and typed request
- *(clob)* Add unit tests for EIP-712 signing, WS types, and auth credentials
- *(clob)* Add live integration tests for CLOB public endpoints
- *(data)* Add unit tests for enum serialization, builders, and type serde
- *(data)* Add live integration tests for data API public endpoints
- *(gamma)* Add unit tests for type deserialization and client builder
- *(relay)* Add unit tests for types serde, address derivation, signature packing, hex constants, contract config, and builder defaults
- *(cli)* Add unit tests for argument parsing across all subcommands
- Add live integration tests for all API endpoints

## [0.8.1] - 2026-02-26

### 🚜 Refactor

- *(core)* Remove verbose request/response body logging from HTTP clients
- *(clob)* Remove verbose request/response body logging from HTTP clients
- *(relay)* Remove verbose request/response body logging and leftover `eprintln!` debug statements

## [0.8.0] - 2026-02-25

### 🚀 Features

- Migrate price and size fields from String to Decimal with `serde(with = "rust_decimal::serde::str")` for accurate serialization

## [0.7.1] - 2026-02-24

### 🐛 Bug Fixes

- *(clob)* Add `canceled_order_id` and `message` fields to `CancelResponse` and mark `success` as default.

## [0.7.0] - 2026-02-24

### 🚀 Features

- *(relay)* Update builder to default to Polygon Mainnet (137) and relay V2 (`https://relayer-v2.polymarket.com/`)
- *(relay)* Update `RelayClientBuilder` to implement `Default`

## [0.6.1] - 2026-02-20

### 🚀 Features

- *(core)* Add unified authentication module with HMAC signing and timestamp generation
- *(core)* Add `Signer` struct supporting multiple base64 formats (URL-safe and standard)
- *(core)* Add `current_timestamp()` function for safe Unix timestamp generation
- *(core)* Add `Base64Format` enum to support both URL-safe and standard base64 encoding
- *(core)* Add `impl_api_error_conversions!` macro to reduce error conversion boilerplate

### 🚜 Refactor

- *(core)* Consolidate HMAC signing logic from CLOB and Relay into shared `Signer` implementation
- *(core)* Consolidate timestamp generation into single safe implementation
- *(clob)* Refactor `Signer` to use `polyoxide_core::Signer` as thin wrapper with CLOB-specific error handling
- *(clob)* Extract market metadata fetching into `get_market_metadata()` helper method
- *(clob)* Extract fee rate fetching into `get_fee_rate()` helper method
- *(clob)* Extract maker address resolution into `resolve_maker_address()` helper method
- *(clob)* Extract order building into `build_order()` helper method
- *(clob)* Simplify `create_order()` and `create_market_order()` by using extracted helpers (~140 lines removed)
- *(relay)* Update to use `polyoxide_core::Signer` and `current_timestamp()` for authentication
- *(gamma)* Use `impl_api_error_conversions!` macro to reduce error conversion boilerplate
- *(data)* Use `impl_api_error_conversions!` macro to reduce error conversion boilerplate

## [0.6.0] - 2026-02-19

### 🚀 Features

- *(relay)* Add gas estimation for redemption transactions with safety buffer and relayer overhead
- *(relay)* Add `estimate_redemption_gas` method to estimate gas costs using RPC provider simulation
- *(relay)* Add `submit_gasless_redemption_with_gas_estimation` method for redemptions with optional gas estimation
- *(relay)* Add default RPC URLs to contract configuration for Polygon mainnet and Amoy testnet
- *(repo)* Rename project from `polyte` to `polyoxide`

## [0.5.0] - 2026-02-19

### 🚀 Features

- *(clob)* Add health API namespace with ping method
- *(relay)* Introduce `polyte-relay` crate for interacting with relayer services
- *(relay)* Add gasless redemption functionality via relayer v2 API
- *(relay)* Introduce `BuilderAccount` for centralized signer and config management
- *(clob)* Introduce `MarketOrderArgs` and market order calculation utilities
- *(clob)* Enhance order creation logic with maker address determination and optional funder parameter
- *(clob)* Integrate polyte-gamma client into Clob and ClobBuilder
- *(clob)* Add `neg_risk` and `tick_size` methods to markets search
- *(clob)* Add `neg_risk` support for orders
- *(clob)* Implement funder and signature type support
- *(clob)* Add `get_by_token_ids` method to retrieve markets by token IDs
- *(clob)* Add `prices_history` method for historical token prices
- *(clob)* Add Display impl for OrderKind and SignatureType
- *(clob)* Introduce PartialCreateOrderOptions for enhanced order creation flexibility
- *(gamma)* Introduce Gamma User API
- *(gamma)* Add `volume_1yr` field to match Gamma API naming conventions
- *(data)* Add USDC balance endpoint to account API
- *(data)* Update `BalanceAllowanceResponse` to use HashMap for allowances
- *(polyte)* Add DataApi to unified Polymarket client
- *(types)* Add `is_proxy` method to `SignatureType` enum
- *(error)* Add service error creation method to ClobError

### 🐛 Bug Fixes

- *(clob)* Use precise decimal arithmetic and explicit TickSize parsing
- *(clob)* Update order amount calculations to support 6 decimal places
- *(clob)* Update owner field in order payload to use account address
- *(clob)* Add custom deserialization for minimum_tick_size to handle both string and number formats
- *(gamma)* Correct typos in Market and Event field names
- *(error)* Enhance API error logging by capturing raw response body
- *(tests)* Update salt generation test to check for non-empty output

### 🚜 Refactor

- *(clob)* Serialize OrderSide enum variants as 'BUY' and 'SELL' strings
- *(clob)* Update ClobBuilder to use optional account and introduce with_account method
- *(clob)* Restructure EIP-712 domain and order definitions into protocol module
- *(clob)* Implement custom serialization and deserialization for `SignatureType` enum
- *(gamma)* Rename `active` filter to `open` for market and series listing
- *(gamma)* Rename user proxy field to `proxyWallet` in API response
- *(gamma)* Rename `wallet_address` query parameter to `address` in public profile API
- *(core)* Add shared HTTP client infrastructure
- Remove Result type aliases in favor of explicit types
- Refactor amount calculations to use f64 arithmetic

### ⚙️ Miscellaneous Tasks

- Add CLAUDE.md with project guidance and architecture overview
- Update `thiserror` dependency to version 2.0.17
- Add `specta` support in multiple modules
- Add `dotenvy` dependency

## [0.4.0] - 2026-01-05

### 🐛 Bug Fixes

- *(clob)* Correct the type of the OrderBook timestamp

### ⚙️ Miscellaneous Tasks

- Add changelog and publish it on Github Releases page

## [cli-v0.3.2] - 2025-12-04

### 🐛 Bug Fixes

- *(cli)* Use limit flag instead of hardcorded value
- *(gamma)* Typo

### 🚜 Refactor

- *(cli)* Move duplicates into `common` module
- Use clap `value_parser` for comma-separated arguments

### ⚙️ Miscellaneous Tasks

- Format
- Remove unnecessary doc

## [cli-v0.3.1] - 2025-12-04

### 🚜 Refactor

- *(cli)* Improve credential error messages for `ws user` command

## [cli-v0.3.0] - 2025-12-03

### 🚀 Features

- *(clob)* Add websocket support
- *(cli)* Add support for Clob websockets

### 🚜 Refactor

- Consolidate auth into account module

### 📚 Documentation

- Update Clob documentation

### ⚙️ Miscellaneous Tasks

- Remove clob examples

## [cli-v0.2.4] - 2025-12-01

### 🐛 Bug Fixes

- Change `comment_count` type from u32 to i64 to prevent sentinel value issues

### 🚜 Refactor

- Extract common Request builder to `polyte-core`

### 📚 Documentation

- Update CLI README
- Update `polyte` README

### ⚙️ Miscellaneous Tasks

- Remove gamma examples
- Update Event type in Gamma

## [cli-v0.2.1] - 2025-12-01

### 🚀 Features

- Add support for Builders API

### 📚 Documentation

- Fix typo

## [cli-v0.2.0] - 2025-11-30

### 🚀 Features

- Add support for Data API

### 🚜 Refactor

- Remove deprecated code
- Reuse `SortOrder` enum

## [cli-v0.1.5] - 2025-11-28

### 🐛 Bug Fixes

- *(gamma)* Change `order_min_price_tick_size` and `order_min_size` to `f64`

### 🚜 Refactor

- *(cli)* Chain builder methods for request construction

## [cli-v0.1.4] - 2025-11-28

### 🚀 Features

- Bump versions
- Release cli-v0.1.4

### 🐛 Bug Fixes

- Clean-up types and make them more exhaustive
- Typo

### ⚙️ Miscellaneous Tasks

- *(cli)* Set default values to flags
- Enable retrieving a market by its slug

## [cli-v0.1.3] - 2025-11-28

### 🚀 Features

- Add cli commands presets and more flags

### 🐛 Bug Fixes

- Deserialize API responses into correct structs

### ⚙️ Miscellaneous Tasks

- Run `cargo fmt`

## [cli-v0.1.2] - 2025-11-27

### 🚀 Features

- *(cli)* Add command to display CLI version

### ⚙️ Miscellaneous Tasks

- Add more unit tests for utils

## [cli-v0.1.1] - 2025-11-27

### 🚀 Features

- Enable generating shell completions

## [cli-v0.1.0] - 2025-11-27

### 🚀 Features

- Add cli

### 📚 Documentation

- Add links to crates documentation
- Say it's wip in README

### ⚙️ Miscellaneous Tasks

- Make Polymarket client clonable
- Bump deps
- Bump `alloy` to latest and move it clob crate
- Add install script and workflow to release binaries on Github Releases
- Fix release workflow
