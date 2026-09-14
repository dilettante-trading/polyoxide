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
