# Review: verify-current lens on the spine update

- **Date:** 2026-10-08
- **Scope:** the facts that the amended AD-10, AD-13, AD-15, AD-16, AD-22 and AD-25 rely on. These are the last eight memlog entries.
- **Method:** each fact was checked against the workspace (`Cargo.lock`, the registry sources the lockfile resolves, and the code), the published crate source, live hosts (Kalshi and crates.io, probed with curl and a User-Agent), and vendor documentation (docs.kalshi.com, docs.github.com, and crates.io's own source).

## Verdict

Every named technology exists and fits. Every flag and endpoint the amendments name is real and current. Four amendments are worded on top of a wrong or partly wrong assumption about how the thing behaves:

- AD-15 assumes a TLS EOF is a `Tls` error.
- AD-22 says "it skips crates absent from the baseline", which is not something the tool does.
- AD-13 adds secret wiring that interacts badly with empty secrets.
- AD-22 pairs the pinned semver-checks version with floating stable Rust.

None of these needs the design to change. Each needs one sentence corrected before the stories built on it start. **Ready, with 4 medium fixes and 1 low.**

## Items

### 1. tungstenite error variants and TLS EOF (AD-15)

| Claim | Status | Evidence |
| --- | --- | --- |
| The workspace uses tokio-tungstenite 0.26 | **verified** | `Cargo.lock:5791-5792` and `5977-5978` resolve `tokio-tungstenite 0.26.2` and `tungstenite 0.26.2`; workspace `Cargo.toml:49` declares `0.26`. |
| `Url`, `HttpFormat`, `AttackAttempt` and `Tls` are `tungstenite::Error` variants | **verified** | `tungstenite-0.26.2/src/error.rs:47` `Tls(#[from] TlsError)`, `:64` `AttackAttempt`, `:67` `Url(#[from] UrlError)`, `:75` `HttpFormat(#[from] http::Error)` (cfg `handshake`). The variant names are unchanged in 0.29.0 and 0.30.0 (released 2026-07-11, the current major, which the spine defers moving to). 0.30 only boxes `Http` and `WriteBufferFull`. |
| `Url`, `HttpFormat` and `AttackAttempt` are client-side, so `InvalidRequest` | **verified** for the async path | Every `UrlError` that tokio-tungstenite produces is client-side: `UnsupportedUrlScheme` (`connect.rs:88`), `NoHostName` (`lib.rs:405`) and `TlsFeatureNotEnabled` (`tls.rs:163`). The one network-like variant, `UrlError::UnableToConnect`, is produced only by the sync `tungstenite::client::connect` (`client.rs:137`). The async path reports a TCP failure as `Error::Io` (`connect.rs:91`). The rule codifies today's `Recovery::Fatal` arm in `polyoxide-binance/src/usdm/ws/error.rs:131`. |
| "a TLS EOF is `Network`; non-EOF `Tls` errors are `InvalidRequest`" | **wrong as worded** | A TLS EOF is never a `Tls` variant. rustls 0.23.38 (the version in the lock) returns `io::ErrorKind::UnexpectedEof` with "peer closed connection without sending TLS close_notify" (`rustls-0.23.38/src/conn.rs:187-190`). tungstenite wraps that as `Error::Io`. The repo's own fixture shows exactly this: `.github/scripts/tests/test_classify_failures.py:227-230` reads "IO error: peer closed connection without sending TLS close_notify". Handshake failures, certificate errors included, also arrive as `Error::Io` (`tokio-tungstenite-0.26.2/src/tls.rs:139`). Under rustls the only `Tls` value ever produced is `TlsError::InvalidDnsName` (`tls.rs:133`). No `native-tls` is in the lock. So "non-EOF Tls" is vacuous, and the EOF case has to be matched as `Io` with `kind() == UnexpectedEof`. A table that sends `Io` to `Network` gets the outcome right, but a mutation test written from the AD's wording cannot construct its input. A certificate failure lands in `Network` and is retried, which matches today's `Reconnect`. |
| The handshake's HTTP status | **gap (low)** | `Error::Http(Response)` (`error.rs:71`) carries a 403, 429 or 451 from the upgrade request. The `WsError` table in AD-15 does not say that it goes through the status map. Today's Binance and perps code does send it there (`usdm/ws/error.rs:123-129`). |

### 2. Kalshi's authenticated limits endpoint (AD-10)

| Claim | Status | Evidence |
| --- | --- | --- |
| `GET /account/limits` returns read and write `refill_rate` and `bucket_capacity` | **verified, current** | The Kalshi OpenAPI downloaded 2026-10-08 from `https://docs.kalshi.com/openapi.yaml` is `info.version 3.34.0`. Line 2823 has `/account/limits` with operationId `GetAccountApiLimits` and `kalshiAccessKey/Signature/Timestamp` security. Line 5209 has `GetAccountApiLimitsResponse { usage_tier, read: BucketLimit, write: BucketLimit, grants }` as required fields. Line 5185 has `BucketLimit { refill_rate: integer, bucket_capacity: integer }` ("tokens per second"; "maximum tokens"). `docs.kalshi.com/getting_started/rate_limits.md` shows the same JSON example. |
| The endpoint requires authentication | **verified live** | `GET https://api.elections.kalshi.com/trade-api/v2/account/limits` without credentials returns **401**. |
| The unauthenticated fallback is provisional until a soak confirms it | **verified (the right call)** | The rate-limits page says "Every authenticated request costs tokens". It documents no limit for unauthenticated requests, so a fallback has nothing published to copy. |
| The buckets are sized from this endpoint | **verified, incomplete** | (a) The other half of the model, the cost of each request, is served live by `GET /account/endpoint_costs` (OpenAPI line 2897, no `security`). A live call returned `default_cost: 10` plus 17 non-default rows, such as `DELETE .../portfolio/events/orders/:order_id` at 2 and `GET /cfbenchmarks` at 50. The docs call it "the authoritative list". AD-10 has `costs` come from the venue's route table, so that table has to be filled from, or tested against, this endpoint. (b) Writes have a Write bucket per shard, and auto-routed orders are "billed to every shard's Write bucket". `/account/limits` reports a single `write`. (c) A 429 carries no `Retry-After`, and "there is no penalty or cooldown", so Kalshi's policy should not inherit the client-wide 429 hold. AD-9 already leaves that choice to each venue's policy. |

### 3. crates.io API: names and versions not yet published (AD-25)

| Claim | Status | Evidence |
| --- | --- | --- |
| `/api/v1/crates/<name>` returns 404 for an unpublished name | **verified live** | `polyoxide-venue`, `polyoxide-polymarket`, `polyoxide-ws`, `polyoxide-test-support` and `polyoxide-kalshi` each return **404** `{"errors":[{"detail":"crate `…` does not exist"}]}`. `polyoxide-core`, `polyoxide-binance` and `polyoxide-sports` return 200. |
| `/api/v1/crates/<name>/<version>` returns 404 for an unpublished version | **verified live** | `polyoxide-core/99.0.0` returns **404** "does not have a version `99.0.0`". `polyoxide-core/0.37.1` returns 200. |
| Implementation notes | **verified** | Without a `User-Agent` header the API returns **403**, so `publish_order.py` must send one, or it will misread every crate as an error rather than as absent. Name lookups are canonicalised (`polyoxide_core` and `Polyoxide-Core` both resolve to `polyoxide-core`), so the check matches cargo's collision rule. |
| "At most five new crate names per release" | **verified** | crates.io `src/rate_limiter.rs:24-38` sets `PublishNew` to a burst of 5 refilling one per 10 minutes, and `PublishUpdate` to a burst of 30 refilling one per minute. `src/controllers/krate/publish.rs:248-256` keys both on the user and picks the action by whether the crate exists. The tombstones' versions are `PublishUpdate`, not `PublishNew`. Because the bucket belongs to the user, two releases within about 50 minutes share it. |

### 4. cargo-semver-checks 0.51.0 (AD-22)

| Claim | Status | Evidence |
| --- | --- | --- |
| 0.51.0 is current | **verified** | crates.io `max_stable_version` is 0.51.0, published 2026-10-03, rust-version 1.93. |
| `--baseline-rev <REV>` | **verified** | `cargo-semver-checks-0.51.0/src/main.rs:471-479` ("Git revision to lookup for a baseline"). It reads the tree with git2, so CI's checkout must fetch the tag, which `actions/checkout` does not do by default. |
| `--release-type patch` makes a 0.x minor bump still report removals | **verified** | `main.rs:509-517` defines `--release-type` ("instead of deriving it from the version number") with `ReleaseType { Major, Minor, Patch }` (`lib.rs:66`). `check_release.rs:287-292` uses the given type in place of the version strings, and `:333-339` runs every lint that needs more than that level. A patch level therefore keeps `function_missing`, `struct_missing`, `module_missing`, `enum_variant_missing`, `feature_missing` and the other removal lints. |
| "It skips crates absent from the baseline" | **wrong if read as tool behaviour** | 0.51.0 does not skip them. A crate that is missing at the baseline rev fails with "package `X` not found in <root>" (`rustdoc_gen.rs:642-646`), and `lib.rs:666` (`let (name, outcome) = outcome?;`) turns any failed crate into a failure of the whole run. The only automatic skip is for `publish = false` crates in workspace mode (`lib.rs:546-566`). That covers polyoxide-py and the Kalshi skeleton but not the new foundation crates. The job or `api_removals.py` must work out which crates exist at the tag and pass `--exclude` for the rest. |
| Pinned 0.51.0 with floating stable Rust | **at risk** | `ci.yml` uses `dtolnay/rust-toolchain@stable` (lines 23, 33 and 56). Current stable is 1.99.0 (static.rust-lang.org channel manifest, 2026-10-01). 0.51.0 reads only rustdoc JSON formats v57, v60 and v61 (`Cargo.toml:186-191`, `trustfall_rustdoc 0.41.0`). Its README promises support only for "the then-current stable and beta" and tells users "to update cargo-semver-checks when updating Rust versions". A future stable release that bumps the format will turn this job red. AD-22 makes it a `ci.yml` job, and the release workflow waits on CI, so a red run silently withholds the release. |
| Side check: `cargo publish --workspace --dry-run` (AD-22) | **verified** | `cargo publish --help` on cargo 1.95.0 lists both `--workspace` and `--dry-run`. |

### 5. GitHub Actions: per-target secrets from a generated region (AD-13)

| Claim | Status | Evidence |
| --- | --- | --- |
| A job's `env` can reference `secrets.<NAME>` | **verified** | The context-availability table at docs.github.com (`/actions/reference/workflows-and-actions/contexts`) lists `jobs.<job_id>.env` as accepting `github, needs, strategy, matrix, vars, secrets, inputs`. `steps.env`, `steps.with` and `steps.run` also accept `secrets`. |
| Where secrets cannot go | **verified** | `jobs.<job_id>.strategy`, and so the matrix, accepts only `github, needs, vars, inputs`. `jobs.<job_id>.if` and `steps.if` do not accept `secrets` either ("Secrets cannot be directly referenced in `if:` conditionals"). A generated matrix row can carry secret names but not values, and environment variable names cannot be generated per row. The region therefore has two options: a static `env` block that gives every row every venue's secrets, or fixed names filled by index (`secrets[matrix.<field>]`). |
| An unset secret | **verified, and it bites** | docs.github.com `/actions/how-tos/write-workflows/choose-what-workflows-do/use-secrets` says "If a secret has not been set, the return value of an expression referencing the secret … will be an empty string". So the generated wiring will **set** an empty variable where today the variable is absent. No venue secret is wired today; the workflow's only secret is `GITHUB_TOKEN` (`nightly-behavioral.yml:168`). The current loaders use `std::env::var(..).ok()?` (`polyoxide-relay/tests/live_api.rs:27-48`) and `env::var(..).map_err` (`polyoxide-clob/src/account/mod.rs:186-200`), which treat `""` as present. An auth-gated skip would then become a parse failure and a **real** nightly verdict. The AD-14 loaders must treat an empty value as absent. |
| Forks and Dependabot | **verified, does not apply** | "With the exception of `GITHUB_TOKEN`, secrets are not passed to the runner when a workflow is triggered from a forked repository". Dependabot events get no secrets either. nightly-behavioral runs on `schedule` and `workflow_dispatch` (`nightly-behavioral.yml:4-13`), and dispatching it on the integration ref (AD-16) runs in this repository, so neither restriction applies. A reusable workflow would need `secrets: inherit`. |

## Problems

1. **Medium (AD-15).** The phrase "a TLS EOF … non-EOF `Tls`" is wrong. Under rustls, an EOF without `close_notify`, and every handshake or certificate failure, arrives as `Error::Io`. `Tls` only ever carries `InvalidDnsName`. Restate the rule as `Io` with `UnexpectedEof` → `Network`, and `Tls` → `InvalidRequest`.
2. **Medium (AD-22).** "It skips crates absent from the baseline" is not what cargo-semver-checks 0.51.0 does. It aborts the whole run with "package `X` not found". Say that the job passes `--exclude` for crates absent at the tag, and that the checkout fetches tags.
3. **Medium (AD-13 with AD-14).** An unset secret expands to `""`, and today's loaders treat `""` as a credential. Wiring the secrets turns auth-gated skips into real failures unless the loaders treat an empty value as absent. Also state that the matrix and `if:` cannot reference secrets.
4. **Medium (AD-22 and the Stack).** cargo-semver-checks is pinned at 0.51.0, which reads rustdoc JSON v57, v60 and v61 only, while CI uses floating stable. A red removal gate withholds the release. Either float the tool with the toolchain or pin the toolchain for that job.
5. **Low (AD-10).** Kalshi's `/account/limits` gives the bucket sizes, but the cost of each route is served by the public `/account/endpoint_costs` (default 10). Writes also have a bucket per shard, and a 429 carries no cooldown. Name the cost source, and require a drift test of the route table against that endpoint.
