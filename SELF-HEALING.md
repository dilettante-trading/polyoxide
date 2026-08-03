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

Runs every crate's `#[ignore]`d live tests against the real Polymarket APIs,
with **no secrets configured**:

| Crate | Test binaries |
|-------|---------------|
| polyoxide-gamma | `live_api` |
| polyoxide-data | `live_api` |
| polyoxide-clob | `live_api`, `live_ws` (built with `--features ws`) |
| polyoxide-relay | `live_api` |
| polyoxide-cli | `live_api` |

Failures are classified by `.github/scripts/classify_failures.py` — the single
place that defines "what counts as a real failure":

| Verdict | Trigger | Consequence |
|---------|---------|-------------|
| **auth-gated** | Panic matches `POLYMARKET_* env vars required` or `POLYMARKET_PRIVATE_KEY required` | Logged, skipped. Lights up automatically once secrets are wired in. |
| **environmental** | Panic contains `legitimately time out` — the test itself declares the world may have no signal (e.g. the sports channel with no live match anywhere at 06:00 UTC) | Logged to `environmental.txt`, skipped. Never retried, never reported. |
| **transient** | HTTP 429/5xx, connection refused/reset, timeouts, DNS failures | Retried in a second nextest pass with `--retries 2`. Passes on retry are forgiven; persistent failures are promoted to real. |
| **real** | Everything else | Aggregated into a single tracking issue. |

The retry pass is driven by `retry-filter.txt`, a nextest filterset the
classifier emits with `binary_id(=crate::binary) & test(=name)` clauses —
libtest-json reports tests as `crate::binary$test`, which a bare `test(=…)`
predicate can never match.

### Self-healing properties

- **One issue, not an avalanche** — a failing night comments on the existing
  open `nightly-behavioral` issue instead of opening a new one.
- **Auto-recovery** — a clean night closes the issue with "Recovered". The
  close only happens when every matrix job actually succeeded; if an entry
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

| Entry | Upstream | Vendored mirror |
|-------|----------|-----------------|
| clob, gamma, data, relay | `docs.polymarket.com/api-spec/*-openapi.yaml` | `docs/specs/<crate>/openapi.yaml` |
| perps | `api-spec/perps-openapi.json` | `docs/specs/perps/openapi.json` |
| bridge | `api-spec/bridge-openapi.yaml` | `docs/specs/bridge/openapi.yaml` |
| combos-rfq | `api-spec/combos-rfq-openapi.yaml` | `docs/specs/combos-rfq/openapi.yaml` |
| clob-ws-market | `asyncapi.json` | `docs/specs/clob/asyncapi-market.json` |
| clob-ws-user | `asyncapi-user.json` | `docs/specs/clob/asyncapi-user.json` |
| perps-ws | `asyncapi-perps.json` | `docs/specs/perps/asyncapi.json` |
| combos-rfq-ws | `asyncapi-rfq.json` | `docs/specs/combos-rfq/asyncapi.json` |

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

- **`docs/specs/clob/asyncapi-sports.json`** — this mirror intentionally does
  *not* match upstream's published document: upstream documents a
  `slug`-keyed payload and text ping/pong that the server never sends, so the
  mirror is modelled on captured wire frames (see its `x-observed-payload`).
  Diffing it would report false drift forever.
- **`user-pnl-api` / `lb-api`** (`docs/specs/undocumented/`) — no published
  spec exists to diff against; their shapes were derived from live responses.

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
- **Adding a new spec to watch**: add a matrix row (`id`, `url`, `vendored`)
  to `nightly-schema.yml`. Nothing else to touch.
- **Adding a live test suite**: extend the crate's `flags` entry in
  `nightly-behavioral.yml`'s matrix. If the new tests have a
  skip-worthy failure mode, encode its panic message in the classifier (and a
  fixture) rather than special-casing the workflow.
- **Tuning classification**: all patterns live in
  `.github/scripts/classify_failures.py`; the fixtures under
  `.github/scripts/tests/fixtures/` are the specification by example — they
  mirror real nextest libtest-json output, qualified names and all.

## Testing the machinery itself

The helper scripts are a `uv` project with a pytest suite
(`.github/scripts/tests/`, 32 tests) that runs on every PR via the
`CI Scripts` job in `ci.yml`. The design history — including the latent bugs
found before first deployment (nextest's experimental-JSON opt-in, the
binary-qualified name mismatch) — is recorded in
`docs/superpowers/specs/2026-05-08-nightly-api-smoketest-design.md`.
