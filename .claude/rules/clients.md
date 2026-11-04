---
paths:
  - "polyoxide-core/src/{config,macros,macro_tests,query,request}.rs"
  - "polyoxide-*/src/**/api/**"
  - "polyoxide-*/src/**/client.rs"
  - "polyoxide-binance/src/usdm/{mod,request}.rs"
  - "polyoxide-binance/src/shared/request.rs"
  - "polyoxide-binance/src/spot/{mod,request}.rs"
  - "polyoxide-kalshi/src/predictions/{mod,request,tickers}.rs"
  - "**/tests/query_setters_*.rs"
---

# Clients, namespaces and query setters

## Builders

- Every client is built: `ClobBuilder::new()`, `Clob::builder(private_key, credentials)`,
  `Gamma::builder()`, `DataApi::builder()`, `Perps::builder()`, `Usdm::builder()`,
  `Spot::builder()`, `Predictions::builder()`, `RelayClient::default_builder()`,
  `Polymarket::builder(account)`.
- Each venue builder holds a `polyoxide_core::ClientConfig` (base URL, timeout, pool size, retry
  config, concurrency, runtime, transport, and the client's default concurrency: relay 2; gamma,
  data, perps, Binance and Kalshi 4; clob 8) and gets its transport setters, `base_url`,
  `timeout_ms`, `pool_size`, `with_retry_config`, `max_concurrent`, `runtime` and `http_transport`,
  from `polyoxide_core::client_config_setters!`; `runtime` names `polyoxide_rt::Runtime` through
  core's doc-hidden `__rt` re-export, so a crate invoking the macro needs no `polyoxide-rt`
  dependency for it. Its `build` resolves the runtime once with `ClientConfig::runtime_or_default()`,
  starts from `ClientConfig::http_builder()` and gives it `HttpClientBuilder::runtime`, and adds the
  throttle (built on the same runtime) and retry policy the venue installs; the config sets no
  `gzip`.
- `http_transport(..)`, from the same macro, injects the `Arc<dyn HttpTransport>` every attempt is
  sent on; with none set, `build` makes `ReqwestTransport` on a host target and `FetchTransport` on
  `wasm32` (`send-loop.md`), and `pool_size` and the client's `gzip` feed only the host default.
- Each socket builder (perps, Binance's `usdm` and `spot`, the CLOB's bare, market and user
  sockets, sports, rtds, Kalshi) gets `runtime(..)` and `socket_transport(..)` from `polyoxide_ws::transport_setters!`,
  beside `timing_setters!` (`transport_setters!` names the runtime through `polyoxide_ws`'s
  doc-hidden `__rt`, as core's macro does); its `build` falls back to `polyoxide_rt::default_runtime()` and the
  target's default transport (the CLOB's, on a host target: `TungsteniteTransport::per_address()`).
  Nothing is installed process-wide.
- A venue's own knobs stay on its builder:
  - perps: `with_rate_limiter`;
  - Binance: `weight_budget`;
  - Kalshi: `credentials` (the key id and private key that sign a keyed client's requests; a
    client built without them sends unsigned ones);
  - relay: `url`, `chain_id`, and its account, auth and wallet setters. `url(&str) -> Result`
    checks the URL at once, beside `base_url`; `build` adds the trailing slash either way, so a
    path prefix is kept;
  - clob: `chain`, `signature_type`, `builder_code`, `with_account`; `gamma` (built only with the
    `gamma` feature, which `clob` does not imply); `data` (built only with `data`, for the `Trades`
    the CLOB reads through the Data API; `build` makes one, and the Gamma one, with the builder's
    timeout, pool size, runtime and transport when none is given, and the unified client hands clob
    its own); `user_socket` (built only with `clob-ws`: the URL, timing, `runtime` and
    `socket_transport` of the supervised user socket each `Trading::events` stream opens; `build`
    gives it the client's runtime when the socket builder has none, and a runtime set on the socket
    builder wins).
- `polyoxide_polymarket::Polymarket`, the unified client (built with `clob`, `gamma` and `data`),
  builds one CLOB, Gamma and Data client from one `Account`, and the CLOB client reads through
  that same Gamma and Data client. `PolymarketBuilder` has `runtime(..)` and `http_transport(..)`
  too, set on all three clients it builds (and so on the signer buckets the CLOB holds).
  `Polymarket::new` and `PolymarketBuilder::build` return a
  `BuildError` whose `Clob`, `Gamma` and `Data` variants wrap the error of the client that could
  not be built and classify as it does. The umbrella re-exports it as
  `polyoxide::polymarket::Polymarket` and from its prelude.

## Namespaces

Each venue's namespace list is in its rules file. An accessor that only clones client fields into
its namespace comes from `polyoxide_core::namespaces!`, invoked in the client's `impl` with the
fields every namespace takes (`http_client;`, Binance's `http;`):
`name: Type { field: other_field }` maps one field from another, and attributes such as
`#[deprecated]` pass through. The namespace structs stay declared by hand, since
their paths are public.

Five accessors are hand-written because they do more than clone: clob's account-gated `orders`,
`account_api`, `notifications`, `rewards` and `auth`, each projecting its own part of the account
and failing with its own message.

## Query setters

Query parameters are chained with builder methods before `.send().await?`
(`gamma.markets().list().open(true).send().await?`, `data.leaderboard().send().await?`).
The setters come from `polyoxide_core::query_setters!`, one entry per setter keeping its doc
comment, name, argument type and key:

- `name: T => "key"`, `name: impl Into<String>`, `name: many T` (repeats the key), `name: csv T`
  and `name: csv<I, S>` (comma-joined through `polyoxide_core::csv`, omitted when the joined value
  is empty), `name(arg: T) => "key" = expr`, `name(arg: T) => "key" if cond` and
  `name(arg: T) => csv "key" = expr`.
- Each writes `self.request` (core's `Request`), or the field a leading `self.<field>;` names (data
  v2's paged builders use `self.inner;`). A field with an inherent `query`, such as Binance's
  private `Routed<T>`, which replaces a repeated key, keeps it.

Eleven setters stay hand-written because they are not single query writes:

- Binance's `GetKlines::limit` and `GetDepth::limit`, and spot's
  `spot::api::market::GetDepth::limit`, which also re-price the route (spot's
  `GetKlines::limit` does not, since a spot `klines` costs 2 at any limit);
- gamma's `GetManyMarkets::include_tag` and the `limit` and `offset` of `QueryByInformation` and
  `QueryAbridged`, which store fields `send` writes;
- Kalshi's `tickers` of `ListMarkets`, `ListMarketWindow` and `ListEvents`, which refuse an empty
  set or one past `TICKERS_QUERY_BUDGET` before anything is sent (`kalshi.md`).

`tests/query_setters_usdm.rs` and `tests/query_setters_spot.rs` (Binance),
`tests/query_setters_clob.rs`, `tests/query_setters_gamma.rs` and `tests/query_setters_data.rs`
(`polyoxide-polymarket`) and `tests/query_setters_predictions.rs` (Kalshi) call every setter with a
typed value and pin the exact ordered pairs it sends; a new setter gets a row there.
