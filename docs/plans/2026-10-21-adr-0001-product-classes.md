# ADR 0001: Product Classes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task (the owner chose inline execution). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every product declares its `ProductClass`, every `MarketKey` carries it, each crate registers it in its metadata, and two class bundles, `PerpetualMarket` and `EventContractMarket`, give consumers one bound per class.

**Architecture:** `polyoxide-venue` gains `ProductClass` (feature `keys`) and a required `KeySpace::CLASS`, which `KeySpace::key` stamps on the key, and the two bundles with blanket impls (feature `market`). Each crate registers `classes = { <product> = "<class>" }` beside `products`; `gen_registry.py` checks it and each venue crate's registration test holds `CLASS` to it. A `const _` per venue crate pins each product to its bundle, and an umbrella test checks the class of every product `full` builds.

**Tech Stack:** Rust (MSRV 1.91, dev shell 1.99.0), dynosaur 0.3.1, toml 0.8 in test-support, Python 3 with pytest for the registry generator.

**Spec:** `docs/decisions/0001-product-classes-over-one-capability-core.md` (ADR 0001, as updated for Epic 9 in 535a9a8).

---

## Decisions this plan makes

The ADR leaves three things open. This plan settles them:

- **Registration shape.** A table keyed by product, `classes = { clob = "event-contract", perps = "perpetual-future" }`, beside `products`. A product without a `KeySpace` (gamma, data, relay, rtds, sports) has no entry. `gen_registry.py` checks that each key is one of the crate's `products` and each value is an id. It does not keep its own list of classes: each venue crate's Rust registration test compares the value with `CLASS.as_str()`, so a class the Rust enum does not know fails there.
- **The class is a field on `MarketKey`.** It is placed after `product`, so the derived `Ord` still sorts by venue, product and id (the class follows from the first two), and `Eq` and `Hash` mean what they meant before.
- **`ProductClass` is written by hand**, not with `wire_enum!`, because `MarketKey` derives `PartialOrd` and `Ord` and `wire_enum!` derives neither. It has `as_str` (the registration's spelling) and `Display`, and no `FromStr` or serde, which nothing needs yet.

## Rules for every task

- **Run every cargo, Python and uv command through the dev shell**: `nix develop --command <cmd>`. Use `CARGO_INCREMENTAL=0` for the workspace-wide builds (memory: local build OOM).
- **The foundation crates name no venue** (`python3 scripts/venue_gate.py`). Docs and tests in `polyoxide-venue` and `polyoxide-test-support` use made-up words (`acme`, `spot`, `perp`), never a venue or product id from the workspace.
- **No AD-12 constraint applies.** Every test file this plan edits was added after `v0.40.0` (checked 2026-10-21), so editing an existing test body needs no `[[drift]]` entry. `python3 scripts/test_body_diff.py --baseline v0.40.0` still runs in the final gates.
- **Generated regions are regenerated, never edited**: run `python3 scripts/gen_registry.py --write` after editing any `[package.metadata.polyoxide]`. `docs/ARCHITECTURE.md` is never edited.
- **README text is prose with no code fence**, because the crate READMEs are doctests.
- **No version bump.** A required associated const on a public trait is a major-level change for an outside implementer of `KeySpace`. The removals gate allows it (it counts only removals), and the release's semver job needs a 0.x minor bump, which the owner's pending 0.42.0 provides. If this lands after 0.42.0 ships, the next release must be 0.43.0.
- **Commit by pathspec**, conventional, naming the ADR. The one-line messages below are the subject; each commit's body ends with the session's attribution lines.

## Files

| File | Change |
|---|---|
| `polyoxide-venue/src/key.rs` | `ProductClass`, `KeySpace::CLASS`, `MarketKey::class`, tests |
| `polyoxide-venue/src/market.rs` | `PerpetualMarket`, `EventContractMarket`, a test; `CLASS` on two test key spaces |
| `polyoxide-venue/src/lib.rs` | re-exports, crate docs |
| `polyoxide-venue/src/records.rs`, `tests/trading.rs`, `tests/candles_intervals.rs` | `CLASS` on test key spaces |
| `polyoxide-polymarket/src/shared/keys.rs`, `polyoxide-binance/src/usdm/key.rs`, `polyoxide-kalshi/src/predictions/key.rs` | `CLASS` on the real key spaces; registration tests assert it |
| `polyoxide-polymarket/tests/{venue_perps,trading_clob,venue_clob}.rs`, `polyoxide-binance/tests/venue_usdm.rs`, `polyoxide-kalshi/tests/venue_predictions.rs` | `CLASS` on test key spaces |
| `polyoxide-{polymarket,binance,kalshi}/src/lib.rs` | bundle pins |
| `polyoxide-{polymarket,binance,kalshi}/Cargo.toml` | `classes` |
| `polyoxide-test-support/src/registration.rs` | `Registration::classes` |
| `scripts/gen_registry.py`, `.github/scripts/tests/test_gen_registry.py` | `classes` key, validation, tests |
| `polyoxide/tests/parse_key.rs` | the class round trip |
| `polyoxide-venue/Cargo.toml` (metadata `readme`), `polyoxide-venue/README.md`, `CLAUDE.md`, `README.md` (generated region) | docs |
| `docs/s3-migration.md` | consumer-impact note |
| `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/adr-0001.md` | the two amendments |
| `docs/decisions/0001-product-classes-over-one-capability-core.md` | status `accepted` |

---

### Task 1: `ProductClass` on every key space and key

**Files:**
- Modify: `polyoxide-venue/src/key.rs`, `polyoxide-venue/src/lib.rs`
- Modify (one `CLASS` line each): the 15 other `impl KeySpace for` sites listed in Step 6

- [ ] **Step 1: Write the failing tests**

Append these two tests at the end of `mod tests` in `polyoxide-venue/src/key.rs`, after `a_static_id_must_be_an_id`:

```rust
    #[test]
    fn a_key_carries_the_class_its_product_declares() {
        assert_eq!(upper("ABC").class(), ProductClass::EventContract);
        let parsed = Numbered::parse(&raw("acme.perpetual:007")).unwrap();
        assert_eq!(parsed.class(), ProductClass::PerpetualFuture);
        assert_eq!(parsed.class(), Numbered::CLASS);
        // The class is not part of the text form.
        assert_eq!(parsed.to_string(), "acme.perpetual:7");
    }

    #[test]
    fn a_class_is_spelled_as_a_registration_spells_it() {
        assert_eq!(ProductClass::EventContract.as_str(), "event-contract");
        assert_eq!(
            ProductClass::PerpetualFuture.to_string(),
            "perpetual-future"
        );
    }
```

In the same module, give the two test key spaces their classes. After `const PRODUCT: ProductId = ProductId::from_static("spot");` in `impl KeySpace for Upper`, add:

```rust
        const CLASS: ProductClass = ProductClass::EventContract;
```

After `const PRODUCT: ProductId = ProductId::from_static("perpetual");` in `impl KeySpace for Numbered`, add:

```rust
        const CLASS: ProductClass = ProductClass::PerpetualFuture;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `nix develop --command cargo test -p polyoxide-venue --features keys --lib key::tests`
Expected: a compile error, `cannot find type ProductClass in this scope`.

- [ ] **Step 3: Add `ProductClass`**

In `polyoxide-venue/src/key.rs`, insert after the `id_type! { … ProductId, "product" }` block:

```rust
/// What a product trades, which fixes what its shared records mean.
///
/// A product declares its class as [`KeySpace::CLASS`], and every key it makes
/// carries it, so [`MarketKey::class`] answers without a request. A class is
/// added only when a product of that class lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ProductClass {
    /// Contracts on an event's outcomes. A key names one outcome, a price lies
    /// between 0 and 1, and a share pays one unit of the settlement currency
    /// when its outcome happens, so a position's size is never negative.
    EventContract,
    /// Perpetual futures. A price is unbounded, a position's size is signed,
    /// and holders pay or receive funding.
    PerpetualFuture,
}

impl ProductClass {
    /// The class's id, as a crate's registration spells it in `classes`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EventContract => "event-contract",
            Self::PerpetualFuture => "perpetual-future",
        }
    }
}

impl fmt::Display for ProductClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
```

- [ ] **Step 4: Stamp the class on the key**

In `polyoxide-venue/src/key.rs`:

Replace the `MarketKey` struct:

```rust
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarketKey {
    venue: VenueId,
    product: ProductId,
    // After `product`: the class follows from the venue and product, so the
    // derived order still sorts by venue, product and id.
    class: ProductClass,
    id: Arc<str>,
}
```

In `impl MarketKey`, after `pub fn product(&self)`, add:

```rust
    /// The product's class, which its [`KeySpace::CLASS`] declares. It follows
    /// from the venue and product, so the text form does not carry it.
    pub fn class(&self) -> ProductClass {
        self.class
    }
```

In `pub trait KeySpace`, after `const PRODUCT: ProductId;` and its doc line, add:

```rust
    /// The product's class, equal to the one the venue crate registers for
    /// the product in `classes`.
    const CLASS: ProductClass;
```

In `KeySpace::key`, add `class: Self::CLASS,` after `product: Self::PRODUCT,`.

Rewrite the `MarketKey` doc example so it shows a class that exists. The heading line `/// One instrument's name, written \`venue.product:id\`, such as` keeps its next line as `/// \`acme.perp:ABCUSD\`.`, and the example becomes:

```rust
/// ```
/// use std::borrow::Cow;
/// use polyoxide_venue::{KeySpace, MarketKey, ProductClass, ProductId, RawKey, VenueId};
///
/// /// A perpetual whose ids are ASCII and uppercased.
/// struct Perp;
///
/// impl KeySpace for Perp {
///     const VENUE: VenueId = VenueId::from_static("acme");
///     const PRODUCT: ProductId = ProductId::from_static("perp");
///     const CLASS: ProductClass = ProductClass::PerpetualFuture;
///     type Id = PerpId;
///
///     fn parse_id(text: &str) -> Result<PerpId, String> {
///         if text.is_ascii() {
///             Ok(PerpId(text.to_ascii_uppercase()))
///         } else {
///             Err("an id is ASCII".into())
///         }
///     }
///
///     fn id_text(id: &PerpId) -> Cow<'_, str> {
///         Cow::Borrowed(&id.0)
///     }
/// }
///
/// /// An id in canonical form: only `Perp::parse_id` makes one.
/// struct PerpId(String);
///
/// let key = Perp::parse(&RawKey::parse("acme.perp:abcusd")?)?;
/// assert_eq!(key, Perp::key(&Perp::parse_id("ABCUSD")?));
/// assert_eq!(key.to_string(), "acme.perp:ABCUSD");
/// assert_eq!(key.class(), ProductClass::PerpetualFuture);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
```

In `polyoxide-venue/src/lib.rs`, change the keys re-export to:

```rust
#[cfg(feature = "keys")]
pub use key::{KeyError, KeySpace, MarketKey, ProductClass, ProductId, RawKey, VenueId};
```

- [ ] **Step 5: Run the key tests to verify they pass**

Run: `nix develop --command cargo test -p polyoxide-venue --features keys --lib key::tests`
Expected: PASS, including the two new tests.

- [ ] **Step 6: Give every other key space its class**

Each of these files has one or two `impl KeySpace for` blocks and imports `ProductId` in a `use` statement before its first `const PRODUCT` line. Save this script as `/tmp/claude-1000/-tb-Source-DilettanteTrading-polyoxide--loom-worktrees-aidanb-asset-event-adr-18dfe2e2345eed97/d94b65a8-93eb-4f6b-b37c-03b84d141657/scratchpad/add_class.py`. It adds the `CLASS` line after each `const PRODUCT` line, with the classes in file order, and adds `ProductClass` before the first `ProductId` (the import):

```python
"""Adds `const CLASS` to each KeySpace impl, and `ProductClass` to the import."""
import re
import sys
from pathlib import Path

EVENT, PERP = "EventContract", "PerpetualFuture"
EDITS = {
    # The real products.
    "polyoxide-polymarket/src/shared/keys.rs": [EVENT, PERP],  # ClobKeys, PerpsKeys
    "polyoxide-binance/src/usdm/key.rs": [PERP],               # UsdmKeys
    "polyoxide-kalshi/src/predictions/key.rs": [EVENT],        # PredictionsKeys
    # Test key spaces: the class is the one the name suggests, else a perpetual.
    "polyoxide-venue/src/market.rs": [PERP, PERP],             # Spot, Other
    "polyoxide-venue/src/records.rs": [PERP],                  # Spot
    "polyoxide-venue/tests/trading.rs": [EVENT, PERP],         # Binary, Swap
    "polyoxide-venue/tests/candles_intervals.rs": [PERP],      # Spot
    "polyoxide-polymarket/tests/venue_perps.rs": [PERP],       # Elsewhere
    "polyoxide-polymarket/tests/trading_clob.rs": [PERP],      # Elsewhere
    "polyoxide-polymarket/tests/venue_clob.rs": [PERP],        # Elsewhere
    "polyoxide-binance/tests/venue_usdm.rs": [PERP],           # Elsewhere
    "polyoxide-kalshi/tests/venue_predictions.rs": [PERP],     # Other
}
PRODUCT = re.compile(r"^(\s*)const PRODUCT: ProductId = ")

for path, classes in EDITS.items():
    lines, out, n = Path(path).read_text().split("\n"), [], 0
    for line in lines:
        out.append(line)
        if m := PRODUCT.match(line):
            out.append(f"{m.group(1)}const CLASS: ProductClass = ProductClass::{classes[n]};")
            n += 1
    if n != len(classes):
        sys.exit(f"{path}: {n} impls, expected {len(classes)}")
    text, imported = re.subn(r"\bProductId\b", "ProductClass, ProductId", "\n".join(out), count=1)
    if not imported:
        sys.exit(f"{path}: no ProductId import")
    Path(path).write_text(text)
    print(f"{path}: {n}")
```

Run: `nix develop --command python3 /tmp/claude-1000/-tb-Source-DilettanteTrading-polyoxide--loom-worktrees-aidanb-asset-event-adr-18dfe2e2345eed97/d94b65a8-93eb-4f6b-b37c-03b84d141657/scratchpad/add_class.py`
Expected: 12 lines, each ending in `: 1` or `: 2` as listed, and no exit message.

Then confirm that every `KeySpace` impl has the line:

Run: `grep -rn --include='*.rs' 'impl KeySpace for' polyoxide* | wc -l; grep -rn --include='*.rs' 'const CLASS: ProductClass' polyoxide* | wc -l`
Expected: `18` twice: the 15 impls the script edited, `Upper` and `Numbered` in `key.rs`, and the `MarketKey` doc example.

- [ ] **Step 7: Build and test the workspace**

Run: `nix develop --command bash -c 'cargo fmt --all && CARGO_INCREMENTAL=0 cargo test --workspace --all-features --no-run'`
Expected: every target compiles. `cargo fmt` re-wraps any `use` line the import pushed past the width.

Run: `nix develop --command cargo test -p polyoxide-venue --all-features`
Expected: PASS, the `MarketKey` doctest included.

- [ ] **Step 8: Commit**

```bash
git add polyoxide-venue/src/key.rs polyoxide-venue/src/lib.rs polyoxide-venue/src/market.rs \
  polyoxide-venue/src/records.rs polyoxide-venue/tests/trading.rs polyoxide-venue/tests/candles_intervals.rs \
  polyoxide-polymarket/src/shared/keys.rs polyoxide-binance/src/usdm/key.rs polyoxide-kalshi/src/predictions/key.rs \
  polyoxide-polymarket/tests/venue_perps.rs polyoxide-polymarket/tests/trading_clob.rs \
  polyoxide-polymarket/tests/venue_clob.rs polyoxide-binance/tests/venue_usdm.rs \
  polyoxide-kalshi/tests/venue_predictions.rs
git commit -m "feat(venue): every product declares its ProductClass, and every key carries it (ADR 0001)"
```

---

### Task 2: Each crate registers its products' classes

**Files:**
- Modify: `scripts/gen_registry.py`, `.github/scripts/tests/test_gen_registry.py`
- Modify: `polyoxide-test-support/src/registration.rs`
- Modify: `polyoxide-polymarket/Cargo.toml`, `polyoxide-binance/Cargo.toml`, `polyoxide-kalshi/Cargo.toml`
- Modify: the registration tests in `polyoxide-polymarket/src/shared/keys.rs`, `polyoxide-binance/src/usdm/key.rs`, `polyoxide-kalshi/src/predictions/key.rs`

- [ ] **Step 1: Write the failing generator tests**

In `.github/scripts/tests/test_gen_registry.py`, add after `test_bad_identifiers_fail`:

```python
def test_a_crate_registers_a_class_per_product(tmp_path: Path) -> None:
    def classes(metadata, workspace, manifests):
        manifests["polyoxide-alpha"]["package"]["metadata"]["polyoxide"]["classes"] = {
            "a": "event-contract"}

    registry = build(tmp_path, classes)
    assert registry.crate("polyoxide-alpha").classes == (("a", "event-contract"),)
    assert registry.crate("polyoxide-omega").classes == ()


@pytest.mark.parametrize(
    ("value", "message"),
    [({"z": "event-contract"}, "names 'z', which is not one of the crate's products"),
     ({"a": "Event Contract"}, "is not an id"),
     (["event-contract"], "must be a table of product = class")],
)
def test_a_bad_class_fails(tmp_path: Path, value, message: str) -> None:
    def bad(metadata, workspace, manifests):
        manifests["polyoxide-alpha"]["package"]["metadata"]["polyoxide"]["classes"] = value

    with pytest.raises(RegistryError, match=message):
        build(tmp_path, bad)
```

- [ ] **Step 2: Run them to verify they fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_gen_registry.py -k class -v'`
Expected: FAIL. The first fails with `unknown key classes`; the parametrized cases fail on the same message, which does not match theirs.

- [ ] **Step 3: Teach the generator `classes`**

In `scripts/gen_registry.py`:

In the module docstring's crate table, after the `products = ["clob"]` line, add:

```
    classes = { clob = "event-contract" }
                              # with `products`: the class of each product that has
                              # a KeySpace, an id; the crate's registration test
                              # holds each KeySpace::CLASS to it
```

Add `"classes"` to `CRATE_KEYS`:

```python
CRATE_KEYS = {"readme", "venue", "products", "classes", "mirrors", "notes", "live",
              "identifiers", "gate_exceptions"}
```

In `class Crate`, add the field last, after `gate_exceptions`, so no positional construction shifts:

```python
    # (product, class) pairs, sorted by product.
    classes: tuple[tuple[str, str], ...] = ()
```

Add this function after `_id`:

```python
def _classes(where: str, value, products: tuple[str, ...]) -> tuple[tuple[str, str], ...]:
    if value is None:
        return ()
    if not isinstance(value, dict):
        raise RegistryError(f"{where} must be a table of product = class, not {value!r}")
    for product, class_ in value.items():
        if product not in products:
            raise RegistryError(f"{where} names {product!r}, which is not one of the crate's "
                                f"products ({', '.join(products) or 'none'})")
        _id(f"{where} {product}", class_)
    return tuple(sorted(value.items()))
```

In `_crate`, after the `for product in products:` loop, add:

```python
    classes = _classes(f"{where} classes", meta.get("classes"), products)
```

and pass `classes=classes,` to `Crate(...)` after `gate_exceptions=exceptions,`.

- [ ] **Step 4: Run the generator tests to verify they pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_gen_registry.py -v'`
Expected: PASS, the whole file.

- [ ] **Step 5: Write the failing test-support tests**

Append at the end of `mod tests` in `polyoxide-test-support/src/registration.rs`, after `a_missing_manifest_names_its_path`:

```rust
    #[test]
    fn classes_are_read_per_product() {
        let declared = Registration::parse(
            "[package.metadata.polyoxide]\nproducts = [\"spot\"]\n\
             classes = { spot = \"perpetual-future\" }\n",
        )
        .unwrap();
        assert_eq!(
            declared.classes.get("spot").map(String::as_str),
            Some("perpetual-future")
        );
        let none = Registration::parse("[package.metadata.polyoxide]\nproducts = [\"spot\"]\n");
        assert!(none.unwrap().classes.is_empty());
    }

    #[test]
    fn classes_of_the_wrong_shape_are_refused() {
        for (manifest, fault) in [
            (
                "[package.metadata.polyoxide]\nclasses = \"spot\"\n",
                "`classes` is not a table",
            ),
            (
                "[package.metadata.polyoxide]\nclasses = { spot = 1 }\n",
                "the class of \"spot\" is not a string",
            ),
        ] {
            let err = Registration::parse(manifest).unwrap_err();
            assert!(err.contains(fault), "{manifest:?}: {err}");
        }
    }
```

Run: `nix develop --command cargo test -p polyoxide-test-support --lib registration`
Expected: a compile error, `no field classes on type Registration`.

- [ ] **Step 6: Read `classes` in `Registration`**

In `polyoxide-test-support/src/registration.rs`:

Change the imports to:

```rust
use std::collections::BTreeMap;
use std::path::Path;
```

Add the field after `products`:

```rust
    /// `classes`: each product's class, as written, keyed by product; empty
    /// when the crate declares none.
    pub classes: BTreeMap<String, String>,
```

In `parse`, before `Ok(Self { venue, products })`, add:

```rust
        let classes = match table.get("classes") {
            None => BTreeMap::new(),
            Some(toml::Value::Table(classes)) => classes
                .iter()
                .map(|(product, class)| {
                    class
                        .as_str()
                        .map(|class| (product.clone(), class.to_owned()))
                        .ok_or_else(|| format!("the class of {product:?} is not a string: {class}"))
                })
                .collect::<Result<_, _>>()?,
            Some(other) => return Err(format!("`classes` is not a table: {other}")),
        };
```

and change the return to `Ok(Self { venue, products, classes })`.

Change the `# Errors` text of `parse` to: "The manifest is not TOML, or its `venue` is not a string, its `products` not an array of strings or its `classes` not a table of strings."

In the struct's doc example, add `classes = { spot = "perpetual-future" }` after the `products` line of the manifest, and after `assert_eq!(declared.products, ["spot", "events"]);` add:

```rust
/// assert_eq!(declared.classes["spot"], "perpetual-future");
```

Run: `nix develop --command cargo test -p polyoxide-test-support --lib registration`
Expected: PASS.

Run: `nix develop --command cargo test -p polyoxide-test-support --doc`
Expected: PASS.

- [ ] **Step 7: Make the crates' registration tests ask for the class**

In `polyoxide-binance/src/usdm/key.rs`, at the end of `the_ids_are_the_ones_the_crate_registers`, after the `products` assertion, add:

```rust
        assert_eq!(
            declared.classes.get(PRODUCT.as_str()).map(String::as_str),
            Some(UsdmKeys::CLASS.as_str()),
            "{declared:?}"
        );
```

In `polyoxide-kalshi/src/predictions/key.rs`, in the same place:

```rust
        assert_eq!(
            declared.classes.get(PRODUCT.as_str()).map(String::as_str),
            Some(PredictionsKeys::CLASS.as_str()),
            "{declared:?}"
        );
```

In `polyoxide-polymarket/src/shared/keys.rs`, at the end of the same test, after the `PERPS` assertion:

```rust
        #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
        assert_eq!(
            declared.classes.get(CLOB.as_str()).map(String::as_str),
            Some(ClobKeys::CLASS.as_str()),
            "{declared:?}"
        );
        #[cfg(feature = "perps")]
        assert_eq!(
            declared.classes.get(PERPS.as_str()).map(String::as_str),
            Some(PerpsKeys::CLASS.as_str()),
            "{declared:?}"
        );
```

Run: `nix develop --command bash -c 'cargo test -p polyoxide-binance --all-features --lib the_ids_are && cargo test -p polyoxide-kalshi --all-features --lib the_ids_are && cargo test -p polyoxide-polymarket --all-features --lib the_ids_are'`
Expected: each FAILS, with `left: None` and `right: Some("perpetual-future")` (or `"event-contract"`).

- [ ] **Step 8: Register the classes**

In `polyoxide-polymarket/Cargo.toml`'s `[package.metadata.polyoxide]`, after `products = [...]`:

```toml
classes = { clob = "event-contract", perps = "perpetual-future" }
```

In `polyoxide-binance/Cargo.toml`, after `products = ["usdm"]`:

```toml
classes = { usdm = "perpetual-future" }
```

In `polyoxide-kalshi/Cargo.toml`, after `products = ["predictions"]`:

```toml
classes = { predictions = "event-contract" }
```

Run the three commands of Step 7 again.
Expected: PASS.

Run: `nix develop --command bash -c 'python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py'`
Expected: both exit 0. The generated regions do not show `classes`, so nothing is regenerated.

- [ ] **Step 9: Prove the registration test bites**

Temporarily change `classes = { usdm = "perpetual-future" }` to `classes = { usdm = "event-contract" }`, and run:

`nix develop --command cargo test -p polyoxide-binance --all-features --lib the_ids_are`
Expected: FAIL, `left: Some("event-contract")`, `right: Some("perpetual-future")`. Restore the line, run it again, and expect PASS.

- [ ] **Step 10: Commit**

```bash
git add scripts/gen_registry.py .github/scripts/tests/test_gen_registry.py \
  polyoxide-test-support/src/registration.rs \
  polyoxide-polymarket/Cargo.toml polyoxide-binance/Cargo.toml polyoxide-kalshi/Cargo.toml \
  polyoxide-polymarket/src/shared/keys.rs polyoxide-binance/src/usdm/key.rs \
  polyoxide-kalshi/src/predictions/key.rs
git commit -m "feat(registry): each crate registers its products' classes, held to KeySpace::CLASS (ADR 0001)"
```

---

### Task 3: The class bundles and their pins

**Files:**
- Modify: `polyoxide-venue/src/market.rs`, `polyoxide-venue/src/lib.rs`
- Modify: `polyoxide-polymarket/src/lib.rs`, `polyoxide-binance/src/lib.rs`, `polyoxide-kalshi/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append at the end of `mod tests` in `polyoxide-venue/src/market.rs`:

```rust
    #[test]
    fn a_product_with_a_bundles_members_is_held_by_its_one_bound() {
        fn perpetual<M: PerpetualMarket>(market: &M, key: &MarketKey) -> (Quote, FundingRate) {
            (
                ready(market.quote(key)).unwrap(),
                ready(market.funding_rate(key)).unwrap(),
            )
        }
        fn event_contract<M: EventContractMarket>(
            market: &M,
            key: &MarketKey,
        ) -> (Quote, EventGroup) {
            (
                ready(market.quote(key)).unwrap(),
                ready(market.event_group(key)).unwrap(),
            )
        }

        let (quote, rate) = perpetual(&Acme, &abc());
        assert_eq!(quote.bid, Some(Level::new(Decimal::ONE, Decimal::TWO)));
        assert_eq!(rate.rate, Decimal::ONE);
        let (quote, group) = event_contract(&Acme, &abc());
        assert_eq!(quote.key, abc());
        assert_eq!(group.outcomes.len(), 1);
    }
```

Run: `nix develop --command cargo test -p polyoxide-venue --features market --lib market::tests`
Expected: a compile error, `cannot find trait PerpetualMarket in this scope`.

- [ ] **Step 2: Add the bundles**

In `polyoxide-venue/src/market.rs`, after the `EventGroups` trait and before `#[cfg(test)]`, add:

```rust
/// What every perpetual-future product offers: [`MarketData`] and
/// [`Funding`], so a function generic over perpetuals needs one bound.
///
/// A class bundle names only what defines its class. A capability that
/// products of other classes share too, such as [`Candles`], stays out, so a
/// later product of the class that lacks it still has the bundle. The bundle
/// is implemented for every type that implements its members, and it has no
/// `Dyn` wrapper: code that holds trait objects holds one per member.
///
/// ```
/// use polyoxide_venue::{ClassifiedError, FundingRate, MarketKey, PerpetualMarket, Quote};
///
/// /// The quote and the funding rate of one perpetual, on any venue.
/// async fn quote_and_rate<M: PerpetualMarket>(
///     market: &M,
///     key: &MarketKey,
/// ) -> Result<(Quote, FundingRate), ClassifiedError> {
///     Ok((market.quote(key).await?, market.funding_rate(key).await?))
/// }
/// ```
pub trait PerpetualMarket: MarketData + Funding {}

impl<T: MarketData + Funding> PerpetualMarket for T {}

/// What every event-contract product offers: [`MarketData`] and
/// [`EventGroups`], so a function generic over event contracts needs one
/// bound.
///
/// Like [`PerpetualMarket`], it names only what defines its class, is
/// implemented for every type that implements its members, and has no `Dyn`
/// wrapper.
pub trait EventContractMarket: MarketData + EventGroups {}

impl<T: MarketData + EventGroups> EventContractMarket for T {}
```

In the module docs at the top of `market.rs`, after the paragraph that ends `that cannot do something does not implement its trait.`, add:

```rust
//!
//! A class bundle names what every product of a [`ProductClass`](crate::ProductClass)
//! offers: [`PerpetualMarket`] is [`MarketData`] and [`Funding`], and
//! [`EventContractMarket`] is [`MarketData`] and [`EventGroups`]. Each is
//! implemented for every type that implements its members.
```

In `polyoxide-venue/src/lib.rs`, change the market re-export to:

```rust
#[cfg(feature = "market")]
pub use market::{
    Candles, DynCandles, DynEventGroups, DynFunding, DynInstruments, DynMarketData, DynTrades,
    EventContractMarket, EventGroups, Funding, Instruments, MarketData, PerpetualMarket, Trades,
};
```

Run: `nix develop --command cargo test -p polyoxide-venue --all-features`
Expected: PASS, the new doctest and unit test included.

- [ ] **Step 3: Pin each product to its bundle**

In `polyoxide-polymarket/src/lib.rs`, after the last `Classify` block (`#[cfg(feature = "sports")]`), add:

```rust

// Every product implements its class's bundle; one that drops a defining
// capability fails the build here.
#[cfg(feature = "perps")]
const _: fn() = || {
    fn bundle<T: polyoxide_venue::PerpetualMarket>() {}
    bundle::<perps::Perps>();
};

#[cfg(all(feature = "clob", feature = "gamma"))]
const _: fn() = || {
    fn bundle<T: polyoxide_venue::EventContractMarket>() {}
    bundle::<clob::Clob>();
};
```

In `polyoxide-binance/src/lib.rs`, after the `Classify` block:

```rust

// Every product implements its class's bundle; one that drops a defining
// capability fails the build here.
#[cfg(feature = "usdm")]
const _: fn() = || {
    fn bundle<T: polyoxide_venue::PerpetualMarket>() {}
    bundle::<usdm::Usdm>();
};
```

In `polyoxide-kalshi/src/lib.rs`, after the `Classify` block:

```rust

// Every product implements its class's bundle; one that drops a defining
// capability fails the build here.
#[cfg(feature = "predictions")]
const _: fn() = || {
    fn bundle<T: polyoxide_venue::EventContractMarket>() {}
    bundle::<predictions::Predictions>();
};
```

Run: `nix develop --command bash -c 'cargo check -p polyoxide-polymarket --all-features && cargo check -p polyoxide-polymarket --no-default-features --features perps && cargo check -p polyoxide-polymarket --no-default-features --features clob,gamma && cargo check -p polyoxide-binance --all-features && cargo check -p polyoxide-kalshi --all-features'`
Expected: all five succeed.

- [ ] **Step 4: Prove a pin bites**

Temporarily change Binance's pin to `fn bundle<T: polyoxide_venue::EventContractMarket>() {}`, and run:

`nix develop --command cargo check -p polyoxide-binance --all-features`
Expected: FAIL with E0277, `Usdm` not satisfying `EventContractMarket`, with rustc's note naming the missing `EventGroups`. Restore the line, run it again, and expect success.

- [ ] **Step 5: Check the classify coverage script still reads the lib files**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -v'`
Expected: PASS. The new blocks call `bundle`, not `is`, so the `Classify` listing is unchanged.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-venue/src/market.rs polyoxide-venue/src/lib.rs \
  polyoxide-polymarket/src/lib.rs polyoxide-binance/src/lib.rs polyoxide-kalshi/src/lib.rs
git commit -m "feat(venue): PerpetualMarket and EventContractMarket, with each product pinned to its bundle (ADR 0001)"
```

---

### Task 4: The umbrella checks every product's class

**Files:**
- Modify: `polyoxide/tests/parse_key.rs`

This test confirms Tasks 1 and 2 end to end, so it passes when written. Step 2 shows it can fail.

- [ ] **Step 1: Write the test**

Append to `polyoxide/tests/parse_key.rs`:

```rust
#[test]
fn each_key_carries_the_class_its_product_declares() {
    use polyoxide::binance::usdm::key::UsdmKeys;
    use polyoxide::kalshi::predictions::key::PredictionsKeys;
    use polyoxide::polymarket::shared::keys::{ClobKeys, PerpsKeys};
    use polyoxide::venue::{KeySpace, ProductClass};

    let token = "71321045679252212594626385532706912750332728571942532289631379312455583992563";
    let clob = format!("polymarket.clob:{token}");
    for (text, declared, class) in [
        (clob.as_str(), ClobKeys::CLASS, ProductClass::EventContract),
        ("polymarket.perps:7", PerpsKeys::CLASS, ProductClass::PerpetualFuture),
        ("binance.usdm:BTCUSDT", UsdmKeys::CLASS, ProductClass::PerpetualFuture),
        (
            "kalshi.predictions:KXTEST-26OCT18-T50.5:yes",
            PredictionsKeys::CLASS,
            ProductClass::EventContract,
        ),
    ] {
        let key = parse(text).unwrap();
        assert_eq!(key.class(), declared, "{text}");
        assert_eq!(key.class(), class, "{text}");
    }
}
```

Run: `nix develop --command cargo test -p polyoxide --features full --test parse_key`
Expected: PASS, 6 tests.

- [ ] **Step 2: Prove it bites**

Temporarily change `ProductClass::EventContract` on the `clob` row to `ProductClass::PerpetualFuture`, run the same command, and expect FAIL naming `polymarket.clob:7132…`. Restore it and expect PASS.

- [ ] **Step 3: Commit**

```bash
git add polyoxide/tests/parse_key.rs
git commit -m "test(polyoxide): every product the umbrella builds keys its own class (ADR 0001)"
```

---

### Task 5: Docs, the spine amendment and the ADR's status

**Files:**
- Modify: `polyoxide-venue/src/lib.rs` (crate docs), `polyoxide-venue/README.md`, `polyoxide-venue/Cargo.toml` (metadata `readme`)
- Modify: `CLAUDE.md`, and `README.md` through the generator
- Modify: `docs/s3-migration.md`
- Create: `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/adr-0001.md`
- Modify: `docs/decisions/0001-product-classes-over-one-capability-core.md`

- [ ] **Step 1: The venue crate's own docs**

In `polyoxide-venue/src/lib.rs`'s crate docs, replace the `keys` bullet with:

```rust
//! - With the `keys` feature, `MarketKey` names one instrument as
//!   `venue.product:id`. Only a product's `KeySpace` makes one, so two keys
//!   are equal exactly when they name the same instrument; `RawKey` is text
//!   split into its parts and checked by no product yet. Each `KeySpace`
//!   declares its product's `ProductClass`, which every key it makes carries.
```

In the `market` bullet, after `` `Candles`, `Funding` and `EventGroups`, each with a `Dyn` wrapper. ``, add the sentence `` The class bundles `PerpetualMarket` and `EventContractMarket` name what every product of a class offers. `` and re-wrap the bullet to the file's width.

In `polyoxide-venue/README.md`, after the sentence that ends `canonical because only a product's \`KeySpace\` makes one.`, add (as prose, no code fence): "Each product declares its `ProductClass`, event contract or perpetual future, and every key it makes carries it." After the sentence that lists `Candles`, `Funding` and `EventGroups`, add: "The class bundles `PerpetualMarket` and `EventContractMarket` name what every product of a class offers, so a function generic over one class needs one bound."

In `polyoxide-venue/Cargo.toml`'s `[package.metadata.polyoxide] readme`, replace `market keys (feature \`keys\`: \`MarketKey\`, \`RawKey\` and the \`KeySpace\` a venue implements per product)` with `market keys (feature \`keys\`: \`MarketKey\`, \`RawKey\`, the \`KeySpace\` a venue implements per product and the \`ProductClass\` it declares)`, and `the market-data traits with the records they return` with `the market-data traits and their class bundles, with the records they return`.

Run: `nix develop --command bash -c 'python3 scripts/gen_registry.py --write && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && cargo test -p polyoxide-venue --all-features --doc'`
Expected: `--write` rewrites the crate-list regions of `CLAUDE.md` and `README.md`, `--check` exits 0, the venue gate exits 0, and the doctests (README included) pass.

- [ ] **Step 2: CLAUDE.md's standing rules (AD-21)**

In `CLAUDE.md`'s **Market keys** paragraph, replace its last sentence ("Each venue crate's `the_ids_are_the_ones_the_crate_registers` test holds its `VENUE` and product constants to its `[package.metadata.polyoxide]`, read with `polyoxide_test_support::registration!()`.") with:

```markdown
Each `KeySpace` also declares its product's `ProductClass` as `CLASS` (ADR 0001):
`EventContract` (the clob and Kalshi's `predictions`) or `PerpetualFuture` (perps and USDⓈ-M),
`#[non_exhaustive]`, with a variant added only when a product of that class lands. Every key
carries it as `MarketKey::class()`. It follows from the venue and product, so the text form,
`Eq` and `Hash` are unchanged. Each venue crate's `the_ids_are_the_ones_the_crate_registers`
test holds its `VENUE`, product and class constants to its `[package.metadata.polyoxide]`, read
with `polyoxide_test_support::registration!()`: `products`, and `classes`, a table of
product = class for each product with a `KeySpace`, which `gen_registry.py` checks names only
the crate's products.
```

Insert a new paragraph after the **Market-data traits** paragraph, before **The trading trait**:

```markdown
**Class bundles** — the `market` feature also holds one bundle per `ProductClass` (ADR 0001):
`PerpetualMarket` is `MarketData + Funding`, and `EventContractMarket` is
`MarketData + EventGroups`. Each has a blanket impl and no `Dyn` wrapper, so a class-generic
function needs one bound, and code that holds trait objects holds one per member. A bundle holds
only what defines its class, and a capability other classes share, such as `Candles`, stays out,
so a bundle never has to lose a member. Each venue crate's `lib.rs` pins its products to their
bundles in a `const _` beside its `Classify` assertions: `Perps` (`perps`) and `Usdm` (`usdm`) to
`PerpetualMarket`, and `Clob` (`all(clob, gamma)`) and `Predictions` (`predictions`) to
`EventContractMarket`. `polyoxide/tests/parse_key.rs` checks every product's class. A class fact
joins a shared record, a capability trait or a bundle only when two products of the class supply
it; until then it rides in the product's `<Record>Ext`.
```

- [ ] **Step 3: The consumer-impact note**

Append to `docs/s3-migration.md`:

```markdown
## ADR 0001: product classes

**Consumer impact: one line for an outside `KeySpace`.** `KeySpace` gains a required
`const CLASS: ProductClass`, so a crate outside the workspace that implements it adds that line.
Code that only uses keys and the market-data traits builds unchanged. This is a major-level change
for such an implementer, and it ships in a 0.x minor release.

- `polyoxide-venue`, feature `keys`: `ProductClass` (`EventContract`, `PerpetualFuture`;
  `#[non_exhaustive]`; `as_str` and `Display` spell it as a registration does), the required
  `KeySpace::CLASS`, and `MarketKey::class()`. The class follows from the venue and product, so a
  key's text form, equality and hash are unchanged.
- `polyoxide-venue`, feature `market`: the class bundles `PerpetualMarket` (`MarketData + Funding`)
  and `EventContractMarket` (`MarketData + EventGroups`), each implemented for every type that
  implements its members. Polymarket's `Perps` and Binance's `Usdm` are perpetual markets;
  Polymarket's `Clob` (with `gamma`) and Kalshi's `Predictions` are event-contract markets.
```

- [ ] **Step 4: The spine amendment**

Create `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/adr-0001.md`:

```markdown
# Proposed spine amendments: ADR 0001

Recorded per AD-21 from `docs/decisions/0001-product-classes-over-one-capability-core.md`. One session later merges this file into the spine, after Epic 8's and Epic 9's amendments, and regenerates the guide.

## ADR1-1: a product declares its class (amends AD-4 and AD-13)

- **Proposed by:** ADR 0001, accepted by the owner, 2026-10-21.
- **Current rule:** AD-4 makes a key `venue.product:id`, made only by its product's `KeySpace`, which declares `VENUE` and `PRODUCT`. AD-13 registers a crate's venue and products in `[package.metadata.polyoxide]`. Nothing says what kind of market a product is.
- **Proposed rule:**
  - `polyoxide-venue` (feature `keys`) holds `ProductClass`, `#[non_exhaustive]`, with `EventContract` and `PerpetualFuture`. A variant is added only when a product of that class lands.
  - Each `KeySpace` declares `const CLASS: ProductClass`. Every key it makes carries the class, and `MarketKey::class()` reads it. The class follows from the venue and product, so a key's text form, `Eq` and `Hash` are unchanged.
  - A crate registers the class of each product with a `KeySpace` in `classes = { <product> = "<class>" }`. `gen_registry.py` checks that the table names only the crate's products, and each venue crate's `the_ids_are_the_ones_the_crate_registers` test holds every `CLASS` to it.
- **Why:**
  - **Venues straddle classes.** Polymarket and Kalshi each list event contracts and perpetual futures, so the class belongs to the product, not the venue.
  - **A consumer reads a key's class without a request.** `SizeUnit` is one `instrument` request away and does not separate a perpetual from a dated future.
- **Follow-up in the code:** Tasks 1 and 2 of `docs/plans/2026-10-21-adr-0001-product-classes.md`. The merge session edits AD-4 and AD-13, and the guide's text.

## ADR1-2: class bundles and the promotion rule (amends AD-6, as A6-1 and A9-2 leave it)

- **Proposed by:** ADR 0001, accepted by the owner, 2026-10-21.
- **Current rule:** AD-6 gives every product `MarketData` and makes each optional ability a capability trait, implemented only where offered. Nothing names what every product of a class offers, and nothing says when a fact one product supplies joins the shared core.
- **Proposed rule:**
  - What only one class has stays a capability trait that only products of that class implement. No trait tree is forked.
  - A class bundle names `MarketData` plus the capabilities that every product of the class offers and that mean nothing outside it: `PerpetualMarket: MarketData + Funding` and `EventContractMarket: MarketData + EventGroups`. Each has a blanket impl and no `Dyn` wrapper. A capability that other classes share too (`Instruments`, `Trades`, `Candles`) stays out, so a bundle never has to lose a member.
  - Each venue crate pins its products to their bundles in a `const _`, so a product that drops a defining capability fails the build.
  - A class fact joins a shared record, a capability trait or a bundle only when two products of the class supply it. Until then it rides in the product's `<Record>Ext` (AD-19). A new field on a shared record, or a new member of a bundle, cites the two products in review.
- **Why:**
  - **One generic function still serves every product** (AD-6's own reason), and a class-generic one needs one bound instead of a list each consumer writes and lets drift.
  - **Both bundles have two implementers.** Polymarket perps and Binance USDⓈ-M agree on `Funding`, and Polymarket's CLOB and Kalshi's `predictions` agree on `EventGroups`.
  - **A9-2 shows why shared capabilities stay out.** Products that offer `Candles` serve different widths, so even a shared capability is not uniform within a class.
- **Follow-up in the code:** Task 3 of `docs/plans/2026-10-21-adr-0001-product-classes.md`. The merge session edits AD-6 and the guide's text.
```

- [ ] **Step 5: Mark the ADR accepted**

In `docs/decisions/0001-product-classes-over-one-capability-core.md`, change the front matter's `status: proposed` to `status: accepted`. In **More Information**, replace the paragraph's opening "**Relation to the spine.** If accepted, this ADR becomes an amendment to" with "**Relation to the spine.** This ADR is recorded as `spine-amendments/adr-0001.md` (ADR1-1 and ADR1-2), which amends", and leave the rest of the paragraph.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-venue/src/lib.rs polyoxide-venue/README.md polyoxide-venue/Cargo.toml \
  CLAUDE.md README.md docs/s3-migration.md \
  _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/adr-0001.md \
  docs/decisions/0001-product-classes-over-one-capability-core.md
git commit -m "docs: product classes in the standing rules, the S3 notes and the spine amendments; ADR 0001 accepted"
```

Run `git status --short` first. Any other file that `gen_registry.py --write` changed in Step 1 is committed with this task, and named in the commit message.

---

### Task 6: The final gates

- [ ] **Step 1: Run the story gates on the workspace**

Run each through `nix develop --command`:

```bash
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo clippy --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo nextest run --all-features --workspace
CARGO_INCREMENTAL=0 cargo test --doc --all-features --workspace
bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace'
python3 scripts/gen_registry.py --check
python3 scripts/venue_gate.py
python3 scripts/live_unwraps.py
python3 scripts/test_body_diff.py --baseline v0.40.0
```

Expected: every one passes. `test_body_diff.py` reports no changed item, since every edited test postdates `v0.40.0`.

- [ ] **Step 2: Run the CI jobs that a feature or a script could break**

```bash
bash -c 'msrv-cargo check --workspace --all-features && msrv-cargo doc --no-deps --workspace --all-features'
cargo hack check --workspace --each-feature --no-dev-deps --ignore-private
bash -c 'cd .github/scripts && uv sync && uv run pytest tests/ -v'
bash -c 'python3 scripts/publish_order.py check-manifests && cargo publish --workspace --dry-run --no-verify --locked'
python3 scripts/api_removals.py check --baseline v0.40.0
```

Expected: every one passes. `api_removals.py` prints `trait_associated_const_added` (or the version of cargo-semver-checks' name for it) for `polyoxide-venue` as an allowed lint, not a removal. It takes about 8.5 minutes cold; delete `target/semver-checks` afterwards (memory: local build OOM).

- [ ] **Step 3: Report**

List each gate with PASS or FAIL. Note for the owner that the release's semver job will see the new required `KeySpace::CLASS` as a major-level change, which the pending 0.42.0 minor bump covers.
