# Data API v2 Python Bindings (Phase 4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the 20 Data API v2 routes in `polyoxide-py` as `DataApi().v2()` and `DataApiSync().v2()`, with typed row classes, `Page` results, page iterators and structured errors, held to the captured fixtures and to the compiled signatures.

**Architecture:** The row classes are `py_type!` wrappers, generated once from the OpenAPI mirror into `src/types/data_v2.rs`. They are registered on a `v2` submodule of the extension and re-exported as `polyoxide.v2`, because v1 already uses `Trade`, `Position` and other v2 names. `src/clients/data_v2.rs` declares every route once in a `v2_namespaces!` invocation, which emits `DataV2` (coroutines) and `DataV2Sync` (blocking, with the GIL released). Each route entry parses its Python arguments into the Rust builder before any request is made. Paged routes return a `Page` whose rows are already wrapped, and also get `iter_<route>`. That iterator runs the Rust `.pages()` stream behind a `futures_util` mutex. A boxed closure wraps each page's rows under the GIL, so one iterator class serves every row type. `data_err` maps `DataApiError::V2` by its code and sets the body's fields on the exception.

**Tech Stack:** PyO3 0.28, `pyo3-async-runtimes` (tokio), `futures-util` (new dependency), `serde` (new dev-dependency), maturin through uv, and pytest with a stdlib HTTP server.

**Design:** Component 6.1 of [`2026-09-14-data-api-v2-design.md`](../specs/2026-09-14-data-api-v2-design.md), including its *Amendments from planning* table, built against the Rust v2 surface in `polyoxide-data/src/v2` on this branch.

---

## Scope, and running alongside Phase 5

This plan runs on `aidanb/data-v2`, in the same worktree as the Phase 5 CLI session. It touches only `polyoxide-py/**` and `Cargo.lock`.

- **`Cargo.lock` is shared.** Tasks 1 and 3 each add one line to its `polyoxide-py` stanza (`serde`, then `futures-util`). Before either commit, run `git diff Cargo.lock`. If it shows anything outside that stanza, the other session has uncommitted lock changes, so agree an order with it before committing.
- **Commit only the paths each commit step lists.** Never use `git add -A` or `git add .`.
- **Not in this plan:** the `CLAUDE.md` note on the Python bindings (its proposed text is in the Phase 4 planning report) and the release.

## How this plan was verified

Every code block below was built and tested before this plan was written. A script replayed all six tasks in order against a fresh copy of this branch. It applied each edit exactly once, ran every command, and required three things: each failing-first step and mutation check fails with the stated message, each gate passes, and each test count matches. The seven live tests in Task 5 passed against the live host on 2026-09-14. The full `uv run pytest tests/`, live suite included, passed with 326 tests on the prototype the replayed code was taken from.

Three things were **not** run during planning. The first is Task 6's workspace-wide `cargo clippy` and `cargo doc`. Only their `polyoxide-py` equivalents ran, because the Phase 5 session was compiling in the same worktree. The second is Task 6's full suite on the final replay: its offline tests ran there, while its live tests ran on the prototype and in Task 5. The third is a release-profile build of the extension, since every build here used `--profile dev`.

## Deviations from design 6.1

| Area | Design 6.1 | This plan | Why |
|------|------------|-----------|-----|
| Where the classes live | `Page[Trade]` and so on, implicitly top-level | Row classes, `Page`, the iterators and `DataV2`/`DataV2Sync` live in `polyoxide.v2` | v1 already exports `Trade`, `Position`, `Activity`, `OpenInterest` and `LiveVolume`; flat names would shadow them |
| Method count | 20 routes | 21 methods | `/v2/leaderboard` is both `leaderboard` (a board page) and `leaderboard_user` (one wallet, `None` if unknown), as in Rust |
| `positions` anchor | `user=` and/or `conditions=` | Without `user`, `conditions` must hold exactly one id; otherwise `ValueError` | Amendment: `PositionAnchor::Condition(String)`, because upstream rejects a list |
| Error attributes | `code`, `retryable`, `trace_id`, `parameter` on v2 errors | Also `status` and `retry_after`. All six are set on every exception the SDK raises, and are `None` unless the error came from a v2 body | The stub declares them on `PolyoxideError`, so they must exist on Gamma and CLOB errors too |
| "Everything else to the base error" | The base error | Other v2 codes map to `ApiError`; `DataApiError::Pagination` maps to `PolyoxideError` itself | `ApiError` is the existing fallback for an error response. A stalled page walk is not a response |
| Getter test | Every getter is non-`None` on the fixtures | Getters equal the row's serialized keys, and each getter reads its own key over a row whose values are its key names | Many fields are `null` in every capture, so "non-`None`" cannot hold, and on those fields it would not catch a wrong key |
| Enum arguments | Not specified | Exact wire spelling. Request-only enums raise `ValueError` listing `ALL`; response enums (`side`, `status`, `types`) pass unknown values through | Mirrors the Rust closed and open enums. v1's upper-casing `parse_enum!` would mangle `1w` and `day` |
| Extra tests | Stub consistency, the getter test, live tests | Also an offline suite against a local server (every route with every argument, anchors, walks, errors) and a stub-versus-compiled signature test | Stub consistency compares names only. Nothing else would catch a kwarg wired to the wrong setter |
| `Page` | `data`, `pagination` | Also `__len__` | Convenience |
| Generator | Not specified | `polyoxide-py/scripts/gen_v2_bindings.py`, one-shot like `scripts/gen_data_v2_types.py` | This phase may only add files under `polyoxide-py/` |

## Conventions

- Run everything from the repository root. Commands that need `polyoxide-py/` run in a `( cd … )` subshell.
- **Keep build output off `/tmp`.** On this machine it is a RAM-backed tmpfs. Leave `CARGO_TARGET_DIR` unset, or point it at disk.
- **Rebuild the extension after every Rust change** with `(cd polyoxide-py && MATURIN_PEP517_ARGS="--profile dev" uv sync --reinstall-package polyoxide)`. The package is installed editable, so pytest imports whatever `.so` was last built. A stale one shows up as `ImportError` or `AttributeError`, and a plain `uv sync` does not rebuild it. `--profile dev` builds in about a minute. CI's `uv sync` builds release, and nothing here depends on the profile.
- **`tests/test_live_api.py` hits the network and is not gated**, so CI runs it on every push. Per-task gates skip it with `--ignore`. Tasks 5 and 6 run it.
- **Mutation checks edit files in place.** The worktree stages new files as intent-to-add, so never create a backup inside it (`sed -i.bak`, `cp f f.orig`). Every mutation and every restore below rewrites the file in place with Python. That also updates the file's mtime, so cargo rebuilds.
- If rustc is killed with signal 15 or exit code 254, that is earlyoom on this machine, not a build failure. Re-run with `-j 4`.
- `docs/specs/**` and `polyoxide-data/tests/fixtures/v2/` are read, never written.
- Commit messages end with the attribution trailer shown in each commit step.

## File map

| File | Responsibility | Task |
|------|----------------|------|
| `polyoxide-py/scripts/gen_v2_bindings.py` | One-shot generator: `py_type!` blocks, route entries, stub classes | 1 |
| `polyoxide-py/src/types/data_v2.rs` | The 22 row classes, `register`, `every_v2_getter_reads_its_own_key` | 1 |
| `polyoxide-py/src/types/mod.rs`, `polyoxide-py/src/lib.rs` | Module wiring and the `v2` submodule | 1, 3 |
| `polyoxide-py/Cargo.toml`, `Cargo.lock` | `serde` dev-dependency, then `futures-util` | 1, 3 |
| `polyoxide-py/python/polyoxide/v2.py` | `polyoxide.v2` re-exports | 2, 3 |
| `polyoxide-py/python/polyoxide/v2.pyi` | Typed stub for rows, pages and routes | 2, 3 |
| `polyoxide-py/python/polyoxide/__init__.py` | `from . import v2` | 2 |
| `polyoxide-py/tests/test_stub_consistency.py` | v2 stub members and signatures | 2 |
| `polyoxide-py/src/clients/data_v2.rs` | Argument parsing, `Page`, iterators, `v2_namespaces!`, the routes | 3 |
| `polyoxide-py/src/clients/mod.rs`, `polyoxide-py/src/clients/data.rs` | `DataApi.v2()` and `DataApiSync.v2()` | 3 |
| `polyoxide-py/python/polyoxide/__init__.pyi` | `v2()` on both clients, error attributes | 3, 4 |
| `polyoxide-py/tests/test_data_v2_offline.py` | Every route and argument against a local server, walks, errors | 3, 4 |
| `polyoxide-py/src/error.rs` | v2 error mapping and attributes | 4 |
| `polyoxide-py/tests/test_live_api.py` | Live v2 tests | 5 |
| `polyoxide-py/README.md` | v2 usage and error attributes | 5 |

---

## Tasks

### Task 1: Row classes and the getter guard

**Files:**
- Create: `polyoxide-py/scripts/gen_v2_bindings.py`
- Create: `polyoxide-py/src/types/data_v2.rs`
- Modify: `polyoxide-py/src/types/mod.rs`, `polyoxide-py/src/lib.rs`, `polyoxide-py/Cargo.toml`, `Cargo.lock`

- [ ] **Step 1: Add the one-shot generator**

It reads the vendored OpenAPI mirror and emits the repetitive parts: the `py_type!` blocks, the `v2_namespaces!` route entries (Task 3) and the typed stub classes (Tasks 2 and 3). Like `scripts/gen_data_v2_types.py`, its output is committed and then owned by hand. The route table lives here so the Rust signatures and the stub signatures come from one list; `test_stub_consistency.py` then holds them together after the generator is gone from the loop.

Create `polyoxide-py/scripts/gen_v2_bindings.py`:

```python
#!/usr/bin/env python3
"""Emit the repetitive parts of the Data API v2 Python bindings.

One-shot scaffolding, like `scripts/gen_data_v2_types.py`: the output is
committed and then owned by hand. It is NOT a build step and is never re-run to
overwrite edited files. The durable guards are
`every_v2_getter_reads_its_own_key` in `src/types/data_v2.rs`,
`tests/test_stub_consistency.py` and `tests/test_data_v2_offline.py`.

Usage (from the repository root):
    python3 polyoxide-py/scripts/gen_v2_bindings.py rows         # py_type! blocks + register()
    python3 polyoxide-py/scripts/gen_v2_bindings.py routes       # the v2_namespaces! invocation
    python3 polyoxide-py/scripts/gen_v2_bindings.py stub-rows    # row classes for v2.pyi
    python3 polyoxide-py/scripts/gen_v2_bindings.py stub-routes  # DataV2 / DataV2Sync for v2.pyi

Rules:
- one class per schema a route returns, plus `Pagination`; nested objects stay dicts
- a getter per property, named after the Rust field; the two `type` properties
  are `activity_type` / `combo_activity_type`, as in polyoxide-data
- stub types: required and non-nullable -> `T`; anything else -> `T | None`
"""
import json
import re
import sys
from pathlib import Path

SPEC = Path(__file__).resolve().parents[2] / "docs/specs/data-v2/openapi.json"
SCHEMAS = json.loads(SPEC.read_text())["components"]["schemas"]

# Every schema a v2 method returns directly, in stub order.
ROWS = [
    "Approvals", "Position", "ComboPosition", "UserPnlSeries", "UserStats", "UserVolume",
    "PortfolioValue", "Activity", "ComboActivity", "Trade", "MetaHolder", "LiveVolume",
    "OpenInterest", "PricePoint", "Resolution", "BiggestWinner", "BuilderStanding",
    "BuilderVolumePoint", "LeaderboardEntry", "LeaderboardUserEntry", "ServiceStatus",
    "Pagination",
]
# (schema, property) -> Rust field, where polyoxide-data renames a keyword.
RENAMED = {("Activity", "type"): "activity_type", ("ComboActivity", "type"): "combo_activity_type"}


def req(name, kind="str"):
    return {"name": name, "kind": kind, "required": True}


def opt(name, kind="str"):
    return {"name": name, "kind": kind, "required": False}


# kind: str | list | int | u32 | float | bool | direction
#       | choice:<Enum> | choices:<Enum>  (request-only enum; unknown values raise ValueError)
#       | open:<Enum>   | opens:<Enum>    (response enum; unknown values are sent verbatim)
# `anchor` params are consumed by the route's constructor rather than a setter.
LIMIT, CURSOR = opt("limit", "u32"), opt("cursor")
ROUTES = [
    dict(name="approvals", shape="plain", row="Approvals", path="/v2/approvals",
         doc="A wallet's Polygon token and operator approvals.",
         params=[req("user")], build="client.approvals(user)"),
    dict(name="positions", shape="paged", row="Position", path="/v2/positions",
         doc="Positions for a wallet (`user`), one market (`conditions` of one id), or a wallet in some markets (both).",
         params=[opt("user"), opt("conditions", "list"), opt("status", "open:PositionStatus"),
                 opt("event_ids", "list"), opt("title"), opt("filter_type", "choice:FilterType"),
                 opt("filter_amount", "float"), opt("include_archived", "bool"),
                 opt("sort_by", "choice:PositionSortBy"), opt("sort_direction", "direction"),
                 opt("start", "int"), opt("end", "int"), LIMIT, CURSOR],
         anchors=["user", "conditions"], build="client.positions(position_anchor(user, conditions)?)"),
    dict(name="combo_positions", shape="paged", row="ComboPosition", path="/v2/positions/combos",
         doc="A wallet's combo positions.",
         params=[req("user"), opt("conditions", "list"), opt("statuses", "choices:ComboPositionStatus"),
                 opt("sort_by", "choice:ComboPositionSortBy"), opt("sort_direction", "direction"),
                 opt("updated_after", "int"), opt("updated_before", "int"), LIMIT, CURSOR],
         build="client.combo_positions(user)"),
    dict(name="user_pnl", shape="plain", row="UserPnlSeries", path="/v2/user-pnl",
         doc="A wallet's cumulative PnL series. Not the same series as `data.pnl()`.",
         params=[req("user"), opt("interval", "choice:PnlInterval"), opt("fidelity", "choice:PnlFidelity")],
         build="client.user_pnl(user)"),
    dict(name="user_stats", shape="optional", row="UserStats", path="/v2/user-stats",
         doc="A wallet's profile card, or None for a wallet the API does not know.",
         params=[req("user")], build="client.user_stats(user)"),
    dict(name="user_volume", shape="plain", row="UserVolume", path="/v2/user-volume",
         doc="A wallet's traded volume over a whole-day window.",
         params=[req("user"), opt("start", "int"), opt("end", "int")], build="client.user_volume(user)"),
    dict(name="value", shape="plain", row="PortfolioValue", path="/v2/value",
         doc="A wallet's portfolio value in USDC.",
         params=[req("user"), opt("conditions", "list")], build="client.value(user)"),
    dict(name="activity", shape="paged", row="Activity", path="/v2/activity",
         doc="A wallet's activity feed.",
         params=[req("user"), opt("types", "opens:ActivityType"), opt("conditions", "list"),
                 opt("event_ids", "list"), opt("side", "open:TradeSide"), opt("start", "int"),
                 opt("end", "int"), opt("sort_by", "choice:ActivitySortBy"),
                 opt("sort_direction", "direction"), opt("exclude_deposits_withdrawals", "bool"),
                 LIMIT, CURSOR],
         build="client.activity(user)"),
    dict(name="combo_activity", shape="paged", row="ComboActivity", path="/v2/activity/combos",
         doc="A wallet's combo lifecycle events.",
         params=[req("user"), opt("conditions", "list"), LIMIT, CURSOR], build="client.combo_activity(user)"),
    dict(name="trades", shape="paged", row="Trade", path="/v2/trades",
         doc="The trade feed, newest first.",
         params=[opt("user"), opt("conditions", "list"), opt("event_ids", "list"),
                 opt("side", "open:TradeSide"), opt("taker_only", "bool"),
                 opt("filter_type", "choice:FilterType"), opt("filter_amount", "float"),
                 opt("start", "int"), opt("end", "int"), LIMIT, CURSOR],
         build="client.trades()"),
    dict(name="holders", shape="paged", row="MetaHolder", path="/v2/holders",
         doc="Top holders per outcome token, for these markets (at most 20).",
         params=[req("conditions", "list"), opt("min_balance", "float"), opt("include_pnl", "bool"),
                 LIMIT, CURSOR],
         build="client.holders(conditions)"),
    dict(name="live_volume", shape="plain", row="LiveVolume", path="/v2/live-volume",
         doc="Taker volume per market under these Gamma events.",
         params=[req("event_ids", "list")], build="client.live_volume(event_ids)"),
    dict(name="open_interest", shape="list", row="OpenInterest", path="/v2/oi",
         doc="Open interest per market; one GLOBAL row without `conditions`.",
         params=[opt("conditions", "list")], build="client.open_interest()"),
    dict(name="prices_history", shape="paged", row="PricePoint", path="/v2/prices-history",
         doc="An outcome token's price series.",
         params=[req("token_id"), opt("start", "int"), opt("end", "int"),
                 opt("interval", "choice:PricesInterval"), opt("bucket_seconds", "u32"),
                 opt("as_of", "int"), LIMIT, CURSOR],
         build="client.prices_history(token_id)"),
    dict(name="resolutions", shape="list", row="Resolution", path="/v2/resolutions",
         doc="Resolution state, by exactly one of `question_id`, `conditions` or `event_ids`.",
         params=[opt("question_id"), opt("conditions", "list"), opt("event_ids", "list")],
         anchors=["question_id", "conditions", "event_ids"],
         build="client.resolutions(resolution_key(question_id, conditions, event_ids)?)"),
    dict(name="biggest_winners", shape="paged", row="BiggestWinner", path="/v2/biggest-winners",
         doc="The largest single winning positions in a window.",
         params=[opt("time_period", "choice:TimePeriod"), opt("category"), LIMIT, CURSOR],
         build="client.biggest_winners()"),
    dict(name="builders_leaderboard", shape="paged", row="BuilderStanding", path="/v2/builders/leaderboard",
         doc="Builders ranked by volume for a window.",
         params=[opt("time_period", "choice:TimePeriod"), LIMIT, CURSOR], build="client.builders_leaderboard()"),
    dict(name="builder_volume", shape="list", row="BuilderVolumePoint", path="/v2/builders/volume",
         doc="Builder volume per time bucket.",
         params=[opt("interval", "choice:TimePeriod"), opt("limit", "u32")], build="client.builder_volume()"),
    dict(name="leaderboard", shape="paged", row="LeaderboardEntry", path="/v2/leaderboard",
         doc="The trader board. `board` picks PNL or VOLUME; volume is in shares, not USDC.",
         params=[opt("time_period", "choice:TimePeriod"), opt("category"),
                 opt("board", "choice:LeaderboardBoard"), LIMIT, CURSOR],
         build="client.leaderboard()"),
    dict(name="leaderboard_user", shape="optional", row="LeaderboardUserEntry", path="/v2/leaderboard",
         doc="One wallet's standing on both boards, or None for a wallet the API does not know.",
         params=[req("user"), opt("time_period", "choice:TimePeriod"), opt("category")],
         build="client.leaderboard_user(user)"),
    dict(name="status", shape="plain", row="ServiceStatus", path="/v2/status",
         doc="How fresh the served data is. This is not a liveness check.",
         params=[], build="client.status()"),
]


# ── rows ─────────────────────────────────────────────────────────────


def rust_path(schema):
    return f"polyoxide_data::v2::{'Pagination' if schema == 'Pagination' else 'types::' + schema}"


def gen_rows():
    out = []
    for schema in ROWS:
        fields = []
        for key in SCHEMAS[schema]["properties"]:
            ident = RENAMED.get((schema, key))
            fields.append(f'    {ident} => "{key}",' if ident else f"    {key},")
        out.append(f'py_type!(\n    PyV2{schema},\n    "{schema}",\n    {rust_path(schema)},\n'
                   + "\n".join(fields) + "\n);\n")
    adds = "\n".join(f"    m.add_class::<PyV2{schema}>()?;" for schema in ROWS)
    out.append("pub fn register(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {\n"
               + adds + "\n    Ok(())\n}")
    return "\n".join(out)


# ── routes ───────────────────────────────────────────────────────────


def rust_param_type(p):
    kind = p["kind"]
    if kind in ("str", "direction") or kind.startswith(("choice:", "open:")):
        base = "String"
    elif kind == "list" or kind.startswith(("choices:", "opens:")):
        base = "Vec<String>"
    else:
        base = {"int": "i64", "u32": "u32", "float": "f64", "bool": "bool"}[kind]
    return base if p["required"] else f"Option<{base}>"


def setter(p):
    name, kind = p["name"], p["kind"]
    enum = kind.split(":", 1)[1] if ":" in kind else None
    if kind.startswith("choice:"):
        value = f'choice("{name}", &v, v2::types::{enum}::ALL)?'
    elif kind.startswith("choices:"):
        value = f'v.iter().map(|s| choice("{name}", s, v2::types::{enum}::ALL)).collect::<PyResult<Vec<_>>>()?'
    elif kind.startswith("open:"):
        value = "open(&v)"
    elif kind.startswith("opens:"):
        value = f"v.iter().map(|s| open::<v2::types::{enum}>(s)).collect::<Vec<_>>()"
    elif kind == "direction":
        value = "direction(&v)?"
    else:
        value = "v"
    return f"            if let Some(v) = {name} {{\n                request = request.{name}({value});\n            }}\n"


def signature(route):
    required = [p["name"] for p in route["params"] if p["required"]]
    optional = [f'{p["name"]}=None' for p in route["params"] if not p["required"]]
    return "(" + ", ".join(required + (["*"] + optional if optional else [])) + ")"


def gen_routes():
    sections = {"plain": [], "optional": [], "list": [], "paged": []}
    for r in ROUTES:
        setters = [p for p in r["params"] if not p["required"] and p["name"] not in r.get("anchors", [])]
        name = f'{r["name"]} / iter_{r["name"]}' if r["shape"] == "paged" else r["name"]
        params = "".join(f'            {p["name"]}: {rust_param_type(p)},\n' for p in r["params"])
        head = f'        /// `GET {r["path"]}`: {r["doc"][0].lower() + r["doc"][1:]}\n'
        head += f"        #[pyo3(signature = {signature(r)})]\n"
        head += f"        fn {name}(\n{params}        ) -> PyV2{r['row']} => |client| " if params else \
            f"        fn {name}() -> PyV2{r['row']} => |client| "
        if setters:
            body = "{\n            let mut request = " + r["build"] + ";\n" + "".join(map(setter, setters)) \
                + "            request\n        };\n"
        else:
            body = r["build"] + ";\n"
        sections[r["shape"]].append(head + body)
    out = []
    for shape, entries in sections.items():
        out.append(f"    {shape} {{\n" + "\n".join(entries) + "    }")
    return "v2_namespaces! {\n" + "\n\n".join(out) + "\n}"


# ── stub ─────────────────────────────────────────────────────────────


def nullable(prop):
    t = prop.get("type")
    return (isinstance(t, list) and "null" in t) or any(o.get("type") == "null" for o in prop.get("oneOf", []))


def py_type(prop):
    if "$ref" in prop:
        return "dict[str, Any]"
    if "oneOf" in prop:
        return py_type(next(o for o in prop["oneOf"] if o.get("type") != "null"))
    t = prop["type"]
    if isinstance(t, list):
        t = next(x for x in t if x != "null")
    if t == "array":
        return f"list[{py_type(prop['items'])}]"
    return {"string": "str", "integer": "int", "number": "float", "boolean": "bool", "object": "dict[str, Any]"}[t]


def summary(text):
    text = " ".join((text or "").split())
    m = re.match(r"(.+?[.;:])(\s|$)", text)
    return (m.group(1) if m else text).rstrip(";:.").strip() + "."


def stub_class(schema):
    s = SCHEMAS[schema]
    required = set(s.get("required", []))
    doc = summary(s.get("description")) if s.get("description") else "Paging state returned with every page."
    lines = [f"class {schema}:", f'    """{doc}"""']
    for key, prop in sorted(s["properties"].items(), key=lambda kv: RENAMED.get((schema, kv[0]), kv[0])):
        ty = py_type(prop)
        if key not in required or nullable(prop):
            ty += " | None"
        lines += ["    @property", f"    def {RENAMED.get((schema, key), key)}(self) -> {ty}: ..."]
    lines += ["    def to_dict(self) -> dict[str, Any]: ...", "    def __repr__(self) -> str: ...",
              "    def __str__(self) -> str: ..."]
    return "\n".join(lines) + "\n"


def stub_param(p):
    kind = p["kind"]
    if kind == "list" or kind.startswith(("choices:", "opens:")):
        base = "list[str]"
    elif kind in ("str", "direction") or kind.startswith(("choice:", "open:")):
        base = "str"
    else:
        base = {"int": "int", "u32": "int", "float": "float", "bool": "bool"}[kind]
    return f'{p["name"]}: {base}' if p["required"] else f'{p["name"]}: {base} | None = None'


def stub_method(r, is_async, iterator):
    required = [stub_param(p) for p in r["params"] if p["required"]]
    optional = [stub_param(p) for p in r["params"] if not p["required"]]
    args = ["self"] + required + (["*"] + optional if optional else [])
    row = r["row"]
    if iterator:
        name, doc = f'iter_{r["name"]}', f'Walk every page of `{r["name"]}`, re-sending these filters on each one.'
        ret = f"PageIterator[{row}]" if is_async else f"PageIteratorSync[{row}]"
    else:
        name, doc = r["name"], r["doc"]
        ret = {"plain": row, "optional": f"{row} | None", "list": f"list[{row}]", "paged": f"Page[{row}]"}[r["shape"]]
        if is_async:
            ret = f"Coroutine[Any, Any, {ret}]"
    arglines = "".join(f"        {a},\n" for a in args)
    return f'    def {name}(\n{arglines}    ) -> {ret}:\n        """{doc}"""\n        ...\n'


def gen_stub_rows():
    return "\n".join(stub_class(schema) for schema in ROWS)


def gen_stub_routes():
    out = []
    for is_async, cls, doc in (
        (True, "DataV2", "Async Data API v2 routes. Obtain with `DataApi().v2()`."),
        (False, "DataV2Sync", "Sync Data API v2 routes. Obtain with `DataApiSync().v2()`."),
    ):
        methods = []
        for r in ROUTES:
            methods.append(stub_method(r, is_async, iterator=False))
            if r["shape"] == "paged":
                methods.append(stub_method(r, is_async, iterator=True))
        out.append(f'class {cls}:\n    """{doc}"""\n' + "".join(methods))
    return "\n".join(out)


def main():
    commands = {"rows": gen_rows, "routes": gen_routes, "stub-rows": gen_stub_rows, "stub-routes": gen_stub_routes}
    if len(sys.argv) != 2 or sys.argv[1] not in commands:
        raise SystemExit("usage: gen_v2_bindings.py " + "|".join(commands))
    print(commands[sys.argv[1]]())


if __name__ == "__main__":
    main()
```

Run:

```bash
python3 polyoxide-py/scripts/gen_v2_bindings.py rows | head -12
```

Expected:

```text
py_type!(
    PyV2Approvals,
    "Approvals",
    polyoxide_data::v2::types::Approvals,
    address,
    chain_id,
    checked_at,
    contracts,
);

py_type!(
    PyV2Position,
```

- [ ] **Step 2: Write the failing getter test**

`test_stub_consistency.py` compares attribute *names*, and `get_field` returns `None` for a key that is not there, so a getter reading the wrong key passes it silently. This test builds each class twice: from every captured row in `polyoxide-data/tests/fixtures/v2/` (its getters must be exactly the row's serialized keys), and over a row whose every value is its own key name (each getter must return its own key). The second check matters because many fields are `null` in every capture, where a wrong key and a right key both read `None`. It needs `serde` as a dev-dependency for the `DeserializeOwned + Serialize` bound.

In `polyoxide-py/Cargo.toml`, replace:

```toml
[dev-dependencies]
pyo3 = { workspace = true, features = ["auto-initialize"] }
```

with:

```toml
[dev-dependencies]
pyo3 = { workspace = true, features = ["auto-initialize"] }
serde = { workspace = true }
```

In `polyoxide-py/src/types/mod.rs`, replace:

```rust
mod data;
mod gamma;
```

with:

```rust
mod data;
pub mod data_v2;
mod gamma;
```

Write the module header and the test, with no classes yet:

Run:

```bash
{
cat <<'EOF'
//! Python classes for Data API v2 rows, registered on the `polyoxide.v2`
//! submodule. Each wraps the serialized Rust value, as the v1 classes do, so a
//! getter returns exactly what the typed row held. Nested objects (approval
//! contracts, combo legs, PnL points, freshness details) come back as dicts.
//!
//! Generated by `polyoxide-py/scripts/gen_v2_bindings.py rows`, then owned by
//! hand.

use pyo3::types::PyModuleMethods;

EOF
cat <<'EOF'

#[cfg(test)]
mod tests {
    //! `test_stub_consistency.py` compares attribute *names* with the stub, and
    //! a getter that looks up the wrong key still has the right name: `get_field`
    //! returns `None` for a key that is not there. So for every class this checks
    //! two things against the captured payloads in
    //! `polyoxide-data/tests/fixtures/v2/`:
    //!
    //! 1. its getters are exactly the keys of each captured row, decoded into the
    //!    Rust type and serialized back, so no field is missing or invented;
    //! 2. each getter reads its own key. The class is built over a row whose
    //!    every value is its own key name, so a field that is `null` in every
    //!    capture cannot hide a wrong key.

    use std::collections::BTreeSet;

    use polyoxide_data::v2::{self, types::*};
    use pyo3::{prelude::*, types::PyDict, PyClass, PyClassInitializer};
    use serde::{de::DeserializeOwned, Serialize};
    use serde_json::{Map, Value};

    use super::*;

    /// The objects at `pointer` in a captured body: the row itself, or each
    /// element of a list.
    fn captured(fixture: &str, pointer: &str) -> Vec<Value> {
        let path = format!(
            "{}/../polyoxide-data/tests/fixtures/v2/{fixture}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let body: Value = serde_json::from_str(&text).expect("fixture is JSON");
        match body.pointer(pointer) {
            Some(Value::Array(rows)) => rows.clone(),
            Some(row @ Value::Object(_)) => vec![row.clone()],
            other => panic!("{fixture}.json has no rows at {pointer}: {other:?}"),
        }
    }

    fn getters<W: PyClass>(py: Python<'_>) -> BTreeSet<String> {
        let locals = PyDict::new(py);
        locals.set_item("cls", py.get_type::<W>()).unwrap();
        py.run(
            c"names = {k for k, v in vars(cls).items() if type(v).__name__ == 'getset_descriptor'}",
            None,
            Some(&locals),
        )
        .unwrap();
        locals
            .get_item("names")
            .unwrap()
            .unwrap()
            .extract()
            .unwrap()
    }

    fn check<T, W>(py: Python<'_>, rows: Vec<Value>, raw: fn(Value) -> W, renames: &[(&str, &str)])
    where
        T: DeserializeOwned + Serialize,
        W: PyClass + Into<PyClassInitializer<W>>,
    {
        let class = std::any::type_name::<T>();
        let key = |getter: &str| {
            renames
                .iter()
                .find(|(field, _)| *field == getter)
                .map_or(getter, |(_, key)| *key)
                .to_owned()
        };
        let getters = getters::<W>(py);
        let covered: BTreeSet<String> = getters.iter().map(|g| key(g)).collect();

        assert!(!rows.is_empty(), "{class}: no captured rows");
        for row in rows {
            let typed: T = serde_json::from_value(row).unwrap_or_else(|e| panic!("{class}: {e}"));
            let Value::Object(fields) = serde_json::to_value(&typed).unwrap() else {
                panic!("{class} does not serialize to an object");
            };
            let keys: BTreeSet<String> = fields.keys().cloned().collect();
            assert_eq!(covered, keys, "{class}: getters differ from the row's keys");
        }

        let marked: Map<String, Value> = covered
            .iter()
            .map(|key| (key.clone(), Value::String(key.clone())))
            .collect();
        let obj = Bound::new(py, raw(Value::Object(marked)))
            .unwrap()
            .into_any();
        for getter in &getters {
            let read: Option<String> = obj.getattr(getter.as_str()).unwrap().extract().unwrap();
            assert_eq!(
                read.as_deref(),
                Some(key(getter).as_str()),
                "{class}.{getter} does not read the {:?} key",
                key(getter)
            );
        }
    }

    #[test]
    fn every_v2_getter_reads_its_own_key() {
        Python::attach(|py| {
            macro_rules! check {
                ($row:ty, $class:ident, [$($fixture:literal),+] $(, $field:literal => $key:literal)?) => {
                    check::<$row, $class>(
                        py,
                        [$($fixture),+].iter().flat_map(|f| captured(f, "/data")).collect(),
                        |inner| $class { inner },
                        &[$(($field, $key))?],
                    )
                };
            }

            check!(Approvals, PyV2Approvals, ["approvals"]);
            check!(Position, PyV2Position, ["positions", "positions_closed"]);
            check!(ComboPosition, PyV2ComboPosition, ["combo_positions"]);
            check!(UserPnlSeries, PyV2UserPnlSeries, ["user_pnl"]);
            check!(UserStats, PyV2UserStats, ["user_stats"]);
            check!(UserVolume, PyV2UserVolume, ["user_volume"]);
            check!(PortfolioValue, PyV2PortfolioValue, ["value"]);
            check!(Activity, PyV2Activity, ["activity", "activity_tips"], "activity_type" => "type");
            check!(ComboActivity, PyV2ComboActivity, ["combo_activity"], "combo_activity_type" => "type");
            check!(Trade, PyV2Trade, ["trades"]);
            check!(MetaHolder, PyV2MetaHolder, ["holders", "holders_pnl"]);
            check!(LiveVolume, PyV2LiveVolume, ["live_volume"]);
            check!(
                OpenInterest,
                PyV2OpenInterest,
                ["open_interest", "open_interest_global"]
            );
            check!(PricePoint, PyV2PricePoint, ["prices_history"]);
            check!(Resolution, PyV2Resolution, ["resolutions"]);
            check!(
                BiggestWinner,
                PyV2BiggestWinner,
                ["biggest_winners", "biggest_winners_combos"]
            );
            check!(
                BuilderStanding,
                PyV2BuilderStanding,
                ["builders_leaderboard"]
            );
            check!(
                BuilderVolumePoint,
                PyV2BuilderVolumePoint,
                ["builder_volume"]
            );
            check!(LeaderboardEntry, PyV2LeaderboardEntry, ["leaderboard"]);
            check!(
                LeaderboardUserEntry,
                PyV2LeaderboardUserEntry,
                ["leaderboard_user"]
            );
            check!(ServiceStatus, PyV2ServiceStatus, ["status"]);

            let paged = [
                "activity",
                "biggest_winners",
                "builders_leaderboard",
                "combo_activity",
                "combo_positions",
                "holders",
                "leaderboard",
                "positions",
                "prices_history",
                "trades",
            ];
            let pagination = paged
                .iter()
                .flat_map(|f| captured(f, "/pagination"))
                .collect();
            check::<v2::Pagination, PyV2Pagination>(
                py,
                pagination,
                |inner| PyV2Pagination { inner },
                &[],
            );
        });
    }
}
EOF
} > polyoxide-py/src/types/data_v2.rs
```

Run:

```bash
cargo test -p polyoxide-py --lib every_v2_getter_reads_its_own_key
```

Expected: a compile error, `cannot find struct, variant or union type `PyV2Approvals` in this scope` (and one per class).

- [ ] **Step 3: Generate the classes and register the `v2` submodule**

Run:

```bash
python3 - <<'EOF'
import subprocess
from pathlib import Path
p = Path('polyoxide-py/src/types/data_v2.rs')
s = p.read_text()
rows = subprocess.run(['python3', 'polyoxide-py/scripts/gen_v2_bindings.py', 'rows'], check=True, capture_output=True, text=True).stdout
i = s.index('\n#[cfg(test)]')
p.write_text(s[:i] + rows + s[i:])
EOF
```

Expected: no output. The classes now sit between the header and the test module.

In `polyoxide-py/src/lib.rs`, replace:

```rust
    clients::register(m)?;
    Ok(())
}
```

with:

```rust
    clients::register(m)?;

    // v2 row classes reuse v1 names (`Trade`, `Position`, ...), so they live
    // in their own submodule, re-exported as `polyoxide.v2`.
    let v2 = PyModule::new(m.py(), "v2")?;
    types::data_v2::register(&v2)?;
    m.add_submodule(&v2)?;
    Ok(())
}
```

Run:

```bash
cargo test -p polyoxide-py --lib every_v2_getter_reads_its_own_key
```

Expected: `test types::data_v2::tests::every_v2_getter_reads_its_own_key ... ok`.

- [ ] **Step 4: Prove the test catches a wrong key and a missing field**

Drop the `type` rename, so `Activity.activity_type` looks up `activityType`/`activity_type`, neither of which the row has:

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/src/types/data_v2.rs')
s = p.read_text()
assert s.count('    activity_type => "type",\n') == 1
p.write_text(s.replace('    activity_type => "type",\n', '    activity_type,\n'))
EOF
cargo test -p polyoxide-py --lib every_v2_getter_reads_its_own_key
```

Expected: `Activity.activity_type does not read the "type" key`, with `left: None`, `right: Some("type")`.

Restore it, then delete `Trade.bio`:

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/src/types/data_v2.rs')
s = p.read_text()
assert s.count('    activity_type,\n') == 1
p.write_text(s.replace('    activity_type,\n', '    activity_type => "type",\n'))
EOF
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/src/types/data_v2.rs')
s = p.read_text()
i = 0
i = s.index('    PyV2Trade,', i)
i = s.index('    bio,\n', i)
p.write_text(s[:i] + '' + s[i + len('    bio,\n'):])
EOF
cargo test -p polyoxide-py --lib every_v2_getter_reads_its_own_key
```

Expected: `Trade: getters differ from the row's keys`, with `bio` only on the right.

Restore it:

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/src/types/data_v2.rs')
s = p.read_text()
i = 0
i = s.index('    PyV2Trade,', i)
i = s.index('    condition_id,\n', i)
p.write_text(s[:i] + '    bio,\n    condition_id,\n' + s[i + len('    condition_id,\n'):])
EOF
cargo test -p polyoxide-py --lib every_v2_getter_reads_its_own_key
```

Expected: `every_v2_getter_reads_its_own_key ... ok`.

- [ ] **Step 5: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-py --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-py
```

Expected: `test result: ok. 4 passed`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.lock polyoxide-py/Cargo.toml polyoxide-py/scripts/gen_v2_bindings.py polyoxide-py/src/lib.rs polyoxide-py/src/types/mod.rs polyoxide-py/src/types/data_v2.rs
git commit -F - <<'EOF'
feat(py): Data API v2 row classes on a polyoxide.v2 submodule

One py_type! class per schema a v2 route returns, plus Pagination, generated
once by polyoxide-py/scripts/gen_v2_bindings.py and owned by hand. They live
on a `v2` submodule because v2 reuses v1 class names (Trade, Position, ...).

every_v2_getter_reads_its_own_key builds each class from the captured v2
fixtures (getters must equal the row's keys) and over a row whose values are
its own key names (each getter must read its own key), so a wrong key cannot
hide behind a field that is null in every capture.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```

### Task 2: Python face of the row classes

**Files:**
- Create: `polyoxide-py/python/polyoxide/v2.py`, `polyoxide-py/python/polyoxide/v2.pyi`
- Modify: `polyoxide-py/python/polyoxide/__init__.py`, `polyoxide-py/tests/test_stub_consistency.py`

- [ ] **Step 1: Rebuild the extension**

The package installs editable, so the compiled module under `python/polyoxide/` only changes on a rebuild, and a stale one shows up as `ImportError`/`AttributeError`. `--profile dev` builds in about a minute; CI's plain `uv sync` builds release, which behaves the same.

Run:

```bash
(cd polyoxide-py && MATURIN_PEP517_ARGS="--profile dev" uv sync --reinstall-package polyoxide)
```

Expected: ends with a `polyoxide==0.31.0 (from file://…/polyoxide-py)` line.

- [ ] **Step 2: Write the failing stub tests**

v2 gets its own stub, resolved on `polyoxide.v2` only: the existing lookup falls back to the top-level module, where `Trade` is the v1 class. Besides members, the new tests compare every stub method's parameters with the compiled `__text_signature__`, which `#[pyo3(signature)]` makes exact. That is what keeps the hand-owned stub and the Rust route list in step once the generator is out of the loop.

In `polyoxide-py/tests/test_stub_consistency.py`, replace:

```python
import ast
import pathlib
```

with:

```python
import ast
import inspect
import pathlib
```

Append to `polyoxide-py/tests/test_stub_consistency.py`:

```python


# ── polyoxide.v2 ──────────────────────────────────────────────────────────────
#
# The v2 classes reuse v1 names (`Trade`, `Position`, ...), so they have their
# own stub and resolve on `polyoxide.v2` only: falling back to the top-level
# module would compare a v2 stub with the v1 class of the same name.

_V2_STUB_PATH = _STUB_PATH.with_name("v2.pyi")
_V2_STUB = {
    node.name: node
    for node in ast.parse(_V2_STUB_PATH.read_text()).body
    if isinstance(node, ast.ClassDef)
}


def _v2_methods(class_name: str) -> list[ast.FunctionDef]:
    """Public stub methods of a v2 class, without its properties."""
    return [
        member
        for member in _V2_STUB[class_name].body
        if isinstance(member, ast.FunctionDef)
        and not member.name.startswith("_")
        and not any(getattr(d, "id", None) == "property" for d in member.decorator_list)
    ]


_V2_METHODS = sorted((c, m.name) for c in _V2_STUB for m in _v2_methods(c))


def test_v2_stub_declares_every_v2_export() -> None:
    assert set(_V2_STUB) == set(polyoxide.v2.__all__)


@pytest.mark.parametrize("class_name", sorted(_V2_STUB))
def test_v2_stub_matches_compiled_class(class_name: str) -> None:
    stub_members = {
        member.name
        for member in _V2_STUB[class_name].body
        if isinstance(member, ast.FunctionDef) and not member.name.startswith("_")
    }
    real_members = _public_members(getattr(polyoxide.v2, class_name))
    assert stub_members == real_members, (
        f"polyoxide/v2.pyi `{class_name}` is out of sync with the compiled class"
    )


def _stub_parameters(fn: ast.FunctionDef) -> list[tuple[str, str, bool]]:
    """(name, positional|keyword, has a default) for each parameter after self."""
    positional = fn.args.args[1:]
    first_default = len(positional) - len(fn.args.defaults)
    return [
        (arg.arg, "positional", i >= first_default) for i, arg in enumerate(positional)
    ] + [
        (arg.arg, "keyword", default is not None)
        for arg, default in zip(fn.args.kwonlyargs, fn.args.kw_defaults)
    ]


def _compiled_parameters(method: object) -> list[tuple[str, str, bool]]:
    kinds = {
        inspect.Parameter.POSITIONAL_OR_KEYWORD: "positional",
        inspect.Parameter.KEYWORD_ONLY: "keyword",
    }
    parameters = list(inspect.signature(method).parameters.values())[1:]
    return [(p.name, kinds[p.kind], p.default is not p.empty) for p in parameters]


@pytest.mark.parametrize(
    ("class_name", "method"), _V2_METHODS, ids=[f"{c}.{m}" for c, m in _V2_METHODS]
)
def test_v2_stub_signature_matches_compiled_method(class_name: str, method: str) -> None:
    """Parameter names, order, kinds and defaults, which member names alone miss.

    Only v2 is checked: its signatures all come from `#[pyo3(signature = ...)]`,
    so the compiled `__text_signature__` is exact.
    """
    stub = next(m for m in _v2_methods(class_name) if m.name == method)
    compiled = getattr(getattr(polyoxide.v2, class_name), method)
    assert _stub_parameters(stub) == _compiled_parameters(compiled)
```

Run:

```bash
(cd polyoxide-py && uv run pytest tests/test_stub_consistency.py -q)
```

Expected: a collection error, `FileNotFoundError: … python/polyoxide/v2.pyi`.

- [ ] **Step 3: Add `polyoxide.v2` and its stub**

Create `polyoxide-py/python/polyoxide/v2.py`:

```python
"""Data API v2 (``/v2/*``): cursor-paged routes, snake_case fields, structured errors.

Obtain the routes with ``DataApi().v2()`` or ``DataApiSync().v2()``. Several
classes here share a name with a v1 class in ``polyoxide`` (``Trade``,
``Position``, ``Activity``, ...); they are different types with different fields.
"""

from ._polyoxide import v2 as _v2

Pagination = _v2.Pagination
Activity = _v2.Activity
Approvals = _v2.Approvals
BiggestWinner = _v2.BiggestWinner
BuilderStanding = _v2.BuilderStanding
BuilderVolumePoint = _v2.BuilderVolumePoint
ComboActivity = _v2.ComboActivity
ComboPosition = _v2.ComboPosition
LeaderboardEntry = _v2.LeaderboardEntry
LeaderboardUserEntry = _v2.LeaderboardUserEntry
LiveVolume = _v2.LiveVolume
MetaHolder = _v2.MetaHolder
OpenInterest = _v2.OpenInterest
PortfolioValue = _v2.PortfolioValue
Position = _v2.Position
PricePoint = _v2.PricePoint
Resolution = _v2.Resolution
ServiceStatus = _v2.ServiceStatus
Trade = _v2.Trade
UserPnlSeries = _v2.UserPnlSeries
UserStats = _v2.UserStats
UserVolume = _v2.UserVolume

__all__ = [
    "Pagination",
    "Activity",
    "Approvals",
    "BiggestWinner",
    "BuilderStanding",
    "BuilderVolumePoint",
    "ComboActivity",
    "ComboPosition",
    "LeaderboardEntry",
    "LeaderboardUserEntry",
    "LiveVolume",
    "MetaHolder",
    "OpenInterest",
    "PortfolioValue",
    "Position",
    "PricePoint",
    "Resolution",
    "ServiceStatus",
    "Trade",
    "UserPnlSeries",
    "UserStats",
    "UserVolume",
]
```

Generate `polyoxide-py/python/polyoxide/v2.pyi`:

```bash
{
cat <<'EOF'
"""Data API v2 (`/v2/*`): cursor-paged routes, snake_case fields, structured errors.

A missing or `None` number means the value was unavailable, never zero.
`outcome_index` is `999` when upstream could not label the outcome.

Generated by `polyoxide-py/scripts/gen_v2_bindings.py stub-rows`, then owned by hand.
"""

from __future__ import annotations

from typing import Any

EOF
python3 polyoxide-py/scripts/gen_v2_bindings.py stub-rows
} > polyoxide-py/python/polyoxide/v2.pyi
```

In `polyoxide-py/python/polyoxide/__init__.py`, replace:

```python
    HealthResponse,
)
```

with:

```python
    HealthResponse,
)
from . import v2
```

and replace:

```python
    "HealthResponse",
]
```

with:

```python
    "HealthResponse",
    # Data API v2 routes and types
    "v2",
]
```

Run:

```bash
(cd polyoxide-py && uv run pytest tests/test_stub_consistency.py -q)
```

Expected: `147 passed`: the 102 existing tests, plus the export check and a member and a `to_dict` signature check for each of the 22 classes.

- [ ] **Step 4: Prove the member test catches a stub drift**

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/python/polyoxide/v2.pyi')
s = p.read_text()
assert s.count('    @property\n    def chain_id(self) -> int: ...\n') == 1
p.write_text(s.replace('    @property\n    def chain_id(self) -> int: ...\n', ''))
EOF
(cd polyoxide-py && uv run pytest tests/test_stub_consistency.py -q -k Approvals)
```

Expected: `FAILED … test_v2_stub_matches_compiled_class[Approvals]`.

Restore it:

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/python/polyoxide/v2.pyi')
s = p.read_text()
assert s.count('    def address(self) -> str: ...\n    @property\n    def checked_at') == 1
p.write_text(s.replace('    def address(self) -> str: ...\n    @property\n    def checked_at', '    def address(self) -> str: ...\n    @property\n    def chain_id(self) -> int: ...\n    @property\n    def checked_at'))
EOF
(cd polyoxide-py && uv run pytest tests/test_stub_consistency.py -q -k Approvals)
```

Expected: `2 passed`.

- [ ] **Step 5: Run the gates**

Run:

```bash
(cd polyoxide-py && uv run pytest tests/ --ignore=tests/test_live_api.py -q)
```

Expected: `150 passed`.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-py/python/polyoxide/v2.py polyoxide-py/python/polyoxide/v2.pyi polyoxide-py/python/polyoxide/__init__.py polyoxide-py/tests/test_stub_consistency.py
git commit -F - <<'EOF'
feat(py): polyoxide.v2 module and typed stub for the v2 rows

polyoxide.v2 re-exports the row classes from the compiled submodule, with a
typed v2.pyi generated from the OpenAPI mirror.

test_stub_consistency.py checks v2.pyi against polyoxide.v2 only, since the
existing lookup would find the v1 class of the same name, and compares each
stub method's parameters with the compiled text signature.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```

### Task 3: Routes, pages and page walks

**Files:**
- Create: `polyoxide-py/src/clients/data_v2.rs`, `polyoxide-py/tests/test_data_v2_offline.py`
- Modify: `polyoxide-py/src/clients/mod.rs`, `polyoxide-py/src/clients/data.rs`, `polyoxide-py/src/lib.rs`, `polyoxide-py/Cargo.toml`, `Cargo.lock`
- Modify: `polyoxide-py/python/polyoxide/v2.py`, `polyoxide-py/python/polyoxide/v2.pyi`, `polyoxide-py/python/polyoxide/__init__.pyi`

- [ ] **Step 1: Write the failing offline tests**

A stdlib `ThreadingHTTPServer` stands in for the host and serves the captured fixtures. Every route is called with every argument it accepts, with a distinct value per argument, and the query that arrives is compared with the one the route must send, so a kwarg wired to the wrong setter fails. The expected keys are written out by hand rather than derived from the generator's table.

Create `polyoxide-py/tests/test_data_v2_offline.py`:

```python
"""Data API v2 bindings against a local server: no network.

Each route is called with every argument it accepts, and the request that
reaches the server is compared with the query the route should send. Values
are distinct per argument, so a kwarg wired to the wrong setter fails. The
responses are the captured payloads in `polyoxide-data/tests/fixtures/v2/`.
"""

from __future__ import annotations

import asyncio
import json
import pathlib
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qsl, urlsplit

import pytest

import polyoxide
from polyoxide import v2

FIXTURES = pathlib.Path(__file__).resolve().parents[2] / "polyoxide-data" / "tests" / "fixtures" / "v2"


def fixture(name: str) -> dict:
    return json.loads((FIXTURES / f"{name}.json").read_text())


class FakeDataApi:
    """Answers each path from a queue of (status, headers, body) and records every request.

    The last response queued for a path repeats.
    """

    def __init__(self) -> None:
        self.requests: list[tuple[str, dict[str, str]]] = []
        self.responses: dict[str, list[tuple[int, dict[str, str], str]]] = {}
        server = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self) -> None:  # noqa: N802 - http.server's naming
                url = urlsplit(self.path)
                server.requests.append((url.path, dict(parse_qsl(url.query, keep_blank_values=True))))
                queue = server.responses.get(url.path) or [(404, {}, "404 page not found")]
                status, headers, body = queue.pop(0) if len(queue) > 1 else queue[0]
                payload = body.encode()
                self.send_response(status)
                for name, value in {"content-type": "application/json", **headers}.items():
                    self.send_header(name, value)
                self.send_header("content-length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

            def log_message(self, *args: object) -> None:
                pass

        self.httpd = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.url = f"http://127.0.0.1:{self.httpd.server_address[1]}"
        # A short poll keeps shutdown() from waiting out the default 0.5s per test.
        threading.Thread(target=self.httpd.serve_forever, args=(0.01,), daemon=True).start()

    def reply(self, path: str, *bodies: dict | str, status: int = 200, headers: dict[str, str] | None = None) -> None:
        self.responses[path] = [
            (status, headers or {}, body if isinstance(body, str) else json.dumps(body)) for body in bodies
        ]


@pytest.fixture
def server():
    fake = FakeDataApi()
    yield fake
    fake.httpd.shutdown()
    fake.httpd.server_close()


# (method, positional args, kwargs, path, query the route must send, fixture, shape, row class)
ROUTES = [
    ("approvals", ["0xuser"], {}, "/v2/approvals", {"user": "0xuser"}, "approvals", "row", "Approvals"),
    (
        "positions",
        [],
        {
            "user": "0xuser",
            "conditions": ["0xc1", "0xc2"],
            "status": "CLOSED",
            "event_ids": ["e1", "e2"],
            "title": "a title",
            "filter_type": "TOKENS",
            "filter_amount": 2.5,
            "include_archived": True,
            "sort_by": "REALIZED_PNL",
            "sort_direction": "ASC",
            "start": 11,
            "end": 12,
            "limit": 13,
            "cursor": "cur",
        },
        "/v2/positions",
        {
            "user": "0xuser",
            "condition": "0xc1,0xc2",
            "status": "CLOSED",
            "event_id": "e1,e2",
            "title": "a title",
            "filter_type": "TOKENS",
            "filter_amount": "2.5",
            "include_archived": "true",
            "sort_by": "REALIZED_PNL",
            "sort_direction": "ASC",
            "start": "11",
            "end": "12",
            "limit": "13",
            "cursor": "cur",
        },
        "positions",
        "page",
        "Position",
    ),
    (
        "combo_positions",
        ["0xuser"],
        {
            "conditions": ["0xc1"],
            "statuses": ["OPEN", "PARTIAL"],
            "sort_by": "UPDATED",
            "sort_direction": "DESC",
            "updated_after": 21,
            "updated_before": 22,
            "limit": 23,
            "cursor": "cur",
        },
        "/v2/positions/combos",
        {
            "user": "0xuser",
            "condition": "0xc1",
            "status": "OPEN,PARTIAL",
            "sort_by": "UPDATED",
            "sort_direction": "DESC",
            "updated_after": "21",
            "updated_before": "22",
            "limit": "23",
            "cursor": "cur",
        },
        "combo_positions",
        "page",
        "ComboPosition",
    ),
    (
        "user_pnl",
        ["0xuser"],
        {"interval": "1w", "fidelity": "12h"},
        "/v2/user-pnl",
        {"user": "0xuser", "interval": "1w", "fidelity": "12h"},
        "user_pnl",
        "row",
        "UserPnlSeries",
    ),
    ("user_stats", ["0xuser"], {}, "/v2/user-stats", {"user": "0xuser"}, "user_stats", "row", "UserStats"),
    (
        "user_volume",
        ["0xuser"],
        {"start": 31, "end": 32},
        "/v2/user-volume",
        {"user": "0xuser", "start": "31", "end": "32"},
        "user_volume",
        "row",
        "UserVolume",
    ),
    (
        "value",
        ["0xuser"],
        {"conditions": ["0xc1", "0xc2"]},
        "/v2/value",
        {"user": "0xuser", "condition": "0xc1,0xc2"},
        "value",
        "row",
        "PortfolioValue",
    ),
    (
        "activity",
        ["0xuser"],
        {
            "types": ["TRADE", "TIP"],
            "conditions": ["0xc1"],
            "event_ids": ["e1"],
            "side": "SELL",
            "start": 41,
            "end": 42,
            "sort_by": "TIMESTAMP",
            "sort_direction": "ASC",
            "exclude_deposits_withdrawals": False,
            "limit": 43,
            "cursor": "cur",
        },
        "/v2/activity",
        {
            "user": "0xuser",
            "type": "TRADE,TIP",
            "condition": "0xc1",
            "event_id": "e1",
            "side": "SELL",
            "start": "41",
            "end": "42",
            "sort_by": "TIMESTAMP",
            "sort_direction": "ASC",
            "exclude_deposits_withdrawals": "false",
            "limit": "43",
            "cursor": "cur",
        },
        "activity",
        "page",
        "Activity",
    ),
    (
        "combo_activity",
        ["0xuser"],
        {"conditions": ["0xc1"], "limit": 51, "cursor": "cur"},
        "/v2/activity/combos",
        {"user": "0xuser", "condition": "0xc1", "limit": "51", "cursor": "cur"},
        "combo_activity",
        "page",
        "ComboActivity",
    ),
    (
        "trades",
        [],
        {
            "user": "0xuser",
            "conditions": ["0xc1"],
            "event_ids": ["e1"],
            "side": "BUY",
            "taker_only": True,
            "filter_type": "CASH",
            "filter_amount": 6.5,
            "start": 61,
            "end": 62,
            "limit": 63,
            "cursor": "cur",
        },
        "/v2/trades",
        {
            "user": "0xuser",
            "condition": "0xc1",
            "event_id": "e1",
            "side": "BUY",
            "taker_only": "true",
            "filter_type": "CASH",
            "filter_amount": "6.5",
            "start": "61",
            "end": "62",
            "limit": "63",
            "cursor": "cur",
        },
        "trades",
        "page",
        "Trade",
    ),
    (
        "holders",
        [["0xc1", "0xc2"]],
        {"min_balance": 7.5, "include_pnl": True, "limit": 71, "cursor": "cur"},
        "/v2/holders",
        {"condition": "0xc1,0xc2", "min_balance": "7.5", "include_pnl": "true", "limit": "71", "cursor": "cur"},
        "holders_pnl",
        "page",
        "MetaHolder",
    ),
    ("live_volume", [["e1", "e2"]], {}, "/v2/live-volume", {"event_id": "e1,e2"}, "live_volume", "row", "LiveVolume"),
    (
        "open_interest",
        [],
        {"conditions": ["0xc1", "0xc2"]},
        "/v2/oi",
        {"condition": "0xc1,0xc2"},
        "open_interest",
        "list",
        "OpenInterest",
    ),
    (
        "prices_history",
        ["tok"],
        {"start": 81, "end": 82, "interval": "6h", "bucket_seconds": 83, "as_of": 84, "limit": 85, "cursor": "cur"},
        "/v2/prices-history",
        {
            "token_id": "tok",
            "start": "81",
            "end": "82",
            "interval": "6h",
            "bucket_seconds": "83",
            "as_of": "84",
            "limit": "85",
            "cursor": "cur",
        },
        "prices_history",
        "page",
        "PricePoint",
    ),
    (
        "resolutions",
        [],
        {"question_id": "0xq"},
        "/v2/resolutions",
        {"question_id": "0xq"},
        "resolutions",
        "list",
        "Resolution",
    ),
    (
        "resolutions",
        [],
        {"conditions": ["0xc1", "0xc2"]},
        "/v2/resolutions",
        {"condition": "0xc1,0xc2"},
        "resolutions",
        "list",
        "Resolution",
    ),
    (
        "resolutions",
        [],
        {"event_ids": ["e1"]},
        "/v2/resolutions",
        {"event_id": "e1"},
        "resolutions",
        "list",
        "Resolution",
    ),
    (
        "biggest_winners",
        [],
        {"time_period": "month", "category": "sports", "limit": 91, "cursor": "cur"},
        "/v2/biggest-winners",
        {"time_period": "month", "category": "sports", "limit": "91", "cursor": "cur"},
        "biggest_winners",
        "page",
        "BiggestWinner",
    ),
    (
        "builders_leaderboard",
        [],
        {"time_period": "day", "limit": 101, "cursor": "cur"},
        "/v2/builders/leaderboard",
        {"time_period": "day", "limit": "101", "cursor": "cur"},
        "builders_leaderboard",
        "page",
        "BuilderStanding",
    ),
    (
        "builder_volume",
        [],
        {"interval": "all", "limit": 111},
        "/v2/builders/volume",
        {"interval": "all", "limit": "111"},
        "builder_volume",
        "list",
        "BuilderVolumePoint",
    ),
    (
        "leaderboard",
        [],
        {"time_period": "week", "category": "crypto", "board": "VOLUME", "limit": 121, "cursor": "cur"},
        "/v2/leaderboard",
        {"time_period": "week", "category": "crypto", "sort_by": "VOLUME", "limit": "121", "cursor": "cur"},
        "leaderboard",
        "page",
        "LeaderboardEntry",
    ),
    (
        "leaderboard_user",
        ["0xuser"],
        {"time_period": "all", "category": "politics"},
        "/v2/leaderboard",
        {"user": "0xuser", "time_period": "all", "category": "politics"},
        "leaderboard_user",
        "row",
        "LeaderboardUserEntry",
    ),
    ("status", [], {}, "/v2/status", {}, "status", "row", "ServiceStatus"),
]


def test_every_route_is_covered() -> None:
    covered = {route[0] for route in ROUTES}
    names = {name for name in dir(v2.DataV2Sync) if not name.startswith(("_", "iter_"))}
    assert covered == names


def assert_shape(result: object, shape: str, row: str) -> None:
    row_class = getattr(v2, row)
    if shape == "page":
        assert isinstance(result, v2.Page)
        assert len(result) == len(result.data) > 0
        assert all(isinstance(r, row_class) for r in result.data)
        assert isinstance(result.pagination, v2.Pagination)
    elif shape == "list":
        assert isinstance(result, list) and result
        assert all(isinstance(r, row_class) for r in result)
    else:
        assert isinstance(result, row_class)


@pytest.mark.parametrize(
    ("method", "args", "kwargs", "path", "query", "body", "shape", "row"),
    ROUTES,
    ids=[f"{r[0]}-{'-'.join(r[2]) or 'bare'}" for r in ROUTES],
)
def test_route_sends_every_argument(server, method, args, kwargs, path, query, body, shape, row) -> None:
    server.reply(path, fixture(body))
    result = getattr(polyoxide.DataApiSync(base_url=server.url).v2(), method)(*args, **kwargs)

    assert server.requests == [(path, query)]
    assert_shape(result, shape, row)


@pytest.mark.parametrize(("method", "args", "kwargs", "path", "query", "body", "shape", "row"), ROUTES[:3], ids=[r[0] for r in ROUTES[:3]])
def test_async_route_sends_every_argument(server, method, args, kwargs, path, query, body, shape, row) -> None:
    server.reply(path, fixture(body))

    async def call():
        return await getattr(polyoxide.DataApi(base_url=server.url).v2(), method)(*args, **kwargs)

    result = asyncio.run(call())
    assert server.requests == [(path, query)]
    assert_shape(result, shape, row)


def test_getters_read_the_payload(server) -> None:
    body = fixture("activity_tips")
    server.reply("/v2/activity", body)
    page = polyoxide.DataApiSync(base_url=server.url).v2().activity("0xuser")

    first = body["data"][0]
    assert page.data[0].activity_type == first["type"]
    assert page.data[0].to_dict().items() >= first.items()
    assert page.pagination.next_cursor == body["pagination"]["next_cursor"]
    assert page.pagination.has_more is body["pagination"]["has_more"]


def test_an_unknown_wallet_is_none(server) -> None:
    server.reply("/v2/user-stats", fixture("user_stats_unknown"))
    server.reply("/v2/leaderboard", fixture("leaderboard_user_unknown"))
    client = polyoxide.DataApiSync(base_url=server.url).v2()

    assert client.user_stats("0xunknown") is None
    assert client.leaderboard_user("0xunknown") is None


def page_of(body: str, cursor: str | None) -> dict:
    page = fixture(body)
    page["pagination"] = {**page["pagination"], "has_more": cursor is not None, "next_cursor": cursor}
    return page


def test_iter_walks_every_page_with_the_same_filters(server) -> None:
    server.reply("/v2/trades", page_of("trades", "c2"), page_of("trades", "c3"), page_of("trades", None))
    walk = polyoxide.DataApiSync(base_url=server.url).v2().iter_trades(user="0xuser", limit=2)

    pages = list(walk)

    assert [p.pagination.next_cursor for p in pages] == ["c2", "c3", None]
    assert server.requests == [
        ("/v2/trades", {"user": "0xuser", "limit": "2"}),
        ("/v2/trades", {"user": "0xuser", "limit": "2", "cursor": "c2"}),
        ("/v2/trades", {"user": "0xuser", "limit": "2", "cursor": "c3"}),
    ]
    assert list(walk) == [], "an exhausted walk stays exhausted"


def test_async_iter_walks_every_page(server) -> None:
    server.reply("/v2/holders", page_of("holders", "c2"), page_of("holders", None))

    async def walk():
        return [page async for page in polyoxide.DataApi(base_url=server.url).v2().iter_holders(["0xc1"])]

    pages = asyncio.run(walk())

    assert [p.pagination.next_cursor for p in pages] == ["c2", None]
    assert all(isinstance(row, v2.MetaHolder) for p in pages for row in p.data)
    assert [q.get("cursor") for _, q in server.requests] == [None, "c2"]


@pytest.mark.parametrize(
    ("call", "message"),
    [
        (lambda c: c.positions(), "positions needs user, conditions, or both"),
        (lambda c: c.positions(conditions=["0xc1", "0xc2"]), "exactly one condition, got 2"),
        (lambda c: c.resolutions(), "exactly one of question_id, conditions or event_ids"),
        (lambda c: c.resolutions(question_id="0xq", event_ids=["e1"]), "exactly one of"),
        (lambda c: c.trades(filter_type="cash"), 'invalid filter_type "cash", expected one of: CASH, TOKENS'),
        (lambda c: c.leaderboard(time_period="WEEK"), "expected one of: day, week, month, all"),
        (lambda c: c.combo_positions("0xuser", statuses=["OPEN", "NOPE"]), 'invalid statuses "NOPE"'),
        (lambda c: c.activity("0xuser", sort_direction="asc"), 'invalid sort_direction "asc", expected one of: ASC, DESC'),
        (lambda c: c.iter_trades(filter_type="cash"), 'invalid filter_type "cash"'),
    ],
)
def test_a_bad_argument_raises_before_any_request(server, call, message) -> None:
    with pytest.raises(ValueError, match=message):
        call(polyoxide.DataApiSync(base_url=server.url).v2())
    assert server.requests == []


def test_async_bad_argument_raises_at_the_call_not_the_await(server) -> None:
    with pytest.raises(ValueError):
        polyoxide.DataApi(base_url=server.url).v2().trades(filter_type="cash")
    assert server.requests == []


def test_positions_anchors_send_their_own_keys(server) -> None:
    server.reply("/v2/positions", fixture("positions"))
    client = polyoxide.DataApiSync(base_url=server.url).v2()

    client.positions(user="0xuser")
    client.positions(conditions=["0xc1"])
    client.positions(user="0xuser", conditions=["0xc1", "0xc2"])

    assert [q for _, q in server.requests] == [
        {"user": "0xuser"},
        {"condition": "0xc1"},
        {"user": "0xuser", "condition": "0xc1,0xc2"},
    ]


def test_response_enums_pass_unknown_values_through(server) -> None:
    server.reply("/v2/activity", fixture("activity"))
    polyoxide.DataApiSync(base_url=server.url).v2().activity("0xuser", types=["FUTURE_TYPE"], side="FUTURE_SIDE")
    assert server.requests[0][1] == {"user": "0xuser", "type": "FUTURE_TYPE", "side": "FUTURE_SIDE"}
```

Run:

```bash
(cd polyoxide-py && uv run pytest tests/test_data_v2_offline.py -q -x)
```

Expected: `AttributeError: module 'polyoxide.v2' has no attribute 'DataV2Sync'` in `test_every_route_is_covered`.

- [ ] **Step 2: Add the bindings**

`v2_namespaces!` emits `DataV2` and `DataV2Sync` from one route list: one `#[pymethods]` block each, since a class may only have one without pyo3's `multiple-pymethods` feature. A paged route also gets `iter_<route>`, which wraps `.pages()` behind a `futures_util` mutex; each page's rows are wrapped under the GIL through a boxed closure, so one iterator class serves every row type. Arguments are parsed before any request, so a bad value raises at the call rather than at the `await`.

In `polyoxide-py/Cargo.toml`, replace:

```toml
pyo3-async-runtimes = { workspace = true }
```

with:

```toml
pyo3-async-runtimes = { workspace = true }
futures-util = "0.3"
```

In `polyoxide-py/src/clients/mod.rs`, replace:

```rust
pub mod data;
pub mod gamma;
```

with:

```rust
pub mod data;
pub mod data_v2;
pub mod gamma;
```

Generate `polyoxide-py/src/clients/data_v2.rs` (hand-written helpers, iterators and macro, then the generated routes):

```bash
{
cat <<'EOF'
//! Data API v2: `DataApi.v2()` / `DataApiSync.v2()`, registered on the
//! `polyoxide.v2` submodule with the row classes in `types::data_v2`.
//!
//! Every route is declared once, in the `v2_namespaces!` invocation at the end
//! of this file, which emits both the async and the sync class. Arguments are
//! parsed before any request is made, so a bad value raises `ValueError` at
//! the call rather than when the coroutine is awaited.

use std::{convert::Infallible, fmt::Display, pin::Pin, str::FromStr, sync::Arc};

use futures_util::{lock::Mutex, Stream, StreamExt};
use polyoxide_data::{
    types::SortDirection,
    v2::{
        self,
        types::{PositionAnchor, ResolutionKey},
        DataV2,
    },
    DataApiError,
};
use pyo3::{
    exceptions::{PyStopAsyncIteration, PyValueError},
    prelude::*,
    types::PyModuleMethods,
    PyClass, PyClassInitializer,
};

use crate::{error::data_err, runtime::runtime, types::data_v2::*};

/// Parses a request-only enum from its wire spelling, listing every accepted
/// value when it is not one of them.
fn choice<T: FromStr + Display>(param: &str, value: &str, all: &[T]) -> PyResult<T> {
    value.parse().map_err(|_| {
        let accepted = all
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        PyValueError::new_err(format!(
            "invalid {param} {value:?}, expected one of: {accepted}"
        ))
    })
}

/// Parses an enum that also appears in responses. A value this SDK does not
/// know is sent verbatim, so a value upstream adds later still works.
fn open<T: FromStr<Err = Infallible>>(value: &str) -> T {
    let Ok(parsed) = value.parse();
    parsed
}

/// `sort_direction`, spelled exactly as on the wire like every other enum here.
fn direction(value: &str) -> PyResult<SortDirection> {
    match value {
        "ASC" => Ok(SortDirection::Asc),
        "DESC" => Ok(SortDirection::Desc),
        _ => Err(PyValueError::new_err(format!(
            "invalid sort_direction {value:?}, expected one of: ASC, DESC"
        ))),
    }
}

/// `positions` takes a wallet, one market, or a wallet narrowed to markets.
fn position_anchor(
    user: Option<String>,
    conditions: Option<Vec<String>>,
) -> PyResult<PositionAnchor> {
    match (user, conditions) {
        (Some(user), None) => Ok(PositionAnchor::User(user)),
        (Some(user), Some(conditions)) => Ok(PositionAnchor::UserInConditions { user, conditions }),
        (None, Some(mut conditions)) if conditions.len() == 1 => {
            Ok(PositionAnchor::Condition(conditions.remove(0)))
        }
        (None, Some(conditions)) => Err(PyValueError::new_err(format!(
            "positions without user takes exactly one condition, got {}",
            conditions.len()
        ))),
        (None, None) => Err(PyValueError::new_err(
            "positions needs user, conditions, or both",
        )),
    }
}

/// `resolutions` takes exactly one selector family.
fn resolution_key(
    question_id: Option<String>,
    conditions: Option<Vec<String>>,
    event_ids: Option<Vec<String>>,
) -> PyResult<ResolutionKey> {
    match (question_id, conditions, event_ids) {
        (Some(id), None, None) => Ok(ResolutionKey::Question(id)),
        (None, Some(ids), None) => Ok(ResolutionKey::Conditions(ids)),
        (None, None, Some(ids)) => Ok(ResolutionKey::Events(ids)),
        _ => Err(PyValueError::new_err(
            "resolutions takes exactly one of question_id, conditions or event_ids",
        )),
    }
}

/// One page of a paged route: its rows, already wrapped, and its pagination.
#[pyclass(name = "Page", skip_from_py_object)]
pub struct PyPage {
    data: Vec<Py<PyAny>>,
    pagination: PyV2Pagination,
}

impl PyPage {
    /// Wraps each row as `W`. The rows become Python objects here, once, rather
    /// than on every `data` access.
    fn new<T, W>(py: Python<'_>, page: v2::Page<T>) -> PyResult<Self>
    where
        W: From<T> + PyClass + Into<PyClassInitializer<W>>,
    {
        let data = page
            .data
            .into_iter()
            .map(|row| Py::new(py, W::from(row)).map(Py::into_any))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            data,
            pagination: PyV2Pagination::from(page.pagination),
        })
    }
}

#[pymethods]
impl PyPage {
    /// The page's rows.
    #[getter]
    fn data(&self, py: Python<'_>) -> Vec<Py<PyAny>> {
        self.data.iter().map(|row| row.clone_ref(py)).collect()
    }

    /// Paging state; follow `next_cursor` until it is `None`.
    #[getter]
    fn pagination(&self) -> PyV2Pagination {
        self.pagination.clone()
    }

    fn __len__(&self) -> usize {
        self.data.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "Page(rows={}, pagination={})",
            self.data.len(),
            self.pagination.inner()
        )
    }
}

/// Builds a `Page` once the GIL is held. Boxing it erases the row type, so one
/// iterator class serves every paged route.
type PageBuilder = Box<dyn for<'py> FnOnce(Python<'py>) -> PyResult<PyPage> + Send>;
type Pages = Arc<Mutex<Pin<Box<dyn Stream<Item = Result<PageBuilder, DataApiError>> + Send>>>>;

fn erase<T, W>(pages: v2::PageStream<T>) -> Pages
where
    T: Send + 'static,
    W: From<T> + PyClass + Into<PyClassInitializer<W>>,
{
    let pages = pages.map(|page| {
        page.map(|page| {
            Box::new(move |py: Python<'_>| PyPage::new::<T, W>(py, page)) as PageBuilder
        })
    });
    Arc::new(Mutex::new(Box::pin(pages)))
}

/// Async iterator over every page of a route, from `DataV2.iter_*`. Each page
/// re-sends the filters the walk started with.
#[pyclass(name = "PageIterator", skip_from_py_object)]
pub struct PyPageIterator {
    pages: Pages,
}

#[pymethods]
impl PyPageIterator {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pages = self.pages.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let next = pages.lock().await.next().await;
            match next {
                Some(Ok(build)) => Python::attach(build),
                Some(Err(e)) => Err(data_err(e)),
                None => Err(PyStopAsyncIteration::new_err(())),
            }
        })
    }
}

/// Iterator over every page of a route, from `DataV2Sync.iter_*`.
#[pyclass(name = "PageIteratorSync", skip_from_py_object)]
pub struct PyPageIteratorSync {
    pages: Pages,
}

#[pymethods]
impl PyPageIteratorSync {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<PyPage>> {
        let pages = self.pages.clone();
        let next = py.detach(|| runtime().block_on(async move { pages.lock().await.next().await }));
        match next {
            Some(Ok(build)) => build(py).map(Some),
            Some(Err(e)) => Err(data_err(e)),
            None => Ok(None),
        }
    }
}

/// Emits `DataV2` and `DataV2Sync` from one list of routes.
///
/// Each entry names its parameters, the row class it returns, and an
/// expression that builds the request from `v2` (a `DataV2`). `plain` routes
/// return the row, `optional` routes the row or `None`, `list` routes a list,
/// and `paged` routes a `Page` plus an `iter_*` method that walks every page.
macro_rules! v2_namespaces {
    (
        plain { $(
            #[doc = $p_doc:literal]
            #[pyo3(signature = $p_sig:tt)]
            fn $p_name:ident($($p_arg:ident: $p_ty:ty),* $(,)?) -> $p_row:ident
                => |$p_v2:ident| $p_build:expr;
        )* }
        optional { $(
            #[doc = $o_doc:literal]
            #[pyo3(signature = $o_sig:tt)]
            fn $o_name:ident($($o_arg:ident: $o_ty:ty),* $(,)?) -> $o_row:ident
                => |$o_v2:ident| $o_build:expr;
        )* }
        list { $(
            #[doc = $l_doc:literal]
            #[pyo3(signature = $l_sig:tt)]
            fn $l_name:ident($($l_arg:ident: $l_ty:ty),* $(,)?) -> $l_row:ident
                => |$l_v2:ident| $l_build:expr;
        )* }
        paged { $(
            #[doc = $g_doc:literal]
            #[pyo3(signature = $g_sig:tt)]
            fn $g_name:ident / $g_iter:ident($($g_arg:ident: $g_ty:ty),* $(,)?) -> $g_row:ident
                => |$g_v2:ident| $g_build:expr;
        )* }
    ) => {
        /// Async Data API v2 routes. Obtain with `DataApi().v2()`.
        #[pyclass(name = "DataV2", skip_from_py_object)]
        pub struct PyDataV2 {
            pub(crate) v2: DataV2,
        }

        #[pymethods]
        impl PyDataV2 {
            $(
                #[doc = $p_doc]
                #[pyo3(signature = $p_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $p_name<'py>(&self, py: Python<'py>, $($p_arg: $p_ty),*) -> PyResult<Bound<'py, PyAny>> {
                    let $p_v2 = &self.v2;
                    let request = $p_build;
                    pyo3_async_runtimes::tokio::future_into_py(py, async move {
                        Ok($p_row::from(request.send().await.map_err(data_err)?))
                    })
                }
            )*
            $(
                #[doc = $o_doc]
                #[pyo3(signature = $o_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $o_name<'py>(&self, py: Python<'py>, $($o_arg: $o_ty),*) -> PyResult<Bound<'py, PyAny>> {
                    let $o_v2 = &self.v2;
                    let request = $o_build;
                    pyo3_async_runtimes::tokio::future_into_py(py, async move {
                        Ok(request.send().await.map_err(data_err)?.map($o_row::from))
                    })
                }
            )*
            $(
                #[doc = $l_doc]
                #[pyo3(signature = $l_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $l_name<'py>(&self, py: Python<'py>, $($l_arg: $l_ty),*) -> PyResult<Bound<'py, PyAny>> {
                    let $l_v2 = &self.v2;
                    let request = $l_build;
                    pyo3_async_runtimes::tokio::future_into_py(py, async move {
                        let rows = request.send().await.map_err(data_err)?;
                        Ok(rows.into_iter().map($l_row::from).collect::<Vec<_>>())
                    })
                }
            )*
            $(
                #[doc = $g_doc]
                #[pyo3(signature = $g_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $g_name<'py>(&self, py: Python<'py>, $($g_arg: $g_ty),*) -> PyResult<Bound<'py, PyAny>> {
                    let $g_v2 = &self.v2;
                    let request = $g_build;
                    pyo3_async_runtimes::tokio::future_into_py(py, async move {
                        let page = request.send().await.map_err(data_err)?;
                        Python::attach(|py| PyPage::new::<_, $g_row>(py, page))
                    })
                }

                #[doc = concat!("Walks every page of `", stringify!($g_name), "`, re-sending these filters on each one.")]
                #[pyo3(signature = $g_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $g_iter(&self, $($g_arg: $g_ty),*) -> PyResult<PyPageIterator> {
                    let $g_v2 = &self.v2;
                    let request = $g_build;
                    Ok(PyPageIterator { pages: erase::<_, $g_row>(request.pages()) })
                }
            )*
        }

        /// Sync Data API v2 routes. Obtain with `DataApiSync().v2()`.
        #[pyclass(name = "DataV2Sync", skip_from_py_object)]
        pub struct PyDataV2Sync {
            pub(crate) v2: DataV2,
        }

        #[pymethods]
        impl PyDataV2Sync {
            $(
                #[doc = $p_doc]
                #[pyo3(signature = $p_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $p_name(&self, py: Python<'_>, $($p_arg: $p_ty),*) -> PyResult<$p_row> {
                    let $p_v2 = &self.v2;
                    let request = $p_build;
                    let row = py.detach(|| runtime().block_on(request.send())).map_err(data_err)?;
                    Ok($p_row::from(row))
                }
            )*
            $(
                #[doc = $o_doc]
                #[pyo3(signature = $o_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $o_name(&self, py: Python<'_>, $($o_arg: $o_ty),*) -> PyResult<Option<$o_row>> {
                    let $o_v2 = &self.v2;
                    let request = $o_build;
                    let row = py.detach(|| runtime().block_on(request.send())).map_err(data_err)?;
                    Ok(row.map($o_row::from))
                }
            )*
            $(
                #[doc = $l_doc]
                #[pyo3(signature = $l_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $l_name(&self, py: Python<'_>, $($l_arg: $l_ty),*) -> PyResult<Vec<$l_row>> {
                    let $l_v2 = &self.v2;
                    let request = $l_build;
                    let rows = py.detach(|| runtime().block_on(request.send())).map_err(data_err)?;
                    Ok(rows.into_iter().map($l_row::from).collect())
                }
            )*
            $(
                #[doc = $g_doc]
                #[pyo3(signature = $g_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $g_name(&self, py: Python<'_>, $($g_arg: $g_ty),*) -> PyResult<PyPage> {
                    let $g_v2 = &self.v2;
                    let request = $g_build;
                    let page = py.detach(|| runtime().block_on(request.send())).map_err(data_err)?;
                    PyPage::new::<_, $g_row>(py, page)
                }

                #[doc = concat!("Walks every page of `", stringify!($g_name), "`, re-sending these filters on each one.")]
                #[pyo3(signature = $g_sig)]
                #[allow(clippy::too_many_arguments)]
                fn $g_iter(&self, $($g_arg: $g_ty),*) -> PyResult<PyPageIteratorSync> {
                    let $g_v2 = &self.v2;
                    let request = $g_build;
                    Ok(PyPageIteratorSync { pages: erase::<_, $g_row>(request.pages()) })
                }
            )*
        }
    };
}

EOF
python3 polyoxide-py/scripts/gen_v2_bindings.py routes
cat <<'EOF'

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDataV2>()?;
    m.add_class::<PyDataV2Sync>()?;
    m.add_class::<PyPage>()?;
    m.add_class::<PyPageIterator>()?;
    m.add_class::<PyPageIteratorSync>()?;
    Ok(())
}
EOF
} > polyoxide-py/src/clients/data_v2.rs
```

In `polyoxide-py/src/lib.rs`, replace:

```rust
    types::data_v2::register(&v2)?;
```

with:

```rust
    types::data_v2::register(&v2)?;
    clients::data_v2::register(&v2)?;
```

In `polyoxide-py/src/clients/data.rs`, replace (in `impl PyDataApi`):

```rust
    fn health(&self) -> PyDataApiHealth {
        PyDataApiHealth {
            client: self.client.clone(),
        }
    }
}
```

with:

```rust
    fn health(&self) -> PyDataApiHealth {
        PyDataApiHealth {
            client: self.client.clone(),
        }
    }

    /// The Data API v2 routes, sharing this client's connection pool and rate limiter.
    fn v2(&self) -> super::data_v2::PyDataV2 {
        super::data_v2::PyDataV2 {
            v2: self.client.v2(),
        }
    }
}
```

and (in `impl PyDataApiSync`):

```rust
    fn health(&self) -> PyDataApiHealthSync {
        PyDataApiHealthSync {
            client: self.client.clone(),
        }
    }
}
```

with:

```rust
    fn health(&self) -> PyDataApiHealthSync {
        PyDataApiHealthSync {
            client: self.client.clone(),
        }
    }

    /// The Data API v2 routes, sharing this client's connection pool and rate limiter.
    fn v2(&self) -> super::data_v2::PyDataV2Sync {
        super::data_v2::PyDataV2Sync {
            v2: self.client.v2(),
        }
    }
}
```

Run:

```bash
cargo clippy -p polyoxide-py --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

- [ ] **Step 3: Expose the new classes in Python**

In `polyoxide-py/python/polyoxide/v2.py`, replace:

```python
from ._polyoxide import v2 as _v2

Pagination = _v2.Pagination
```

with:

```python
from ._polyoxide import v2 as _v2

DataV2 = _v2.DataV2
DataV2Sync = _v2.DataV2Sync
Page = _v2.Page
PageIterator = _v2.PageIterator
PageIteratorSync = _v2.PageIteratorSync
Pagination = _v2.Pagination
```

and replace:

```python
__all__ = [
    "Pagination",
```

with:

```python
__all__ = [
    "DataV2",
    "DataV2Sync",
    "Page",
    "PageIterator",
    "PageIteratorSync",
    "Pagination",
```

Regenerate `polyoxide-py/python/polyoxide/v2.pyi` with the paging classes and both route classes:

```bash
{
cat <<'EOF'
"""Data API v2 (`/v2/*`): cursor-paged routes, snake_case fields, structured errors.

A missing or `None` number means the value was unavailable, never zero.
`outcome_index` is `999` when upstream could not label the outcome.

Generated by `polyoxide-py/scripts/gen_v2_bindings.py stub-rows` and
`stub-routes`, then owned by hand.
"""

from __future__ import annotations

from collections.abc import Coroutine
from typing import Any, Generic, TypeVar

T = TypeVar("T")

class Page(Generic[T]):
    """One page of a paged route. Follow `pagination.next_cursor` until it is None."""
    @property
    def data(self) -> list[T]: ...
    @property
    def pagination(self) -> Pagination: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class PageIterator(Generic[T]):
    """Async iterator over every page of a route, from `DataV2.iter_*`."""
    def __aiter__(self) -> PageIterator[T]: ...
    def __anext__(self) -> Coroutine[Any, Any, Page[T]]: ...

class PageIteratorSync(Generic[T]):
    """Iterator over every page of a route, from `DataV2Sync.iter_*`."""
    def __iter__(self) -> PageIteratorSync[T]: ...
    def __next__(self) -> Page[T]: ...

EOF
python3 polyoxide-py/scripts/gen_v2_bindings.py stub-rows
python3 polyoxide-py/scripts/gen_v2_bindings.py stub-routes
} > polyoxide-py/python/polyoxide/v2.pyi
```

In `polyoxide-py/python/polyoxide/__init__.pyi`, replace (the aliases keep `DataApi.v2` from shadowing the module inside the class body):

```python
from collections.abc import Coroutine
from typing import Any
```

with:

```python
from collections.abc import Coroutine
from typing import Any

from . import v2 as v2
from .v2 import DataV2 as _DataV2, DataV2Sync as _DataV2Sync
```

and replace:

```python
    def health(self) -> DataApiHealth:
        """Access health endpoints."""
        ...
```

with:

```python
    def health(self) -> DataApiHealth:
        """Access health endpoints."""
        ...
    def v2(self) -> _DataV2:
        """The Data API v2 routes, sharing this client's connection pool and rate limiter."""
        ...
```

and replace:

```python
    def health(self) -> DataApiHealthSync:
        """Access health endpoints."""
        ...
```

with:

```python
    def health(self) -> DataApiHealthSync:
        """Access health endpoints."""
        ...
    def v2(self) -> _DataV2Sync:
        """The Data API v2 routes, sharing this client's connection pool and rate limiter."""
        ...
```

Run:

```bash
(cd polyoxide-py && MATURIN_PEP517_ARGS="--profile dev" uv sync --reinstall-package polyoxide)
```

Expected: ends with a `polyoxide==0.31.0 (from file://…/polyoxide-py)` line.

Run:

```bash
(cd polyoxide-py && uv run pytest tests/test_data_v2_offline.py tests/test_stub_consistency.py -q)
```

Expected: `257 passed`: 43 offline tests, and 214 stub tests now that `DataV2`, `DataV2Sync` and the paging classes are declared.

- [ ] **Step 4: Prove the route and signature tests catch miswiring**

Send `trades(start=…)` to the `end` setter:

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/src/clients/data_v2.rs')
s = p.read_text()
i = 0
i = s.index('fn trades / iter_trades(', i)
i = s.index('request = request.start(v);', i)
p.write_text(s[:i] + 'request = request.end(v);' + s[i + len('request = request.start(v);'):])
EOF
(cd polyoxide-py && MATURIN_PEP517_ARGS="--profile dev" uv sync --reinstall-package polyoxide)
(cd polyoxide-py && uv run pytest tests/test_data_v2_offline.py -q -k trades)
```

Expected: `FAILED … test_route_sends_every_argument[trades-user-conditions-…]`: the query has no `start` and `end` is `62`.

Restore it, and drop `taker_only` from the sync `trades` stub instead. The rebuild picks up the restored Rust:

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/src/clients/data_v2.rs')
s = p.read_text()
i = 0
i = s.index('fn trades / iter_trades(', i)
i = s.index('request = request.end(v);', i)
p.write_text(s[:i] + 'request = request.start(v);' + s[i + len('request = request.end(v);'):])
EOF
```

Expected: no output.

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/python/polyoxide/v2.pyi')
s = p.read_text()
i = 0
i = s.index('class DataV2Sync:', i)
i = s.index('    def trades(', i)
i = s.index('        taker_only: bool | None = None,\n', i)
p.write_text(s[:i] + '' + s[i + len('        taker_only: bool | None = None,\n'):])
EOF
(cd polyoxide-py && MATURIN_PEP517_ARGS="--profile dev" uv sync --reinstall-package polyoxide)
(cd polyoxide-py && uv run pytest tests/test_stub_consistency.py -q -k 'DataV2Sync.trades')
```

Expected: `FAILED … test_v2_stub_signature_matches_compiled_method[DataV2Sync.trades]`.

Restore the stub:

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/python/polyoxide/v2.pyi')
s = p.read_text()
i = 0
i = s.index('class DataV2Sync:', i)
i = s.index('    def trades(', i)
i = s.index('        filter_type: str | None = None,\n', i)
p.write_text(s[:i] + '        taker_only: bool | None = None,\n        filter_type: str | None = None,\n' + s[i + len('        filter_type: str | None = None,\n'):])
EOF
(cd polyoxide-py && uv run pytest tests/test_data_v2_offline.py tests/test_stub_consistency.py -q)
```

Expected: `257 passed`.

- [ ] **Step 5: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-py --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-py
```

Expected: `test result: ok. 4 passed`.

Run:

```bash
(cd polyoxide-py && uv run pytest tests/ --ignore=tests/test_live_api.py -q)
```

Expected: `260 passed`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.lock polyoxide-py/Cargo.toml polyoxide-py/src/clients/data_v2.rs polyoxide-py/src/clients/mod.rs polyoxide-py/src/clients/data.rs polyoxide-py/src/lib.rs polyoxide-py/python/polyoxide/v2.py polyoxide-py/python/polyoxide/v2.pyi polyoxide-py/python/polyoxide/__init__.pyi polyoxide-py/tests/test_data_v2_offline.py
git commit -F - <<'EOF'
feat(py): DataApi.v2() with every Data API v2 route

DataApi.v2() and DataApiSync.v2() expose the 20 routes (21 methods) as
keyword-argument methods, declared once in v2_namespaces!. Paged routes
return a Page of wrapped rows and have iter_<route>, an async or sync
iterator over .pages(). Enum arguments take the wire spelling: request-only
enums raise ValueError listing the accepted values, response enums pass an
unknown value through. positions and resolutions validate their anchors.

test_data_v2_offline.py calls every route with every argument against a
local server serving the captured fixtures and checks the exact query sent.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```

### Task 4: Structured errors

**Files:**
- Modify: `polyoxide-py/src/error.rs`
- Modify: `polyoxide-py/python/polyoxide/__init__.pyi`, `polyoxide-py/tests/test_data_v2_offline.py`

- [ ] **Step 1: Write the failing error tests**

Today every Data API error goes through the v1 message matching, so a v2 `invalid_request` arrives as `ApiError` and none of the body's fields survive. The 429 test is slow on purpose: the client retries a 429 three times with backoff before giving up, and the test counts all four requests.

Append to `polyoxide-py/tests/test_data_v2_offline.py`:

```python


def test_a_walk_that_repeats_its_cursor_stops_with_the_base_error(server) -> None:
    server.reply("/v2/trades", page_of("trades", "same"))
    walk = polyoxide.DataApiSync(base_url=server.url).v2().iter_trades(cursor="same")

    with pytest.raises(polyoxide.PolyoxideError) as err:
        next(walk)

    assert type(err.value) is polyoxide.PolyoxideError
    assert "server returned the cursor it was sent" in str(err.value)
    assert err.value.code is None


def v2_error(code: str, **extra: object) -> dict:
    return {"error": f"{code} happened", "code": code, "retryable": code != "invalid_request", "trace_id": "t-1", **extra}


@pytest.mark.parametrize(
    ("status", "body", "headers", "error"),
    [
        (400, v2_error("invalid_request", parameter="user"), {}, polyoxide.ValidationError),
        (503, v2_error("request_timeout"), {"retry-after": "2"}, polyoxide.TimeoutError),
        (503, v2_error("dependency_unavailable"), {}, polyoxide.ApiError),
        (500, v2_error("brand_new_code"), {}, polyoxide.ApiError),
    ],
    ids=["invalid_request", "request_timeout", "dependency_unavailable", "unknown_code"],
)
def test_a_v2_error_body_maps_by_code_and_keeps_its_fields(server, status, body, headers, error) -> None:
    server.reply("/v2/user-pnl", body, status=status, headers=headers)

    with pytest.raises(error) as raised:
        polyoxide.DataApiSync(base_url=server.url).v2().user_pnl("0xuser")

    e = raised.value
    assert type(e) is error
    assert e.status == status
    assert e.code == ("unknown" if body["code"] == "brand_new_code" else body["code"])
    assert e.retryable is body["retryable"]
    assert e.trace_id == "t-1"
    assert e.parameter == body.get("parameter")
    assert e.retry_after == (2.0 if headers else None)
    assert "trace_id t-1" in str(e)


def test_rate_limited_is_a_rate_limit_error(server) -> None:
    # The client retries 429 three times with backoff first, so this takes ~4s.
    server.reply("/v2/status", v2_error("rate_limited"), status=429, headers={"retry-after": "0"})

    with pytest.raises(polyoxide.RateLimitError) as raised:
        polyoxide.DataApiSync(base_url=server.url).v2().status()

    assert raised.value.code == "rate_limited"
    assert raised.value.retryable is True
    assert len(server.requests) == 4


def test_async_error_carries_the_same_fields(server) -> None:
    server.reply("/v2/trades", v2_error("invalid_request", parameter="side"), status=400)

    async def call():
        await polyoxide.DataApi(base_url=server.url).v2().trades()

    with pytest.raises(polyoxide.ValidationError) as raised:
        asyncio.run(call())
    assert raised.value.parameter == "side"


def test_an_error_without_a_v2_body_has_none_fields(server) -> None:
    server.reply("/v2/status", "upstream exploded", status=500)

    with pytest.raises(polyoxide.PolyoxideError) as raised:
        polyoxide.DataApiSync(base_url=server.url).v2().status()

    e = raised.value
    assert (e.status, e.code, e.retryable, e.trace_id, e.parameter, e.retry_after) == (None,) * 6


def test_v1_errors_also_carry_the_attributes(server) -> None:
    server.reply("/", "nope", status=500)

    with pytest.raises(polyoxide.PolyoxideError) as raised:
        polyoxide.DataApiSync(base_url=server.url).health().ping()

    assert raised.value.code is None
```

Run:

```bash
(cd polyoxide-py && uv run pytest tests/test_data_v2_offline.py -q)
```

Expected: `9 failed, 43 passed`. The v2 bodies arrive as `ApiError` (or, by message matching, `TimeoutError`/`RateLimitError`) with no `status` attribute, and the stalled walk raises `ApiError` rather than the base class.

- [ ] **Step 2: Map v2 errors by code and carry their fields**

`with_details` sets the six attributes on every exception the SDK raises, `None` unless it came from a v2 body. That keeps the stub's `PolyoxideError` annotations true for Gamma and CLOB errors too. They are instance attributes, so the class members `test_stub_consistency.py` compares are unchanged.

In `polyoxide-py/src/error.rs`, replace:

```rust
pub fn gamma_err(e: polyoxide_gamma::GammaError) -> PyErr {
    map_api_err(&e)
}

pub fn data_err(e: polyoxide_data::DataApiError) -> PyErr {
    map_api_err(&e)
}

pub fn clob_err(e: polyoxide_clob::ClobError) -> PyErr {
    map_api_err(&e)
}
```

with:

```rust
pub fn gamma_err(e: polyoxide_gamma::GammaError) -> PyErr {
    with_details(map_api_err(&e), None)
}

/// A Data API v2 error body maps by its stable `code`; anything else keeps the
/// message matching the v1 routes have always used.
pub fn data_err(e: polyoxide_data::DataApiError) -> PyErr {
    use polyoxide_data::{v2::ErrorCode, DataApiError};

    match &e {
        DataApiError::V2(v2) => {
            let msg = e.to_string();
            let err = match v2.code {
                ErrorCode::InvalidRequest => ValidationError::new_err(msg),
                ErrorCode::RateLimited => RateLimitError::new_err(msg),
                ErrorCode::RequestTimeout => TimeoutError::new_err(msg),
                _ => ApiError::new_err(msg),
            };
            with_details(err, Some(v2))
        }
        DataApiError::Pagination(_) => with_details(PolyoxideError::new_err(e.to_string()), None),
        _ => with_details(map_api_err(&e), None),
    }
}

pub fn clob_err(e: polyoxide_clob::ClobError) -> PyErr {
    with_details(map_api_err(&e), None)
}
```

and replace:

```rust
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
```

with:

```rust
/// Sets the Data API v2 error fields on the exception: the server's values for
/// a v2 error body and `None` for any other error, so every exception this SDK
/// raises has all six attributes.
fn with_details(err: PyErr, v2: Option<&polyoxide_data::v2::V2Error>) -> PyErr {
    Python::attach(|py| -> PyResult<()> {
        let value = err.value(py);
        value.setattr("status", v2.map(|v| v.status))?;
        value.setattr("code", v2.map(|v| v.code.as_str()))?;
        value.setattr("retryable", v2.map(|v| v.retryable))?;
        value.setattr("trace_id", v2.map(|v| v.trace_id.as_str()))?;
        value.setattr("parameter", v2.and_then(|v| v.parameter.as_deref()))?;
        value.setattr(
            "retry_after",
            v2.and_then(|v| v.retry_after).map(|d| d.as_secs_f64()),
        )?;
        Ok(())
    })
    .expect("an exception instance accepts attributes");
    err
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
```

In `polyoxide-py/python/polyoxide/__init__.pyi`, replace:

```python
class PolyoxideError(Exception):
    """Base exception for all polyoxide errors."""
    ...
```

with:

```python
class PolyoxideError(Exception):
    """Base exception for all polyoxide errors.

    The attributes below carry a Data API v2 error body's fields. Each is set on
    every exception the SDK raises, and is None unless the error came from a v2
    route.
    """
    status: int | None
    """HTTP status."""
    code: str | None
    """Stable classification, e.g. `invalid_request`, `rate_limited`, `dependency_unavailable`."""
    retryable: bool | None
    """Whether the server says the request may be retried unchanged."""
    trace_id: str | None
    """Id to quote when reporting a failure."""
    parameter: str | None
    """The query parameter a validation failure concerns, when the server names one."""
    retry_after: float | None
    """The `Retry-After` delay in seconds, when the response carried one."""
```

Run:

```bash
(cd polyoxide-py && MATURIN_PEP517_ARGS="--profile dev" uv sync --reinstall-package polyoxide)
```

Expected: ends with a `polyoxide==0.31.0 (from file://…/polyoxide-py)` line.

Run:

```bash
(cd polyoxide-py && uv run pytest tests/test_data_v2_offline.py -q)
```

Expected: `52 passed`, in about 5 seconds (the 429 test takes about 3).

- [ ] **Step 3: Prove the mapping test can fail**

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/src/error.rs')
s = p.read_text()
assert s.count('ErrorCode::InvalidRequest => ValidationError::new_err(msg),') == 1
p.write_text(s.replace('ErrorCode::InvalidRequest => ValidationError::new_err(msg),', 'ErrorCode::InvalidRequest => ApiError::new_err(msg),'))
EOF
(cd polyoxide-py && MATURIN_PEP517_ARGS="--profile dev" uv sync --reinstall-package polyoxide)
(cd polyoxide-py && uv run pytest tests/test_data_v2_offline.py -q -k 'invalid_request or async_error')
```

Expected: `FAILED … test_a_v2_error_body_maps_by_code_and_keeps_its_fields[invalid_request]` and `FAILED … test_async_error_carries_the_same_fields`.

Run:

```bash
python3 - <<'EOF'
from pathlib import Path
p = Path('polyoxide-py/src/error.rs')
s = p.read_text()
assert s.count('ErrorCode::InvalidRequest => ApiError::new_err(msg),') == 1
p.write_text(s.replace('ErrorCode::InvalidRequest => ApiError::new_err(msg),', 'ErrorCode::InvalidRequest => ValidationError::new_err(msg),'))
EOF
(cd polyoxide-py && MATURIN_PEP517_ARGS="--profile dev" uv sync --reinstall-package polyoxide)
```

Expected: the rebuild succeeds.

- [ ] **Step 4: Run the gates**

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy -p polyoxide-py --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
cargo test -p polyoxide-py
```

Expected: `test result: ok. 4 passed`.

Run:

```bash
(cd polyoxide-py && uv run pytest tests/ --ignore=tests/test_live_api.py -q)
```

Expected: `269 passed`.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-py/src/error.rs polyoxide-py/python/polyoxide/__init__.pyi polyoxide-py/tests/test_data_v2_offline.py
git commit -F - <<'EOF'
feat(py): map Data API v2 errors by code and keep their fields

A v2 error body now maps by its stable code: invalid_request to
ValidationError, rate_limited to RateLimitError, request_timeout to
TimeoutError, any other code to ApiError. A stalled page walk raises the base
PolyoxideError. Every exception the SDK raises carries status, code,
retryable, trace_id, parameter and retry_after, None unless the error came
from a v2 route.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```

### Task 5: Live tests and README

**Files:**
- Modify: `polyoxide-py/tests/test_live_api.py`, `polyoxide-py/README.md`

- [ ] **Step 1: Add live tests mirroring `live_v2_*` in `polyoxide-data/tests/live_api.rs`**

`test_live_api.py` is not gated: CI's python job runs it on every push. These seven tests send about 35 requests, with inputs taken from the live feed rather than hardcoded.

In `polyoxide-py/tests/test_live_api.py`, add the v2 section before the `Error Hierarchy` banner. Replace:

```python
# ══════════════════════════════════════════════════════════════════
# Error Hierarchy
```

with:

```python
# ══════════════════════════════════════════════════════════════════
# Data API v2
# ══════════════════════════════════════════════════════════════════
#
# Mirrors the `live_v2_*` tests in polyoxide-data/tests/live_api.rs. Inputs
# come from the live feed, never hardcoded: a fixed wallet goes quiet.


def _recent_trade():
    """(wallet, condition_id, token_id) of the newest trade on the bare feed."""
    trade = polyoxide.DataApiSync().v2().trades(limit=1).data[0]
    return trade.proxy_wallet, trade.condition_id, trade.token_id


def _unknown_wallet():
    """A random address: `0x…0001` appears on chain and is a known wallet."""
    import secrets

    return "0x" + secrets.token_hex(20)


class TestDataV2Sync:
    def test_trades_walk_follows_the_cursor(self):
        walk = polyoxide.DataApiSync().v2().iter_trades(limit=2)
        first, second = next(walk), next(walk)

        assert isinstance(first, polyoxide.v2.Page)
        assert first.pagination.next_cursor, "the feed has more than 2 rows"
        assert isinstance(first.data[0], polyoxide.v2.Trade)
        assert first.data[0].transaction_hash != second.data[0].transaction_hash, "page 2 repeated page 1"

    def test_wallet_routes(self):
        wallet, _, _ = _recent_trade()
        v2 = polyoxide.DataApiSync().v2()

        assert isinstance(v2.activity(wallet, limit=2), polyoxide.v2.Page)
        assert isinstance(v2.combo_activity(wallet, limit=2), polyoxide.v2.Page)
        assert isinstance(v2.positions(user=wallet, limit=2), polyoxide.v2.Page)
        assert isinstance(v2.positions(user=wallet, status="CLOSED", limit=2), polyoxide.v2.Page)
        assert isinstance(v2.combo_positions(wallet, limit=2), polyoxide.v2.Page)
        assert isinstance(v2.approvals(wallet), polyoxide.v2.Approvals)
        pnl = v2.user_pnl(wallet, interval="1w", fidelity="1d")
        assert pnl.proxy_wallet.lower() == wallet.lower()
        assert isinstance(v2.user_stats(wallet), polyoxide.v2.UserStats), "a wallet that just traded is known"
        assert isinstance(v2.user_volume(wallet), polyoxide.v2.UserVolume)
        assert isinstance(v2.value(wallet), polyoxide.v2.PortfolioValue)

    def test_unknown_wallet_is_none_not_an_error(self):
        wallet = _unknown_wallet()
        v2 = polyoxide.DataApiSync().v2()

        assert v2.user_stats(wallet) is None
        assert v2.leaderboard_user(wallet) is None

    def test_market_routes(self):
        _, condition, token_id = _recent_trade()
        v2 = polyoxide.DataApiSync().v2()

        assert isinstance(v2.holders([condition], limit=2), polyoxide.v2.Page)
        assert isinstance(v2.positions(conditions=[condition], limit=2), polyoxide.v2.Page)
        assert isinstance(v2.open_interest(conditions=[condition]), list)
        global_oi = v2.open_interest()
        assert [row.condition_id for row in global_oi] == ["GLOBAL"]
        assert isinstance(v2.prices_history(token_id, interval="1d", limit=10), polyoxide.v2.Page)

        winners = v2.biggest_winners(time_period="week", limit=5)
        resolved = next(w for w in winners.data if w.kind == "market")
        assert v2.resolutions(conditions=[resolved.condition_id]), "a market on the winners board has resolved"
        assert isinstance(v2.live_volume([str(resolved.event_id)]), polyoxide.v2.LiveVolume)

    def test_board_routes(self):
        v2 = polyoxide.DataApiSync().v2()

        board = v2.leaderboard(time_period="week", limit=2)
        leader = board.data[0].user_id
        standing = v2.leaderboard_user(leader, time_period="week")
        assert standing is not None, "the board's leader has a standing"
        assert standing.rank_pnl is not None or standing.rank_volume is not None
        assert isinstance(v2.builders_leaderboard(limit=2), polyoxide.v2.Page)
        assert isinstance(v2.builder_volume(limit=2), list)
        assert isinstance(v2.status(), polyoxide.v2.ServiceStatus)

    def test_errors_are_structured(self):
        try:
            polyoxide.DataApiSync().v2().trades(cursor="garbage")
        except polyoxide.ValidationError as e:
            assert e.status == 400
            assert e.code == "invalid_request"
            assert e.trace_id
            assert e.retryable is False
        else:
            raise AssertionError("a garbage cursor is refused")


class TestDataV2Async:
    def test_trades_walk(self):
        async def go():
            pages = []
            async for page in polyoxide.DataApi().v2().iter_trades(limit=2):
                pages.append(page)
                if len(pages) == 2:
                    break
            return pages

        first, second = run_async(go())
        assert first.data[0].transaction_hash != second.data[0].transaction_hash


# ══════════════════════════════════════════════════════════════════
# Error Hierarchy
```

Run:

```bash
(cd polyoxide-py && uv run pytest tests/test_live_api.py -q -k V2)
```

Expected: `7 passed`.

- [ ] **Step 2: Document v2 in the README**

In `polyoxide-py/README.md`, replace:

````markdown
    positions = await data.user("0xADDRESS").list_positions(limit=5)
    for p in positions:
        print(p.title, p.size)

asyncio.run(main())
```
````

with:

````markdown
    positions = await data.user("0xADDRESS").list_positions(limit=5)
    for p in positions:
        print(p.title, p.size)

asyncio.run(main())
```

### Data API v2 (cursor pages)

`DataApi().v2()` and `DataApiSync().v2()` expose the `/v2` routes: snake_case
fields, cursor-only paging, and errors with a stable `code`. Their result classes
live in `polyoxide.v2`, since several share a name with a v1 class.

```python
from polyoxide import DataApiSync, v2

data = DataApiSync().v2()

# One page, and the cursor for the next one
page = data.trades(limit=50)
print(len(page), page.pagination.next_cursor)

# Every page, re-sending the same filters each time
for page in data.iter_activity("0xADDRESS", types=["TRADE"], limit=500):
    for row in page.data:
        print(row.activity_type, row.usdc_size)

# None, not an error, for a wallet the API does not know
stats = data.user_stats("0xADDRESS")
```

Async walks use `async for page in DataApi().v2().iter_trades(...)`. Enum
arguments take the exact wire spelling (`time_period="week"`,
`sort_direction="DESC"`), and a value the route does not accept raises
`ValueError` before any request is sent.
````

and replace:

```markdown
| `.health()` | `ping()` |

## Result Objects
```

with:

```markdown
| `.health()` | `ping()` |
| `.v2()` | `approvals`, `positions`, `combo_positions`, `user_pnl`, `user_stats`, `user_volume`, `value`, `activity`, `combo_activity`, `trades`, `holders`, `live_volume`, `open_interest`, `prices_history`, `resolutions`, `biggest_winners`, `builders_leaderboard`, `builder_volume`, `leaderboard`, `leaderboard_user`, `status`; each paged route also has `iter_<route>(...)` |

## Result Objects
```

and replace:

```markdown
| `TimeoutError` | Request timed out |
```

with:

```markdown
| `TimeoutError` | Request timed out |

A Data API v2 error maps by its `code`: `invalid_request` to `ValidationError`,
`rate_limited` to `RateLimitError`, `request_timeout` to `TimeoutError`, and
any other code to `ApiError`. Every exception also carries `status`, `code`,
`retryable`, `trace_id`, `parameter` and `retry_after`, which are `None` unless
the error came from a v2 route.
```

and replace:

```markdown
A `.pyi` stub file is included at `python/polyoxide/__init__.pyi` for editor autocomplete and type checking.
```

with:

```markdown
`.pyi` stub files are included at `python/polyoxide/__init__.pyi` and `python/polyoxide/v2.pyi` for editor autocomplete and type checking.
```

- [ ] **Step 3: Commit**

```bash
git add polyoxide-py/tests/test_live_api.py polyoxide-py/README.md
git commit -F - <<'EOF'
test(py): live Data API v2 tests and README usage

Seven live tests mirror the live_v2_* tests in polyoxide-data: a two-page
walk (sync and async), the wallet, market and board routes with inputs taken
from the live feed, an unknown wallet reading as None, and a structured
error. The README gains a v2 section and the error attributes.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01PHCoySzmmAem89S7HttcK5
EOF
```

### Task 6: Final gate

**Files:**
- No changes

- [ ] **Step 1: Run every gate CI runs for this change**

Workspace-wide, because `Cargo.lock` changed. If rustc is killed with signal 15 or exit 254, that is earlyoom on this machine; re-run with `-j 4`.

Run:

```bash
cargo fmt --all -- --check
```

Expected: no output, exit 0.

Run:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: finishes with no warnings.

Run:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
```

Expected: no warnings.

Run:

```bash
cargo test -p polyoxide-py
```

Expected: `test result: ok. 4 passed`.

Run:

```bash
(cd polyoxide-py && MATURIN_PEP517_ARGS="--profile dev" uv sync --reinstall-package polyoxide)
(cd polyoxide-py && uv run pytest tests/ -q)
```

Expected: `326 passed` (155 before this plan), including the live suite. CI builds the extension with the release profile; nothing here depends on the profile.

Run:

```bash
git status --short -- polyoxide-py Cargo.lock
```

Expected: no output. (Other paths may show the Phase 5 session's work.)
