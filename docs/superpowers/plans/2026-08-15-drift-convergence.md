# Drift Findings Converge — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every drift finding reaches one of three terminal states — adopted, acknowledged, or recovered — and the nightly workflow stops writing to the repository entirely.

**Architecture:** `diff_openapi.py` gains a SHA-256 fingerprint of the canonical diff, an acknowledgement check driven by a committed JSON file, and paste-ready adopt/acknowledge blocks in the issue body. `nightly-schema.yml` loses its branch and PR machinery, replaces its boolean `drift` output with a three-valued `state`, and drops to read-only repository permissions.

**Tech Stack:** Python 3.11+ (stdlib + PyYAML), pytest, `uv`, GitHub Actions, `gh` CLI.

**Spec:** [docs/superpowers/specs/2026-08-15-drift-convergence-design.md](../specs/2026-08-15-drift-convergence-design.md)

---

## Background an implementer needs

`.github/scripts/diff_openapi.py` is run nightly by a matrix job, once per Polymarket API spec. It compares upstream against the copy vendored in `docs/specs/`, writes `summary.md` / `unified-diff.txt` / `issue-body.md` into an artifact directory, and exits 0 (no drift), 1 (drift), or 2 (parse error). The workflow reads that exit code and files or closes a GitHub issue, identified by a `spec:<id>` label.

Two things change here.

**Branches and PRs go away.** The workflow currently force-pushes a `nightly-schema-drift/<id>` branch holding one line of work — `shutil.copyfile(upstream, vendored)` — then tries to open a PR. Run 31811673456 logged **12** refusals of `gh pr create` and produced **zero** PRs; the block is not fixable here (`can_approve_pull_request_reviews: false`, the org endpoint needs `admin:org`, no PAT secret exists). Since adopting a drift by hand is a single `curl`, the branch buys nothing and costs `contents: write`.

**Acknowledgement replaces perpetual refiling.** Issue #25 (`clob`) reports upstream's own regression: a re-serialization dropped the quotes on `example: 'Yes'`, so YAML now reads bare `Yes` as boolean `true` on a `type: string` field. We will never sync it, and it refiles nightly. A committed acknowledgement file keyed by the diff's SHA-256 silences it — and stops silencing it the moment either side moves.

Run all tests from `.github/scripts`:

```bash
cd .github/scripts && uv run pytest tests/ -v
```

68 tests currently pass.

## File structure

| File | Responsibility | Change |
|---|---|---|
| `.github/scripts/diff_openapi.py` | Fingerprint, acknowledgement check, paste blocks | Modify |
| `.github/scripts/tests/test_diff_openapi.py` | Unit + CLI tests | Modify |
| `.github/workflows/nightly-schema.yml` | Orchestration; loses branch/PR machinery | Rewrite steps |
| `docs/specs/.drift-acknowledged.json` | Human-authored won't-sync decisions | Create |
| `CLAUDE.md` | Nightly schema paragraph | Modify |

---

## Task 1: Diff fingerprint

One definition of the hash, computed in Python and used by both the acknowledgement check and the issue body. Computing it in bash with `sha256sum` would create a second definition of the same concept — the failure class behind both of S1's bugs.

**Files:**
- Modify: `.github/scripts/diff_openapi.py`
- Test: `.github/scripts/tests/test_diff_openapi.py`

- [ ] **Step 1: Write the failing tests**

Add `diff_fingerprint` to the existing `from diff_openapi import (...)` block in `.github/scripts/tests/test_diff_openapi.py`, keeping alphabetical order (it goes after `detect_drift`). Then append:

```python
def test_diff_fingerprint_is_stable() -> None:
    """Same diff text must hash identically across calls, or an
    acknowledgement would expire at random."""
    text = "-old line\n+new line\n"
    assert diff_fingerprint(text) == diff_fingerprint(text)
    assert len(diff_fingerprint(text)) == 64


def test_diff_fingerprint_changes_with_diff() -> None:
    """A one-character change must break the match, so acknowledging one
    disagreement never silences a different one."""
    assert diff_fingerprint("-a\n+b\n") != diff_fingerprint("-a\n+c\n")


def test_diff_fingerprint_of_empty_string() -> None:
    assert len(diff_fingerprint("")) == 64
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -k "fingerprint" -v
```

Expected: FAIL at collection with `ImportError: cannot import name 'diff_fingerprint' from 'diff_openapi'`.

- [ ] **Step 3: Implement it**

Add `import hashlib` to the imports at the top of `.github/scripts/diff_openapi.py`, in alphabetical order (after `import difflib`). Then add this function immediately after `compose_issue_body`:

```python
def diff_fingerprint(diff_text: str) -> str:
    """SHA-256 hex digest of a canonical unified diff.

    Fingerprints the *disagreement* rather than the upstream document, so an
    acknowledgement expires in both directions: it stops matching if upstream
    changes, and also if we adopt part of the drift. Hashing upstream alone
    would stay silent after a partial adoption, exactly when a fresh look is
    warranted.
    """
    return hashlib.sha256(diff_text.encode("utf-8")).hexdigest()
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd .github/scripts && uv run pytest tests/ -v
```

Expected: PASS — 68 existing + 3 new = 71.

- [ ] **Step 5: Commit**

```bash
git add .github/scripts/diff_openapi.py .github/scripts/tests/test_diff_openapi.py
git commit -m "feat(ci): fingerprint the canonical diff

Hashes the disagreement rather than upstream, so an acknowledgement expires
if either side moves rather than blinding us permanently."
```

---

## Task 2: Acknowledgement check and exit code 3

**Files:**
- Modify: `.github/scripts/diff_openapi.py` — `_cmd_check` (lines 300-334), argparse (lines 351-362)
- Test: `.github/scripts/tests/test_diff_openapi.py`

- [ ] **Step 1: Write the failing tests**

Add `load_acknowledged` to the test file's import block (alphabetically, after `diff_tree`). Add `import json` to the test file's imports if not already present. Then append:

```python
def test_load_acknowledged_missing_file_is_empty(tmp_path: Path) -> None:
    """Absent file must mean 'nothing acknowledged', never a crash — the
    failure direction has to be toward speaking up."""
    assert load_acknowledged(tmp_path / "nope.json") == {}


def test_load_acknowledged_malformed_file_is_empty(tmp_path: Path) -> None:
    bad = tmp_path / "bad.json"
    bad.write_text("{not json at all")
    assert load_acknowledged(bad) == {}


def test_load_acknowledged_reads_entries(tmp_path: Path) -> None:
    good = tmp_path / "ack.json"
    good.write_text(json.dumps({"clob": {"diff_sha256": "abc", "reason": "x"}}))
    assert load_acknowledged(good) == {"clob": {"diff_sha256": "abc", "reason": "x"}}


def _run_check(tmp_path: Path, ack_file: Path | None, vendored: Path | None = None):
    """Run `check` on the added-endpoint fixture pair, optionally with an
    acknowledgement file. Returns (CompletedProcess, out_dir)."""
    out_dir = tmp_path / "out"
    out_dir.mkdir(exist_ok=True)
    if vendored is None:
        vendored = FIXTURES / "openapi-added-endpoint" / "old.yaml"
    cmd = [
        sys.executable, str(SCRIPT), "check",
        "--crate", "test",
        "--upstream-yaml", str(FIXTURES / "openapi-added-endpoint" / "new.yaml"),
        "--vendored-yaml", str(vendored),
        "--upstream-url", "https://example.com/test.yaml",
        "--output-dir", str(out_dir),
    ]
    if ack_file is not None:
        cmd += ["--acknowledged-file", str(ack_file)]
    return subprocess.run(cmd, capture_output=True, text=True), out_dir


def test_cli_check_writes_fingerprint_file(tmp_path: Path) -> None:
    result, out_dir = _run_check(tmp_path, None)
    assert result.returncode == 1
    digest = (out_dir / "diff-sha256.txt").read_text().strip()
    assert len(digest) == 64


def test_cli_check_acknowledged_exits_three(tmp_path: Path) -> None:
    _, out_dir = _run_check(tmp_path, None)
    digest = (out_dir / "diff-sha256.txt").read_text().strip()
    ack = tmp_path / "ack.json"
    ack.write_text(json.dumps({"test": {"diff_sha256": digest, "reason": "declined"}}))
    result, _ = _run_check(tmp_path, ack)
    assert result.returncode == 3, result.stderr


def test_cli_check_stale_acknowledgement_exits_one(tmp_path: Path) -> None:
    """A hash that no longer matches must refile. This is the expiry working."""
    ack = tmp_path / "ack.json"
    ack.write_text(json.dumps({"test": {"diff_sha256": "0" * 64, "reason": "stale"}}))
    result, _ = _run_check(tmp_path, ack)
    assert result.returncode == 1


def test_cli_check_acknowledgement_for_other_spec_exits_one(tmp_path: Path) -> None:
    _, out_dir = _run_check(tmp_path, None)
    digest = (out_dir / "diff-sha256.txt").read_text().strip()
    ack = tmp_path / "ack.json"
    ack.write_text(json.dumps({"someothercrate": {"diff_sha256": digest}}))
    result, _ = _run_check(tmp_path, ack)
    assert result.returncode == 1


def test_cli_check_missing_acknowledged_file_exits_one(tmp_path: Path) -> None:
    result, _ = _run_check(tmp_path, tmp_path / "does-not-exist.json")
    assert result.returncode == 1


def test_cli_check_malformed_acknowledged_file_exits_one(tmp_path: Path) -> None:
    bad = tmp_path / "bad.json"
    bad.write_text("{ broken")
    result, _ = _run_check(tmp_path, bad)
    assert result.returncode == 1


def test_cli_check_acknowledged_does_not_apply_upstream(tmp_path: Path) -> None:
    """The one silent-corruption path: acknowledging means we decided NOT to
    sync, so the vendored bytes must survive untouched even with
    --apply-on-drift passed."""
    vendored = tmp_path / "vendored.yaml"
    original = (FIXTURES / "openapi-added-endpoint" / "old.yaml").read_bytes()
    vendored.write_bytes(original)
    out_dir = tmp_path / "out2"
    out_dir.mkdir()

    base = [
        sys.executable, str(SCRIPT), "check",
        "--crate", "test",
        "--upstream-yaml", str(FIXTURES / "openapi-added-endpoint" / "new.yaml"),
        "--vendored-yaml", str(vendored),
        "--upstream-url", "https://example.com/test.yaml",
        "--output-dir", str(out_dir),
    ]
    # First pass with no acknowledgement, to learn the digest.
    subprocess.run(base, capture_output=True, text=True)
    digest = (out_dir / "diff-sha256.txt").read_text().strip()

    ack = tmp_path / "ack.json"
    ack.write_text(json.dumps({"test": {"diff_sha256": digest}}))
    result = subprocess.run(
        base + ["--acknowledged-file", str(ack), "--apply-on-drift"],
        capture_output=True, text=True,
    )
    assert result.returncode == 3
    assert vendored.read_bytes() == original, "acknowledged drift overwrote the mirror"
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -k "acknowledg or fingerprint_file" -v
```

Expected: FAIL at collection with `ImportError: cannot import name 'load_acknowledged' from 'diff_openapi'`.

- [ ] **Step 3: Implement `load_acknowledged`**

Add after `diff_fingerprint` in `.github/scripts/diff_openapi.py`:

```python
def load_acknowledged(path: Path | None) -> dict:
    """Read the acknowledgement file, tolerating absence and corruption.

    Every failure resolves to 'nothing is acknowledged'. Silence must only
    ever be produced by an exact hash match, never by a missing or unreadable
    file — a config mistake has to make the workflow louder, not quieter.
    """
    if path is None or not path.exists():
        return {}
    try:
        parsed = json.loads(path.read_text())
    except (json.JSONDecodeError, OSError) as exc:
        print(f"::warning::could not read {path}: {exc}", file=sys.stderr)
        return {}
    if not isinstance(parsed, dict):
        print(f"::warning::{path} is not a JSON object; ignoring", file=sys.stderr)
        return {}
    return parsed
```

- [ ] **Step 4: Wire it into `_cmd_check`**

Replace the tail of `_cmd_check` (currently lines 329-334, from the `unified-diff.txt` write through `return 1`) with:

```python
    (args.output_dir / "unified-diff.txt").write_text(diff)

    fingerprint = diff_fingerprint(diff)
    (args.output_dir / "diff-sha256.txt").write_text(fingerprint + "\n")

    entry = load_acknowledged(args.acknowledged_file).get(args.crate) or {}
    if entry.get("diff_sha256") == fingerprint:
        # Acknowledged: we decided not to sync, so the mirror must not be
        # overwritten even if --apply-on-drift was passed.
        return 3

    if args.apply_on_drift:
        shutil.copyfile(args.upstream_yaml, args.vendored_yaml)

    return 1
```

- [ ] **Step 5: Add the argparse flag**

In `main`, after the existing `--apply-on-drift` argument and before `p.set_defaults(func=_cmd_check)`:

```python
    p.add_argument("--acknowledged-file", type=Path, default=None,
                   help="JSON file of accepted-as-different specs, keyed by crate id "
                        "with a diff_sha256 field; a match exits 3 instead of 1")
```

- [ ] **Step 6: Run the full suite**

```bash
cd .github/scripts && uv run pytest tests/ -v
```

Expected: PASS — 71 existing + 10 new = 81. (`_run_check` is a shared helper, not a test; pytest ignores it because of the leading underscore.)

- [ ] **Step 7: Commit**

```bash
git add .github/scripts/diff_openapi.py .github/scripts/tests/test_diff_openapi.py
git commit -m "feat(ci): exit 3 for drift we have accepted as permanent

An acknowledged spec must not be synced, so exit 3 also suppresses
--apply-on-drift. Missing or malformed acknowledgement files resolve to
'nothing acknowledged' — silence is only ever produced by an exact match."
```

---

## Task 3: Paste-ready adopt and acknowledge blocks

Both terminal states become copy-paste. The hash is pre-computed so declining a finding never requires reproducing the canonicalization by hand.

The blocks are appended to the summary before composition rather than passed separately, so they inherit the summary's budget priority without new arithmetic. `CHANGE_LIMIT = 200` bounds the summary at roughly 30KB, so the overflow path where these could be truncated is unreachable in practice.

**Files:**
- Modify: `.github/scripts/diff_openapi.py` — add `render_actions`, extend `_cmd_render_issue` (lines 337-343) and its argparse (lines 364-369)
- Test: `.github/scripts/tests/test_diff_openapi.py`

- [ ] **Step 1: Write the failing tests**

Add `render_actions` to the test file's import block (alphabetically, after `load_acknowledged`). Append:

```python
def test_render_actions_contains_adopt_command() -> None:
    text = render_actions(
        spec="clob",
        adopt_url="https://docs.polymarket.com/api-spec/clob-openapi.yaml",
        vendored_path="docs/specs/clob/openapi.yaml",
        fingerprint="a" * 64,
    )
    assert "curl -fsSL https://docs.polymarket.com/api-spec/clob-openapi.yaml -o docs/specs/clob/openapi.yaml" in text


def test_render_actions_contains_acknowledge_snippet_with_hash() -> None:
    text = render_actions(
        spec="clob",
        adopt_url="https://x/clob.yaml",
        vendored_path="docs/specs/clob/openapi.yaml",
        fingerprint="b" * 64,
    )
    assert "docs/specs/.drift-acknowledged.json" in text
    assert '"clob"' in text
    assert '"diff_sha256": "' + "b" * 64 + '"' in text


def test_cli_render_issue_embeds_actions(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    subprocess.run(
        [
            sys.executable, str(SCRIPT), "check",
            "--crate", "clob",
            "--upstream-yaml", str(FIXTURES / "openapi-added-endpoint" / "new.yaml"),
            "--vendored-yaml", str(FIXTURES / "openapi-added-endpoint" / "old.yaml"),
            "--upstream-url", "https://example.com/clob.yaml",
            "--output-dir", str(out_dir),
        ],
        capture_output=True, text=True,
    )
    digest = (out_dir / "diff-sha256.txt").read_text().strip()
    result = subprocess.run(
        [
            sys.executable, str(SCRIPT), "render-issue",
            "--output-dir", str(out_dir),
            "--spec", "clob",
            "--adopt-url", "https://example.com/clob.yaml",
            "--vendored-path", "docs/specs/clob/openapi.yaml",
        ],
        capture_output=True, text=True,
    )
    assert result.returncode == 0, result.stderr
    body = (out_dir / "issue-body.md").read_text()
    assert "curl -fsSL https://example.com/clob.yaml -o docs/specs/clob/openapi.yaml" in body
    assert digest in body
    assert len(body) <= 65536


def test_cli_render_issue_without_action_args_omits_blocks(tmp_path: Path) -> None:
    """The action args are optional so no-drift runs, which have no diff and
    nothing to adopt, still render."""
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    subprocess.run(
        [
            sys.executable, str(SCRIPT), "check",
            "--crate", "test",
            "--upstream-yaml", str(FIXTURES / "openapi-no-drift" / "old.yaml"),
            "--vendored-yaml", str(FIXTURES / "openapi-no-drift" / "new.yaml"),
            "--upstream-url", "https://example.com/test.yaml",
            "--output-dir", str(out_dir),
        ],
        capture_output=True, text=True,
    )
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "render-issue", "--output-dir", str(out_dir)],
        capture_output=True, text=True,
    )
    assert result.returncode == 0, result.stderr
    body = (out_dir / "issue-body.md").read_text()
    assert "### Adopt" not in body
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd .github/scripts && uv run pytest tests/test_diff_openapi.py -k "actions" -v
```

Expected: FAIL at collection with `ImportError: cannot import name 'render_actions' from 'diff_openapi'`.

- [ ] **Step 3: Implement `render_actions`**

Add after `diff_fingerprint` in `.github/scripts/diff_openapi.py`:

```python
def render_actions(spec: str, adopt_url: str, vendored_path: str, fingerprint: str) -> str:
    """Render the two terminal states as copy-paste blocks.

    Adopting is a single curl; declining is a single JSON entry whose hash is
    filled in here so nobody has to reproduce the canonicalization by hand.
    """
    return "\n".join([
        "",
        "## What to do",
        "",
        "### Adopt",
        "",
        "```bash",
        f"curl -fsSL {adopt_url} -o {vendored_path}",
        "```",
        "",
        "### Or acknowledge",
        "",
        "Record the decision in `docs/specs/.drift-acknowledged.json` and this stops",
        "refiling until upstream changes or we adopt part of it:",
        "",
        "```json",
        f'  "{spec}": {{',
        f'    "diff_sha256": "{fingerprint}",',
        '    "reason": "why this will not be synced",',
        '    "acknowledged": "YYYY-MM-DD"',
        "  }",
        "```",
        "",
    ])
```

- [ ] **Step 4: Extend `_cmd_render_issue`**

Replace `_cmd_render_issue` (currently lines 337-343) with:

```python
def _cmd_render_issue(args: argparse.Namespace) -> int:
    summary = (args.output_dir / "summary.md").read_text()
    diff_path = args.output_dir / "unified-diff.txt"
    diff = diff_path.read_text() if diff_path.exists() else ""

    if diff and args.spec and args.adopt_url and args.vendored_path:
        summary += render_actions(
            spec=args.spec,
            adopt_url=args.adopt_url,
            vendored_path=args.vendored_path,
            fingerprint=diff_fingerprint(diff),
        )

    body = compose_issue_body(summary, diff, reserve=args.reserve)
    (args.output_dir / "issue-body.md").write_text(body)
    return 0
```

- [ ] **Step 5: Add the argparse flags**

In `main`, after the existing `--reserve` argument and before `p2.set_defaults(func=_cmd_render_issue)`:

```python
    p2.add_argument("--spec", default=None, help="Spec id, for the acknowledge snippet")
    p2.add_argument("--adopt-url", default=None, help="Upstream URL, for the adopt command")
    p2.add_argument("--vendored-path", default=None,
                    help="Repo-relative vendored file path, for the adopt command")
```

- [ ] **Step 6: Run the full suite**

```bash
cd .github/scripts && uv run pytest tests/ -v
```

Expected: PASS — 81 existing + 4 new = 85.

- [ ] **Step 7: Commit**

```bash
git add .github/scripts/diff_openapi.py .github/scripts/tests/test_diff_openapi.py
git commit -m "feat(ci): put adopt and acknowledge commands in the issue body

Both terminal states become copy-paste, with the diff hash pre-computed so
declining a finding never means reproducing canonicalization by hand."
```

---

## Task 4: Rewrite the workflow

The step list shrinks substantially, so this task replaces the file wholesale rather than applying eight anchored edits. Read the existing file first to confirm you are replacing what you think you are.

**Files:**
- Modify: `.github/workflows/nightly-schema.yml`

- [ ] **Step 1: Replace the file**

Write `.github/workflows/nightly-schema.yml` with exactly this content:

```yaml
name: Nightly schema drift

on:
  schedule:
    - cron: "0 6 * * *"
  workflow_dispatch: {}

# Read-only on the repository. This workflow files issues; it does not commit,
# push branches, or open PRs. Actions cannot open PRs here anyway (org policy),
# and adopting a drift is a one-line curl a maintainer runs from the issue.
permissions:
  contents: read
  issues: write

concurrency:
  group: nightly-schema
  cancel-in-progress: false

jobs:
  check:
    name: Check ${{ matrix.id }}
    runs-on: ubuntu-latest
    timeout-minutes: 10
    strategy:
      fail-fast: false
      matrix:
        # One entry per (upstream URL, vendored mirror) pair. REST contracts
        # are OpenAPI under /api-spec/; WebSocket contracts are AsyncAPI at
        # the docs root. Deliberately absent:
        #   - clob sports AsyncAPI (docs/specs/clob/asyncapi-sports.json):
        #     the mirror is modelled on captured wire frames because
        #     upstream's published contract does not match the wire — diffing
        #     it would report perpetual false drift.
        #   - user-pnl-api / lb-api (docs/specs/undocumented/): no published
        #     spec exists to diff against.
        include:
          - { id: clob,           url: "https://docs.polymarket.com/api-spec/clob-openapi.yaml",       vendored: docs/specs/clob/openapi.yaml }
          - { id: gamma,          url: "https://docs.polymarket.com/api-spec/gamma-openapi.yaml",      vendored: docs/specs/gamma/openapi.yaml }
          - { id: data,           url: "https://docs.polymarket.com/api-spec/data-openapi.yaml",       vendored: docs/specs/data/openapi.yaml }
          - { id: relay,          url: "https://docs.polymarket.com/api-spec/relayer-openapi.yaml",    vendored: docs/specs/relay/openapi.yaml }
          - { id: perps,          url: "https://docs.polymarket.com/api-spec/perps-openapi.json",      vendored: docs/specs/perps/openapi.json }
          - { id: bridge,         url: "https://docs.polymarket.com/api-spec/bridge-openapi.yaml",     vendored: docs/specs/bridge/openapi.yaml }
          - { id: combos-rfq,     url: "https://docs.polymarket.com/api-spec/combos-rfq-openapi.yaml", vendored: docs/specs/combos-rfq/openapi.yaml }
          - { id: clob-ws-market, url: "https://docs.polymarket.com/asyncapi.json",                    vendored: docs/specs/clob/asyncapi-market.json }
          - { id: clob-ws-user,   url: "https://docs.polymarket.com/asyncapi-user.json",               vendored: docs/specs/clob/asyncapi-user.json }
          - { id: perps-ws,       url: "https://docs.polymarket.com/asyncapi-perps.json",              vendored: docs/specs/perps/asyncapi.json }
          - { id: combos-rfq-ws,  url: "https://docs.polymarket.com/asyncapi-rfq.json",                vendored: docs/specs/combos-rfq/asyncapi.json }
    steps:
      - uses: actions/checkout@v5
      - uses: astral-sh/setup-uv@v7
      - run: uv sync
        working-directory: .github/scripts

      - name: Fetch upstream spec (with retries)
        id: fetch
        run: |
          UPSTREAM_URL="${{ matrix.url }}"
          UPSTREAM_FILE=$(mktemp)
          echo "url=$UPSTREAM_URL" >> "$GITHUB_OUTPUT"
          echo "file=$UPSTREAM_FILE" >> "$GITHUB_OUTPUT"
          # 3 retries, 5s initial backoff, doubling each time.
          for delay in 0 5 10 20; do
            sleep "$delay"
            if curl -fsSL --max-time 30 -o "$UPSTREAM_FILE" "$UPSTREAM_URL"; then
              echo "fetched=true" >> "$GITHUB_OUTPUT"
              exit 0
            fi
          done
          echo "fetched=false" >> "$GITHUB_OUTPUT"
          echo "::warning::Failed to fetch ${UPSTREAM_URL} after retries; skipping drift check for this entry."
          # Exit success so the matrix entry doesn't count as "infra failure" — we
          # explicitly choose to skip rather than file false-positive PRs.
          exit 0

      - name: Detect drift
        id: drift
        if: steps.fetch.outputs.fetched == 'true'
        run: |
          set +e
          uv run python ${{ github.workspace }}/.github/scripts/diff_openapi.py check \
            --crate ${{ matrix.id }} \
            --upstream-yaml "${{ steps.fetch.outputs.file }}" \
            --vendored-yaml ${{ github.workspace }}/${{ matrix.vendored }} \
            --upstream-url "${{ steps.fetch.outputs.url }}" \
            --vendored-label "${{ matrix.vendored }}" \
            --output-dir ${{ github.workspace }}/artifacts/${{ matrix.id }} \
            --acknowledged-file ${{ github.workspace }}/docs/specs/.drift-acknowledged.json
          ec=$?
          set -e
          # Three-valued, because three exit codes do not fit in a boolean.
          # `drift` used to be true/false here; leaving it alongside `state`
          # would give one fact two sources of truth.
          if [ "$ec" -eq 0 ]; then
            echo "state=clean" >> "$GITHUB_OUTPUT"
          elif [ "$ec" -eq 3 ]; then
            echo "state=acknowledged" >> "$GITHUB_OUTPUT"
          elif [ "$ec" -eq 1 ]; then
            echo "state=drift" >> "$GITHUB_OUTPUT"
            uv run python ${{ github.workspace }}/.github/scripts/diff_openapi.py render-issue \
              --output-dir ${{ github.workspace }}/artifacts/${{ matrix.id }} \
              --spec "${{ matrix.id }}" \
              --adopt-url "${{ steps.fetch.outputs.url }}" \
              --vendored-path "${{ matrix.vendored }}"
          else
            echo "::error::diff_openapi.py exited with $ec — see logs"
            exit "$ec"
          fi
        working-directory: .github/scripts

      - name: Open or update drift issue
        if: steps.fetch.outputs.fetched == 'true' && steps.drift.outputs.state == 'drift'
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          SPEC_ID: ${{ matrix.id }}
        run: |
          ARTIFACT_DIR="${{ github.workspace }}/artifacts/${{ matrix.id }}"

          # --force makes these idempotent; without the labels existing,
          # `gh issue create --label` fails outright.
          gh label create schema-drift --force \
            --description "Filed by the nightly schema drift check" --color 1D76DB || true
          gh label create "spec:${SPEC_ID}" --force \
            --description "Schema drift tracking for the ${SPEC_ID} spec" --color 0E8A16 || true

          ISSUE_TITLE="Schema drift: ${SPEC_ID}"
          # Label intersection is exact. `--search "<title> in:title"` is a
          # tokenized full-text search, so `perps` also matched `perps-ws` and
          # one spec's job edited and closed another spec's issue.
          MATCHES=$(gh issue list --label schema-drift --label "spec:${SPEC_ID}" \
            --state open --json number --jq 'sort_by(.number) | map(.number) | join(" ")')
          set -- $MATCHES
          if [ "$#" -gt 1 ]; then
            echo "::warning::${SPEC_ID} has $# open drift issues ($MATCHES); using #$1."
          fi
          EXISTING_ISSUE="${1:-}"
          if [ -z "$EXISTING_ISSUE" ]; then
            gh issue create --title "${ISSUE_TITLE}" \
              --label schema-drift --label "spec:${SPEC_ID}" \
              --body-file "${ARTIFACT_DIR}/issue-body.md"
          else
            gh issue edit "$EXISTING_ISSUE" --body-file "${ARTIFACT_DIR}/issue-body.md"
          fi

      - name: Upload drift artifacts
        # Explicitly positive: when the fetch fails this step is reached with an
        # empty `state`, and a `!= 'clean'` test would try to upload artifacts
        # that were never written.
        if: always() && (steps.drift.outputs.state == 'drift' || steps.drift.outputs.state == 'acknowledged')
        uses: actions/upload-artifact@v6
        with:
          name: schema-${{ matrix.id }}
          path: artifacts/${{ matrix.id }}/

      - name: Converge (close the tracking issue)
        # Exit 0 and exit 3 reach the same end state — no open issue — and
        # differ only in the comment, so they share this step.
        if: steps.fetch.outputs.fetched == 'true' && steps.drift.outputs.state != 'drift'
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          if [ "${{ steps.drift.outputs.state }}" = "acknowledged" ]; then
            COMMENT="Drift acknowledged $(date -u +%Y-%m-%d) — the canonical diff still matches the entry in \`docs/specs/.drift-acknowledged.json\`. This will refile automatically if upstream changes or the mirror is partly adopted."
          else
            COMMENT="Drift recovered $(date -u +%Y-%m-%d)"
          fi
          # Must match the open path exactly. The tokenized title search that
          # used to be here is what closed combos-rfq-ws's issue from the
          # combos-rfq job.
          EXISTING_ISSUE=$(gh issue list --label schema-drift --label "spec:${{ matrix.id }}" \
            --state open --json number --jq 'sort_by(.number) | .[0].number // empty')
          if [ -n "$EXISTING_ISSUE" ]; then
            gh issue close "$EXISTING_ISSUE" --comment "$COMMENT"
          fi
```

- [ ] **Step 2: Verify the workflow parses**

```bash
cd .github/scripts && uv run python -c "import yaml; yaml.safe_load(open('../workflows/nightly-schema.yml')); print('parsed OK')"
```

Expected: `parsed OK`

- [ ] **Step 3: Verify all branch and PR machinery is gone**

```bash
grep -nE "gh pr |BRANCH|force-with-lease|git commit|git push|pull-requests|apply-on-drift|contents: write" .github/workflows/nightly-schema.yml || echo "all removed"
```

Expected: `all removed`

- [ ] **Step 4: Verify no boolean `drift` output survives**

```bash
grep -n "outputs.drift" .github/workflows/nightly-schema.yml || echo "no boolean drift output remains"
```

Expected: `no boolean drift output remains`

- [ ] **Step 5: Verify both issue lookups still use label intersection**

```bash
grep -c '\-\-label "spec:' .github/workflows/nightly-schema.yml
```

Expected: `3` — one label creation plus the two lookups (open path and converge path). A count below 3 means a lookup regressed to title matching.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/nightly-schema.yml
git commit -m "fix(ci): drop branch and PR machinery from the nightly drift check

The branch held one line of work — copying upstream bytes over the vendored
mirror — which a maintainer does with a single curl from the issue. For that
convenience the workflow held contents: write, force-pushed six branches
nightly, and attempted PRs that org policy refuses (12 refusals, 0 PRs in run
31811673456). Permissions drop to contents: read plus issues: write.

The boolean drift output becomes three-valued state, since exit codes 0, 1
and 3 do not fit in a boolean; clean and acknowledged share one converge step."
```

---

## Task 5: Acknowledgement file and documentation

Bootstraps `clob`'s entry against the live upstream spec, so the first run after this lands closes #25 rather than refiling it.

**Files:**
- Create: `docs/specs/.drift-acknowledged.json`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Compute clob's current diff fingerprint**

```bash
cd /tb/Source/DilettanteTrading/polyoxide/.loom/worktrees/aidanb/issues_18cba355a50e009e
curl -fsSL https://docs.polymarket.com/api-spec/clob-openapi.yaml -o /tmp/clob-upstream.yaml
cd .github/scripts && uv run python diff_openapi.py check \
  --crate clob \
  --upstream-yaml /tmp/clob-upstream.yaml \
  --vendored-yaml ../../docs/specs/clob/openapi.yaml \
  --upstream-url https://docs.polymarket.com/api-spec/clob-openapi.yaml \
  --vendored-label docs/specs/clob/openapi.yaml \
  --output-dir /tmp/clob-out
cat /tmp/clob-out/diff-sha256.txt
```

Expected: exit code 1 and a 64-character digest.

Also run `cat /tmp/clob-out/summary.md` and confirm the only finding is the `example` pointer changing from `"Yes"` to `true`. **If the summary shows anything else, STOP and report** — the drift has changed since this plan was written, and acknowledging it would silence something nobody has reviewed.

- [ ] **Step 2: Create the acknowledgement file**

Write `docs/specs/.drift-acknowledged.json`, substituting the digest from Step 1 for `<DIGEST>`:

```json
{
  "clob": {
    "diff_sha256": "<DIGEST>",
    "reason": "Upstream's re-serialization dropped the quotes on `example: 'Yes'` for the outcome-label field, so YAML now parses it as boolean true on a `type: string` property. Adopting these bytes would import an invalid example into the mirror. Revisit if upstream fixes the quoting.",
    "acknowledged": "2026-08-15"
  }
}
```

- [ ] **Step 3: Verify the acknowledgement takes effect**

```bash
cd .github/scripts && uv run python diff_openapi.py check \
  --crate clob \
  --upstream-yaml /tmp/clob-upstream.yaml \
  --vendored-yaml ../../docs/specs/clob/openapi.yaml \
  --upstream-url https://docs.polymarket.com/api-spec/clob-openapi.yaml \
  --vendored-label docs/specs/clob/openapi.yaml \
  --output-dir /tmp/clob-out2 \
  --acknowledged-file ../../docs/specs/.drift-acknowledged.json
echo "exit=$?"
```

Expected: `exit=3`.

Then confirm the mirror was untouched:

```bash
cd /tb/Source/DilettanteTrading/polyoxide/.loom/worktrees/aidanb/issues_18cba355a50e009e && git status --short docs/specs/clob/openapi.yaml
```

Expected: no output.

- [ ] **Step 4: Update CLAUDE.md**

Find this text in the `nightly-schema.yml` bullet:

```markdown
On drift, opens an auto-PR (deterministic branch `nightly-schema-drift/<id>`) and a tracking issue labelled `schema-drift` **and** `spec:<id>`.
```

Replace it with:

```markdown
On drift, files a tracking issue labelled `schema-drift` **and** `spec:<id>`. It creates no branches and opens no PRs — Actions cannot open PRs here (org policy: 12 refusals, 0 PRs in run 31811673456), and adopting a drift is one `curl`, which the issue body spells out. The workflow holds `contents: read` and `issues: write` only.
```

Then find:

```markdown
The PR commits Polymarket's raw upstream bytes.
```

Replace it with:

```markdown
A spec we deliberately will not sync is recorded in `docs/specs/.drift-acknowledged.json`, keyed by the SHA-256 of the canonical diff, which makes the check exit 3 and close the issue. Fingerprinting the *disagreement* rather than upstream means the acknowledgement expires the moment either side moves, so it is never permanent blindness. `clob` is acknowledged because upstream's own re-serialization made `example: 'Yes'` parse as boolean `true` on a `type: string` field.
```

- [ ] **Step 5: Verify CLAUDE.md no longer describes branches or PRs for this workflow**

```bash
grep -n "nightly-schema-drift" CLAUDE.md || echo "no stale branch references"
```

Expected: `no stale branch references`

- [ ] **Step 6: Run the full suite once more**

```bash
cd .github/scripts && uv run pytest tests/ -v
```

Expected: 85 passed.

- [ ] **Step 7: Commit**

```bash
git add docs/specs/.drift-acknowledged.json CLAUDE.md
git commit -m "chore(specs): acknowledge clob's upstream quoting regression

Upstream dropped the quotes on \`example: 'Yes'\`, so YAML reads it as boolean
true on a type: string field. We will not import that, and it has refiled
nightly since 2026-08-13. Keyed by the canonical diff's hash, so it refiles
again the moment upstream changes."
```

---

## Task 6: Migration and rollout

**Confirm with the user before executing.** Deletes remote branches and dispatches a workflow that closes a real issue. Runs only after Tasks 1-5 are merged to `main`, because `gh workflow run` dispatches from the default branch.

**Files:** none — GitHub and remote state only.

- [ ] **Step 1: Confirm the work is on main**

```bash
git fetch origin main -q && git log --oneline -1 origin/main
```

Expected: a commit that includes this work. If the branch has not been merged, STOP.

- [ ] **Step 2: Delete the six drift branches**

```bash
for b in bridge clob combos-rfq-ws data perps perps-ws; do
  git push origin --delete "nightly-schema-drift/$b"
done
```

- [ ] **Step 3: Confirm no drift branches remain**

```bash
git ls-remote --heads origin 'nightly-schema-drift/*' | wc -l
```

Expected: `0`

- [ ] **Step 4: Dispatch the workflow**

```bash
gh workflow run nightly-schema.yml --ref main
sleep 30
gh run list --workflow=nightly-schema.yml --limit 1 --json databaseId,status -q '.[] | "run=\(.databaseId) status=\(.status)"'
```

- [ ] **Step 5: Verify the outcome once the run completes**

```bash
gh run view <RUN_ID> --json conclusion -q .conclusion
gh issue list --label schema-drift --state open --json number,title --jq '.[] | "\(.number) \(.title)"'
git ls-remote --heads origin 'nightly-schema-drift/*' | wc -l
```

Expected: conclusion `success`; **five** open issues (`data`, `bridge`, `perps`, `perps-ws`, `combos-rfq-ws`) with `clob` absent; `0` branches.

- [ ] **Step 6: Confirm clob closed for the right reason**

```bash
gh issue list --label "spec:clob" --state closed --limit 1 --json number -q '.[0].number' \
  | xargs -I{} gh issue view {} --json comments -q '.comments[-1].body'
```

Expected: the "Drift acknowledged" comment. A "Drift recovered" comment instead would mean upstream fixed its quoting rather than the acknowledgement firing — a different outcome worth knowing about.

- [ ] **Step 7: Confirm no PR attempt was logged**

```bash
gh run view <RUN_ID> --log 2>/dev/null | grep -c "not permitted to create or approve pull requests"
```

Expected: `0`. Run 31811673456 logged 12.

---

## Out of scope

S3: `polyoxide-data` support for `GET /v1/approvals` and the `Position` fee-basis fields (`grossInitialValue`, `entryFeesUsdc`), plus the per-outcome `REDEEM` row semantics on `/activity`. The `data` issue stays open as its tracking ticket.
