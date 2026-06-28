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

### 🧪 Tests

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

### ♻️ Refactoring

- Consolidate keychain service name strings into shared `KEYCHAIN_SERVICE` constants

### ⚙️ Build

- Add `keyring` dependency with `apple-native`, `windows-native`, `async-secret-service`, and `crypto-rust` backends
- Add `keychain` feature flag to core, clob, relay, polyoxide, and cli crates

### 🧪 Tests

- *(clob)* Add keychain roundtrip and delete integration tests
- *(relay)* Add keychain roundtrip, no-config, and stale passphrase integration tests
- *(cli)* Add parsing tests for credentials delete and keychain credential source

## [0.12.5] - 2026-04-15

### 🚀 Features

- *(relay)* Add relayer API key authentication as alternative to HMAC builder credentials
- *(relay)* Add `AuthConfig` enum and `RelayerApiKeyConfig` for dual auth support
- *(relay)* Wire `AuthConfig` into request signing and add builder convenience methods

### ♻️ Refactoring

- *(relay)* Encapsulate `RelayerApiKeyConfig` and validate inputs
- *(relay)* Remove deprecated `config()` and extract `parse_signer`

### 🧪 Tests

- *(relay)* Add relayer API key integration tests

### 📝 Documentation

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

### 📝 Documentation

- Fix `transactionsHashes` field name in CLOB orders spec

### 🔧 Other

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

### 🧪 Tests

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

### 🧪 Tests

- *(clob)* Add 73 new tests: WebSocket message types, utils, error, mock API (retry, errors, order creation), rejection and edge cases
- *(gamma)* Add mock tests for open() inversion, volume serde renames, and events namespace
- *(data)* Add 12 mock HTTP tests bootstrapping polyoxide-data coverage
- *(core)* Fix retry mock strictness and add 401/403/408 error tests
- *(cli)* Add multibyte and emoji edge case tests for truncate
- Add mockito HTTP mock tests for core, gamma, and clob

### 📚 Documentation

- Add docstrings across workspace, complete relay crate coverage
- Fix incorrect API examples and update project documentation

### ⚙️ Build

- Add mockito workspace dev-dependency for HTTP mock tests
- Move futures-util from workspace deps to per-crate
- Specify per-crate tokio features instead of workspace-wide

### 💅 Style

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

### 🔒 Security

- *(clob)* Redact `private_key` in `AccountConfig` `Debug` impl to prevent secret leakage in logs
- *(relay)* Redact signer key in `BuilderAccount` `Debug` impl, showing only address
- Harden `.gitignore` to cover `.env.*`, `*.pem`, `*.key`, and `account.json`

### 🧪 Tests

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

### 💅 Style

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

### 🧪 Tests

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
