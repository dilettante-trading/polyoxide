# Self-Healing API Maintenance

Polyoxide tracks venues it does not control. Polymarket, Binance and Kalshi ship schema
changes, new endpoints, and behavioral tweaks without notice; this document
describes the machinery that detects that drift every night and reports it
without a human in the loop. Polymarket publishes specs, so its drift shows two
ways, in the live tests and against the vendored mirrors, and so does Kalshi's drift; Binance publishes none
for USDⓈ-M, so its live suites are its only drift check.

## The loop at a glance

```
                     ┌──────────────────────────────────────────────┐
                     │        06:00 UTC nightly (or dispatch)       │
                     └───────────┬──────────────────┬───────────────┘
                                 │                  │
                 nightly-behavioral.yml      nightly-schema.yml
                 (does the SDK still         (did the published
                  work against the            contracts change?)
                  live venues?)                      │
                                 │                  │
              real failure ──► GitHub issue     drift ──► tracking issue per spec
              (deduped, one     (label:                   (labels: schema-drift,
               open at a time)  nightly-behavioral)        spec:<id>), with the curl
                                 │                         that adopts it
                                 │                  │
              clean night ──► issue auto-closed  drift gone or ──► issue
                              "Recovered"        acknowledged      auto-closed
```

Both workflows are idempotent across nights: re-runs update the same issue
rather than creating duplicates, and recovery closes it.

## Behavioral drift — `.github/workflows/nightly-behavioral.yml`

Runs every crate's `#[ignore]`d live tests against the real upstream APIs,
with **no secrets configured**. Each crate and suite is its own job, generated
from the crate's `[package.metadata.polyoxide.live]` entries, and each job's
`env:` names only the secrets its tests read; unset, they arrive as `""`, which
the live loaders treat as absent. Besides the 06:00 UTC run, it also runs on
Saturday and Sunday at 18:30 UTC. The sports feed carries only what is live,
and that is when North American leagues and weekend soccer are on.

<!-- generated:begin selfheal-behavioral -->
| Crate | Test binaries |
|-------|---------------|
| polyoxide-binance | `live_usdm` (built with `--features usdm`), `live_usdm_ws` (built with `--features usdm-ws`) |
| polyoxide-kalshi | `live_predictions` (built with `--features predictions`), `live_predictions_account` (built with `--features predictions`), `live_predictions_venue` (built with `--features predictions`), `live_predictions_ws` (built with `--features predictions-ws`) |
| polyoxide-polymarket | `clob`: `live_clob` (built with `--features clob,gamma`), `live_clob_trading` (built with `--features clob-ws,data,gamma`), `live_clob_ws` (built with `--features clob-ws`); `clob-session-keys`: `live_clob_session_keys` (built with `--features clob,relay,gamma`; 40-minute budget in its own job, since its registry and transaction waits can take ~25 minutes); `gamma`: `live_gamma` (built with `--features gamma`); `data`: `live_data` (built with `--features data`); `relay`: `live_relay` (built with `--features relay`); `perps`: `live_perps` (built with `--features perps`), `live_perps_ws` (built with `--features perps-ws`); `rtds`: `live_rtds` (built with `--features rtds`); `sports`: `live_sports` (built with `--features sports`; 20-minute budget for its 180 s wire-agreement window) |
| polyoxide | `live_venue` (built with `--features polymarket-clob,polymarket-perps,binance-usdm,kalshi-predictions`) |
| polyoxide-cli | `live_api` |
<!-- generated:end selfheal-behavioral -->

Failures are classified by `.github/scripts/classify_failures.py`. It reads one
line: the `polyoxide-class=<tag>` a test prints just before it panics, through
`polyoxide-test-support`. The test decides what its failure means at the place
it fails; the classifier never reads the panic text, and a failure with no tag
is real.

| Verdict | Tag printed by | Consequence |
|---------|----------------|-------------|
| **auth-gated** | A credential loader (`load_env`, `optional_env`, `keychain`) that found a secret absent or empty | Logged, skipped. Lights up automatically once secrets are wired in. |
| **environmental** | `environmental(reason)`, where the test declares the world has no signal (the sports feed with no live match anywhere at 06:00 UTC, no market satisfying an order test's precondition); or an error whose class is `Restricted` and not a fault, such as Binance refusing the runner's location with HTTP 451 | Logged to `environmental.txt`, skipped. Never retried, never reported. |
| **transient** | An error whose class is `Network`, `Unavailable` or `RateLimited` (HTTP 408/425/429/5xx, connection refused/reset, timeouts, DNS failures, a dropped WebSocket and close codes 1000/1001/1006/1011–1013); or `transient(reason)`, where a bare stream ends without a close code | Retried in a second nextest pass with `--retries 2`. Passes on retry are forgiven; persistent failures are promoted to real, except that an environmental or auth-gated retry keeps its own verdict. |
| **real** | Every other error class, a `Restricted` error that is a fault (a 418 ban, Binance's firewall), and every untagged failure: a bare assertion, or an unwrap a test opted out with `// live-unwraps:` | Aggregated into a single tracking issue. |

The retry pass is driven by `retry-filter.txt`, a nextest filterset the
classifier emits with `binary_id(=crate::binary) & test(=name)` clauses —
libtest-json reports tests as `crate::binary$test`, which a bare `test(=…)`
predicate can never match.

### Self-healing properties

- **One issue, not an avalanche** — a failing night comments on the existing
  open `nightly-behavioral` issue instead of opening a new one.
- **Only main's runs touch it** — the issue step runs only on
  `refs/heads/main`. A dispatch on another branch, such as an integration
  branch tried before it merges, still classifies and uploads every job's
  report, but cannot comment on main's issue or close it. Instead the run's
  summary holds the combined report, and a real failure turns the run red.
- **Auto-recovery** — a clean night closes the issue with "Recovered". The
  close only happens when every live job actually succeeded; if one
  died on infrastructure (build failure, classifier crash, timeout), the
  issue stays open rather than declaring a recovery nothing proved.
- **Flake absorption** — rate limits and network blips are retried away and
  never reach the issue tracker; only *persistent* transients are reported.
- **Silent-green protection** — nextest refusing to run at all (build error,
  bad flags) produces an empty JSON file; the workflow fails that step
  explicitly instead of letting an empty file classify as "no failures".

## Schema drift — `.github/workflows/nightly-schema.yml`

Fetches every spec Polymarket publishes and canonically compares it (YAML/JSON
parsed, keys sorted — formatting and comments erased) against our vendored
mirror in `docs/specs/`:

<!-- generated:begin selfheal-watch -->
| Entry | Upstream | Vendored mirror |
|-------|----------|-----------------|
| clob | `docs.polymarket.com/api-spec/clob-openapi.yaml` | `docs/specs/clob/openapi.yaml` |
| gamma | `docs.polymarket.com/api-spec/gamma-openapi.yaml` | `docs/specs/gamma/openapi.yaml` |
| data | `docs.polymarket.com/api-spec/data-openapi.yaml` | `docs/specs/data/openapi.yaml` |
| data-v2 | `data-api.polymarket.com/v2/openapi.json` (served by the API host, not the docs site) | `docs/specs/data-v2/openapi.json` |
| relay | `docs.polymarket.com/api-spec/relayer-openapi.yaml` | `docs/specs/relay/openapi.yaml` |
| perps | `docs.polymarket.com/api-spec/perps-openapi.json` | `docs/specs/perps/openapi.json` |
| bridge | `docs.polymarket.com/api-spec/bridge-openapi.yaml` | `docs/specs/bridge/openapi.yaml` |
| combos-rfq | `docs.polymarket.com/api-spec/combos-rfq-openapi.yaml` | `docs/specs/combos-rfq/openapi.yaml` |
| kalshi | `docs.kalshi.com/openapi.yaml` | `docs/specs/kalshi/openapi.yaml` |
| clob-ws-market | `docs.polymarket.com/asyncapi.json` | `docs/specs/clob/asyncapi-market.json` |
| clob-ws-user | `docs.polymarket.com/asyncapi-user.json` | `docs/specs/clob/asyncapi-user.json` |
| perps-ws | `docs.polymarket.com/asyncapi-perps.json` | `docs/specs/perps/asyncapi.json` |
| combos-rfq-ws | `docs.polymarket.com/asyncapi-rfq.json` | `docs/specs/combos-rfq/asyncapi.json` |
| kalshi-ws | `docs.kalshi.com/asyncapi.yaml` | `docs/specs/kalshi/asyncapi.yaml` |
<!-- generated:end selfheal-watch -->

On drift, `.github/scripts/diff_openapi.py` summarizes endpoints (OpenAPI
`paths`) and channels (AsyncAPI `channels`) added/removed/modified, and the
workflow:

1. Renders an issue body: a key-path summary (each changed JSON pointer, with
   before → after, so drift inside `components.schemas` is named rather than
   counted), the canonical diff, capped under GitHub's 65,536-character limit,
   and the one `curl` that adopts the **raw upstream bytes** into the mirror
   (canonicalization is diff-only — a re-serialized mirror would erase
   upstream's comments and create perpetual self-noise).
2. Files, or updates, a `Schema drift: <id>` tracking issue labelled
   `schema-drift` and `spec:<id>`. It finds the issue by that label
   intersection, never by title: a title search is tokenized, so `perps` also
   matched `perps-ws`.
3. When drift disappears (upstream reverted, or a maintainer adopted it),
   closes the issue. A drift we deliberately will not sync is recorded in
   `docs/specs/.drift-acknowledged.json`, keyed by the SHA-256 of the canonical
   diff, which closes the issue too; the acknowledgement expires the moment
   either side moves, so it is never permanent blindness.

It commits nothing, pushes no branch and opens no PR: the workflow holds
`contents: read` and `issues: write`, and the organization does not let
Actions open pull requests anyway.

### Deliberate exclusions

<!-- generated:begin selfheal-exclusions -->
- **Sports** (`docs/specs/sports/`) — Upstream's AsyncAPI documents a
  `slug`-keyed payload and a text ping/pong that the server never sends, so the
  mirror is modelled on captured wire frames, and diffing it would report drift
  forever.
- **Undocumented hosts** (`docs/specs/undocumented/`) — `user-pnl-api` and
  `lb-api` publish no spec to diff against; their shapes were derived from live
  responses, and `polyoxide-polymarket`'s `live_data` suite is the drift check.
- **RTDS** (`docs/specs/rtds/`) — Upstream publishes no AsyncAPI for
  `ws-live-data`; the mirror is modelled on captured wire frames, so there is
  nothing to diff it against.
- **Deposit Wallets and session keys** (`docs/specs/session-keys/`) — The
  surface is almost entirely absent from the published CLOB and relayer OpenAPI
  (only `/deployed?type=WALLET` appears), so there is no mirror to diff; the
  SDK-generated fixtures are the drift check.
- **Binance USDⓈ-M** (`docs/specs/binance/`) — Not a Polymarket host, and
  Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M futures; the live suites
  in `polyoxide-binance` are the drift check.
<!-- generated:end selfheal-exclusions -->

## Failure taxonomy — what goes where

| Category | Surface | Label |
|----------|---------|-------|
| API behavioral drift | Tracking issue | `nightly-behavioral` |
| Schema drift | Tracking issue, one per spec | `schema-drift` and `spec:<id>` |
| Our own tooling broke | Red workflow run only — **never** an issue | (none) |

The third row is load-bearing: infrastructure failures (upstream unreachable,
script crash, runner death) must not pollute the drift channels. An
unreachable upstream is skipped with a warning — better to miss a night than
file a false-positive issue.

## Operations

- **Manual run**: `gh workflow run nightly-behavioral.yml` /
  `gh workflow run nightly-schema.yml` (both take `workflow_dispatch`).
- **Labels** are auto-created idempotently on first use; no repo setup needed.
- **Permissions**: `GITHUB_TOKEN` only — behavioral needs `contents: read`
  and `issues: write`; schema needs `contents: read` and `issues: write`. No
  external secrets beyond the live suites' own.
- **Adopting a schema drift** is a maintainer's commit: run the `curl` the
  issue spells out, review the diff, and commit the mirror. The workflow opens
  no PR because the organization forbids Actions from opening pull requests
  (the repository-level setting answers `409 Conflict` while the org policy
  stands), and because one `curl` is all an adoption needs.
- **Enabling authenticated coverage** (CLOB's authenticated tests, the
  session-key round trip, relay's credentialed tests and Kalshi's demo-host tests): set the
  `POLYMARKET_*`, `BUILDER_*`, `RELAYER_*` and `KALSHI_DEMO_*` repo secrets each target declares.
  The loaders find them and the tests light up with no other changes; until
  then each fails as `auth-gated` and is skipped.
- **Adding a new spec to watch**: add a spec (`id`, `kind`, `url`,
  `vendored`) to its directory's `[workspace.metadata.polyoxide.mirrors]`
  entry in the root `Cargo.toml` and run `python3 scripts/gen_registry.py
  --write`, which regenerates `nightly-schema.yml`'s rows and the tables here.
- **Adding a live test suite**: add a
  `[package.metadata.polyoxide.live.<target>]` entry (`suite`, `timeout`,
  `features`, `secrets`) to the crate's `Cargo.toml` and run
  `python3 scripts/gen_registry.py --write`; CI fails a `tests/live_*.rs`
  without one. Fail its tests through `polyoxide-test-support`: `.or_fail(ctx)`
  or `fail(ctx, &err)` for an error, the loaders for credentials, and
  `environmental(reason)` or `transient(reason)` for a skip-worthy failure mode
  no error carries, rather than special-casing the workflow.
  `scripts/live_unwraps.py` fails CI on an `.unwrap()`, `.expect(`, `panic!`
  or `unreachable!` in a live test, and on an assertion whose message names an
  error or a status, unless the line opts out with `// live-unwraps: <reason>`
  and the baseline's `opted_out` count is raised by hand.
- **Tuning classification**: there are no patterns to tune. A failure's
  verdict is the tag its test prints, so a misclassified failure is fixed in
  the test, or in the error's `Classify` impl, never in the classifier, whose
  regexes `scripts/live_unwraps.py` freezes. The fixtures under
  `.github/scripts/tests/fixtures/` are the specification by example — they
  mirror real nextest libtest-json output, tag lines, qualified names and all —
  and `test_classify_failures.py` holds the regex era's cases as a tag table,
  each row naming an error twinned by a Rust test that asserts its tag.

## Testing the machinery itself

The helper scripts are a `uv` project with a pytest suite
(`.github/scripts/tests/`) that runs on every PR via the
`CI Scripts` job in `ci.yml`. The design history — including the latent bugs
found before first deployment (nextest's experimental-JSON opt-in, the
binary-qualified name mismatch) — is recorded in
`docs/superpowers/specs/2026-05-08-nightly-api-smoketest-design.md`.
