# Retire Data API v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The owner chose inline execution (superpowers:executing-plans) on 2026-11-04.

**Goal:** Remove Polymarket's retired Data API v1 from polyoxide, make v2 the `data` module's only API at its root, and make v2's records constructible (prader-rs #179).

**Architecture:** Six commits, each building and passing the gates it touches. The spec is `docs/superpowers/specs/2026-11-04-retire-data-v1-design.md` (D1–D16), and where this plan and the spec disagree, the spec binds. The order: teach the test-body gate to retire a test by its removal entry; move the CLOB's two v1 readers onto v2; move the send-loop proofs onto v2; delete v1 workspace-wide; flatten `data::v2` into `data`; then fix up the docs and spec mirrors.

**Tech Stack:** Rust 1.91 MSRV / 1.99.0 dev shell, mockito, PyO3 + maturin (`uv`), Python gate scripts under `scripts/` with pytest under `.github/scripts/tests/`.

**Ground rules for every task:**

- Run every cargo and gate command through `direnv exec .` from the worktree root. Never use `nix develop --command`.
- Stage by path (`git add <paths>`, `git rm <paths>`, `git mv`), never `-A` or `.`. Commit with `git commit -- <paths>` or after path-staging. End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_013q61bHPZYguEucdtVYY9QG
  ```
- Scratch files go in `/tmp/claude-1000/-tb-Source-DilettanteTrading-polyoxide--loom-worktrees-aidanb-retire-data-v1-18e4d4336e910aa7/fd24225c-ee00-46cd-8618-52a533dd4fa0/scratchpad` (`$SCRATCH` below), never in the worktree.
- Run the removals gate with `env RUSTFLAGS= RUSTC_WRAPPER= CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 direnv exec . python3 scripts/api_removals.py check --baseline v0.40.0 2>&1`. Edit nothing while it runs, and run nothing alongside cargo-hack.
- The test-body gate pairs only tracked files. Stage moves before running `direnv exec . python3 scripts/test_body_diff.py --baseline v0.40.0`.
- A `[[drift]]` entry's sha256 is the one the gate prints for that item. An entry pins one exact diff, so a later commit that changes the same body replaces the entry's sha256.

```bash
export SCRATCH=/tmp/claude-1000/-tb-Source-DilettanteTrading-polyoxide--loom-worktrees-aidanb-retire-data-v1-18e4d4336e910aa7/fd24225c-ee00-46cd-8618-52a533dd4fa0/scratchpad
```

---

## File map

| Area | Files | Tasks |
|---|---|---|
| Gate | `scripts/test_body_diff.py`, `.github/scripts/tests/test_test_body_diff.py`, `ci/test-body-diff.toml` (header), `.claude/rules/ci-gates.md` | 1 |
| CLOB | `polyoxide-polymarket/src/clob/venue.rs`, `src/clob/venue/trading.rs`, `tests/trading_clob.rs`, `tests/venue_clob.rs`, `tests/fixtures/clob/venue/*`, `scripts/capture_polymarket_clob.py`, `.claude/rules/polymarket-clob.md` | 2, 5 |
| DRIFT | `_bmad-output/specs/spec-venue-extensibility/divergences.md` (R27), `ci/test-body-diff.toml` | 2–5 |
| Send-loop proofs | `polyoxide-polymarket/tests/mock_data.rs`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py` (its `SNIPPETS`) | 3, 4, 5 |
| v1 deletion | `polyoxide-polymarket/src/data/{api/,types.rs,client.rs,mod.rs,README.md}`, `src/shared/limits.rs`, `tests/{mock_data,query_setters_data,headers_data,error_classes_data,holders_shape_data,live_data}.rs`, `tests/common/`, `examples/closed_positions_*.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-cli/src/commands/polymarket/data/mod.rs`, `polyoxide-cli/tests/live_api.rs`, `polyoxide-py/src/{clients,types}/data.rs`, `polyoxide-py/python/polyoxide/__init__.py(i)`, `polyoxide-py/tests/*.py`, `scripts/compare_v2_overlaps.py`, `docs/s3-removals.md` | 4 |
| Flatten | `polyoxide-polymarket/src/data/**`, `src/lib.rs`, `src/clob/venue*.rs`, the CLI's data commands, `polyoxide-py/src/**`, `polyoxide-py/python/polyoxide/*`, the `*_data_v2` test files, `tests/fixtures/data/v2/`, `scripts/{gen_data_v2_types,capture_polymarket_data}.py`, `examples/v2_soak/` | 5 |
| Docs and specs | `docs/specs/data/`, `docs/specs/undocumented/`, root `Cargo.toml` mirrors, generated regions, `.claude/rules/*.md`, CLAUDE.md, READMEs, `docs/s3-migration.md` | 6 |

---

## Task 1: A retired test may cite its removal (D9)

**Files:**
- Modify: `scripts/test_body_diff.py` (`Entry` ~1486, `_entries` ~1564, `parse_config` ~1593, `compare`'s retirement loop ~1956, `report` ~2030, `main` ~2088, module docstring ~76)
- Modify: `.github/scripts/tests/test_test_body_diff.py`
- Modify: `ci/test-body-diff.toml` (the `[[retired]]` header comment, lines 80–87)
- Modify: `.claude/rules/ci-gates.md` (the test-bodies "Fails" bullet)

- [ ] **Step 1: Write the failing tests.** In `.github/scripts/tests/test_test_body_diff.py`, add these four parametrize rows to `test_a_malformed_config_cannot_be_decided`'s list, after the `[[retired]] … * 2` row:

```python
    ("[[retired]]\nfile = \"a\"\nitem = \"t\"\n", "exactly one of row or removal"),
    ("[[retired]]\nfile = \"a\"\nitem = \"t\"\nrow = \"x::Y\"\nremoval = \"k\"\n",
     "exactly one of row or removal"),
    ("[[retired]]\nfile = \"a\"\nitem = \"t\"\nremoval = \"\"\n", "exactly one of row or removal"),
    (f"[[drift]]\nfile = \"a\"\nitem = \"t\"\nrow = \"R6\"\nsha256 = \"{SHA}\"\nremoval = \"k\"\n",
     "has unknown \\['removal'\\]"),
```

Add this test after `test_a_malformed_config_cannot_be_decided`:

```python
def test_a_retirement_may_cite_a_removal_instead_of_a_row() -> None:
    config = body_diff.parse_config(
        "[[retired]]\nfile = \"a/tests/x.rs\"\nitem = \"t\"\n"
        "removal = \"polyoxide-x crate_missing\"\n")
    assert config.retired[0].removal == "polyoxide-x crate_missing"
    assert config.retired[0].row == ""
```

Change the `compare` helper (line ~774) so the tests can pass the listing:

```python
def compare(repo: Path, rows=(), config: Config | None = None,
            removals: frozenset[str] = frozenset(), listing: str = ""):
    return body_diff.compare(repo, "v0", tuple(rows), config or Config(), DRIFT_ROWS,
                             removals=removals, listing=listing)
```

Add these after `test_a_retirement_of_a_test_that_is_not_lost_is_stale`:

```python
KEY = "polyoxide-x function_missing: polyoxide_x::add (src/lib.rs)"


def retired_by(removal: str) -> Config:
    return Config(retired=(body_diff.Entry("polyoxide-x/tests/mock_api.rs", "another", "fn",
                                           "", removal=removal),))


def test_a_test_retired_with_a_listed_removal_passes(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved(without_another()))
    status, out, err = reported(compare(repo, [CRATE_ROW], retired_by(KEY),
                                        frozenset({KEY}), "docs/s3-removals.md"))
    assert (status, err) == (0, "")
    assert (f"Retired with the removal `{KEY}`: "
            "polyoxide-x/tests/mock_api.rs:15 fn another") in out


def test_a_retirement_citing_an_unlisted_removal_fails(repo: Path) -> None:
    baseline(repo, TREE)
    edit(repo, moved(without_another()))
    result = compare(repo, [CRATE_ROW], retired_by(KEY),
                     frozenset({"polyoxide-y crate_missing"}), "docs/s3-removals.md")
    assert result.status == 1 and len(result.lost) == 1
    assert result.problems == [f"`[[retired]]` for polyoxide-x/tests/mock_api.rs fn another "
                               f"cites the removal `{KEY}`, which docs/s3-removals.md does "
                               f"not list under `## Removed`"]


def test_a_retirement_by_removal_of_a_test_that_is_not_lost_is_stale(repo: Path) -> None:
    baseline(repo, TREE)
    result = compare(repo, config=retired_by(KEY), removals=frozenset({KEY}))
    assert result.status == 1 and "is stale" in result.problems[0]


def test_main_reads_the_stage_listing_only_when_a_retirement_cites_a_removal(
        tmp_path: Path) -> None:
    (tmp_path / "Cargo.toml").write_text(
        "[workspace]\n[workspace.metadata.polyoxide]\nstage = \"S3\"\n")
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs/s3-removals.md").write_text(
        "---\nbaseline: v0.40.0\n---\n\n# S3 removals\n\n## Removed\n\n"
        f"- `{KEY}`: story 4; use `polyoxide_x::sum`.\n\n"
        "## Doc-hidden paths consumers import\n\n")
    assert body_diff._removals(tmp_path) == (frozenset({KEY}), "docs/s3-removals.md")
```

(If `api_removals.parse_listing` rejects that listing text, copy the header and section layout from `docs/s3-removals.md` exactly. The test pins only `_removals`' return value.)

- [ ] **Step 2: Run them and see them fail.**

Run: `cd .github/scripts && direnv exec . uv run pytest tests/test_test_body_diff.py -q -k "removal or malformed or main_reads" 2>&1 | tail -15`
Expected: FAIL. The new parametrize rows raise nothing (no `exactly one of` check yet), `Entry` takes no `removal`, `compare` takes no `removals`, and `_removals` is missing.

- [ ] **Step 3: Implement.** In `scripts/test_body_diff.py`:

`Entry` gains a field, as the last one:

```python
    sha256: str = ""  # `[[drift]]` only
    removal: str = ""  # `[[retired]]` only, in place of `row`: a key of the stage's listing
```

Replace `_entries` with this version, which adds a `one_of` group:

```python
def _entries(data: dict, name: str, fields: tuple[str, ...],
             one_of: tuple[str, ...] = ()) -> tuple[Entry, ...]:
    entries = data.get(name, [])
    if not isinstance(entries, list):
        raise BodyDiffError(f"{CONFIG}: `{name}` must be an array of tables, `[[{name}]]`")
    out = []
    for n, entry in enumerate(entries, 1):
        where = f"`[[{name}]]` entry {n}"
        if not isinstance(entry, dict):
            raise BodyDiffError(f"{CONFIG}: {where} is not a table")
        unknown = sorted(set(entry) - set(fields) - set(one_of) - {"kind"})
        missing = [f for f in fields if not isinstance(entry.get(f), str) or not entry[f]]
        if unknown or missing:
            raise BodyDiffError(f"{CONFIG}: {where} must give {', '.join(fields)} as strings, "
                                f"and `kind` optionally; it lacks {missing or 'nothing'} and "
                                f"has unknown {unknown or 'nothing'}")
        if one_of:
            given = [f for f in one_of if f in entry]
            if (len(given) != 1 or not isinstance(entry[given[0]], str)
                    or not entry[given[0]]):
                raise BodyDiffError(f"{CONFIG}: {where} must give exactly one of "
                                    f"{' or '.join(one_of)}, as a string")
        kind = entry.get("kind", "fn")
        if kind not in KINDS:
            raise BodyDiffError(f"{CONFIG}: {where} has kind {kind!r}, not one of {KINDS}")
        if name == "drift" and not SHA256.match(entry["sha256"]):
            raise BodyDiffError(f"{CONFIG}: {where} has sha256 {entry['sha256']!r}, which is "
                                f"not 64 lowercase hex digits")
        found = Entry(entry["file"], entry["item"], kind, entry.get("row", ""),
                      entry.get("sha256", ""), entry.get("removal", ""))
        if any(other.key == found.key for other in out):
            raise BodyDiffError(f"{CONFIG}: {where} names {found.where}, which an earlier "
                                f"`[[{name}]]` entry names too")
        out.append(found)
    return tuple(out)
```

In `parse_config`'s return, the retired line becomes:

```python
                  _entries(data, "retired", ("file", "item"), one_of=("row", "removal")),
```

`compare`'s signature gains two keyword parameters:

```python
def compare(root: Path, baseline: str, rows: tuple[RowLike, ...], config: Config,
            valid_drift: frozenset[str], old_tree: Tree | None = None,
            new_tree: Tree | None = None, features: tuple[RowLike, ...] = (),
            removals: frozenset[str] = frozenset(), listing: str = "") -> Result:
```

In its lost-item loop, after `if entry is None: … continue`, insert:

```python
        if entry.removal:
            if entry.removal in removals:
                result.retired.append((item, entry))
            else:
                result.problems.append(f"`[[retired]]` for {entry.where} cites the removal "
                                       f"`{entry.removal}`, which {listing or 'the listing'} "
                                       f"does not list under `## Removed`")
                result.lost.append(item)
            continue
```

In `report`, the retired lines become:

```python
    for item, entry in result.retired:
        if entry.removal:
            print(f"Retired with the removal `{entry.removal}`: {item.where}", file=out)
        else:
            print(f"Retired with the manifest's `{entry.row}`: {item.where}", file=out)
```

Add `_removals` beside `_manifest`:

```python
def _removals(root: Path) -> tuple[frozenset[str], str]:
    """The `## Removed` keys of the stage's removal listing, and the listing's path."""
    try:
        import api_removals
    except ImportError as error:
        raise BodyDiffError(f"could not import scripts/api_removals.py: {error}") from error
    try:
        listing = api_removals.stage_listing(root)
        return api_removals.read_listing(root, listing).removed, listing
    except api_removals.RemovalsError as error:
        raise BodyDiffError(str(error)) from error
```

In `main`, before `result = compare(...)`:

```python
        removals, listing = frozenset(), ""
        if any(entry.removal for entry in config.retired):
            removals, listing = _removals(root)
        result = compare(root, baseline, tuple(manifest.rows), config, valid,
                         features=tuple(manifest.features), removals=removals,
                         listing=listing)
```

In the module docstring (around line 77), replace "unless a `[[retired]]` entry names it and a manifest row giving a reason, not a new path, covers the entry's `row`." with "unless a `[[retired]]` entry names it and either a manifest row giving a reason, not a new path, covers the entry's `row`, or the entry's `removal` is a key under `## Removed` in the stage's removal listing (`docs/s3-removals.md` in S3)."

In the lost-item hint printed by `report` ("…or retire it with a `[[retired]]` entry citing the manifest row whose reason covers its subject."), append " or the removal listing's key for it".

- [ ] **Step 4: Run them and see them pass, then the whole file.**

Run: `cd .github/scripts && direnv exec . uv run pytest tests/test_test_body_diff.py -q 2>&1 | tail -3`
Expected: all pass.

- [ ] **Step 5: Document the form.** In `ci/test-body-diff.toml`'s header, replace the `[[retired]]` block (lines 80–86) with:

```toml
# [[retired]]
#   file    = "<baseline file>"
#   item    = "<module path and name>"
#   kind    = "fn"                      (optional)
#   row     = "<old path in docs/rename-manifest.md>"
#   removal = "<key under ## Removed in the stage's listing>"   in place of `row`
#   Lets a test go with its subject. An entry gives `row` or `removal`, not both. The
#   manifest row covering `row`, the longest old path that is a prefix of it at `::`,
#   must give a reason, not a new path; a `removal` must be a key the stage's removal
#   listing (docs/s3-removals.md in S3) holds under `## Removed`.
```

In `.claude/rules/ci-gates.md`, replace "on a lost item unless a `[[retired]]` entry names it and an old path whose covering manifest row gives a reason;" with "on a lost item unless a `[[retired]]` entry names it and either an old path whose covering manifest row gives a reason (`row`) or a key under `## Removed` in the stage's removal listing (`removal`, S3's way: the manifest is S2's and final);".

- [ ] **Step 6: Gates.** These must stay green, with nothing retired yet:

```bash
direnv exec . python3 scripts/test_body_diff.py --baseline v0.40.0 2>&1 | tail -2
cd .github/scripts && direnv exec . uv run pytest tests/ -q 2>&1 | tail -3; cd -
```

Expected: the test-body gate exits 0 with "0 entries stale or invalid", and the scripts suite passes.

- [ ] **Step 7: Review (full three layers: it is a gate).** Dispatch three reviewers over `git diff`: blind adversarial, edge cases (an empty `removal`, both keys given, an entry with a `removal` while the stage is S2, a listing that fails to parse), and verification gaps (could an entry pass vacuously?). Fix what they confirm.

- [ ] **Step 8: Commit.**

```bash
git add scripts/test_body_diff.py .github/scripts/tests/test_test_body_diff.py ci/test-body-diff.toml .claude/rules/ci-gates.md
git commit -m "feat(ci): a retired test may cite its removal in the stage's listing

A [[retired]] entry gives \`removal = \"<key>\"\` in place of \`row\`: the key
must be under \`## Removed\` in the stage's removal listing
(docs/s3-removals.md in S3), since S2's rename manifest is final. An entry
gives exactly one of the two; an unlisted key fails, and a stale entry
fails as before.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013q61bHPZYguEucdtVYY9QG"
```

---

## Task 2: The CLOB reads v2 (D4, D5, R27)

**Files:**
- Modify: `polyoxide-polymarket/src/clob/venue.rs` (the module docs' table row for `trades`, lines 13–18; `mod trades`, ~302–370)
- Modify: `polyoxide-polymarket/src/clob/venue/trading.rs` (imports line 19; `PositionExt` ~107–134; `positions` ~721–758; the constants ~780–784; `position` ~884–904)
- Modify: `polyoxide-polymarket/tests/trading_clob.rs` (~1011–1168), `tests/venue_clob.rs` (~293–345)
- Modify: `scripts/capture_polymarket_clob.py`; re-capture `polyoxide-polymarket/tests/fixtures/clob/venue/`
- Modify: `_bmad-output/specs/spec-venue-extensibility/divergences.md` (R27 row and the heading's date list)
- Modify: `ci/test-body-diff.toml` (R27 `[[drift]]` entries), `docs/s3-removals.md`, `.claude/rules/polymarket-clob.md`

- [ ] **Step 1: Add DRIFT R27.** In `divergences.md`, append `, R27 2026-11-04` inside the `## DRIFT: decided …` heading's parenthesis, and add this row after R26:

```markdown
| R27 | Polymarket retired Data API v1 on 2026-11-02: every v1 route on `data-api.polymarket.com`, and the `user-pnl-api` and `lb-api` hosts, answer `410 Gone`. polyoxide removes v1 and flattens v2 into the `data` module's root (spec `2026-11-04-retire-data-v1-design.md`) | the test items `ci/test-body-diff.toml`'s `[[drift]]` entries name with row `R27`; the `[[retired]]` entries that cite a `docs/s3-removals.md` key of the same change | The CLOB's `Trades` and `Trading::positions` read v2, so their mocks serve v2 rows on v2 routes; the send-loop tests that drove v1 routes drive v2 routes with their assertions unchanged; a test whose body names `data.v2()` or a v1 row drops it; a test of v1 alone goes with its subject. Each changed body keeps its assertions, except where a v1 fact has no v2 counterpart (the offset ceiling of `/positions`), and is acknowledged by its sha256. |
```

- [ ] **Step 2: Rewrite the positions tests against v2 (they fail first).** In `tests/trading_clob.rs`, replace `position_row`, `positions_query` and `position_rows` with:

```rust
/// A `/v2/positions` row with the token `token`.
fn position_row(token: String) -> Value {
    json!({
        "archived": false,
        "avg_price": 0.4,
        "condition_id": MARKET,
        "current_price": 0.5,
        "current_size": 12.5,
        "current_value": 6.25,
        "end_date": "",
        "entry_cost_usdc": 5.0,
        "entry_fees_usdc": 0.0,
        "event_id": "1",
        "event_slug": "an-event",
        "first_entry_at": 1_792_000_000_i64,
        "icon": "",
        "last_event_at": 1_792_000_000_i64,
        "mergeable": false,
        "name": "",
        "negative_risk": false,
        "opposite_outcome": "No",
        "opposite_token_id": "101",
        "outcome": "Yes",
        "outcome_index": 0,
        "percent_pnl": 25.0,
        "percent_realized_pnl": 0.0,
        "profile_image": "",
        "proxy_wallet": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "realized_pnl": 0.0,
        "redeemable": false,
        "slug": "a-market",
        "status": "OPEN",
        "title": "A market",
        "token_id": token,
        "total_cost_usdc": 5.0,
        "total_pnl": 1.25,
        "total_size": 12.5,
        "unrealized_pnl": 1.25,
        "verified": false
    })
}

/// The query of one `/v2/positions` page: the maker, every open (and so
/// redeemable) row, no dust filter, 500 rows, and the cursor after the first.
fn positions_query(user: &str, cursor: Option<&str>) -> Matcher {
    let mut matchers = vec![
        Matcher::UrlEncoded("user".into(), user.into()),
        Matcher::UrlEncoded("status".into(), "OPEN".into()),
        Matcher::UrlEncoded("filter_type".into(), "TOKENS".into()),
        Matcher::UrlEncoded("filter_amount".into(), "0".into()),
        Matcher::UrlEncoded("limit".into(), "500".into()),
    ];
    if let Some(cursor) = cursor {
        matchers.push(Matcher::UrlEncoded("cursor".into(), cursor.into()));
    }
    Matcher::AllOf(matchers)
}

/// One `/v2/positions` page of `count` rows, pointing at `next` when given.
fn position_page(first: usize, count: usize, next: Option<&str>) -> String {
    let rows: Vec<Value> = (first..first + count)
        .map(|n| position_row((1_000 + n).to_string()))
        .collect();
    json!({
        "data": rows,
        "pagination": {
            "limit": 500,
            "offset": first,
            "has_more": next.is_some(),
            "next_cursor": next
        }
    })
    .to_string()
}
```

The `Position` struct's fields are listed in `src/data/v2/types/wallet.rs` (~227–312). If the struct has a required field the row above lacks, decoding fails with serde's "missing field" message. Add that field from the struct, and from `tests/fixtures/data/v2/positions.json` for its spelling.

Replace `positions_pages_by_offset_with_no_dust_filter_for_the_maker_address` with:

```rust
#[tokio::test]
async fn positions_walk_every_cursor_page_with_no_dust_filter_for_the_maker_address() {
    let mut server = Server::new_async().await;
    let clob = client(&server);
    let user = clob.account().unwrap().address().to_string();
    let first = server
        .mock("GET", "/v2/positions")
        .match_query(positions_query(&user, None))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(position_page(0, 500, Some("c1")))
        .expect(1)
        .create_async()
        .await;
    let second = server
        .mock("GET", "/v2/positions")
        .match_query(positions_query(&user, Some("c1")))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(position_page(500, 3, None))
        .expect(1)
        .create_async()
        .await;

    let positions = clob.positions().await.unwrap();

    assert_eq!(positions.len(), 503);
    assert_eq!(positions[0].key, keys::clob(&TokenId::new("1000").unwrap()));
    assert_eq!(positions[0].size, d("12.5"));
    assert_eq!(positions[0].average_price, Some(d("0.4")));
    let ext = positions[0].extensions.get::<PositionExt>().unwrap();
    assert_eq!(ext.condition_id.as_deref(), Some(MARKET));
    assert_eq!(ext.current_price, Some(d("0.5")));
    assert_eq!(ext.entry_cost, Some(d("5")));
    assert_eq!(ext.unrealized_pnl, Some(d("1.25")));
    assert_eq!(ext.outcome_index, Some(0));
    assert_eq!(ext.end_date, None, "v2 sends an empty end date for none");
    assert_eq!(ext.negative_risk, Some(false));
    first.assert_async().await;
    second.assert_async().await;
}
```

Check how `ListPositions` sends the cursor before relying on `cursor=c1` in that matcher: read `Paged::send` in `src/data/v2/envelope.rs`, and change the key in `positions_query` if it sends another.

In `a_proxy_client_lists_the_positions_of_its_proxy_wallet`, change the mock path to `"/v2/positions"`, the query to `positions_query(PROXY, None)` and the body to `position_page(0, 2, None)`.

Delete `positions_refuse_a_wallet_with_more_rows_than_the_offset_reaches`. v2 pages by cursor with no offset ceiling, so its subject is gone (R27). It is retired by a `removal` entry in step 9.

- [ ] **Step 3: Run them and see them fail.**

Run: `direnv exec . cargo test -p polyoxide-polymarket --features clob-ws,data,gamma --test trading_clob positions 2>&1 | tail -15`
Expected: compile error: no field `entry_cost` / `unrealized_pnl` on `PositionExt`.

- [ ] **Step 4: Port `Trading::positions`.** In `src/clob/venue/trading.rs`, line 19 becomes:

```rust
use crate::data::v2::types::{FilterType, Position as DataPosition, PositionStatus};
use futures_util::StreamExt;
```

(Keep the file's existing import grouping. `cargo fmt` sorts the group.)

In `PositionExt`, replace the two fields:

```rust
    /// The fee-exclusive cost of the position's entry, in USDC (v2's
    /// `entry_cost_usdc`).
    pub entry_cost: Option<Decimal>,
    /// The unrealized profit and loss, in USDC.
    pub unrealized_pnl: Option<Decimal>,
```

and the `end_date` doc becomes `/// When the market ends, as the Data API writes it; \`None\` when it sends none.`

Replace the body of `positions` after the `maker` binding:

```rust
        // `OPEN` also returns redeemable rows. No dust filter: v2's default
        // floor of 0.1 shares hides small positions.
        let mut pages = self
            .data
            .v2()
            .positions(maker.to_string())
            .status(PositionStatus::Open)
            .filter_type(FilterType::Tokens)
            .filter_amount(0.0)
            .limit(POSITIONS_PAGE)
            .pages();
        let mut positions = Vec::new();
        while let Some(page) = pages.next().await {
            for row in &page?.data {
                positions.push(position(row)?);
            }
        }
        Ok(positions)
```

Replace the two constants with:

```rust
/// The rows of a `/v2/positions` page.
const POSITIONS_PAGE: u32 = 500;
```

Replace `position`:

```rust
/// A Data API position, as the trait's [`Position`].
fn position(row: &DataPosition) -> Result<Position, ClassifiedError> {
    let size = decimal(row.current_size)
        .ok_or_else(|| decode("a position size that is not a decimal"))?;
    let mut position = Position::new(key_of(&row.token_id)?, size);
    position.average_price = decimal(row.avg_price);
    position.extensions = Extensions::of(PositionExt {
        condition_id: Some(row.condition_id.clone()),
        outcome: Some(row.outcome.clone()),
        outcome_index: row.outcome_index.get(),
        current_price: decimal(row.current_price),
        current_value: decimal(row.current_value),
        entry_cost: decimal(row.entry_cost_usdc),
        unrealized_pnl: decimal(row.unrealized_pnl),
        realized_pnl: decimal(row.realized_pnl),
        redeemable: Some(row.redeemable),
        mergeable: Some(row.mergeable),
        negative_risk: Some(row.negative_risk),
        title: Some(row.title.clone()),
        end_date: (!row.end_date.is_empty()).then(|| row.end_date.clone()),
    });
    Ok(position)
}
```

If `refusal` is now unused, the compiler says so: remove it only if no other caller remains.

- [ ] **Step 5: Run the positions tests and see them pass.**

Run: `direnv exec . cargo test -p polyoxide-polymarket --features clob-ws,data,gamma --test trading_clob positions 2>&1 | tail -5`, then the same with `--features clob-ws,data` (the proxy-without-gamma test).
Expected: PASS.

- [ ] **Step 6: Port the capture script to v2 and re-capture.** In `scripts/capture_polymarket_clob.py`, the trades request and its rows become:

```python
        trades = get(f"{DATA}/v2/trades", condition=market["condition_id"], taker_only="true", limit=TRADES_PAGE)
        rows = trades.body.get("data", []) if trades.status == 200 and isinstance(trades.body, dict) else []
        if not any(r.get("token_id") == token for r in rows) or not any(r.get("token_id") != token for r in rows):
            continue
```

Add `TRADES_PAGE = 100` beside `TRADES_KEPT`. The kept-rows loop counts on `row.get("token_id")`. `keep("trades", …)` writes the envelope with its rows trimmed:

```python
    keep("trades", trades, {**trades.body, "data": kept},
         f"`data` to its first {len(kept)} rows, newest first, {ours} of the token and {theirs} of another")
```

The docstring's "market has public trades" stays. Then re-capture (public GETs only, no credentials):

```bash
python3 scripts/capture_polymarket_clob.py polyoxide-polymarket/tests/fixtures/clob/venue
git diff --stat polyoxide-polymarket/tests/fixtures/clob/venue
```

Expected: every fixture and `PROVENANCE.md` rewritten for one freshly chosen token.

- [ ] **Step 7: Port `Trades` and its test.** In `src/clob/venue.rs`'s `mod trades`, the import becomes `use crate::data::v2::types::TradeSide;`. Add:

```rust
    /// The rows one call reads: v1's default page, which the call relied on.
    const TRADES_PAGE: u32 = 100;
```

and the body from `let rows = …` on becomes:

```rust
            let page = self
                .data
                .v2()
                .trades()
                .conditions([found.condition_id])
                .taker_only(true)
                .limit(TRADES_PAGE)
                .send()
                .await?;
            // The Data API sends the newest first, and both outcomes' trades.
            page.data
                .iter()
                .rev()
                .filter(|row| row.token_id == token.as_str())
                .map(|row| {
                    let price = decimal(row.price)
                        .ok_or_else(|| decode("a price that is not a decimal"))?;
                    let size =
                        decimal(row.size).ok_or_else(|| decode("a size that is not a decimal"))?;
                    let seconds =
                        u64::try_from(row.timestamp).map_err(|_| decode("a trade before 1970"))?;
                    let mut trade = Trade::new(
                        key.clone(),
                        price,
                        size,
                        UnixMillis(seconds.saturating_mul(1_000)),
                    );
                    // A taker-only row is the taker's own side.
                    trade.taker_side = match row.side {
                        TradeSide::Buy => Some(Side::Buy),
                        TradeSide::Sell => Some(Side::Sell),
                        _ => None,
                    };
                    trade.extensions = Extensions::of(TradeExt {
                        transaction_hash: Some(row.transaction_hash.clone()),
                        proxy_wallet: Some(row.proxy_wallet.clone()),
                        outcome: Some(row.outcome.clone()),
                    });
                    Ok(trade)
                })
                .collect()
```

In the module docs' table, the `trades` row reads: `| \`trades\` (\`data\`) | \`/markets-by-token/{token}\`, then the Data API's \`/v2/trades\` |`.

In `tests/venue_clob.rs`'s `trades_are_the_data_apis_public_trades_of_the_token_oldest_first`:
- the query becomes `format!("condition={}&taker_only=true&limit=100", condition())` and the route `"/v2/trades"`;
- `native` is `DataApi::builder().base_url(server.url()).build().unwrap().v2().trades().conditions([condition()]).taker_only(true).limit(100).send().await.unwrap().data`;
- every `row.asset` becomes `row.token_id`;
- the `TradeSide` import becomes `polyoxide_polymarket::data::v2::types::TradeSide`;
- the extension asserts compare with `Some(row.transaction_hash.clone())`.

Read the test's tail (after line 345) and port each remaining v1 field the same way.

Run: `direnv exec . cargo test -p polyoxide-polymarket --features clob,data,gamma --test venue_clob 2>&1 | tail -5`
Expected: PASS, the re-captured fixtures included.

- [ ] **Step 8: Rules.** In `.claude/rules/polymarket-clob.md`:
  - The `trades` bullet becomes "`trades` read the Data API's taker-only `/v2/trades` (one page of 100) for the token's market and keep the token's rows, oldest first, with no trade id (the Data API gives none)."
  - The `positions` sentence becomes "`positions` is Data v2 `/v2/positions` for the maker address, `status=OPEN` (which includes redeemable rows) with no dust filter (`filter_type=TOKENS`, `filter_amount=0`), walked by cursor in pages of 500."

- [ ] **Step 9: Gates and records.**

```bash
direnv exec . cargo fmt --all
direnv exec . cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings 2>&1 | tail -3
direnv exec . python3 scripts/test_body_diff.py --baseline v0.40.0 2>&1 | grep -E 'differs|has no counterpart|lost|::error' | head -40
```

For each item the gate reports as differing (the two changed positions tests, the helpers `position_row`/`positions_query`, the new `position_page`, the venue trades test, and each re-captured fixture), add a `[[drift]]` entry citing `R27` with the printed sha256, under a comment `# R27: the CLOB reads Data v2 (retire-data-v1 story 2).` Then run the removals gate and add each key it prints to `docs/s3-removals.md` under `## Removed`, in the listing's form, with the exact key text it prints:

- the `PositionExt::initial_value` key: "retire-data-v1 story 2: `Trading::positions` reads `/v2/positions`, walked by cursor, so the refusal past v1's 10,000 offset is gone; v2 sends no initial value, use `PositionExt::entry_cost` (`entry_cost_usdc`)."
- the `PositionExt::cash_pnl` key: "retire-data-v1 story 2: use `PositionExt::unrealized_pnl`."

`positions_refuse_a_wallet_with_more_rows_than_the_offset_reaches` tested a refusal held in private constants, so no removal key names it. Its `[[retired]]` entry cites the `initial_value` key, whose entry records the same change to `positions`. Put the comment `# The offset ceiling went with v1's /positions; see the initial_value entry.` above it.

Re-run both gates until each exits 0.

- [ ] **Step 10: Review (full: it re-types `Trading`'s records), then commit.**

```bash
git add polyoxide-polymarket/src/clob/venue.rs polyoxide-polymarket/src/clob/venue/trading.rs polyoxide-polymarket/tests/trading_clob.rs polyoxide-polymarket/tests/venue_clob.rs polyoxide-polymarket/tests/fixtures/clob/venue scripts/capture_polymarket_clob.py _bmad-output/specs/spec-venue-extensibility/divergences.md ci/test-body-diff.toml docs/s3-removals.md .claude/rules/polymarket-clob.md
git commit -m "fix(clob)!: Trades and Trading::positions read Data API v2

Data API v1 answers 410 since 2026-11-02, so both failed. Trades reads one
taker-only /v2/trades page of 100; positions walks /v2/positions by cursor
with status=OPEN and no dust filter, so the 10,000-offset refusal goes.
PositionExt's initial_value and cash_pnl become entry_cost and
unrealized_pnl, v2's fields. DRIFT R27.

BREAKING CHANGE: PositionExt::initial_value is entry_cost, and cash_pnl is
unrealized_pnl.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013q61bHPZYguEucdtVYY9QG"
```

---

## Task 3: The send-loop proofs leave v1 (D11)

**Files:**
- Modify: `polyoxide-polymarket/tests/mock_data.rs` (~1513–1600)
- Modify: `docs/MUTANTS.md` (rows (a), (a, the policy), (b), and line 220's note), `.github/scripts/tests/test_mutants_ledger.py` (`SNIPPETS`, if it quotes these tests)
- Modify: `ci/test-body-diff.toml`

- [ ] **Step 1: Move the two tests onto v2 routes.** In `retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop`, `data.user("0xaddr").closed_positions().send().await` becomes:

```rust
    let result = data
        .v2()
        .positions("0xaddr")
        .status(PositionStatus::Closed)
        .send()
        .await;
```

In `a_429_makes_the_next_request_wait_even_though_it_never_saw_one`, the first request becomes the same `positions` call, and `data.trades().list().send().await` becomes `data.v2().trades().send().await`. Add `use polyoxide_polymarket::data::v2::types::PositionStatus;` to the file's imports. `rate_limited_server` answers every path, so the assertions hold unchanged. `/v2/positions` and `/v2/trades` are separate rows of `data_limits_on`, so "a different endpoint, its own token bucket untouched" stays true.

- [ ] **Step 2: Leave the pnl-host test.** `a_429_on_the_pnl_host_holds_the_data_host` still compiles against v1's `pnl()`. It goes in Task 4 with its subject, retired with the `DataApiBuilder::pnl_base_url` removal key.

- [ ] **Step 3: Prove the rows still hold.** For each row, apply its mutation in `polyoxide-core`, then run the cited `mock_data` tests with `--no-fail-fast` and see them fail. Restore the file from a scratch copy.

```bash
cp polyoxide-core/src/send.rs $SCRATCH/send.rs.orig
# row (a): call `hold` only in the retry branch (docs/MUTANTS.md, row (a)'s mutation)
direnv exec . cargo test -p polyoxide-polymarket --features data --test mock_data --no-fail-fast a_429_makes_the_next_request_wait 2>&1 | grep -E 'test result|FAILED|panicked'
cp $SCRATCH/send.rs.orig polyoxide-core/src/send.rs && touch polyoxide-core/src/send.rs
```

Do the same for row (a, the policy) (`polyoxide-core/src/hooks.rs`'s 429 arm: `hold` only when `retries_left > 0`) and row (b) (the third `retry_after.rs` mutation, `requested.unwrap_or(computed)`, with `retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop`).
Expected: each cited test FAILS under its mutant and passes restored.

- [ ] **Step 4: Records.** Run `direnv exec . python3 scripts/test_body_diff.py --baseline v0.40.0 --moves 2>&1 | grep mock_data`. Add `[[drift]]` R27 entries for the two changed tests with the printed sha256s, and update `docs/MUTANTS.md`'s rows (a), (a, the policy) and (b), and the note at line ~220, to the new `mock_data.rs` lines the `--moves` output gives. Update `test_mutants_ledger.py`'s `SNIPPETS` where it quotes a moved line. Then:

```bash
cd .github/scripts && direnv exec . uv run pytest tests/test_mutants_ledger.py -q 2>&1 | tail -2; cd -
direnv exec . python3 scripts/test_body_diff.py --baseline v0.40.0 2>&1 | tail -1
```

Expected: both green.

- [ ] **Step 5: Review (one pass), then commit.**

```bash
git add polyoxide-polymarket/tests/mock_data.rs docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py ci/test-body-diff.toml
git commit -m "test(data): the send-loop proofs drive v2 routes

The 429-hold and zero-Retry-After tests that docs/MUTANTS.md cites drove
v1's /closed-positions and /trades, which Task 4 removes; they now drive
/v2/positions and /v2/trades with their assertions unchanged, and each
row's mutation was re-run against them. DRIFT R27.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013q61bHPZYguEucdtVYY9QG"
```

---

## Task 4: v1 goes, workspace-wide (D1, D7, D8, D10, D13, D15)

v2 stays at `data::v2` in this task. Only v1 goes.

**Files:** see the file map's "v1 deletion" row.

- [ ] **Step 1: Keep what v2 borrows from v1.** v2 imports two v1 items: `data::types::SortDirection` (`v2/api/feeds.rs`, `v2/api/wallet.rs`, the CLI's `SortOrder`, `polyoxide-py`) and `data::types::Allowance` (`v2/types/wallet.rs`).
  - Move `SortDirection` and its `Display` impl to `src/data/v2/types/common.rs`, and `Allowance` and its `Deserialize` impl to `src/data/v2/types/wallet.rs`, each with its doc comment unchanged.
  - Move their inline tests from `src/data/types.rs`'s `mod tests` (the four `deserialize_allowance_*` tests, and any test that names only `SortDirection`) into a `#[cfg(test)] mod tests` in each destination, bodies unchanged.
  - Point the v2 imports at `crate::data::v2::types::{SortDirection, Allowance}`.

  After Task 5's flatten, both are at `data::types::*` again, their baseline path.

- [ ] **Step 2: Delete the v1 module.**

```bash
git rm -r polyoxide-polymarket/src/data/api polyoxide-polymarket/src/data/types.rs
```

In `src/data/mod.rs`, drop `pub mod api;` and `pub mod types;`, and rewrite the module docs and example for v2 (`data.v2().positions(user).send()`). In `src/data/client.rs`:
  - drop every v1 namespace from `namespaces!` (keep `v2`), and `user`, `positions` and `traded`;
  - drop `Traded`, the `pnl_http_client`/`rankings_http_client` fields, `DEFAULT_PNL_BASE_URL`/`DEFAULT_RANKINGS_BASE_URL`, and `pnl_base_url`/`rankings_base_url`;
  - give `build` one client;
  - keep the docs true ("All namespaces target `data-api.polymarket.com`").

  `the_builders_runtime_is_every_hosts` becomes `assert!(Arc::ptr_eq(data.http_client.runtime(), &runtime));` over the one client (R27 drift).

- [ ] **Step 3: Rate limits (D13).** In `src/shared/limits.rs`'s `data_limits_on`, delete the `/closed-positions`, `/positions`, `/trades` and `/user-pnl` rows and their doc bullets. Rewrite the `/` row's justification: "Every route not listed: the most conservative of the measured v2 figures, since v2 publishes none." Then fix any test there that names a deleted row, and `rate-limits.md`'s data table to match.

- [ ] **Step 4: Tests.** Delete what tests v1 alone, and port what keeps a v2 subject:

```bash
git rm polyoxide-polymarket/tests/holders_shape_data.rs polyoxide-polymarket/examples/closed_positions_burst_probe.rs polyoxide-polymarket/examples/closed_positions_soak.rs
grep -n 'holders_shape_data\|closed_positions\|live_data\|mock_data\b' polyoxide-polymarket/Cargo.toml
```

  - Drop the deleted targets' `[[test]]`/`[[example]]` stanzas from `polyoxide-polymarket/Cargo.toml`.
  - In `tests/mock_data.rs`, delete every v1 test and helper. Keep only the two Task 3 tests, `rate_limited_server`, `throttled_data`, and the imports they use. `a_429_on_the_pnl_host_holds_the_data_host` goes.
  - In `tests/live_data.rs`, delete the v1 tests and keep the v2 ones. Prune `tests/common/mod.rs` to what `live_data` still uses, or delete it if nothing uses it.
  - In `tests/query_setters_data.rs`, delete the v1 cases.
  - `tests/headers_data.rs` and `tests/error_classes_data.rs` send a v2 request in place of a v1 one, assertions unchanged.

- [ ] **Step 5: CLI.** In `polyoxide-cli/src/commands/polymarket/data/mod.rs`:
  - The `Health` doc becomes `/// Check the Data API's status (\`/v2/status\`)`.
  - Its arm becomes:

```rust
            Self::Health => {
                let status = data.v2().status().send().await?;
                paging::print_pretty(&status, out)
            }
```

  - The `SortOrder` impls name `polyoxide_polymarket::data::v2::types::SortDirection`.
  - Run `grep -rn 'data::\(api\|types\)\|\.user(\|\.health()' polyoxide-cli/` and fix each hit, `src/commands/common/parsing.rs` and `tests/live_api.rs` included. `live_api`'s data check reads v2.
  - In `.claude/rules/cli.md`, replace "except `data health`, which stays on v1's `/` because `/v2/status` reports data freshness, not liveness." with "`data health` prints `/v2/status`, since `/` answers 410."

- [ ] **Step 6: Python (D8's removals).**
  - Delete the v1 classes in `polyoxide-py/src/clients/data.rs` (`PyDataApiUser*` and every v1 namespace class with its sync twin) and the v1 methods of `PyDataApi`/`PyDataApiSync`, keeping `new` and `v2`.
  - Delete `polyoxide-py/src/types/data.rs`'s v1 row classes, and their `register` lines in `src/types/mod.rs` and `src/clients/mod.rs`.
  - Delete the v1 names from `python/polyoxide/__init__.py` (imports and `__all__`) and `__init__.pyi`.
  - Delete the `@v1_retired` tests and the `v1_retired` marker from `tests/test_live_api.py`.
  - Run `grep -rn 'user(\|\.trades()\|\.holders()\|\.leaderboard()\|\.builders()\|\.health()' polyoxide-py/tests/` and port or delete each v1 call in the offline suites (`test_errors_offline.py` included), keeping each v2 assertion.

Then:

```bash
cd polyoxide-py && direnv exec .. uv sync --reinstall-package polyoxide && direnv exec .. uv run pytest tests/ -q 2>&1 | tail -3; cd -
```

Expected: pass.

- [ ] **Step 7: Scripts and doctests.** `git rm scripts/compare_v2_overlaps.py`, along with any `.github/scripts/tests` test of it. Run `grep -rln 'data::\(api\|types\)::\|\.user(\|pnl()\|rankings()\|\.health()' --include='*.rs' --include='*.md' polyoxide*/src polyoxide*/README.md polyoxide/src` and fix every doctest and doc link that names a removed item. `src/data/README.md` is a doctest and shows v2 calls.

- [ ] **Step 8: Gates and records.**

```bash
direnv exec . cargo fmt --all
direnv exec . cargo clippy --all-targets --all-features --keep-going -- -D warnings 2>&1 | grep -E '^(error|warning)' | head
direnv exec . cargo nextest run --all-features --workspace 2>&1 | tail -3
direnv exec . cargo test --doc --all-features --workspace 2>&1 | grep -E 'test result|FAILED' | tail -3
RUSTDOCFLAGS="-D warnings" direnv exec . cargo doc --no-deps --all-features --workspace 2>&1 | tail -2
env RUSTFLAGS= RUSTC_WRAPPER= CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 direnv exec . python3 scripts/api_removals.py check --baseline v0.40.0 2>&1 | tee $SCRATCH/removals-task4.txt | tail -20
```

Add every key the removals gate prints to `docs/s3-removals.md` under `## Removed`, each followed by "retire-data-v1 story 4: Data API v1 answers 410 since 2026-11-02; use …" and its v2 replacement:
  - `user(addr).list_positions()` → `v2().positions(addr)`; `trades()` → `v2().trades()`; `holders()` → `v2().holders(condition)`; `leaderboard()` → `v2().leaderboard()`; `builders()` → `v2().builders_leaderboard()`; `live_volume()` → `v2().live_volume(event)`; `open_interest()` → `v2().open_interest()`; `market_positions()` → `v2().positions(PositionAnchor::Condition(..))`; `combos()` → `v2().combo_positions(user)` / `combo_activity(user)`; `accounting()` and `misc()` → none; `approvals()` → `v2().approvals(user)`; `pnl()` → `v2().user_pnl(user)` (not the same series); `rankings()` → `v2().leaderboard()` (shares, not USDC); `health()` → `v2().status()`.
  - Check each replacement's name against `src/data/v2/api/*.rs` before writing it.

  Group the keys by module if the listing allows a `group` form. Check how earlier S3 entries group before inventing one.

Then:

```bash
direnv exec . python3 scripts/test_body_diff.py --baseline v0.40.0 2>&1 | tee $SCRATCH/body-task4.txt | tail -3
```

For each item it lists as lost, add a `[[retired]]` entry to `ci/test-body-diff.toml`: `file`, `item`, `kind` as printed, and `removal` set to the `s3-removals.md` key of the item the test exercised (a `DataApi::user` test cites `DataApi::user`'s key; a v1 type's test cites the type's). Write them with a script into a scratch file, review it, then append:

```bash
python3 - "$SCRATCH/body-task4.txt" > "$SCRATCH/retired.toml" <<'PY'
import re, sys
# Each lost line: "::error::<file>:<line> <kind> <item> has no counterpart…"
for line in open(sys.argv[1]):
    m = re.match(r"::error::(\S+?):\d+ (\w+) (\S+) has no counterpart", line)
    if m:
        file, kind, item = m.groups()
        print(f'[[retired]]\nfile = "{file}"\nitem = "{item}"\nkind = "{kind}"\nremoval = "TODO"\n')
PY
```

Fill each `removal` by hand from `docs/s3-removals.md`. The plan's no-placeholder rule applies to the committed file: no `TODO` survives. Add `[[drift]]` R27 entries for each changed item it lists (headers, error classes, query setters, `client.rs`'s runtime test, `live_data`'s ported tests). Re-run until the gate exits 0. Then the CI-scripts, features and wasm jobs:

```bash
cd .github/scripts && direnv exec . uv run pytest tests/ -q 2>&1 | tail -3; cd -
direnv exec . python3 scripts/runtime_gate.py
direnv exec . cargo hack check --workspace --each-feature --no-dev-deps --ignore-private 2>&1 | tail -2
direnv exec . cargo test -p polyoxide-polymarket --no-default-features --features data --test headers_data 2>&1 | tail -2
direnv exec . bash scripts/wasm_check.sh 2>&1 | tail -2
```

If `test_dep_allowlists` fails, regenerate the files with `uv run python -m tests.test_dep_allowlists --write` in `.github/scripts` and say why in the commit.

- [ ] **Step 9: Review (full: it removes public API), then commit** every path the task touched, staged by name (`git status --short` lists them):

```bash
git commit -m "feat(data)!: remove Data API v1

Every v1 route on data-api.polymarket.com, and the user-pnl-api and lb-api
hosts, answer 410 since 2026-11-02. The v1 namespaces, types, sibling-host
clients and their setters go, with their rate-limit rows, tests, examples
and Python classes; \`data health\` prints /v2/status. SortDirection and
Allowance move into v2, which used them. Each removal is in
docs/s3-removals.md; each retired test cites its removal. DRIFT R27.

BREAKING CHANGE: Data API v1 is removed; use \`data.v2()\`.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013q61bHPZYguEucdtVYY9QG"
```

---

## Task 5: The flatten (D2, D3, D6, D7, D8, D10, D12, D15)

- [ ] **Step 1: Move the files.**

```bash
cd polyoxide-polymarket/src/data
git mv v2/envelope.rs envelope.rs
mkdir -p api types && for f in v2/api/*.rs; do git mv "$f" "api/$(basename "$f")"; done
for f in v2/types/*.rs; do git mv "$f" "types/$(basename "$f")"; done
cd -
```

Merge `v2/error.rs` into `error.rs`, renaming `V2Error` to `VenueError` and the arm `DataError::V2` to `DataError::Venue`. Keep `ErrorCode` and every doc. `git rm` the old `v2/error.rs` and `v2/mod.rs`, and move `v2/mod.rs`'s module docs into `mod.rs`.

- [ ] **Step 2: Rewrite paths.**
  - Run `grep -rl 'data::v2' polyoxide*/ scripts/ --include='*.rs' --include='*.py' --include='*.md' --include='*.toml'` (the examples are under `polyoxide-polymarket/examples/`), then over those files replace `data::v2::` with `data::` and `crate::data::v2` with `crate::data`. That covers the `$crate::data::v2::Page` paths inside the paging macro.
  - Each `impl DataV2 {` in `src/data/api/*.rs` becomes `impl DataApi {`, importing `crate::data::DataApi`.
  - `DataApi` loses its `v2` namespace.
  - `mod.rs` declares `pub mod api; pub mod error; pub mod types; #[macro_use] mod envelope;` (envelope's macro must be defined before `api` uses it, so check the order rustfmt leaves), and `pub use envelope::{Page, PageStream, Pagination}; pub use error::{DataError, ErrorCode, VenueError}; pub use client::{DataApi, DataApiBuilder};`.
  - Then run `grep -rn '\.v2()' polyoxide*/` and delete each `.v2()` call.
  - Run `grep -rn 'V2Error\|DataError::V2\b\|DataV2' polyoxide*/` and rename each hit: `polyoxide-polymarket/src/lib.rs`'s `const _` assertion names `data::VenueError`.

- [ ] **Step 3: Constructible records (D6).** Remove `#[non_exhaustive]` from every `pub struct` in `src/data/types/*.rs` and from `Page` and `Pagination` in `src/data/envelope.rs`. Leave every enum's. Add to `OutcomeIndex`:

```rust
    /// An index from its raw wire value, `999` ([`UNLABELED`](Self::UNLABELED))
    /// included.
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }
```

with a test beside the type's tests:

```rust
    #[test]
    fn an_outcome_index_is_built_from_any_raw_value() {
        assert_eq!(OutcomeIndex::new(1).get(), Some(1));
        assert_eq!(OutcomeIndex::new(OutcomeIndex::UNLABELED).get(), None);
        assert_eq!(OutcomeIndex::new(OutcomeIndex::UNLABELED).raw(), 999);
    }
```

Add an integration test, `polyoxide-polymarket/tests/records_data.rs` (with a `[[test]]` stanza needing `data`, if the crate declares its tests), that builds every struct of `data::types` and `Page`/`Pagination` by struct literal outside the crate. That is prader's use, and it fails to compile while any struct keeps `#[non_exhaustive]`. Write one literal per struct, listing each field from its definition.

- [ ] **Step 4: CLI, CLOB and Python.**
  - The CLI's data commands and the CLOB already compile after Step 2.
  - In Python, rename `src/clients/data_v2.rs` to `src/clients/data.rs` (replacing the v1-trimmed file, whose `PyDataApi`/`PyDataApiSync` move into the macro) and `src/types/data_v2.rs` to `src/types/data.rs`.
  - The `v2_namespaces!` macro emits `#[pyclass(name = "DataApi")] pub struct PyDataApi { client: Arc<DataApi> }` and `DataApiSync`, each with today's `#[new]` (`base_url`, `timeout_ms`, `pool_size`), every route binding `$v2` to `&*self.client`, and docs saying `DataApi()`, not `DataApi().v2()`.
  - `lib.rs` drops the `v2` submodule and registers the row classes, `Page` and the iterators on the main module.
  - Fold `python/polyoxide/v2.py`'s names into `__init__.py` and `v2.pyi` into `__init__.pyi`, then `git rm` both.
  - Rename `tests/test_data_v2_offline.py` to `tests/test_data_offline.py`, with `DataApi().x()` calls and `from polyoxide import …`.
  - `test_stub_consistency.py` reads `__init__.pyi`.
  - `error.rs`'s `with_details` takes `Option<&VenueError>`, and the docs say "came from the Data API's structured body".
  - Rewrite `py.md`'s data bullets to match (spec D8).

- [ ] **Step 5: Renames and fixtures (D12).**

```bash
cd polyoxide-polymarket/tests
git rm mock_data.rs   # only the two send-loop tests remain; re-add them below
git mv mock_data_v2.rs mock_data.rs
git mv spec_agreement_data_v2.rs spec_agreement_data.rs
git mv wire_agreement_data_v2.rs wire_agreement_data.rs
git mv enum_wire_data_v2.rs enum_wire_data.rs
mkdir -p fixtures/data.tmp && for f in fixtures/data/v2/*; do git mv "$f" "fixtures/data.tmp/$(basename "$f")"; done
rmdir fixtures/data/v2 fixtures/data && git mv fixtures/data.tmp fixtures/data
cd -
git mv polyoxide-cli/tests/data_v2.rs polyoxide-cli/tests/data.rs
```

Paste the two send-loop tests, `rate_limited_server` and `throttled_data` (from `git show HEAD:polyoxide-polymarket/tests/mock_data.rs`) back into the new `mock_data.rs`, bodies unchanged. Replace `fixtures!("data/v2")` with `fixtures!("data")` everywhere. Rename the `[[test]]` stanzas in both crates' `Cargo.toml`, and point `scripts/gen_data_v2_types.py` and `scripts/capture_polymarket_data.py` at the new paths. Rename `.claude/rules/polymarket-data.md`'s `paths:` globs to match, and `test_claude_rules.py` will catch a glob that matches nothing.

- [ ] **Step 6: Records.** In `ci/test-body-diff.toml`:

```toml
[files]
"polyoxide-polymarket/tests/mock_data_v2.rs" = "polyoxide-polymarket/tests/mock_data.rs"
"polyoxide-polymarket/tests/spec_agreement_data_v2.rs" = "polyoxide-polymarket/tests/spec_agreement_data.rs"
"polyoxide-polymarket/tests/wire_agreement_data_v2.rs" = "polyoxide-polymarket/tests/wire_agreement_data.rs"
"polyoxide-polymarket/tests/enum_wire_data_v2.rs" = "polyoxide-polymarket/tests/enum_wire_data.rs"
"polyoxide-cli/tests/data_v2.rs" = "polyoxide-cli/tests/data.rs"

[paths]
"polyoxide_polymarket::data::v2::V2Error" = "polyoxide_polymarket::data::VenueError"
"polyoxide_polymarket::data::v2::error::V2Error" = "polyoxide_polymarket::data::VenueError"
"polyoxide_polymarket::data::DataError::V2" = "polyoxide_polymarket::data::DataError::Venue"
"polyoxide_polymarket::data::v2" = "polyoxide_polymarket::data"

[literals."polyoxide-polymarket/tests"]
"data/v2" = "data"
```

Merge these into the file's existing `[files]`, `[paths]` and `[literals]` tables rather than adding second ones. Keep only the `[files]` rows the gate needs, since it fails on a pair that pairs nothing: try without each one, and add it back only if its file's items go unpaired. Then:

```bash
direnv exec . python3 scripts/test_body_diff.py --baseline v0.40.0 2>&1 | tee $SCRATCH/body-task5.txt | grep -E 'differs|stale|no counterpart' | head -80
```

For every item that differs only by a removed `.v2()` or a path `[paths]` cannot reach, add a `[[drift]]` R27 entry with its sha256. Replace the sha256 of each entry Tasks 2–4 added whose item changed again. Update `docs/MUTANTS.md` and `SNIPPETS` from `--moves` for the two send-loop tests, and re-run their mutations as in Task 3, Step 3.

- [ ] **Step 7: Every gate.** These are the ten jobs. Run them one at a time:

```bash
direnv exec . cargo fmt --all -- --check
direnv exec . cargo clippy --all-targets --all-features --keep-going -- -D warnings
direnv exec . cargo nextest run --all-features --workspace
direnv exec . cargo test --doc --all-features --workspace
RUSTDOCFLAGS="-D warnings" direnv exec . cargo doc --no-deps --all-features --workspace
direnv exec . python3 scripts/publish_order.py check-manifests && direnv exec . cargo publish --workspace --dry-run --no-verify --locked --allow-dirty
direnv exec . msrv-cargo check --workspace --all-features && direnv exec . msrv-cargo doc --no-deps --workspace --all-features
direnv exec . cargo hack check --workspace --each-feature --no-dev-deps --ignore-private
env RUSTFLAGS= RUSTC_WRAPPER= CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 direnv exec . python3 scripts/api_removals.py check --baseline v0.40.0 2>&1 | tail -30
direnv exec . python3 scripts/test_body_diff.py --baseline v0.40.0 | tail -1
cd polyoxide-py && direnv exec .. uv sync --reinstall-package polyoxide && direnv exec .. uv run pytest tests/ -q; cd -
cd .github/scripts && direnv exec . uv run pytest tests/ -q; cd - && direnv exec . python3 scripts/runtime_gate.py
direnv exec . bash scripts/wasm_check.sh && direnv exec . bash scripts/wasm_smoke.sh
```

Plus the features job's pins (`ci-gates.md`): `headers_data` with `--no-default-features --features data`, and with `-p polyoxide --features polyoxide/full`. Then the specta check, `cargo check -p polyoxide-polymarket --no-default-features --features data,specta`.

The removals gate prints the flatten's moves: every `data::v2::…` path, `DataV2`, `DataApi::v2`, `V2Error` and `DataError::V2`. Add each to `docs/s3-removals.md` with its new path ("retire-data-v1 story 5: moved to `data::…`").

- [ ] **Step 8: Review (full), then commit** by path:

```bash
git commit -m "feat(data)!: v2 is the data module's root, and its records are constructible

data::v2::* moves to data::*, and every v2 route is a DataApi method
(data.trades(), data.positions(..)); DataV2 and DataApi::v2 go. V2Error is
data::VenueError and DataError::V2 is DataError::Venue. The row structs,
Page and Pagination drop #[non_exhaustive] (prader-rs #179; the enums keep
it), and OutcomeIndex::new takes any raw value. Python flattens the same
way: DataApi().trades(), classes on polyoxide. DRIFT R27.

BREAKING CHANGE: data::v2::X is data::X, data.v2().x() is data.x(),
V2Error is VenueError, DataError::V2 is DataError::Venue; in Python,
DataApi().v2() is DataApi() and polyoxide.v2 is polyoxide.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013q61bHPZYguEucdtVYY9QG"
```

---

## Task 6: Docs, specs and the nightly (D14, D16)

- [ ] **Step 1: Spec mirrors.** Check `docs/specs/data/*.md` and `docs/specs/undocumented/*` for any fact that still holds for v2, such as a host-wide Cloudflare limit, and move it into `docs/specs/data-v2/OBSERVED.md`. Then:

```bash
git rm -r docs/specs/data docs/specs/undocumented
```

Delete `[workspace.metadata.polyoxide.mirrors.data]` and the undocumented hosts' mirror table from the root `Cargo.toml`, along with the undocumented hosts' exclusion if it lives in metadata. Then run `direnv exec . python3 scripts/gen_registry.py --write`, and check that the CLAUDE.md watch line now says eight OpenAPI. `grep -rn 'specs/data/\|specs/undocumented' --include='*.md' --include='*.rs' --include='*.py' --include='*.yml' .` must come back empty, apart from history under `docs/plans/`, `docs/superpowers/` and `CHANGELOG.md`.

- [ ] **Step 2: Rules and guides.**
  - Rewrite `.claude/rules/polymarket-data.md` for one API: its namespace list is the v2 routes on `DataApi`; its `paths:` globs follow the renames; the v1, pnl, rankings and approvals bullets go; the v2 bullets stay, with `data.x()` calls and `DataError::Venue`.
  - Update `errors.md` (`VenueError`, `Venue`, "data's turns a v2 envelope into `Venue`"), `clients.md` (data's knobs: none; the hand-written accessors are now clob's five), `testing.md` (`mock_data` is the data suite, no `mock_data_v2`), `rate-limits.md`, `specs.md` and `ci-gates.md` (R27 in the drift list; `[[retired]]`'s `removal` entries).
  - In CLAUDE.md:
    - "user data, v1 and v2" becomes "user data";
    - the module line's "`data` (user data, v1 and v2)" becomes "`data` (user data)";
    - the API-specs paragraph's Data v2 sentence becomes "**Data API v2** (`data-v2/`, 21 endpoints under `/v2` on `data-api.polymarket.com`) is the `data` module; v1 was retired upstream on 2026-11-02."
    - Check the generated regions are untouched by hand.
  - Update `polyoxide-polymarket/README.md` and the umbrella's README examples to `data.trades()`.
  - Run `grep -rn 'v2()\|data::v2\|DataV2\|V2Error' --include='*.md' . | grep -v 'docs/plans\|docs/superpowers\|CHANGELOG\|_bmad-output'`, which must come back empty.

- [ ] **Step 3: Migration notes.** Add a section "Data API v1 retired (retire-data-v1)" to `docs/s3-migration.md`, with:
  - a table of each v1 call and its v2 replacement (from Task 4 Step 8's list, with `pnl` → `user_pnl` "not the same series" and `rankings` → `leaderboard` "ranks volume in shares");
  - the flatten's path renames;
  - `VenueError` / `DataError::Venue`;
  - `PositionExt`'s `entry_cost` and `unrealized_pnl`;
  - the constructible records and `OutcomeIndex::new`;
  - Python's `DataApi().x()` and `from polyoxide import Trade`.

- [ ] **Step 4: Gates.** Run `cd .github/scripts && direnv exec . uv run pytest tests/ -q` (rules files, spec docs, generated regions, module READMEs), `gen_registry.py --check`, and `cargo test --doc --all-features --workspace`.

- [ ] **Step 5: Review (one pass), then commit.**

```bash
git commit -m "docs: Data API v1 is gone from the specs, rules and guides

docs/specs/data and docs/specs/undocumented go with their mirror entries
(upstream's v1 spec is 404 and both sibling hosts are 410); the rules
files, CLAUDE.md and READMEs describe one Data API, and
docs/s3-migration.md maps every v1 call to its v2 replacement.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_013q61bHPZYguEucdtVYY9QG"
```

---

## After the six commits

- Run the whole-branch review: three layers, since the branch removes public API and changes a gate.
- Update `polyoxide-open-threads` memory: v1 retired on the branch, waiting for merge and the 0.47.0 bump.
- Hand off to the owner: the merge to local `main` (main-first), the version bump (its own commit, when the owner says), and the push. After the release publishes, message the prader-rs #179 session with the version, the path and error renames, and the per-type list (every row struct and `Page`/`Pagination` dropped `#[non_exhaustive]`; `OutcomeIndex::new` added; enums unchanged).
