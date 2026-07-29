# Nightly Polymarket API Smoketest — Design

**Status:** Approved (2026-05-08)
**Author:** aidanb
**Branch:** `aidanb/nightly-api-smoketest`

## Goal

Detect drift between Polyoxide's SDK and the live Polymarket API every night, before users do. Two failure modes are in scope:

1. **Behavioral drift** — endpoints return data that breaks our deserializers, auth flow, or test assertions.
2. **Schema drift** — Polymarket publishes a new OpenAPI spec that differs from our vendored copy at `docs/specs/<crate>/openapi.yaml`.

Out of scope: response-snapshot drift (recording canonical responses and diffing nightly captures). Considered, rejected as too flaky given live data churn.

## Decisions

| # | Question | Decision |
|---|----------|----------|
| 1 | Scope: which crates? | All four crates (clob, gamma, data, relay), no-auth tests only. CLOB and relay's auth-gated tests skip silently until secrets are wired up later. |
| 2 | Failure surfacing? | GitHub issues with reuse-on-repeat dedup. `GITHUB_TOKEN` only — no new secrets. Webhook integration deferred until issue cadence proves quiet. |
| 3 | Schema drift action? | Auto-PR replacing vendored YAML, plus a linked tracking issue. PR body summarizes endpoints added/removed/modified and embeds a canonical-form diff. |
| 4 | Flake tolerance? | Strict by default. Conditional retry only on transient infrastructure failures (HTTP 429/5xx, connection errors, timeouts, DNS failures). |
| 5 | Schedule? | Nightly at `0 6 * * *` UTC, plus `workflow_dispatch` for ad-hoc runs. Seven days a week. |

## Architecture

Two GitHub Actions workflows under `.github/workflows/`:

```
.github/workflows/
├── nightly-behavioral.yml   # Runs --ignored live tests across no-auth surfaces
└── nightly-schema.yml       # Diffs upstream OpenAPI YAML vs docs/specs/<crate>/openapi.yaml
```

Two helper scripts under `.github/scripts/`:

```
.github/scripts/
├── classify_failures.py     # Reads nextest libtest-json, classifies failures
├── diff_openapi.py          # Fetches upstream, canonicalizes, diffs, summarizes
└── tests/                   # pytest fixtures + tests for both scripts
```

**Why two workflows, not one:** independent triggers (workflow_dispatch each separately), independent badges, independent reruns, distinct failure semantics (issue vs PR+issue).

**Why Python, not Rust:** scripts are 100-line glue; `uv` is already present in CI for `polyoxide-py`. A Rust binary would mean a new workspace crate purely for CI tooling, which is the kind of premature structure to avoid.

Issue/PR mutations use `gh` CLI directly in workflow YAML — cleaner than wrapping in Python.

## Component 1 — `nightly-behavioral.yml`

### Matrix structure

```yaml
strategy:
  fail-fast: false
  matrix:
    crate: [polyoxide-gamma, polyoxide-data, polyoxide-clob, polyoxide-relay]
```

`fail-fast: false` ensures one crate's drift doesn't cancel the others.

### Auth filtering — runtime classification

CI deliberately does **not** set `POLYMARKET_*` or `BUILDER_*` env vars. Auth-gated tests panic with the helper's standard message: `"POLYMARKET_* env vars required for authenticated tests"`. The classifier matches this pattern and treats those tests as a third category (auth-gated, silently skipped) rather than failures.

This avoids invasive code changes (~25 test renames or feature flags) and centralizes "what counts as a real failure" in one place. When secrets are wired up later, the classifier stops matching the auth-gated pattern and those tests automatically light up — no code changes required.

Audited test counts:

| Crate | Live tests | Auth-gated | No-auth |
|-------|-----------|------------|---------|
| gamma | 42 | 0 | 42 |
| data | 18 | 0 | 18 |
| clob | 45 | ~25 | ~20 |
| relay | 8 | 8 | 0 |

Day-one no-auth coverage: ~80 tests.

### Test invocation

```yaml
- name: Run live (no-auth) tests
  run: |
    cargo nextest run \
      -p ${{ matrix.crate }} \
      --test live_api \
      --run-ignored only \
      --no-fail-fast \
      --message-format libtest-json \
      > nextest-output.json || true
```

### Classify-and-retry flow

Transient-error patterns matched by the classifier (case-insensitive):
- `HTTP 429` / `Too Many Requests` / `rate limit`
- `HTTP 5\d\d` (any 5xx status code)
- `Connection refused` / `Connection reset by peer` / `broken pipe`
- `request timed out` / `operation timed out`
- `DNS lookup failed` / `failed to lookup address` / `name resolution failed`

Auth-gated pattern: `POLYMARKET_\* env vars required` (literal helper message).

Anything not matching either set is a **real** failure.

```
1. nextest run (1st pass) → JSON
2. classify_failures.py classifies each failure into:
     - auth-gated (matches auth-gated pattern)    → log + skip
     - transient (matches transient patterns)     → emit retry list
     - real    (everything else)                  → emit failure list
3. if retry list non-empty:
     nextest run -E '<retry list>' --retries 2
4. final verdict per test:
     - 1st-pass pass                              → pass
     - 1st-pass auth-gated                        → skip (not in report)
     - 1st-pass transient + retry pass            → pass (logged, not in report)
     - 1st-pass transient + retry still failing   → real failure (in report)
     - 1st-pass real                              → real failure (in report)
5. report.md is non-empty iff there are real failures.
```

### Issue creation/update aggregator

Aggregator job runs with `needs: [test]` and `if: always()`:

```bash
EXISTING=$(gh issue list --label nightly-behavioral --state open --json number --jq '.[0].number')
if [ "$ANY_REAL_FAILURE" = "true" ]; then
  if [ -n "$EXISTING" ]; then
    gh issue comment "$EXISTING" --body "$(cat report.md)"
  else
    gh issue create --title "Nightly behavioral check failed: $(date -u +%Y-%m-%d)" \
                    --label nightly-behavioral --body "$(cat report.md)"
  fi
elif [ -n "$EXISTING" ]; then
  gh issue close "$EXISTING" --comment "Recovered $(date -u +%Y-%m-%d)"
fi
```

## Component 2 — `nightly-schema.yml`

### Matrix structure

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - { crate: clob,  upstream: clob-openapi.yaml }
      - { crate: gamma, upstream: gamma-openapi.yaml }
      - { crate: data,  upstream: data-openapi.yaml }
      - { crate: relay, upstream: relayer-openapi.yaml }
```

Note `relay` maps to `relayer-openapi.yaml` per `docs/specs/INDEX.md`.

### Per-entry flow

```
1. curl https://docs.polymarket.com/api-spec/${upstream} → /tmp/upstream.yaml
2. diff_openapi.py canonicalizes both sides:
     yaml.safe_load(file) → json.dumps(sort_keys=True)
3. if canonical_upstream == canonical_vendored: exit clean (no drift)
4. else:
     a. cp /tmp/upstream.yaml docs/specs/${crate}/openapi.yaml
        (commit RAW upstream bytes — preserves Polymarket's formatting/comments)
     b. emit markdown summary: endpoints added/removed/modified
        (uses deepdiff on canonicalized JSON to enumerate path changes)
     c. signal "drift detected" to aggregator
```

The canonicalization is **diff-only**. The committed file is always the upstream's raw bytes — re-canonicalizing-then-committing would lose Polymarket's comments and formatting and create perpetual self-noise.

### Auto-PR with deterministic branch names

```bash
BRANCH="nightly-schema-drift/${crate}"
git checkout -B "$BRANCH"
cp /tmp/upstream.yaml "docs/specs/${crate}/openapi.yaml"
git add docs/specs/${crate}/openapi.yaml
git commit -m "chore(specs): sync ${crate} OpenAPI from upstream"
git push --force-with-lease origin "$BRANCH"

EXISTING_PR=$(gh pr list --head "$BRANCH" --state open --json number --jq '.[0].number')
if [ -n "$EXISTING_PR" ]; then
  gh pr edit "$EXISTING_PR" --body "$(cat pr-body.md)"
else
  gh pr create --title "chore(specs): ${crate} drift on $(date -u +%Y-%m-%d)" \
               --body "$(cat pr-body.md)" --label "schema-drift"
fi
```

`--force-with-lease` (vs `--force`) refuses the push if the branch has been updated by a maintainer. If someone pushes a manual SDK adaptation onto `nightly-schema-drift/clob`, the next nightly leaves their work alone.

Same dedup pattern for the per-crate tracking issue (label `schema-drift`, deterministic title `Schema drift: ${crate}`). The PR body includes `Closes #<issue>` so merging the PR auto-closes the issue.

### On recovery

If drift was detected previously but upstream now matches vendored (Polymarket reverted, or maintainer manually updated):

```bash
if [ -z "$DRIFT" ]; then
  EXISTING_PR=$(gh pr list --head "$BRANCH" --state open --json number --jq '.[0].number')
  EXISTING_ISSUE=$(gh issue list --label schema-drift --search "Schema drift: ${crate} in:title" --state open --json number --jq '.[0].number')
  [ -n "$EXISTING_PR" ]    && gh pr close "$EXISTING_PR"    --comment "Drift no longer present"
  [ -n "$EXISTING_ISSUE" ] && gh issue close "$EXISTING_ISSUE" --comment "Drift recovered $(date -u +%Y-%m-%d)"
fi
```

### PR body structure

```markdown
## Schema drift in ${crate}

Upstream OpenAPI at <url> differs from vendored `docs/specs/${crate}/openapi.yaml`.

### Endpoints added
- POST /new-thing

### Endpoints removed
- GET /deprecated-thing

### Endpoints modified
- GET /markets — response schema changed (added field `creator_address`)

<details>
<summary>Full canonicalized diff</summary>

```diff
... unified diff against canonicalized JSON ...
```
</details>

Closes #<issue-number>
```

## Failure handling

### Failure taxonomy

| Category | Trigger | Surface | Label |
|----------|---------|---------|-------|
| API behavioral drift | Test fails for non-transient, non-auth reason | Issue | `nightly-behavioral` |
| Schema drift | Upstream OpenAPI ≠ vendored | Auto-PR + issue | `schema-drift` |
| Infrastructure failure | Script crash, gh CLI error, runner dead | Workflow goes red; no issue filed | (none) |

The third row is critical: when our own tooling breaks, we **don't pollute the API-drift channels**. The maintainer notices via the failed-workflow notification or red badge.

### Specific edge cases

- **Upstream unreachable** — retry the curl 3× with exponential backoff. If still failing, log warning and exit success. Better to miss a night than file false-positive PRs.
- **Invalid YAML upstream** — log clearly, exit non-zero, no PR. Maintainer investigates.
- **Test process hang** — `timeout-minutes: 15` per matrix entry. Polymarket's documented rate limits suggest 5–8 minute clean runs; 15 is a generous ceiling.
- **Concurrent runs** — `concurrency:` group keyed on workflow name with `cancel-in-progress: false`. Manual `workflow_dispatch` queues behind a scheduled run; no two runs fight over the same auto-PR branch.

### Permission boundary

```yaml
permissions:
  contents: write       # push schema-drift branches
  pull-requests: write  # create/update PRs
  issues: write         # create/update issues
```

Day-one secrets in scope: zero (per scope decision). When CLOB/relay auth is later wired in, secrets become `POLYMARKET_PRIVATE_KEY`, `POLYMARKET_API_KEY`, `POLYMARKET_API_SECRET`, `POLYMARKET_API_PASSPHRASE`, `BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE`.

## Testing

### Unit tests for helper scripts

`.github/scripts/tests/` with pytest:

```
tests/
├── test_classify_failures.py
├── test_diff_openapi.py
└── fixtures/
    ├── nextest-real-failure.json        # assertion failure
    ├── nextest-transient-429.json       # rate-limit failure
    ├── nextest-transient-503.json       # server failure
    ├── nextest-auth-gated.json          # POLYMARKET_* env panic
    ├── openapi-no-drift/{old,new}.yaml
    ├── openapi-added-endpoint/{old,new}.yaml
    ├── openapi-removed-endpoint/{old,new}.yaml
    └── openapi-modified-schema/{old,new}.yaml
```

Fixtures double as **specification by example** — the answer to "what does the classifier consider transient?" is "look at the fixtures."

A `scripts` job is added to `ci.yml` to run these on every PR:

```yaml
scripts:
  name: CI Scripts
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: astral-sh/setup-uv@v6
    - run: uv run pytest .github/scripts/tests/ -v
      working-directory: .github/scripts
```

### Workflow-level smoke test (pre-merge)

```bash
gh workflow run nightly-behavioral.yml --ref aidanb/nightly-api-smoketest
gh workflow run nightly-schema.yml --ref aidanb/nightly-api-smoketest
```

Run twice to confirm dedup (second run updates same issue/PR, doesn't create new ones).

### Pre-merge schema preview

Run `diff_openapi.py` locally to predict what the first post-merge nightly will do:

```bash
for c in clob gamma data relay; do
  uv run .github/scripts/diff_openapi.py --crate $c \
    --upstream-base https://docs.polymarket.com/api-spec \
    --vendored docs/specs
done
```

If vendored YAMLs are heavily out of sync with current upstream, expect four PRs to open the morning after merge. Desired behavior, but worth predicting.

## Rollout

1. **Merge** the PR (after writing-plans → execution cycle).
2. **Day 1**: manually `workflow_dispatch` both workflows from `main` to confirm production behavior.
3. **Days 1–7 calibration window** — watch for:
   - False positives in transient classification (real failures retried away)
   - False negatives (transient blips slipping through to issues)
   - Auth-gated panic patterns we missed (new tests with non-standard error messages)
4. **Day 7 review**: tune transient-error regex if needed.
5. **Future**: wire up `POLYMARKET_*` and `BUILDER_*` secrets when ready. Update classifier to stop matching auth-gated pattern. Auth tests start contributing real signal automatically.

## Documentation

Add a paragraph to `CLAUDE.md` under "Testing Conventions" describing:
- The two nightly workflows
- Classifier categories (real / transient / auth-gated)
- How to flip CLOB/relay auth on later (set secrets, remove auth-gated regex)

## Files to be created/modified

**New:**
- `.github/workflows/nightly-behavioral.yml`
- `.github/workflows/nightly-schema.yml`
- `.github/scripts/classify_failures.py`
- `.github/scripts/diff_openapi.py`
- `.github/scripts/pyproject.toml` (uv project for the scripts)
- `.github/scripts/tests/test_classify_failures.py`
- `.github/scripts/tests/test_diff_openapi.py`
- `.github/scripts/tests/fixtures/...` (per the layout above)

**Modified:**
- `.github/workflows/ci.yml` — add the `scripts` job for unit-testing helpers
- `CLAUDE.md` — add nightly-smoketest paragraph under Testing Conventions

## Open questions / future work

- **Webhook notification** — deferred per Q2. Add when issue cadence proves too quiet to notice in time.
- **Auth-gated test enablement** — deferred per Q1. Future PR provisions secrets and removes the auth-gated classifier regex.
- **Response-snapshot drift** — explicitly out of scope. Reconsider if behavioral + schema drift miss real regressions.
- **Sub-hourly cron / continuous monitoring** — if Polymarket ships a breaking change at 02:00 UTC and this runs at 06:00 UTC, there's up to a 4-hour detection gap. Acceptable for an SDK; not acceptable if this ever monitors production trading. Out of scope today.

## Addendum (2026-07-29) — coverage extension and latent-bug fixes

The branch sat unmerged for ~12 weeks while main moved from 0.15.0 to 0.23.x.
Before shipping, the following was amended:

**Coverage gaps closed:**

- **Behavioral matrix** now includes `polyoxide-cli` (`--test live_api`) and
  runs clob's `live_ws` binary (`--features ws --test live_api --test live_ws`,
  added to main 2026-07-25). Matrix entries carry a per-crate `flags` string.
- **Schema matrix** grew from 4 to 11 entries: perps/bridge/combos-rfq OpenAPI
  plus the four faithful AsyncAPI mirrors (clob market/user, perps WS,
  combos-rfq WS; upstream URLs from `docs/specs/polymarket-llms.txt`).
  `diff_openapi.py` enumerates AsyncAPI `channels` alongside OpenAPI `paths`
  and takes `--vendored-label` since mirrors no longer all live at
  `docs/specs/<crate>/openapi.yaml`. Deliberately excluded: the sports
  AsyncAPI mirror (modelled on the wire, never matches upstream's published
  doc) and the undocumented pnl/rankings hosts (no spec to diff).

**Latent bugs found while extending (the workflows had never run):**

- nextest's `--message-format libtest-json` refuses to run without
  `NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1`; with `|| true` swallowing the error,
  every nightly would have reported green on an empty JSON file. The env var
  is now set workflow-wide.
- libtest-json names tests `crate::binary$test`, which the retry step's
  `test(=name)` filterset can never match — and an unmatched retry was
  counted as a pass by `merge`. The classifier now emits `retry-filter.txt`
  with `binary_id(=…) & test(=…)` clauses, and fixtures use realistic
  qualified names.

**Classifier taxonomy changes:**

- `AUTH_GATED_RE` also matches `POLYMARKET_PRIVATE_KEY required` (live_ws
  derives L2 credentials from the private key alone).
- New **environmental** category (pattern: `legitimately time out`) for tests
  whose failure states the world can't provide signal — e.g. the sports
  channel with no live match anywhere at 06:00 UTC. Logged to
  `environmental.txt`, never retried, never filed as an issue.
