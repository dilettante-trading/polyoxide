---
review: verify-current
target: ../ARCHITECTURE-SPINE.md
inputs:
  - ../.memlog.md
  - ../../../../specs/spec-venue-extensibility/venue-landscape.md
  - ../../../../../Cargo.toml
  - ../../../../../Cargo.lock
date: 2026-10-08
lens: "Verify every committed decision was web-researched or reality-checked rather than asserted from training data: current library/framework versions, that each named technology still exists and fits, and the live defaults of anything it leans on."
---

# Verify-current review

**Verdict: mostly sound, with one rule that does not compile and one MSRV hazard.**
Most version claims check out against crates.io, the workspace and the live docs. dynosaur
works for the spine's purpose on Rust 1.91.0. I compiled, linted, documented and ran the
spine's trait shapes on 1.91.0. The whole workspace also checks on 1.91.0 with today's lock.

These need fixing before epics copy them:

- AD-5's attribute syntax is the pre-0.3 form, and dynosaur 0.3.1 rejects it at compile time.
- AD-5 also leaves out the `Send + Sync` supertraits that `Arc<DynThrottle>` needs.
- The Stack table reads as current, but five rows are knowingly older majors.
- The Stack table omits alloy, and the MSRV job meets a resolver that ignores MSRV.
- AD-8/AD-18 overlook one HTTP path the project already has outside core.

Base: worktree at `e3d8c3e`. Checked 2026-10-08. Current stable Rust is 1.99.0, released
2026-10-01 ([blog.rust-lang.org/releases](https://blog.rust-lang.org/releases)).

## How things were checked

- **crates.io.** `curl -s -A 'polyoxide-review' https://crates.io/api/v1/crates/<name>`
  gave `max_stable_version`, release dates and each version's `rust_version`.
- **Sources.** I downloaded the crate sources for dynosaur 0.3.1 and dynosaur_derive 0.3.1,
  and governor 0.8.1 and 0.10.4, from crates.io. reqwest 0.12.28 and 0.13.5, tracing 0.1.44
  and tracing-subscriber 0.3.23 were read from the local registry cache.
- **A Rust 1.91.0 toolchain.** I assembled one from `static.rust-lang.org` components
  (rustc, rust-std, clippy), because there is no rustup on this box. With it:
  - `cargo check --workspace --all-features --locked --offline` passed on this worktree in
    37 s.
  - A scratch crate passed build, test, clippy and rustdoc on 1.91.0. It holds the spine's
    `Throttle`, `Authenticator` and `MarketData` shapes through dynosaur, held as
    `Arc<Dyn…<'static>>` and called from inside `tokio::spawn`. Clippy and rustdoc both ran
    with `-D warnings`, and the crate denies `missing_docs`.
- **`cargo metadata --locked`.** The highest `rust-version` in the locked graph is 1.91, set
  by alloy 1.8.3 and its subcrates.
- **mermaid-cli 12.0.0.** I re-rendered all three of the spine's mermaid blocks with it,
  against the system chromium. All rendered, with no error nodes.
- **Kalshi.** I read `docs.kalshi.com/openapi.yaml` (fetched today), `llms.txt` and
  `getting_started/api_environments.md`.

## Findings

### F1 — AD-5's dynosaur attribute does not compile on 0.3.1 — **wrong** (Medium)

The spine says traits carry `#[dynosaur::dynosaur(Dyn<Trait>)]`, for example
`#[dynosaur::dynosaur(DynThrottle)]`. That is the 0.1/0.2 syntax. In 0.3 the macro's parser
requires `= dyn(box) TraitName`. The bare form fails:

```
error: unexpected end of input, expected `= dyn(box) TraitName`; dynosaur 0.3 requires this
  --> src/lib.rs:16:1
16 | #[dynosaur::dynosaur(DynThrottle)]
```

The parser is `dynosaur_derive-0.3.1/src/lib.rs` (`impl Parse for Attrs`). The crate's own
examples (`examples/pub_trait.rs`, `next.rs`) all use `= dyn(box)`.

The rule has two more gaps the scratch crate exposed:

- **Exporting the wrapper.** The generated type is re-exported with the attribute's
  visibility, so `polyoxide-venue` needs `pub DynThrottle = dyn(box) Throttle`. Without
  `pub`, `DynThrottle` stays private to the defining module.
- **Sharing the wrapper.** `Arc<DynThrottle>` (AD-10) is `Send + Sync` only if the trait
  declares `Send + Sync` supertraits. dynosaur copies supertraits onto the erased trait.
  Without them:

  ```
  error[E0277]: `(dyn ErasedThrottle + 'static)` cannot be sent between threads safely
  ```

  The wrapper also has a lifetime parameter, so the held type is
  `Arc<DynThrottle<'static>>`. It is built with `DynThrottle::new_arc(t)`, not by coercion.

What does hold, all checked on 1.91.0:

- A method declared `fn m(&self, …) -> impl Future<Output = …> + Send` erases to
  `Pin<Box<dyn Future + Send>>`. dynosaur's `is_future` recognises a `Future`,
  `future::Future` or `{core,std}::future::Future` bound.
- An associated error type becomes a type parameter on the wrapper:
  `DynMarketData<'static, E>`.
- An `&mut RequestParts` argument works.
- An impl may write `async fn` against the trait's `-> impl Future + Send` declaration.
- clippy and rustdoc both pass with `-D warnings`.

So AD-5's fallback trigger does not fire.

**Fix.** Rewrite the AD-5 rule as:

> Traits held through `dyn` declare `Send + Sync` supertraits and carry
> `#[dynosaur::dynosaur(pub Dyn<Trait> = dyn(box) <Trait>)]`. They are held as
> `Arc<Dyn<Trait><'static>>`, built with `Dyn<Trait>::new_arc`.

Also update memlog line 60 ("Holders use DynThrottle/DynAuthenticator").

### F2 — MSRV 1.91 with a resolver that ignores MSRV; alloy missing from the Stack — **unverified** (Medium)

AD-22 adds an MSRV job, and a red job withholds the release. The workspace sets
`resolver = "2"`, whose `incompatible-rust-versions` default is `allow`. Cargo therefore
resolves to the newest version even when its `rust-version` exceeds 1.91. Only resolver
`"3"` defaults to `fallback`. It needs Cargo 1.84 or later, which 1.91 satisfies
([doc.rust-lang.org/cargo/reference/resolver.html](https://doc.rust-lang.org/cargo/reference/resolver.html)).

The lock is at the limit today: alloy 1.8.3 declares exactly 1.91. The signing stack shows
how a later bump would break the job:

| alloy | Released | Declared rust-version |
| --- | --- | --- |
| 1.8.3 (last 1.x, locked) | 2026-03-27 | 1.91 |
| 2.0.0 – 2.1.1 | 2026-04-13 – 2026-07-06 | 1.91 |
| 2.2.0 – 2.5.0 | 2026-07-17 – 2026-09-23 | **1.94.1** |

A routine move to `alloy = "2"` under resolver 2 resolves to 2.5.0 and reddens the MSRV job.
That silently withholds the next release, the failure CLAUDE.md warns about. AD-2 names
alloy, but the Stack table omits it. keyring is also missing, though the Conventions lean on
core's keychain. The workspace pins keyring 3 (locked 3.6.3); the latest is 4.2.0, which
needs Rust 1.88.

**Fix.**

- Set `resolver = "3"` in the workspace, or `[resolver] incompatible-rust-versions =
  "fallback"` in `.cargo/config.toml`, in the same change as the MSRV job.
- Add Stack rows for alloy (`1.x, 1.8.3 locked; 2.0–2.1.1 fit 1.91; 2.2+ need Rust 1.94.1`)
  and keyring (`3; 4.x available`).

### F3 — Stack rows are workspace pins several majors behind, unlabelled — **outdated** (Medium)

Every Stack row does match the workspace, but the table does not say that some rows are
knowingly behind. S1 creates the crates that will own these dependencies (`polyoxide-ws`,
and core's transport ownership under AD-18). That is the natural point to upgrade, or to
record the pin on purpose.

| Row | Spine | Workspace / lock | Latest (date, rust-version) | Status |
| --- | --- | --- | --- | --- |
| tokio-tungstenite | 0.26 | 0.26 / 0.26.2 | **0.30.0** (2026-07-11, 1.85) | outdated, 4 breaking releases |
| reqwest | 0.12 | 0.12 / 0.12.28 (last 0.12: 2025-12-22) | **0.13.5** (2026-09-08, 1.85) | outdated. The lock already carries 0.13.2 via alloy. 0.13 has no `rustls-tls` feature (TLS is `rustls`, aws-lc-rs by default), so the workspace feature list must change on upgrade. |
| governor | 0.8 | 0.8 / 0.8.1 | **0.10.4** (2025-12-16) | outdated. `Quota::with_period` (burst 1) and `allow_burst` are unchanged in 0.10.4. |
| pyo3 | 0.28 | 0.28 / 0.28.3 | **0.29.3** (2026-09-30, 1.83) | outdated (pyo3 minors break) |
| rustls | 0.23 | 0.23 / 0.23.38 | 0.23.45 (0.24 is only `-dev`) | verified |

**Fix.** Add a "Basis" column to the Stack table, with values such as "workspace pin, latest
0.30.0, upgrade in polyoxide-ws creation" or "kept: …". Then a parallel agent creating
`polyoxide-ws` knows whether to bump.

### F4 — AD-8/AD-18 miss an existing HTTP path outside core — **unverified against the project** (Medium)

AD-8 binds "every HTTP request in every venue crate", and AD-18 says only `polyoxide-core`
depends on reqwest. But `polyoxide-relay/src/client.rs:1579` estimates gas with
`alloy::providers::ProviderBuilder::new().connect_http(rpc_url)`. That runs through alloy's
own reqwest 0.13 client (relay enables alloy's `provider-http`). It bypasses the send loop,
the throttle and the retry log line, and AD-18's header-pin builds cannot see it. Its gzip and
TLS features also come from alloy, not from core.

**Fix.** State the exception in AD-8/AD-18: calls to a caller-configured RPC node go through
alloy's transport and are outside the venue send loop. Or route alloy through a transport
built on core's client. Either way, the AD-18 CI check ("no direct reqwest in a venue crate")
must allow `alloy/provider-http` deliberately rather than by oversight.

### F5 — AD-11's "pings on the wall clock" with "a full queue backpressures" — **reality-checked: they conflict** (Low)

tokio's bounded `Sender::send` waits for capacity
([docs.rs/tokio 1.53.2](https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Sender.html)).
In today's perps pump (`polyoxide-perps/src/ws/supervised.rs:500-524`), `events.send(..).await`
runs inside a `select!` arm. While the 1024-slot queue is full, the task sends no ping. If a
consumer stalls for longer than the host's idle timeout (60 s on perps), the host closes the
connection, which contradicts the wall-clock rule.

**Fix.** Either state the exception, or have the `Supervisor` race `events.reserve()` against
the ping timer in `select!`. tokio documents `reserve` as the way to avoid losing a message
when a send is cancelled in `select!`, so this keeps both "never drops" and "pings on the wall
clock".

### F6 — Smaller recorded facts that are stale or imprecise (Low)

- **dynosaur's MSRV.** Memlog line 16 says dynosaur 0.3.1 has MSRV 1.75. Its manifest
  declares `rust-version = "1.84"`; 1.75 belongs to 0.3.0 and to the README's stale prose.
  It still fits 1.91.
- **mermaid-cli version.** Memlog line 53 used mermaid-cli 11.17.0. The latest is 12.0.0
  (2026-09-24). All three diagrams render on 12.0.0 as well.
- **Retry log target.** AD-8 and the Conventions say "target `polyoxide_core`". The default
  target is the module path (`module_path!()` in tracing 0.1.44's macros), so today's line
  carries `polyoxide_core::request`. EnvFilter matches targets by prefix
  (`starts_with`, tracing-subscriber 0.3.23 `filter/env/directive.rs:246`), and the soak
  harness also uses `starts_with("polyoxide_core")`. So the rule holds for filters, but an
  exact-match reader would miss the line.
  Fix: say "target prefix `polyoxide_core`", or pin `target: "polyoxide_core"` in the macro.
- **futures-core.** The Stack row says 0.3.34, which is the latest. The workspace has no
  direct futures-core dependency, and the lock has 0.3.32. That is harmless; say "minimum
  0.3.34 (new dependency)".
- **Edition.** Edition 2021 matches the workspace. Edition 2024 has been available since
  1.85. The spine does not say whether the new crates stay on 2021; if they move, resolver
  `"3"` comes with it (see F2).
- **Kalshi facts (venue-landscape, which the spine leans on in AD-4, AD-7, AD-10 and the CI
  diagram):**
  - The OpenAPI is version **3.34.0** today; the landscape says "3.30.0 seen".
  - The signing header accepts **Ed25519** keys as well as RSA-PSS/SHA-256. That matters to
    the Kalshi credential type later.
  - The V2 order surface is **YES-side only**: `BookSide` `bid` means buy YES, `ask` means
    sell YES. "Selling YES is economically equivalent to buying NO at `1 - price`."
    AD-7 states the book conversion but not the order one. Add: "an order for
    `kalshi.events:<t>:no` at p is sent as the YES-side opposite at 1 − p".

## Verified as stated

| Item | Evidence |
| --- | --- |
| Rust MSRV 1.91, edition 2021 | Match workspace `rust-version = "1.91"` and `edition = "2021"`. The workspace checks on 1.91.0 (`--all-features --locked`). The locked graph's highest declared rust-version is 1.91. |
| tokio 1.41 | Workspace pin; lock 1.52.0; latest 1.53.2 (2026-10-03, rust-version 1.71). No RustSec advisory since RUSTSEC-2025-0023. |
| rust_decimal 1.37 | Workspace pin; lock 1.41.0; latest 1.43.0 (rust-version 1.67.1). Semver-compatible, as memlog line 16 says. |
| dynosaur 0.3.1 | Latest (2026-07-03), rust-version 1.84. Supports RPITIT `-> impl Future + Send` and generates a boxed `Dyn` wrapper, as F1 shows. Async fn through `dyn` is still not stable, so dynosaur remains necessary. |
| futures-core 0.3.34 | Latest (2026-08-11), rust-version 1.36. |
| thiserror 2.0 | 2.0.21 (2026-09-23), rust-version 1.77. |
| serde 1.0, tracing 0.1, mockito 1.7 | 1.0.229, 0.1.44 and 1.7.2 are the latest. |
| cargo-hack 0.6.45 | Latest (2026-05-30). Its own rust-version is 1.85, which only matters if built on the MSRV job; install-action uses binaries. `cargo hack check --each-feature --no-dev-deps` is the README's recommended invocation. `--each-feature` includes the default-features run and the `--no-default-features` run. `--no-dev-deps` edits manifests while it runs. |
| governor depth-1 default and `allow_burst` | `Quota::with_period` sets `max_burst: 1` and `allow_burst` sets the capacity, in both 0.8.1 and 0.10.4 (`src/quota.rs`). Core's `quota()` uses `with_period` (`rate_limit.rs:166`), and `signer_limit.rs:262-264` uses `allow_burst`. |
| tokio mpsc backpressure | `send` waits for capacity (docs). The perps and Binance supervisors use `mpsc::channel(1024)` with `send().await`. |
| RPITIT with `+ Send` | Stable since 1.75; the blog post recommends `fn → impl Future + Send` for Send bounds ([blog.rust-lang.org, 2023-12-21](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/)). Compiled on 1.91.0. |
| Async fn in dyn traits is not stable | The 2026 project goal "Native async fn dynamic dispatch in traits" is Accepted for 2026–2027, with a nightly-only `std::preview::dyn_box!` ([goals.rust-lang.org/2026/afidt-box.html](https://goals.rust-lang.org/2026/afidt-box.html)). |
| crates.io strips path-only dev-dependencies | Cargo Book: "only dev-dependencies that specify a `version` will be included in the published crate". Path-only normal dependencies are refused ([specifying-dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)). AD-13's "publish order ignores path-only dev-deps" agrees. |
| reqwest gzip default (AD-18) | "If the `gzip` feature is turned on, the default option is enabled": reqwest 0.12.28 and 0.13.5, `ClientBuilder::gzip`. Core's builder holds `gzip: Option<bool>` and calls `.gzip()` only when set (`polyoxide-core/src/client.rs:264-334`). |
| tracing targets | The default is `module_path!()`, and EnvFilter matches by prefix. See F6 for the wording. |
| 425 in the caller rule | RFC 8470 §5.2 defines 425 (Too Early). |
| Binance never retried 425 (AD-17) | `polyoxide-binance/src/usdm/request.rs:127` retries only `TOO_MANY_REQUESTS`. Core's `should_retry` retries 429 and 425 (`client.rs:132`). |
| Kalshi RSA-PSS auth | `KALSHI-ACCESS-SIGNATURE`: "RSA-PSS with SHA-256 for RSA keys, Ed25519 for Ed25519 keys". The signed path has no query string. |
| Kalshi demo host | `external-api.demo.kalshi.co/trade-api/v2` and WS `external-api-ws.demo.kalshi.co/trade-api/ws/v2`. Demo keys work only on demo. |
| Kalshi OpenAPI | `docs.kalshi.com/openapi.yaml` returns 200, version 3.34.0, with 100 paths. |
| Kalshi token buckets (AD-10) | `BucketLimit` is "refills at `refill_rate` tokens per second up to `bucket_capacity`", per tier via `GET /account/limits`. That is a published capacity, so `allow_burst(capacity)` is the right model. |
| Kalshi yes/no book (AD-7) | `GetMarketOrderbook` "returns yes bids and no bids only". A yes bid at X is a no ask at 100 − X. |
| Kalshi perps (AD-4's events/perps split) | A separate "Perps API" (`docs.kalshi.com/margin.md`, `margin-rest/*`), with `ExchangeInstance` `event_contract` and `margined`. |
| Kalshi socket auth at handshake (AD-11) | "Authentication is required to establish the connection; include API key headers during the WebSocket handshake", including for public channels. |
| mermaid diagrams | All three render on mermaid-cli 12.0.0 (latest) as well as 11.17.0. |
