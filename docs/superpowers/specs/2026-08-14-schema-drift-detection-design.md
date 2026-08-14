# Trustworthy Schema Drift Findings — Design

**Status:** Approved (2026-08-14)
**Author:** aidanb
**Branch:** `aidanb/issues`
**Supersedes:** parts of [2026-05-08-nightly-api-smoketest-design.md](2026-05-08-nightly-api-smoketest-design.md) — Component 2 (`nightly-schema.yml`) issue identity and PR body structure.

## Goal

Make every open `schema-drift` issue satisfy two properties:

1. **It belongs to exactly one spec.** No matrix entry can read, edit, or close another entry's issue.
2. **It states what changed** precisely enough to triage without leaving GitHub — including drift that is not at path or channel level.

## Context — why this is S1 of three

Investigation on 2026-08-14 found the drift pipeline reports unreliably in both directions. The remediation splits into three independent projects; this spec covers the first.

| | Project | Outcome |
|---|---|---|
| **S1** | *(this spec)* Make drift findings true | The issue list means what it says |
| **S2** | Make drift findings converge | Findings end, instead of accumulating |
| **S3** | Absorb the current backlog | The open issues close for real |

S1 → S2 is a real dependency: triage states cannot be designed on top of findings nobody trusts. S3 depends on neither but generates S2's evidence — the clob rejection is the first "won't sync" case, the data adoption the first "adopted" case.

### Observed failures

**Issue identity collides.** The workflow resolves a spec's tracking issue with `gh issue list --search "Schema drift: <id> in:title"`. That is GitHub's tokenized full-text search, not an exact title match; the hyphen splits tokens, so `Schema drift: perps` matches the issue titled `Schema drift: perps-ws`. `.[0]` then takes the fuzzy hit whenever the exact-title issue does not exist.

Confirmed in run 31778984093 (2026-08-14):

- `Check perps` found drift, pushed its branch, then wrote its summary into **#12 (`Schema drift: perps-ws`)**. `Check perps-ws` overwrote the same issue six seconds later. perps drift has been invisible since 2026-08-03 and has never had an issue.
- `Check combos-rfq` found no drift and closed **#17 (`Schema drift: combos-rfq-ws`)** — `gh` printed the mismatched title in the log. This is the cause of the #11 / #14 / #15 / #17 file-then-close flapping, which is a job race, not upstream instability.

`clob` vs `clob-ws-market` / `clob-ws-user` is the same latent collision, currently masked because the clob-ws specs have no drift.

**Findings render as mysteries.** `render_summary` enumerates only `paths` and `channels`. Issue #18 (`clob`) reports a heading, one sentence, and no findings section, because its drift lives in `components.schemas`. Its canonical diff is two lines:

- `3` → `3.0` on `total_daily_rate` — YAML parses these to Python `int` and `float`, so `json.dumps` renders them differently. The same JSON number. Provably noise.
- `'Yes'` → `Yes` on a `type: string` field — upstream's re-serialization dropped the quotes, and YAML parses bare `Yes` as boolean `true`. A genuine type change, correctly detected, and an upstream regression that must not be adopted.

Detection is not the problem. Presentation is: a correct finding rendered as 653 lines of apparent churn with nothing said about it.

**The explanation already exists and is discarded.** The workflow composes a canonicalized diff into a `<details>` block for the PR body. Org policy forbids Actions from opening PRs, so that body is built nightly and thrown away while the issue receives the bare `summary.md`.

## Decisions

| # | Question | Decision |
|---|----------|----------|
| 1 | Suppress findings, or explain them? | Explain fully; suppress exactly one thing — int/float equivalence, which is meaningless by definition. No judgment calls at the gate. |
| 2 | How to report sub-path drift? | Key-path summary (changed JSON pointers with before → after) plus the existing canonical diff in a collapsible block. |
| 3 | How does an entry find its issue? | A per-spec `spec:<id>` label, resolved by label intersection. Exact by construction; the collision class stops existing rather than being handled. |
| 4 | Where does body assembly live? | Python emits a finished, pre-truncated `issue-body.md`; bash calls `gh issue create --body-file`. |
| 5 | Migrating the four open issues? | Close them as superseded and let the next nightly refile with correct labels. All four carry zero comments, and bodies regenerate nightly, so nothing is lost. |

## Architecture

No new files. Three existing files change, preserving the split established by the original design — logic in Python under pytest, orchestration in workflow YAML.

```
.github/scripts/diff_openapi.py          # detection + rendering (extended)
.github/scripts/tests/test_diff_openapi.py  # pytest (extended)
.github/workflows/nightly-schema.yml     # orchestration (edited)
```

**Why not fix the lookup inline in bash and stop.** The exact-match jq filter is a two-line change, but it leaves identity coupled to a title string, so retitling an issue orphans it and the nightly files a duplicate. Labels are exact because GitHub does not tokenize them — the failure mode is removed rather than made less likely.

## Component 1 — Numeric normalization

`canonicalize` erases YAML syntax by round-tripping `yaml.safe_load` → `json.dumps`, which is why it correctly ignores `>-` folding and key ordering. The value tree it produces is Python-typed, so YAML's type coercion leaks through.

Add a recursive `_normalize(value)` applied before `json.dumps`:

- `bool` → returned untouched. **Checked first.**
- `float` where `.is_integer()` → `int`
- `dict` / `list` → recurse
- everything else → unchanged

The bool case is not defensive padding. Python's `bool` subclasses `int`, so a normalizer that tests `isinstance(v, int)` before `isinstance(v, bool)` would rewrite `True` and erase exactly the clob finding this spec exists to surface.

**Accepted risk:** normalization could in principle mask a change where int-vs-float matters. In OpenAPI that distinction is carried by `type: integer` vs `type: number`, not by example literals, so `3` and `3.0` are the same JSON number. Accepted deliberately.

## Component 2 — Key-path walker

New `diff_tree(old, new) -> list[Change]` over the two canonical value trees, where `Change` carries `pointer`, `kind` (`added` / `removed` / `changed`), `before`, and `after`.

- **Pointer format:** dotted path with list indices — `components.schemas.Position.properties.grossInitialValue`.
- **Subtree-root rule:** stop descending on an add or remove and record the root. A newly added schema is one line, not forty. Only `changed` leaves descend to scalars.
- **Value rendering:** scalars inline, truncated to 120 characters; containers as `{…}` / `[…]` with a child count.

`DriftResult` gains `changes: list[Change]`. The existing `endpoints_*` and `channels_*` lists remain as the top-level index; key paths are the detail layer beneath them.

## Component 3 — Summary rendering

`render_summary` gains a Changes section after the existing path and channel sections, grouped by top-level key (`paths`, `components`, `channels`, `info`) so a reader sees shape before detail. Enumeration caps at 200 pointers, followed by an `N more — see diff` line.

This also sharpens findings that already render: issue #10 currently says `GET /positions` modified; the walker says `components.schemas.Position.properties.grossInitialValue added`, which is the actual news.

## Component 4 — `render-issue` subcommand

Reads the artifact directory, writes `issue-body.md`:

````
<summary sections>

<details><summary>Canonicalized diff</summary>

```diff
<canonical unified diff>
```
</details>
````

**Budget:** the composed body must not exceed GitHub's 65536-character cap.

1. Summary has priority.
2. The diff receives the remainder, minus fence and boilerplate overhead.
3. If the summary alone exceeds budget, truncate the summary and omit the diff entirely, replacing it with an explicit pointer to the run artifacts.

The current bash does `DIFF_LIMIT=50000` and concatenates summary + diff + boilerplate, leaving ~15KB of unchecked headroom for the summary. That holds today only because summaries are four lines. Component 3 makes summaries grow, and perps-ws has 814 lines of canonical diff — the cap stops being theoretical exactly when this change lands.

## Component 5 — Workflow edits

**Label creation**, beside the existing idempotent `schema-drift` create:

```
gh label create "spec:${{ matrix.id }}" --force \
  --description "Schema drift tracking for the ${{ matrix.id }} spec" --color 0E8A16
```

**Lookup**, in *both* the open and the close paths:

```
gh issue list --label schema-drift --label "spec:${{ matrix.id }}" \
  --state open --json number --jq 'sort_by(.number) | .[0].number // empty'
```

Multiple `--label` flags are an AND filter. `sort_by(.number)` is not cosmetic: `gh issue list` returns newest-first, so a bare `.[0]` would pick the *newest* duplicate, contradicting the lowest-number rule below and making the duplicate case resolve differently on different runs.

**Create** passes `--label schema-drift --label "spec:${{ matrix.id }}"`.

**Body** switches to `--body-file ${ARTIFACT_DIR}/issue-body.md` for both create and edit. The PR body keeps using the same file; still refused by policy, harmless.

## Data flow

```
fetch upstream
  → check          canonicalize both sides, detect drift,
                   write summary.md + unified-diff.txt
  → render-issue   compose issue-body.md within budget
  → workflow       resolve issue by label intersection, create or edit
```

## Failure handling

| Condition | Behavior |
|---|---|
| Spec parse failure | Exit 2, job fails loudly. Unchanged. |
| Upstream fetch failure | Warn and skip the entry. Unchanged. |
| Missing `spec:` label | `gh label create --force` is idempotent and runs before lookup. |
| Body exceeds cap | Deterministic truncation per Component 4. Never a failed `gh` call. |
| Two open issues share one `spec:` label | Use the lowest number, emit `::warning::`. Cheap, and precisely the condition that would have made the current mess visible on day one. |

## Testing

Fixtures derive from the **real** clob and data spec pairs, so tests encode drift that actually occurred rather than invented shapes. This follows the repo's standing rule that a test must fail without its fix.

| Test | Pins |
|---|---|
| `test_canonicalize_treats_integral_float_as_int` | `3` vs `3.0` → no drift. Fails today. |
| `test_canonicalize_distinguishes_bool_from_string` | bare `Yes` vs `'Yes'` → drift. Guards the bool-subclass trap and pins clob #18's genuine finding. |
| `test_diff_tree_reports_added_schema_property` | The data #10 case — `Position.properties.grossInitialValue`. |
| `test_diff_tree_reports_subtree_root_not_leaves` | A newly added schema yields one pointer. |
| `test_render_issue_body_within_cap` | Oversized summary + oversized diff → body ≤ 65536, summary intact, diff truncated with marker. |
| `test_render_issue_body_drops_diff_when_summary_exceeds_budget` | Summary truncated, diff omitted, artifact pointer present. |

## Rollout

1. Land the Python and workflow changes with tests green.
2. Close #10, #12, #16, #18 with a comment naming this spec as the reason.
3. Trigger `nightly-schema.yml` via `workflow_dispatch`.
4. Verify five issues exist — data, bridge, clob, perps-ws, and **perps**, which has never had one — each carrying exactly one `spec:` label, each stating its findings.

The first run exercises the create path for every entry, which is the path the collision bug corrupted.

## Scope boundary

Deferred to S2, deliberately out of scope here: branch and PR delivery under the no-Actions-PR constraint, a durable won't-sync state so upstream regressions stop refiling, stale-branch cleanup, and severity classification.

## Documentation

Update `CLAUDE.md`'s nightly schema paragraph to describe label-based issue identity and the key-path summary, replacing the current description of PR-body-only diffs.
