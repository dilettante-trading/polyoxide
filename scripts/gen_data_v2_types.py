#!/usr/bin/env python3
"""Emit Rust response structs for Data API v2 from the vendored OpenAPI mirror.

One-shot scaffolding for the v2 implementation: the output is committed and
then owned by hand. It is NOT a build step and is never re-run to overwrite
edited files. The durable guard is `polyoxide-data/tests/v2_spec_agreement.rs`,
which fails if a struct disagrees with the spec.

Usage:
    python3 scripts/gen_data_v2_types.py Trade UserStats UserPnlPoint > out.rs

Rules (see docs/superpowers/specs/2026-09-14-data-api-v2-design.md, Component 2):
- required and non-nullable  -> plain field; anything else -> Option<T>
- no skip_serializing_if, no serde(default)
- i64/u64 fields get `specta(type = f64)` like v1's timestamps
- FIELD_OVERRIDES swaps in hand-written enums/newtypes
"""
import json
import re
import sys
from pathlib import Path

SPEC = Path(__file__).resolve().parent.parent / "docs/specs/data-v2/openapi.json"

# (schema, property) -> Rust type used in place of the mechanical mapping.
# Optionality is still applied on top, so give the inner type only.
FIELD_OVERRIDES = {
    ("Trade", "side"): "TradeSide",
    ("Activity", "side"): "ActivitySide",
    ("Activity", "type"): "ActivityType",
    ("Position", "status"): "PositionStatus",
    ("ApprovalContract", "amount"): "Allowance",
}
# Any property with this name and an integer type becomes OutcomeIndex.
OUTCOME_INDEX = "outcome_index"

RUST_KEYWORDS = {"type", "match", "ref", "move", "use", "mod", "fn", "impl", "self", "struct", "enum", "trait", "where", "loop", "box", "static", "const", "crate", "super", "in", "as", "for", "if", "else", "while", "return", "let", "mut", "pub", "true", "false", "async", "await", "dyn"}


def is_nullable(prop):
    t = prop.get("type")
    if isinstance(t, list) and "null" in t:
        return True
    if "oneOf" in prop and any(o.get("type") == "null" for o in prop["oneOf"]):
        return True
    return False


def base_type(prop):
    """Rust type ignoring nullability. Returns (rust, needs_specta_f64)."""
    if "$ref" in prop:
        return prop["$ref"].rsplit("/", 1)[1], False
    if "oneOf" in prop:
        arms = [o for o in prop["oneOf"] if o.get("type") != "null"]
        if len(arms) != 1:
            raise SystemExit(f"unsupported oneOf: {prop}")
        return base_type(arms[0])
    t = prop.get("type")
    if isinstance(t, list):
        t = next(x for x in t if x != "null")
    fmt = prop.get("format")
    unsigned = prop.get("minimum") == 0
    if t == "string":
        return "String", False
    if t == "boolean":
        return "bool", False
    if t == "number":
        return "f64", False
    if t == "integer":
        if fmt == "int32":
            return ("u32" if unsigned else "i32"), False
        return ("u64" if unsigned else "i64"), True
    if t == "array":
        inner, big = base_type(prop["items"])
        return f"Vec<{inner}>", big
    raise SystemExit(f"unsupported property: {prop}")


def doc(text, indent):
    """Rustdoc lines. Escapes `[`/`]` outside code spans (broken intra-doc
    links) and wraps bare URLs (rustdoc::bare_urls): both are errors under
    RUSTDOCFLAGS=-D warnings."""
    out = []
    for line in (text or "").strip().splitlines():
        parts = re.split(r"(`[^`]*`)", line)
        for i, p in enumerate(parts):
            if i % 2 == 0:
                p = p.replace("[", r"\[").replace("]", r"\]")
                p = re.sub(r"(?<![<(])(https?://[^\s)>]+)", r"<\1>", p)
                parts[i] = p
        line = "".join(parts).rstrip()
        out.append(f"{indent}///" + (f" {line}" if line else ""))
    return "\n".join(out)


def emit(name, schema):
    lines = []
    if schema.get("description"):
        lines.append(doc(schema["description"], ""))
    lines.append('#[cfg_attr(feature = "specta", derive(specta::Type))]')
    lines.append("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]")
    lines.append("#[non_exhaustive]")
    lines.append(f"pub struct {name} {{")
    required = set(schema.get("required", []))
    for key, prop in sorted(schema.get("properties", {}).items()):
        rust, big = base_type(prop)
        if (name, key) in FIELD_OVERRIDES:
            rust, big = FIELD_OVERRIDES[(name, key)], False
        elif key == OUTCOME_INDEX and rust in ("i32", "u32", "i64", "u64"):
            rust, big = "OutcomeIndex", False
        optional = key not in required or is_nullable(prop)
        ty = f"Option<{rust}>" if optional else rust
        desc = prop.get("description")
        if desc is None and "oneOf" in prop:
            desc = next((o.get("description") for o in prop["oneOf"] if o.get("description")), None)
        if desc:
            lines.append(doc(desc, "    "))
        if big:
            specta_ty = re.sub(r"\b[iu]64\b", "f64", ty)
            lines.append(f'    #[cfg_attr(feature = "specta", specta(type = {specta_ty}))]')
        field = key
        if key in RUST_KEYWORDS:
            field = re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower() + f"_{key}"
            lines.append(f'    #[serde(rename = "{key}")]')
        lines.append(f"    pub {field}: {ty},")
    lines.append("}")
    return "\n".join(lines)


def main():
    schemas = json.loads(SPEC.read_text())["components"]["schemas"]
    names = sys.argv[1:]
    if not names:
        raise SystemExit("usage: gen_data_v2_types.py SchemaName [SchemaName ...]")
    print("\n\n".join(emit(n, schemas[n]) for n in names))


if __name__ == "__main__":
    main()
