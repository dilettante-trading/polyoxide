---
paths:
  - "polyoxide-core/src/{config,macros,macro_tests,query,request}.rs"
  - "polyoxide-*/src/**/api/**"
  - "polyoxide-*/src/**/client.rs"
  - "polyoxide-binance/src/usdm/{mod,request}.rs"
  - "polyoxide-kalshi/src/predictions/{mod,request,tickers}.rs"
  - "**/tests/query_setters_*.rs"
---

# Clients, namespaces and query setters

## Builders

- Every client is built: `ClobBuilder::new()`, `Clob::builder(private_key, credentials)`,
  `Gamma::builder()`, `DataApi::builder()`, `Perps::builder()`, `Usdm::builder()`,
  `Predictions::builder()`, `RelayClient::default_builder()`, `Polymarket::builder(account)`.
- Each venue builder holds a `polyoxide_core::ClientConfig` (base URL, timeout, pool size, retry
  config, concurrency, and the client's default concurrency: relay 2; gamma, data, perps, Binance
  and Kalshi 4; clob 8) and gets its five transport setters, `base_url`, `timeout_ms`,
  `pool_size`, `with_retry_config` and `max_concurrent`, from
  `polyoxide_core::client_config_setters!`. Its `build` starts from `ClientConfig::http_builder()`
  and adds the throttle and retry policy the venue installs; the config sets no `gzip`.
- A venue's own knobs stay on its builder:
  - data: `pnl_base_url`, `rankings_base_url`;
  - perps: `with_rate_limiter`;
  - Binance: `weight_budget`;
  - Kalshi: `credentials` (the key id and private key that sign a keyed client's requests; a
    client built without them sends unsigned ones);
  - relay: `url`, `chain_id`, and its account, auth and wallet setters. `url(&str) -> Result`
    checks the URL at once, beside `base_url`; `build` adds the trailing slash either way, so a
    path prefix is kept;
  - clob: `chain`, `signature_type`, `builder_code`, `with_account`; `gamma` (built only with the
    `gamma` feature, which `clob` does not imply); `data` (built only with `data`, for the `Trades`
    the CLOB reads through the Data API; `build` makes one with the builder's timeout and pool
    size when none is given, and the unified client hands clob its own); `user_socket` (built
    only with `clob-ws`: the URL and timing of the supervised user socket each
    `Trading::events` stream opens).
- `polyoxide_polymarket::Polymarket`, the unified client (built with `clob`, `gamma` and `data`),
  builds one CLOB, Gamma and Data client from one `Account`, and the CLOB client reads through
  that same Gamma and Data client. `Polymarket::new` and `PolymarketBuilder::build` return a
  `BuildError` whose `Clob`, `Gamma` and `Data` variants wrap the error of the client that could
  not be built and classify as it does. The umbrella re-exports it as
  `polyoxide::polymarket::Polymarket` and from its prelude.

## Namespaces

Each venue's namespace list is in its rules file. An accessor that only clones client fields into
its namespace comes from `polyoxide_core::namespaces!`, invoked in the client's `impl` with the
fields every namespace takes (`http_client;`, Binance's `http;`):
`pnl: PnlApi { http_client: pnl_http_client }` maps one field from another, and attributes such
as `approvals`' `#[deprecated]` pass through. The namespace structs stay declared by hand, since
their paths are public.

Eight accessors are hand-written because they do more than clone: data's `user(addr)`, its alias
`positions` and the `traded` wrapper, and clob's account-gated `orders`, `account_api`,
`notifications`, `rewards` and `auth`, each projecting its own part of the account and failing
with its own message.

## Query setters

Query parameters are chained with builder methods before `.send().await?`
(`gamma.markets().list().open(true).send().await?`, `data.leaderboard().get().send().await?`).
The setters come from `polyoxide_core::query_setters!`, one entry per setter keeping its doc
comment, name, argument type and key:

- `name: T => "key"`, `name: impl Into<String>`, `name: many T` (repeats the key), `name: csv T`
  and `name: csv<I, S>` (comma-joined through `polyoxide_core::csv`, omitted when the joined value
  is empty), `name(arg: T) => "key" = expr`, `name(arg: T) => "key" if cond` and
  `name(arg: T) => csv "key" = expr`.
- Each writes `self.request` (core's `Request`), or the field a leading `self.<field>;` names (data
  v2's paged builders use `self.inner;`). A field with an inherent `query`, such as Binance's
  private `Routed<T>`, which replaces a repeated key, keeps it.

Ten setters stay hand-written because they are not single query writes:

- Binance's `GetKlines::limit` and `GetDepth::limit`, which also re-price the route;
- gamma's `GetManyMarkets::include_tag` and the `limit` and `offset` of `QueryByInformation` and
  `QueryAbridged`, which store fields `send` writes;
- Kalshi's `tickers` of `ListMarkets`, `ListMarketWindow` and `ListEvents`, which refuse an empty
  set or one past `TICKERS_QUERY_BUDGET` before anything is sent (`kalshi.md`).

`tests/query_setters_usdm.rs` (Binance), `tests/query_setters_clob.rs`,
`tests/query_setters_gamma.rs` and `tests/query_setters_data.rs` (`polyoxide-polymarket`) and
`tests/query_setters_predictions.rs` (Kalshi) call every setter with a typed value and pin the
exact ordered pairs it sends; a new setter gets a row there.
