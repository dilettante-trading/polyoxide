# Self-Healing API Maintenance

Polyoxide tracks a venue it does not control. Polymarket ships schema changes,
new endpoints, and behavioral tweaks without notice; this document describes
the machinery that detects that drift every night and — where it safely can —
repairs or reports it without a human in the loop.

## The loop at a glance

```
                     ┌──────────────────────────────────────────────┐
                     │        06:00 UTC nightly (or dispatch)       │
                     └───────────┬──────────────────┬───────────────┘
                                 │                  │
                 nightly-behavioral.yml      nightly-schema.yml
                 (does the SDK still         (did the published
                  work against the            contracts change?)
                  live venue?)                       │
                                 │                  │
              real failure ──► GitHub issue     drift ──► auto-PR with the new
              (deduped, one     (label:                   spec + tracking issue
               open at a time)  nightly-behavioral)       (label: schema-drift)
                                 │                  │
              clean night ──► issue auto-closed  drift gone ──► PR + issue
                              "Recovered"                       auto-closed
```

Both workflows are idempotent across nights: re-runs update the same issue/PR
rather than creating duplicates, and recovery closes them.

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
| polyoxide-binance | `live_api`, `live_ws` (built with `--features ws`) |
| polyoxide-data | `live_api` |
| polyoxide-gamma | `live_api` |
| polyoxide-perps | `live_api`, `live_ws` (built with `--features ws`) |
| polyoxide-relay | `live_api` |
| polyoxide-clob | `live`: `live_api`, `live_ws` (built with `--features ws`); `session-keys`: `live_session_keys` (40-minute budget in its own job, since its registry and transaction waits can take ~25 minutes) |
| polyoxide-rtds | `live_api` |
| polyoxide-sports | `live_api` (20-minute budget for its 180 s wire-agreement window) |
| polyoxide-cli | `live_api` |
<!-- generated:end selfheal-behavioral -->

Failures are classified by `.github/scripts/classify_failures.py` — the single
place that defines "what counts as a real failure":

| Verdict | Trigger | Consequence |
|---------|---------|-------------|
| **auth-gated** | Panic matches `POLYMARKET_* env vars required` or `POLYMARKET_PRIVATE_KEY required` | Logged, skipped. Lights up automatically once secrets are wired in. |
| **environmental** | Panic contains `legitimately time out` — the test itself declares the world may have no signal (e.g. the sports feed with no live match anywhere at 06:00 UTC), or Binance refuses the runner's location with HTTP 451 | Logged to `environmental.txt`, skipped. Never retried, never reported. |
| **transient** | HTTP 429/5xx, connection refused/reset, timeouts, DNS failures; a dropped WebSocket (reset without a closing handshake, TLS EOF without `close_notify`, close codes 1001/1011/1012/1013, or a test's own "server ended the connection") | Retried in a second nextest pass with `--retries 2`. Passes on retry are forgiven; persistent failures are promoted to real. |
| **real** | Everything else | Aggregated into a single tracking issue. |

The retry pass is driven by `retry-filter.txt`, a nextest filterset the
classifier emits with `binary_id(=crate::binary) & test(=name)` clauses —
libtest-json reports tests as `crate::binary$test`, which a bare `test(=…)`
predicate can never match.

### Self-healing properties

- **One issue, not an avalanche** — a failing night comments on the existing
  open `nightly-behavioral` issue instead of opening a new one.
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
| clob-ws-market | `docs.polymarket.com/asyncapi.json` | `docs/specs/clob/asyncapi-market.json` |
| clob-ws-user | `docs.polymarket.com/asyncapi-user.json` | `docs/specs/clob/asyncapi-user.json` |
| perps-ws | `docs.polymarket.com/asyncapi-perps.json` | `docs/specs/perps/asyncapi.json` |
| combos-rfq-ws | `docs.polymarket.com/asyncapi-rfq.json` | `docs/specs/combos-rfq/asyncapi.json` |
<!-- generated:end selfheal-watch -->

On drift, `.github/scripts/diff_openapi.py` summarizes endpoints (OpenAPI
`paths`) and channels (AsyncAPI `channels`) added/removed/modified, and the
workflow:

1. Commits the **raw upstream bytes** to the deterministic branch
   `nightly-schema-drift/<id>` (canonicalization is diff-only — committing a
   re-serialized form would erase upstream's comments and create perpetual
   self-noise).
2. Opens (or updates) a PR with the summary and a truncation-capped canonical
   diff, plus a `Schema drift: <id>` tracking issue the PR closes on merge.
3. Pushes with `--force-with-lease`, so a maintainer's manual work on the
   drift branch is never clobbered — the nightly loses that race on purpose.
4. When drift disappears (upstream reverted, or the mirror was updated by
   hand), auto-closes the stale PR and issue.

### Deliberate exclusions

<!-- generated:begin selfheal-exclusions -->
- **Sports** (`docs/specs/sports/`) — Upstream's AsyncAPI documents a
  `slug`-keyed payload and a text ping/pong that the server never sends, so the
  mirror is modelled on captured wire frames, and diffing it would report drift
  forever.
- **Undocumented hosts** (`docs/specs/undocumented/`) — `user-pnl-api` and
  `lb-api` publish no spec to diff against; their shapes were derived from live
  responses, and `polyoxide-data`'s live suite is the drift check.
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
| Schema drift | Auto-PR + tracking issue | `schema-drift` |
| Our own tooling broke | Red workflow run only — **never** an issue or PR | (none) |

The third row is load-bearing: infrastructure failures (upstream unreachable,
script crash, runner death) must not pollute the drift channels. An
unreachable upstream is skipped with a warning — better to miss a night than
file a false-positive PR.

## Operations

- **Manual run**: `gh workflow run nightly-behavioral.yml` /
  `gh workflow run nightly-schema.yml` (both take `workflow_dispatch`).
- **Labels** are auto-created idempotently on first use; no repo setup needed.
- **Permissions**: `GITHUB_TOKEN` only — behavioral needs `issues: write`;
  schema needs `contents: write`, `pull-requests: write`, `issues: write`.
  No external secrets.
- **Auto-PRs need a setting the workflow cannot grant itself.** "Allow GitHub
  Actions to create and approve pull requests"
  (`can_approve_pull_request_reviews`) must be on. With it off, `gh pr create`
  is refused with `GitHub Actions is not permitted to create or approve pull
  requests` regardless of how the `GITHUB_TOKEN` is scoped — a `permissions:`
  block cannot substitute for it.

  It exists at **two levels, and the org wins**. Setting it per-repo
  (*Settings → Actions → General → Workflow permissions*) returns `409
  Conflict — The organization does not allow GitHub Actions to create or
  approve pull requests` while the org policy forbids it, so an org owner must
  enable it first at
  `https://github.com/organizations/<org>/settings/actions`. That is an
  org-wide change affecting every repository, which is why it is not something
  this repo can fix on its own.

  Until then the schema workflow degrades deliberately: it treats that one
  refusal as a warning, not a failure. The drift branch is still pushed and the
  tracking issue still filed, so no signal is lost — a maintainer just opens
  the PR by hand. Any other `gh pr create` failure still fails the job.
- **Enabling authenticated coverage** (~25 CLOB + 8 relay tests): set the
  `POLYMARKET_*` and `BUILDER_*` repo secrets and remove the auth patterns
  from `AUTH_GATED_RE` in `.github/scripts/classify_failures.py`. The tests
  light up with no other changes.
- **Adding a new spec to watch**: add a spec (`id`, `kind`, `url`,
  `vendored`) to its directory's `[workspace.metadata.polyoxide.mirrors]`
  entry in the root `Cargo.toml` and run `python3 scripts/gen_registry.py
  --write`, which regenerates `nightly-schema.yml`'s rows and the tables here.
- **Adding a live test suite**: add a
  `[package.metadata.polyoxide.live.<target>]` entry (`suite`, `timeout`,
  `features`, `secrets`) to the crate's `Cargo.toml` and run
  `python3 scripts/gen_registry.py --write`; CI fails a `tests/live_*.rs`
  without one. If the new tests have a
  skip-worthy failure mode, encode its panic message in the classifier (and a
  fixture) rather than special-casing the workflow.
- **Tuning classification**: all patterns live in
  `.github/scripts/classify_failures.py`; the fixtures under
  `.github/scripts/tests/fixtures/` are the specification by example — they
  mirror real nextest libtest-json output, qualified names and all.

## Testing the machinery itself

The helper scripts are a `uv` project with a pytest suite
(`.github/scripts/tests/`) that runs on every PR via the
`CI Scripts` job in `ci.yml`. The design history — including the latent bugs
found before first deployment (nextest's experimental-JSON opt-in, the
binary-qualified name mismatch) — is recorded in
`docs/superpowers/specs/2026-05-08-nightly-api-smoketest-design.md`.
