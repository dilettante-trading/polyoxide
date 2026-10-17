# Epic 6: Venue-neutral Market Data Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Under the supervised option, `/flow-supervise` runs it story by story.

**Goal:** One generic function reads quotes and books from the Polymarket CLOB, Polymarket perps and Binance USDⓈ-M, through canonical market keys, shared records that keep each venue's own facts in typed extensions, and one set of market-data traits (Epic 6, Stories 6.1 to 6.6).

**Architecture:** `polyoxide-venue` gains two features: `keys` (`MarketKey`, `RawKey` and the `KeySpace` each venue product implements) and `market` (the records, `Extensions`, and the traits `MarketData`, `Instruments`, `Trades`, `Candles`, `Funding` and `EventGroups`, each with a dynosaur `Dyn` wrapper). Each venue crate implements `KeySpace` once per product and the traits in `src/<module>/venue.rs` over its native client, returning `ClassifiedError`. The umbrella adds `polyoxide::parse_key`, carries the traits on `polyoxide::venue`, and ships an example whose one `best_bid_ask` and one error classifier a mock suite and a live suite run against all three modules.

**Tech Stack:** Rust (MSRV 1.91, dev shell 1.99.0), dynosaur 0.3.1, rust_decimal, serde, tokio, mockito; `toml` 0.8 (new, `polyoxide-test-support` only).

**Spec:** `_bmad-output/planning-artifacts/epics.md` (Epic 6, from line 1394), read with the architecture spine, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/ARCHITECTURE-SPINE.md` (AD-3 to AD-7, AD-19), and the decisions below, which bind where the epic leaves a choice open or the user changed it.

Every code block below was built, formatted and tested in a scratch copy of this tree on 2026-10-16 (the `aidanb/init` worktree at `e748853`): the per-story gates passed, as did the workspace's fmt, clippy, rustdoc with `-D warnings`, the MSRV check, `cargo hack --each-feature`, nextest (2,570 tests), the doctests, the Python bindings (344 tests), `.github/scripts`' pytest suite (1,353 tests), the package dry run and `test_body_diff.py --baseline v0.39.0`, and the live suite against the three hosts. The plan was then replayed onto a fresh copy of that tree and reproduced the prototype file for file.

---

## Before this plan runs

Epic 6 is stage S3: it adds public API and removes none. The workspace is still in S2 (version 0.39.0, `stage = "S2"`, no `v0.40.0` tag), so these land on `main` first, in order:

1. **The S2 integration PR merges.** PR #55 (`aidanb/restructure-3`) reaches `main` with CI green.
2. **0.40.0 is released.** The version-bump commit on `main` (AD-25: never on a loom or integration branch) publishes 0.40.0 and tags `v0.40.0`. Before pushing it, confirm the crates.io token can publish the new crate name `polyoxide-polymarket` (deferred-work entry from Stories 5.11 and 5.12).
3. **The S3 switch commit lands on `main`.** It deletes `tombstones/`, sets `[workspace.metadata.polyoxide] stage = "S3"`, makes the removals job `python3 scripts/api_removals.py check --baseline v0.40.0` with its pin test, updates `test_the_real_workspace_is_in_stage_s2` and the `S2_BASELINE` pins in `.github/scripts/tests/test_ci_workflow.py`, decides whether the test-bodies job retires or rebases onto `v0.40.0`, rewrites CLAUDE.md's removals paragraph (and copies `ci.yml`'s guarded tombstone loop into CLAUDE.md's Build block), and runs `python3 scripts/gen_registry.py --write` (the deferred-work entry "The S3 switch commit must edit more than the spec's Decisions list").
4. **The dev shell reaches `main`.** `flake.nix` and CLAUDE.md's dev-shell paragraph (`f1b85b0` and `e748853`, on `aidanb/init` only) merge, since the flow profile's prefix is `nix develop --command`.
5. **The flow profile follows the switch.** In `~/.claude/flow/repos/-tb-Source-DilettanteTrading-polyoxide.toml`, replace the story gate `python3 scripts/test_body_diff.py --baseline v0.39.0` and the final `removals` and `test-bodies` gates with what `ci.yml` runs after the switch, and drop the note that says the profile's gates are S2's.
6. **The run's loom session branches from `main`** at or after those commits.

The spine amendments of Epics 2 to 5 need not be merged first. Network access is needed twice: Story 1 fetches `toml` 0.8 from crates.io, and Story 5 runs a capture script against `clob.polymarket.com`, `data-api.polymarket.com` and `gamma-api.polymarket.com`.

## Decisions (binding)

Each settles something the epic leaves open, or records the user's choice. A developer who finds the code below disagreeing with one of them stops and asks.

- **D1. `MarketData` reads one instrument by key; listing is a capability (the user, 2026-10-16).** `MarketData` is `instrument(&MarketKey)`, `quote` and `book`. Listing every instrument is the separate trait `Instruments`, which Binance USDⓈ-M (`exchangeInfo`) and perps (`/v1/info/instruments`) implement and the CLOB does not: on 2026-10-16 the CLOB's `/sampling-markets` alone ran past 12 pages of 1,000 open markets. Story 3 records this as amendment A6-1 in `spine-amendments/epic-6.md`, since AD-6 names `instruments` in `MarketData`.
- **D2. `Clob` reads public trades through an embedded Data client (the user, 2026-10-16).** The CLOB serves no public trades (only the signed `/data/trades`). With the `data` feature, `Clob` holds a `DataApi` as it holds a `Gamma` with `gamma` (`ClobBuilder::data`, built by default with the builder's timeout and pool size, failing as the new `ClobError::Data`), and `Trades for Clob` is gated `all(clob, data)`: it resolves the token's market with `/markets-by-token/{token}`, lists the Data API's taker-only `/trades` for that market, and keeps the token's rows. The unified `Polymarket` client hands clob its own Data client.
- **D3. Keys land before records.** `Instrument` carries a `MarketKey`, so Story 6.2 runs first (Story 1 here) and Story 6.1 second.
- **D4. Two features, no tree change.** `polyoxide-venue`'s `keys` adds serde; `market` adds `keys`, `decimal` and dynosaur. `clob`, `perps` and `usdm` turn on `market`, `gamma` and `data` turn on `keys`, and the socket-only modules turn on neither. Every module that turns one on already builds those crates, so no `ci/dep-allowlists/*.txt` file changes; if `test_dep_allowlists.py` fails, stop and ask.
- **D5. Only a product's `KeySpace` makes a `MarketKey`.** `KeySpace` has the consts `VENUE` and `PRODUCT`, a canonical `Id` type, `parse_id` (text to `Id`, the one place a product checks and canonicalizes) and `id_text`, and provides `key`, `parse` (from a `RawKey`) and `id` (back from a key). `MarketKey` has private fields and no public constructor, serializes as its text form, and has no `Deserialize`: text is read as a `RawKey` (which round-trips through serde) and handed to a product's constructors or to `polyoxide::parse_key`.
- **D6. `RawKey::parse` splits at the first `:`, then the part before it at its first `.`.** The venue and product must be ids (`^[a-z][a-z0-9-]*$`, as `gen_registry.py` requires of registered ids) and the id non-empty; the id is kept as written. Every `KeyError` is an `InvalidRequest`.
- **D7. Product ids.** Binance's id is `Symbol` (ASCII letters uppercased). The clob's is `TokenId`, 1 to 78 decimal digits with no leading zero, built by any of `clob`, `gamma` and `data` (a product id names the module that trades it, so gamma and data name the clob's outcomes). Perps' is `InstrumentId`, written without leading zeros (`polymarket.perps:007` is `polymarket.perps:7`).
- **D8. Records.** Every record is `#[non_exhaustive]` with a `new` taking its required fields, derives `Clone`, `Debug` and `PartialEq`, and carries `Extensions`. `Level` (price and size) is a value inside a record, not a record, and carries none. A size is in native quantity and a quote-currency amount is a separate `notional`. Bids are best (highest) first and asks best (lowest) first, and every list a trait returns is oldest first, one request's worth. Records have no serde, since `Extensions` has none, so `UnixMillis` gains no serde impl here (the S1 note that S3's records would add one does not hold).
- **D9. `Extensions`** holds at most one value per type in a `BTreeMap<TypeId, Box<dyn …>>`, compares by value, has no serde, and refuses a second value of a type with `DuplicateExtension` (`InvalidRequest`), keeping the first. `Extensions::of(value)` starts one with a single value.
- **D10. `NotListed { key }`** (`VenueRefusal`) is what a trait returns for an instrument the venue's own listing lacks; the CLOB's own `404` for an unknown token is its `ClobError`, also a `VenueRefusal`.
- **D11. Candles.** `CandleInterval` has the seven widths every candle venue here serves (1 minute to 1 week); `CandleQuery` is an interval, a `start` and an optional `end`. A venue that samples one price per interval (the CLOB's `/prices-history`) gives only `close`.
- **D12. No new routes (the spec's non-goal).** Binance's quote is a 5-level `depth` (weight 2) and its book an unlimited one (500 levels, weight 1), since the client has no `bookTicker`.
- **D13. Perps.** `tick_size` is `None` (a non-integer price keeps at most five significant figures; the decimals are in `InstrumentExt`), `size_increment` is `10^-quantity_decimals`, sizes are `Contracts`, and fee tiers come from `/v1/info/fees`' entry for the instrument's type and category, so `instrument` is two requests. A `bbo` side of zero quantity is no level, and an instrument with no `bbo` row has an empty quote, as its book is empty.
- **D14. The CLOB.** `instrument` is `/markets-by-token/{token}` then `/markets/{condition}`; `quote` and `book` read `/book`, whose sides arrive worst first and are sorted; `event_group` reads the event id from Gamma's `/markets?clob_token_ids=` (a private decode, since Gamma's `Market` does not model `events`) and the event from `/events/{id}`, keeping only `v1` markets' outcomes. A number the CLOB or Data API sends as a float is read through its shortest decimal spelling.
- **D15. Test support.** `Registration` and `registration!()` read a crate's `[package.metadata.polyoxide]` (with `toml` 0.8), for the test that holds each venue crate's id constants to it. `ClassifiedResultExt::or_fail` and `fail_classified` tag a `ClassifiedError`, which `ResultExt` cannot cover (deferred-work entry from Stories 2.3 and 2.4); both `or_fail`s may be in scope at once.
- **D16. The umbrella.** `polymarket-clob`, `polymarket-perps` and `binance-usdm` turn on `polyoxide-venue/market`, and `polymarket-gamma` and `polymarket-data` turn on `keys`. `examples/venue_neutral.rs` holds `best_bid_ask` and the classifier; `tests/venue_neutral.rs` (mock) and `tests/live_venue.rs` (live, nightly suite `venue`) include it by `#[path]`.
- **D17. The release note.** Story 6 writes Epic 6's consumer-impact note, "additive", into a new `docs/s3-migration.md`. The version-bump commit on `main` that first ships this work quotes it in `CHANGELOG.md`; the run makes no version bump.
- **D18. The guide is not edited.** `docs/ARCHITECTURE.md` is only regenerated from the spine. Stories 3 and 6 record amendments A6-1 and A6-2; merging them is an owner session after the run (below).

## Not in this plan

The other S3 items in `_bmad-output/implementation-artifacts/deferred-work.md` belong to other work: `Secret` on the public credential fields, moving Binance's funding bucket onto `WindowQuotaTable`, converting the hand-written enums to `wire_enum!`/`open_enum!`, removing `data.pnl()` and `data.rankings()`, pruning `ClobWsError::{ConnectionClosed, Authentication}`, and a clock beside `UnixMillis`. Each removal among them needs an entry in `docs/s3-removals.md`; this plan removes nothing. So do Epic 7's supervised clob sockets and `Trading`, and Epic 8's Kalshi skeleton, whose market data will implement these traits.

## Rules for every story

- **Put the code in place from this file by line range**, as the developers' habits describe: a new file is the block after its `Create` line, and each edit replaces the block after a `Current (...)` line with the block after the `Replace with:` line that follows it. Each `Current` block occurs exactly once in its file at that point; apply a task's edits top to bottom.
- **Add tests; never change an existing test's body.** If the test-bodies job still runs in S3, it fails on a changed test item. Story 3 adds items to `polyoxide-test-support/tests/tags.rs` beside the existing ones, and no story touches another existing test.
- **Generated regions are regenerated, never edited.** After a story changes a crate's `[package.metadata.polyoxide]` (Stories 1, 2, 3 and 6), run `python3 scripts/gen_registry.py --write`; `--check` must then exit 0. Only CLAUDE.md's hand-written paragraphs are edited, and each story that changes a rule CLAUDE.md states edits it in the same story (AD-21).
- **The foundation crates name no venue.** `python3 scripts/venue_gate.py` must report no hit outside `gate_exceptions`, so `polyoxide-venue` and `polyoxide-test-support` docs and tests use made-up venues (`acme`).
- **Every new public error type** is in its crate's `Classify` assertion in `lib.rs`.
- **Commit by pathspec**, with the story named in a conventional message, and include `Cargo.lock` when cargo changes it.
- **Run the commands with the dev shell's prefix** the supervisor passes (`nix develop --command`). A story's gates are the profile's; its **Verify** line is the developer's done-check.

## Story 1: Canonical market keys (Story 6.2)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-venue/src/key.rs`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/Cargo.toml`, `polyoxide-test-support/src/registration.rs`, `polyoxide-test-support/src/lib.rs`, `polyoxide-test-support/Cargo.toml`, `polyoxide-binance/src/usdm/key.rs`, `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-polymarket/src/shared/keys.rs`, `polyoxide-polymarket/src/shared/mod.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/Cargo.toml`, `polyoxide/tests/parse_key.rs`, `CLAUDE.md`, `README.md`, `Cargo.lock`
- **Interfaces:** exposes `polyoxide_venue::{MarketKey, RawKey, VenueId, ProductId, KeySpace, KeyError}` (feature `keys`); `polyoxide_test_support::{Registration, registration!}`; `polyoxide_binance::usdm::key` (the module, and its function `key` re-exported as `usdm::key`) with `UsdmKeys`, `VENUE`, `PRODUCT`, `parse`, `symbol`; `polyoxide_polymarket::shared::keys::{VENUE, CLOB, PERPS, TokenId, ClobKeys, PerpsKeys, clob, token_id, perps, instrument_id, parse}`; `polyoxide::parse_key`. Uses nothing from earlier stories.
- **Verify:** `cargo test -p polyoxide-venue -p polyoxide-test-support --all-features && cargo test -p polyoxide-binance --all-features --lib usdm::key && cargo test -p polyoxide-polymarket --all-features --lib shared::keys && cargo test -p polyoxide --all-features --test parse_key`

The keys are canonical by construction: a `MarketKey` has private fields, and only a product's `KeySpace` makes one, after its `parse_id` has checked and canonicalized the id (D5 to D7).

### Task 1.1: `MarketKey`, `RawKey` and `KeySpace` in `polyoxide-venue`

**Files:**
- Create: `polyoxide-venue/src/key.rs`
- Modify: `polyoxide-venue/src/lib.rs`, `polyoxide-venue/Cargo.toml`

- [ ] **Step 1: Create the module, with its tests.**

Create `polyoxide-venue/src/key.rs` with:

```rust
//! Market keys: one canonical name per instrument, whichever venue lists it.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::{Class, Classify};

/// Whether `id` is a venue or product id: an ASCII lowercase letter, then
/// lowercase letters, digits and `-`, the form a crate's registration declares.
const fn is_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut i = 1;
    while i < bytes.len() {
        let b = bytes[i];
        if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
            return false;
        }
        i += 1;
    }
    true
}

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident, $what:literal) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Cow<'static, str>);

        impl $name {
            #[doc = concat!("A ", $what, " id known when the program is built.")]
            ///
            /// # Panics
            ///
            /// Unless `id` is an ASCII lowercase letter followed by lowercase
            /// letters, digits and `-`. In a `const` the panic is a build error.
            pub const fn from_static(id: &'static str) -> Self {
                assert!(
                    is_id(id),
                    "an id is a lowercase letter, then lowercase letters, digits and `-`"
                );
                Self(Cow::Borrowed(id))
            }

            #[doc = concat!("The ", $what, " id.")]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.as_str()).finish()
            }
        }
    };
}

id_type! {
    /// A venue's id: the `venue` of `venue.product:id`.
    ///
    /// A venue crate declares its own with [`VenueId::from_static`], equal to
    /// the `venue` its registration declares.
    VenueId, "venue"
}

id_type! {
    /// A product's id: the `product` of `venue.product:id`, naming the module
    /// that trades the instrument.
    ///
    /// A venue crate declares each with [`ProductId::from_static`], equal to one
    /// of the `products` its registration declares.
    ProductId, "product"
}

/// One instrument's name, written `venue.product:id`, such as
/// `acme.spot:ABCUSD`.
///
/// A key has no public constructor from text or from parts. Only a product's
/// [`KeySpace`] makes one, and it canonicalizes the id first, so two keys are
/// equal exactly when they name the same instrument. On an event-contract
/// venue a key names one outcome, not a market.
///
/// It serializes as its text form. To read one back, parse a [`RawKey`] and
/// hand it to the product's constructors, which canonicalize it again.
///
/// ```
/// use std::borrow::Cow;
/// use polyoxide_venue::{KeySpace, MarketKey, ProductId, RawKey, VenueId};
///
/// /// A product whose ids are ASCII and uppercased.
/// struct Spot;
///
/// impl KeySpace for Spot {
///     const VENUE: VenueId = VenueId::from_static("acme");
///     const PRODUCT: ProductId = ProductId::from_static("spot");
///     type Id = String;
///
///     fn parse_id(text: &str) -> Result<String, String> {
///         if text.is_ascii() {
///             Ok(text.to_ascii_uppercase())
///         } else {
///             Err("an id is ASCII".into())
///         }
///     }
///
///     fn id_text(id: &String) -> Cow<'_, str> {
///         Cow::Borrowed(id)
///     }
/// }
///
/// let key = Spot::parse(&RawKey::parse("acme.spot:abcusd")?)?;
/// assert_eq!(key, Spot::key(&"ABCUSD".to_owned()));
/// assert_eq!(key.to_string(), "acme.spot:ABCUSD");
/// # Ok::<(), polyoxide_venue::KeyError>(())
/// ```
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarketKey {
    venue: VenueId,
    product: ProductId,
    id: Arc<str>,
}

impl MarketKey {
    /// The venue that lists the instrument.
    pub fn venue(&self) -> &VenueId {
        &self.venue
    }

    /// The product, the module that trades it.
    pub fn product(&self) -> &ProductId {
        &self.product
    }

    /// The id, in its product's canonical form. Opaque to everything but its
    /// product.
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for MarketKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}:{}", self.venue, self.product, self.id)
    }
}

impl fmt::Debug for MarketKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MarketKey")
            .field(&format_args!("{self}"))
            .finish()
    }
}

impl Serialize for MarketKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// A key's text split into venue, product and id, checked by no product yet.
///
/// [`RawKey::parse`] splits at the first `:`, then what comes before it at its
/// first `.`. The id is everything after the `:`, so it may hold `:` and `.`
/// of its own. The venue and product must be ids (an ASCII lowercase letter,
/// then lowercase letters, digits and `-`) and the id must not be empty; the
/// id is otherwise kept as written, since only its product knows its
/// canonical form.
///
/// It serializes as its text form and deserializes through
/// [`RawKey::parse`].
///
/// ```
/// use polyoxide_venue::RawKey;
///
/// let raw = RawKey::parse("acme.events:FED-25DEC:yes")?;
/// assert_eq!(raw.venue().as_str(), "acme");
/// assert_eq!(raw.product().as_str(), "events");
/// assert_eq!(raw.id(), "FED-25DEC:yes");
/// assert!(RawKey::parse("acme:spot").is_err());
/// # Ok::<(), polyoxide_venue::KeyError>(())
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RawKey {
    venue: VenueId,
    product: ProductId,
    id: String,
}

impl RawKey {
    /// Splits `text`, written `venue.product:id`.
    ///
    /// # Errors
    ///
    /// [`KeyError::Malformed`] when there is no `:`, no `.` before it, a venue
    /// or product that is not an id, or an empty id.
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        let malformed = |reason| KeyError::Malformed {
            text: text.to_owned(),
            reason,
        };
        let (scope, id) = text
            .split_once(':')
            .ok_or_else(|| malformed("no `:` before the id"))?;
        let (venue, product) = scope
            .split_once('.')
            .ok_or_else(|| malformed("no `.` between the venue and the product"))?;
        if !is_id(venue) {
            return Err(malformed(
                "the venue is not a lowercase letter followed by lowercase letters, digits and `-`",
            ));
        }
        if !is_id(product) {
            return Err(malformed(
                "the product is not a lowercase letter followed by lowercase letters, digits and `-`",
            ));
        }
        if id.is_empty() {
            return Err(malformed("the id is empty"));
        }
        Ok(Self {
            venue: VenueId(Cow::Owned(venue.to_owned())),
            product: ProductId(Cow::Owned(product.to_owned())),
            id: id.to_owned(),
        })
    }

    /// The venue, as written.
    pub fn venue(&self) -> &VenueId {
        &self.venue
    }

    /// The product, as written.
    pub fn product(&self) -> &ProductId {
        &self.product
    }

    /// The id, as written.
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl FromStr for RawKey {
    type Err = KeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl fmt::Display for RawKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}:{}", self.venue, self.product, self.id)
    }
}

impl fmt::Debug for RawKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RawKey")
            .field(&format_args!("{self}"))
            .finish()
    }
}

impl Serialize for RawKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RawKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<'de, str>::deserialize(deserializer)?;
        Self::parse(&text).map_err(de::Error::custom)
    }
}

/// A product's keys. A venue crate implements it once for each product it
/// trades, and its methods are the only way to make a [`MarketKey`].
///
/// The product's [`Id`](Self::Id) type holds an id in canonical form, so
/// [`key`](Self::key) cannot fail, and [`parse_id`](Self::parse_id) is the
/// one place the product checks and canonicalizes text.
pub trait KeySpace {
    /// The venue id, equal to the `venue` the venue crate registers.
    const VENUE: VenueId;
    /// The product id, one of the `products` the venue crate registers.
    const PRODUCT: ProductId;
    /// The product's own id, in canonical form.
    type Id;

    /// Reads an id from text, canonicalizing it.
    ///
    /// # Errors
    ///
    /// Why the text is not one of the product's ids.
    fn parse_id(text: &str) -> Result<Self::Id, String>;

    /// The id's canonical text, the `id` of its key.
    fn id_text(id: &Self::Id) -> Cow<'_, str>;

    /// The key of `id`.
    fn key(id: &Self::Id) -> MarketKey {
        MarketKey {
            venue: Self::VENUE,
            product: Self::PRODUCT,
            id: Arc::from(Self::id_text(id).as_ref()),
        }
    }

    /// The key `raw` names, canonicalized.
    ///
    /// # Errors
    ///
    /// [`KeyError::WrongProduct`] when `raw` names another product, and
    /// [`KeyError::InvalidId`] when its id is not one of this product's.
    fn parse(raw: &RawKey) -> Result<MarketKey, KeyError> {
        Self::id_of(raw.venue(), raw.product(), raw.id()).map(|id| Self::key(&id))
    }

    /// The product's id that `key` names.
    ///
    /// # Errors
    ///
    /// [`KeyError::WrongProduct`] when `key` names another product.
    fn id(key: &MarketKey) -> Result<Self::Id, KeyError> {
        Self::id_of(key.venue(), key.product(), key.id())
    }

    #[doc(hidden)]
    fn id_of(venue: &VenueId, product: &ProductId, id: &str) -> Result<Self::Id, KeyError> {
        if venue != &Self::VENUE || product != &Self::PRODUCT {
            return Err(KeyError::WrongProduct {
                expected: format!("{}.{}", Self::VENUE, Self::PRODUCT),
                found: format!("{venue}.{product}"),
            });
        }
        Self::parse_id(id).map_err(|reason| KeyError::InvalidId {
            product: format!("{}.{}", Self::VENUE, Self::PRODUCT),
            id: id.to_owned(),
            reason,
        })
    }
}

/// Why text or a key named no instrument a product could read. Every one is
/// an [`InvalidRequest`](Class::InvalidRequest): the caller named something
/// no request can carry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyError {
    /// The text is not `venue.product:id`.
    Malformed {
        /// The text, as given.
        text: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// No product this build enables has the key's venue and product ids.
    UnknownProduct {
        /// The venue id, as written.
        venue: String,
        /// The product id, as written.
        product: String,
    },
    /// A product was handed a key of another product.
    WrongProduct {
        /// The product that was asked, as `venue.product`.
        expected: String,
        /// The key's product, as `venue.product`.
        found: String,
    },
    /// The product does not read the id as one of its own.
    InvalidId {
        /// The product, as `venue.product`.
        product: String,
        /// The id, as given.
        id: String,
        /// Why the product refused it.
        reason: String,
    },
}

impl KeyError {
    /// The error for a raw key whose venue and product no enabled product has.
    pub fn unknown(raw: &RawKey) -> Self {
        Self::UnknownProduct {
            venue: raw.venue().as_str().to_owned(),
            product: raw.product().as_str().to_owned(),
        }
    }
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { text, reason } => {
                write!(
                    f,
                    "{text:?} is not a market key (`venue.product:id`): {reason}"
                )
            }
            Self::UnknownProduct { venue, product } => {
                write!(f, "no product this build enables is {venue}.{product}")
            }
            Self::WrongProduct { expected, found } => {
                write!(f, "a {found} key was given where a {expected} key belongs")
            }
            Self::InvalidId {
                product,
                id,
                reason,
            } => write!(f, "{id:?} is not a {product} id: {reason}"),
        }
    }
}

impl Error for KeyError {}

/// An [`InvalidRequest`](Class::InvalidRequest), whatever the case.
impl Classify for KeyError {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ids are ASCII; letters are uppercased.
    struct Upper;

    impl KeySpace for Upper {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("spot");
        type Id = String;

        fn parse_id(text: &str) -> Result<String, String> {
            if text.is_ascii() {
                Ok(text.to_ascii_uppercase())
            } else {
                Err("an id is ASCII".to_owned())
            }
        }

        fn id_text(id: &String) -> Cow<'_, str> {
            Cow::Borrowed(id)
        }
    }

    /// Ids are unsigned integers, written without leading zeros.
    struct Numbered;

    impl KeySpace for Numbered {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("perpetual");
        type Id = u64;

        fn parse_id(text: &str) -> Result<u64, String> {
            text.parse().map_err(|e| format!("{e}"))
        }

        fn id_text(id: &u64) -> Cow<'_, str> {
            Cow::Owned(id.to_string())
        }
    }

    fn raw(text: &str) -> RawKey {
        RawKey::parse(text).unwrap()
    }

    #[test]
    fn text_splits_at_the_first_dot_and_the_first_colon() {
        let key = raw("acme.spot:abc");
        assert_eq!(key.venue().as_str(), "acme");
        assert_eq!(key.product().as_str(), "spot");
        assert_eq!(key.id(), "abc");

        let dotted = raw("acme.spot:a.b");
        assert_eq!(dotted.product().as_str(), "spot");
        assert_eq!(dotted.id(), "a.b");
    }

    #[test]
    fn a_synthetic_id_holding_a_second_colon_is_kept_whole() {
        let key = raw("acme.events:FED-25DEC:yes");
        assert_eq!(key.id(), "FED-25DEC:yes");
        assert_eq!(key.to_string(), "acme.events:FED-25DEC:yes");
        let parsed = Upper::parse(&raw("acme.spot:fed-25dec:yes")).unwrap();
        assert_eq!(parsed.id(), "FED-25DEC:YES");
    }

    #[test]
    fn malformed_text_names_its_fault() {
        for (text, fault) in [
            ("acme-spot-abc", "no `:`"),
            ("acme:abc", "no `.`"),
            ("Acme.spot:abc", "the venue"),
            (".spot:abc", "the venue"),
            ("acme.Spot:abc", "the product"),
            ("acme.sp.ot:abc", "the product"),
            ("acme.:abc", "the product"),
            ("acme.spot:", "the id is empty"),
            (" acme.spot:abc", "the venue"),
        ] {
            let err = RawKey::parse(text).unwrap_err();
            assert!(
                matches!(&err, KeyError::Malformed { text: t, reason } if t == text && reason.contains(fault)),
                "{text:?}: {err}"
            );
        }
    }

    #[test]
    fn case_canonicalization_makes_two_spellings_one_key() {
        let lower = Upper::parse(&raw("acme.spot:abcusd")).unwrap();
        let upper = Upper::parse(&raw("acme.spot:ABCUSD")).unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.id(), "ABCUSD");
        let mut map = std::collections::HashMap::new();
        map.insert(lower, 1);
        assert_eq!(map.get(&upper), Some(&1));
    }

    #[test]
    fn a_numbered_id_loses_its_leading_zeros() {
        let key = Numbered::parse(&raw("acme.perpetual:007")).unwrap();
        assert_eq!(key, Numbered::key(&7));
        assert_eq!(key.id(), "7");
        assert_eq!(Numbered::id(&key), Ok(7));
    }

    #[test]
    fn a_product_refuses_another_products_key_and_an_id_it_cannot_read() {
        let spot = Upper::key(&"ABC".to_owned());
        assert_eq!(
            Numbered::id(&spot),
            Err(KeyError::WrongProduct {
                expected: "acme.perpetual".to_owned(),
                found: "acme.spot".to_owned(),
            })
        );
        assert!(matches!(
            Numbered::parse(&raw("other.perpetual:7")),
            Err(KeyError::WrongProduct { .. })
        ));
        let err = Numbered::parse(&raw("acme.perpetual:seven")).unwrap_err();
        assert!(
            matches!(&err, KeyError::InvalidId { product, id, .. } if product == "acme.perpetual" && id == "seven"),
            "{err}"
        );
        assert_eq!(err.class(), Class::InvalidRequest);
    }

    #[test]
    fn a_key_and_a_raw_key_serialize_as_their_text() {
        let key = Upper::key(&"ABC".to_owned());
        assert_eq!(serde_json::to_string(&key).unwrap(), r#""acme.spot:ABC""#);
        let raw: RawKey = serde_json::from_str(r#""acme.spot:abc""#).unwrap();
        assert_eq!(raw, RawKey::parse("acme.spot:abc").unwrap());
        assert_eq!(serde_json::to_string(&raw).unwrap(), r#""acme.spot:abc""#);
        assert!(serde_json::from_str::<RawKey>(r#""acme-spot""#).is_err());
        assert_eq!(Upper::parse(&raw).unwrap(), key);
    }

    #[test]
    fn debug_shows_the_text_form() {
        let key = Upper::key(&"ABC".to_owned());
        assert_eq!(format!("{key:?}"), "MarketKey(acme.spot:ABC)");
        assert_eq!(
            format!("{:?}", raw("acme.spot:abc")),
            "RawKey(acme.spot:abc)"
        );
        assert_eq!(format!("{:?}", Upper::VENUE), r#"VenueId("acme")"#);
    }

    #[test]
    fn every_key_error_is_an_invalid_request() {
        let errors = [
            RawKey::parse("x").unwrap_err(),
            KeyError::unknown(&raw("acme.spot:abc")),
            Numbered::id(&Upper::key(&"A".to_owned())).unwrap_err(),
            Numbered::parse(&raw("acme.perpetual:x")).unwrap_err(),
        ];
        for err in errors {
            assert_eq!(err.class(), Class::InvalidRequest, "{err}");
            assert!(err.is_fault());
            assert!(!err.is_retriable());
        }
    }

    #[test]
    #[should_panic(expected = "an id is a lowercase letter")]
    fn a_static_id_must_be_an_id() {
        let _ = VenueId::from_static("Acme");
    }
}
```

- [ ] **Step 2: Declare and export it behind the `keys` feature, and list `KeyError` in the crate's `Classify` assertion.**

Current (`polyoxide-venue/src/lib.rs`):

```rust
//!
//! By default the crate depends on nothing, so a credential-free socket crate
```

Replace with:

```rust
//! - With the `keys` feature, `MarketKey` names one instrument as
//!   `venue.product:id`. Only a product's `KeySpace` makes one, so two keys
//!   are equal exactly when they name the same instrument; `RawKey` is text
//!   split into its parts and checked by no product yet.
//!
//! By default the crate depends on nothing, so a credential-free socket crate
```

Current (`polyoxide-venue/src/lib.rs`):

```rust
#[cfg(feature = "decimal")]
pub mod positional;
```

Replace with:

```rust
#[cfg(feature = "keys")]
mod key;
#[cfg(feature = "decimal")]
pub mod positional;
```

Current (`polyoxide-venue/src/lib.rs`):

```rust
pub use retry_after::{parse_retry_after, retry_delay};
```

Replace with:

```rust
#[cfg(feature = "keys")]
pub use key::{KeyError, KeySpace, MarketKey, ProductId, RawKey, VenueId};
pub use retry_after::{parse_retry_after, retry_delay};
```

Current (`polyoxide-venue/src/lib.rs`):

```rust
    fn is<T: crate::Classify>() {}
    is::<UnknownVariant>();
```

Replace with:

```rust
    fn is<T: crate::Classify>() {}
    is::<UnknownVariant>();
    #[cfg(feature = "keys")]
    is::<KeyError>();
```

- [ ] **Step 3: Add the `keys` feature, document it on docs.rs, and name it in the registration's README line.**

Current (`polyoxide-venue/Cargo.toml`):

```toml
decimal = ["dep:rust_decimal", "dep:serde"]
```

Replace with:

```toml
decimal = ["dep:rust_decimal", "dep:serde"]
# `MarketKey`, `RawKey`, `KeySpace` and `KeyError`: one canonical name per
# instrument, which serializes as its text form.
keys = ["dep:serde"]
```

Current (`polyoxide-venue/Cargo.toml`):

```toml
# docs.rs documents `positional`, which only the `decimal` feature builds.
[package.metadata.docs.rs]
features = ["decimal"]
```

Replace with:

```toml
# docs.rs documents `positional` and the keys, which only their features build.
[package.metadata.docs.rs]
features = ["decimal", "keys"]
```

Current (`polyoxide-venue/Cargo.toml`):

```toml
readme = "Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`) and `UnixMillis`"
```

Replace with:

```toml
readme = "Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`), `UnixMillis`, and market keys (feature `keys`: `MarketKey`, `RawKey` and the `KeySpace` a venue implements per product)"
```

- [ ] **Step 4: Run the tests.**

Run: `cargo test -p polyoxide-venue --all-features`
Expected: PASS, with `key::tests` among them (`text_splits_at_the_first_dot_and_the_first_colon`, `a_synthetic_id_holding_a_second_colon_is_kept_whole`, `case_canonicalization_makes_two_spellings_one_key`, `a_static_id_must_be_an_id` and the rest) and the two doctests in `key.rs`.

Run: `cargo check -p polyoxide-venue --no-default-features && cargo check -p polyoxide-venue --no-default-features --features keys`
Expected: both build with no warning; the default build still depends on nothing.

- [ ] **Step 5: Commit.**

```bash
git add polyoxide-venue/src/key.rs polyoxide-venue/src/lib.rs polyoxide-venue/Cargo.toml
git commit -m "feat(venue): canonical market keys that only a product's KeySpace makes (Story 6.2)" -- polyoxide-venue/src/key.rs polyoxide-venue/src/lib.rs polyoxide-venue/Cargo.toml
```

### Task 1.2: `registration!()` in `polyoxide-test-support`

Each venue crate's test that its id constants equal its registration reads the registration through this one helper, so Kalshi's skeleton copies nothing (AD-1).

**Files:**
- Create: `polyoxide-test-support/src/registration.rs`
- Modify: `polyoxide-test-support/src/lib.rs`, `polyoxide-test-support/Cargo.toml`

- [ ] **Step 1: Create the module.**

Create `polyoxide-test-support/src/registration.rs` with:

```rust
//! What a crate registers in its `[package.metadata.polyoxide]`, read from its
//! own `Cargo.toml`, so its tests can hold the ids it declares in code to the
//! ones its registration declares.

use std::path::Path;

/// The ids a crate's `[package.metadata.polyoxide]` declares.
///
/// ```
/// use polyoxide_test_support::Registration;
///
/// let declared = Registration::parse(
///     r#"
///     [package]
///     name = "polyoxide-acme"
///
///     [package.metadata.polyoxide]
///     venue = "acme"
///     products = ["spot", "events"]
///     "#,
/// )
/// .unwrap();
/// assert_eq!(declared.venue.as_deref(), Some("acme"));
/// assert_eq!(declared.products, ["spot", "events"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Registration {
    /// `venue`, when the crate declares one.
    pub venue: Option<String>,
    /// `products`, empty when the crate declares none.
    pub products: Vec<String>,
}

impl Registration {
    /// The registration in `<dir>/Cargo.toml`. Panics with the path when the
    /// file cannot be read, is not TOML, or declares a venue or products of
    /// the wrong shape.
    #[track_caller]
    pub fn of(dir: impl AsRef<Path>) -> Self {
        let path = dir.as_ref().join("Cargo.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => panic!("{}: {e}", path.display()),
        };
        match Self::parse(&text) {
            Ok(registration) => registration,
            Err(e) => panic!("{}: {e}", path.display()),
        }
    }

    /// The registration in a manifest's text. A manifest without the table
    /// declares nothing.
    ///
    /// # Errors
    ///
    /// The manifest is not TOML, or its `venue` is not a string or its
    /// `products` not an array of strings.
    pub fn parse(manifest: &str) -> Result<Self, String> {
        let manifest: toml::Table = toml::from_str(manifest).map_err(|e| e.to_string())?;
        let Some(table) = manifest
            .get("package")
            .and_then(|package| package.get("metadata"))
            .and_then(|metadata| metadata.get("polyoxide"))
        else {
            return Ok(Self::default());
        };
        let venue = match table.get("venue") {
            None => None,
            Some(toml::Value::String(venue)) => Some(venue.clone()),
            Some(other) => return Err(format!("`venue` is not a string: {other}")),
        };
        let products = match table.get("products") {
            None => Vec::new(),
            Some(toml::Value::Array(products)) => products
                .iter()
                .map(|product| {
                    product
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("a product is not a string: {product}"))
                })
                .collect::<Result<_, _>>()?,
            Some(other) => return Err(format!("`products` is not an array: {other}")),
        };
        Ok(Self { venue, products })
    }
}

/// The calling crate's registration, read from the `Cargo.toml` beside its
/// sources.
///
/// It has to be a macro: `env!("CARGO_MANIFEST_DIR")` written in this crate
/// would name this crate's directory, so the macro expands it in the calling
/// crate instead.
///
/// ```
/// let declared = polyoxide_test_support::registration!();
/// assert_eq!(declared.venue, None, "test support names no venue");
/// assert!(declared.products.is_empty());
/// ```
#[macro_export]
macro_rules! registration {
    () => {
        $crate::Registration::of(env!("CARGO_MANIFEST_DIR"))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_without_the_table_declares_nothing() {
        let declared = Registration::parse("[package]\nname = \"x\"\n").unwrap();
        assert_eq!(declared, Registration::default());
    }

    #[test]
    fn a_venue_or_products_of_the_wrong_shape_is_refused() {
        for (manifest, fault) in [
            (
                "[package.metadata.polyoxide]\nvenue = 1\n",
                "`venue` is not a string",
            ),
            (
                "[package.metadata.polyoxide]\nproducts = \"spot\"\n",
                "`products` is not an array",
            ),
            (
                "[package.metadata.polyoxide]\nproducts = [\"spot\", 2]\n",
                "a product is not a string",
            ),
        ] {
            let err = Registration::parse(manifest).unwrap_err();
            assert!(err.contains(fault), "{manifest:?}: {err}");
        }
        assert!(Registration::parse("[package\n").is_err(), "not TOML");
    }

    #[test]
    #[should_panic(expected = "Cargo.toml")]
    fn a_missing_manifest_names_its_path() {
        let _ = Registration::of("/no/such/crate");
    }
}
```

- [ ] **Step 2: Declare it, export `Registration`, and list it in the crate docs.**

Current (`polyoxide-test-support/src/lib.rs`):

```rust
//! - `soak` (feature `soak`) is the rate-limit harnesses' pacer, arguments,
```

Replace with:

```rust
//! - [`registration!`] and [`Registration`] read the ids a crate declares in
//!   its `[package.metadata.polyoxide]`, for the test that holds the crate's
//!   id constants to them.
//! - `soak` (feature `soak`) is the rate-limit harnesses' pacer, arguments,
```

Current (`polyoxide-test-support/src/lib.rs`):

```rust
pub mod query;
#[cfg(feature = "soak")]
pub mod soak;
```

Replace with:

```rust
pub mod query;
mod registration;
#[cfg(feature = "soak")]
pub mod soak;
```

Current (`polyoxide-test-support/src/lib.rs`):

```rust
pub use tag::{environmental, fail, tag_for, transient, ResultExt, Tag};
```

Replace with:

```rust
pub use registration::Registration;
pub use tag::{environmental, fail, tag_for, transient, ResultExt, Tag};
```

- [ ] **Step 3: Add the `toml` dependency.**

Current (`polyoxide-test-support/Cargo.toml`):

```toml
# `query`: the query keys a builder sends, read off a mock server.
```

Replace with:

```toml
# `registration!`: the ids a crate declares in its own `Cargo.toml`.
toml = "0.8"
# `query`: the query keys a builder sends, read off a mock server.
```

- [ ] **Step 4: Run the tests.**

Run: `cargo test -p polyoxide-test-support --lib registration && cargo test -p polyoxide-test-support --doc registration`
Expected: PASS: three unit tests (`a_manifest_without_the_table_declares_nothing`, `a_venue_or_products_of_the_wrong_shape_is_refused`, `a_missing_manifest_names_its_path`) and two doctests. cargo adds `toml` 0.8 and its four dependencies to `Cargo.lock`.

- [ ] **Step 5: Commit.**

```bash
git add polyoxide-test-support/src/registration.rs polyoxide-test-support/src/lib.rs polyoxide-test-support/Cargo.toml Cargo.lock
git commit -m "test(support): registration!() reads the ids a crate registers (Story 6.2)" -- polyoxide-test-support/src/registration.rs polyoxide-test-support/src/lib.rs polyoxide-test-support/Cargo.toml Cargo.lock
```

### Task 1.3: Binance's `usdm::key`

**Files:**
- Create: `polyoxide-binance/src/usdm/key.rs`
- Modify: `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/Cargo.toml`

- [ ] **Step 1: Create the module.** `key.rs`'s test `the_ids_are_the_ones_the_crate_registers` holds `VENUE` and `PRODUCT` to the crate's `[package.metadata.polyoxide]`.

Create `polyoxide-binance/src/usdm/key.rs` with:

```rust
//! USDⓈ-M's market keys: `binance.usdm:<SYMBOL>`, one per contract.
//!
//! The id is the contract's [`Symbol`], whose ASCII letters are uppercased, so
//! `binance.usdm:btcusdt` and `binance.usdm:BTCUSDT` are one key.
//!
//! ```
//! use polyoxide_binance::usdm::{key, types::Symbol};
//! use polyoxide_venue::RawKey;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let parsed = key::parse(&RawKey::parse("binance.usdm:btcusdt")?)?;
//! assert_eq!(parsed, key::key(&Symbol::new("BTCUSDT")?));
//! assert_eq!(parsed.to_string(), "binance.usdm:BTCUSDT");
//! assert_eq!(key::symbol(&parsed)?.as_str(), "BTCUSDT");
//! # Ok(())
//! # }
//! ```

use std::borrow::Cow;

use polyoxide_venue::{KeyError, KeySpace, MarketKey, ProductId, RawKey, VenueId};

use crate::usdm::types::Symbol;

/// Binance's venue id, as the crate registers it.
pub const VENUE: VenueId = VenueId::from_static("binance");

/// USDⓈ-M's product id, as the crate registers it.
pub const PRODUCT: ProductId = ProductId::from_static("usdm");

/// USDⓈ-M's keys. Each names one contract by its [`Symbol`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UsdmKeys;

impl KeySpace for UsdmKeys {
    const VENUE: VenueId = VENUE;
    const PRODUCT: ProductId = PRODUCT;
    type Id = Symbol;

    fn parse_id(text: &str) -> Result<Symbol, String> {
        Symbol::new(text).map_err(|err| err.to_string())
    }

    fn id_text(id: &Symbol) -> Cow<'_, str> {
        Cow::Borrowed(id.as_str())
    }
}

/// The key of the contract `symbol`.
pub fn key(symbol: &Symbol) -> MarketKey {
    UsdmKeys::key(symbol)
}

/// The key `raw` names, with its symbol's ASCII letters uppercased.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `raw` is not `binance.usdm`, and
/// [`KeyError::InvalidId`] when its id is not a symbol.
pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError> {
    UsdmKeys::parse(raw)
}

/// The symbol `key` names.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `key` is not a `binance.usdm` key.
pub fn symbol(key: &MarketKey) -> Result<Symbol, KeyError> {
    UsdmKeys::id(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> RawKey {
        RawKey::parse(text).unwrap()
    }

    #[test]
    fn the_ids_are_the_ones_the_crate_registers() {
        let declared = polyoxide_test_support::registration!();
        assert_eq!(declared.venue.as_deref(), Some(VENUE.as_str()));
        assert!(
            declared
                .products
                .iter()
                .any(|product| product == PRODUCT.as_str()),
            "{declared:?}"
        );
    }

    #[test]
    fn a_lowercase_symbol_names_the_same_key() {
        let lower = parse(&raw("binance.usdm:btcusdt")).unwrap();
        let upper = parse(&raw("binance.usdm:BTCUSDT")).unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.to_string(), "binance.usdm:BTCUSDT");
        assert_eq!(lower, key(&Symbol::new("BTCUSDT").unwrap()));
        assert_eq!(
            parse(&raw("binance.usdm:btcusdt_261225")).unwrap().id(),
            "BTCUSDT_261225"
        );
    }

    #[test]
    fn a_key_gives_back_its_symbol() {
        let btc = Symbol::new("BTCUSDT").unwrap();
        assert_eq!(symbol(&key(&btc)), Ok(btc));
    }

    #[test]
    fn a_symbol_binance_cannot_carry_and_another_product_are_refused() {
        assert!(matches!(
            parse(&raw("binance.usdm:BTC/USDT")),
            Err(KeyError::InvalidId { .. })
        ));
        assert!(matches!(
            parse(&raw("binance.coinm:BTCUSD_PERP")),
            Err(KeyError::WrongProduct { .. })
        ));
    }
}
```

- [ ] **Step 2: Declare it, and re-export its constructor as `usdm::key`.** The module `key` and the function `key` live in different namespaces, so both resolve.

Current (`polyoxide-binance/src/usdm/mod.rs`):

```rust
pub mod api;
pub mod error;
mod policy;
pub mod request;
```

Replace with:

```rust
pub mod api;
pub mod error;
pub mod key;
mod policy;
pub mod request;
```

Current (`polyoxide-binance/src/usdm/mod.rs`):

```rust
pub use error::UsdmError;
pub use weight::WeightBudget;
```

Replace with:

```rust
pub use error::UsdmError;
pub use key::key;
pub use weight::WeightBudget;
```

- [ ] **Step 3: Turn on `polyoxide-venue/keys` with `usdm`.**

Current (`polyoxide-binance/Cargo.toml`):

```toml
    "polyoxide-venue/decimal",
    "dep:rust_decimal",
```

Replace with:

```toml
    "polyoxide-venue/decimal",
    "polyoxide-venue/keys",
    "dep:rust_decimal",
```

- [ ] **Step 4: Run the tests.**

Run: `cargo test -p polyoxide-binance --all-features --lib usdm::key && cargo test -p polyoxide-binance --all-features --doc usdm::key`
Expected: PASS: four unit tests and the module doctest.

- [ ] **Step 5: Commit.**

```bash
git add polyoxide-binance/src/usdm/key.rs polyoxide-binance/src/usdm/mod.rs polyoxide-binance/Cargo.toml
git commit -m "feat(binance): usdm::key, USDⓈ-M's market keys over Symbol (Story 6.2)" -- polyoxide-binance/src/usdm/key.rs polyoxide-binance/src/usdm/mod.rs polyoxide-binance/Cargo.toml
```

### Task 1.4: Polymarket's `shared::keys`

**Files:**
- Create: `polyoxide-polymarket/src/shared/keys.rs`
- Modify: `polyoxide-polymarket/src/shared/mod.rs`, `polyoxide-polymarket/Cargo.toml`

- [ ] **Step 1: Create the module.** The clob's constructors are built with any of `clob`, `gamma` and `data`, and perps' with `perps` (D7).

Create `polyoxide-polymarket/src/shared/keys.rs` with:

```rust
//! Polymarket's market keys, one place for each of its products' constructors.
//!
//! - `polymarket.clob:<token id>` names one outcome token, which an order buys
//!   or sells, and whose book is bids and asks in that outcome's own price
//!   terms. Gamma and data name the clob's outcomes with it, so it is built
//!   with any of `clob`, `gamma` and `data`.
//! - `polymarket.perps:<instrument id>` names one perpetual (`perps`).
//!
//! ```
//! # #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
//! # fn main() -> Result<(), polyoxide_venue::KeyError> {
//! use polyoxide_polymarket::shared::keys::{self, TokenId};
//! use polyoxide_venue::RawKey;
//!
//! let token = TokenId::new("71321045679252212594626385532706912750332728571942532289631379312455583992563")?;
//! let key = keys::parse(&RawKey::parse(&format!("polymarket.clob:{token}"))?)?;
//! assert_eq!(key, keys::clob(&token));
//! assert_eq!(keys::token_id(&key)?, token);
//! # Ok(())
//! # }
//! # #[cfg(not(any(feature = "clob", feature = "gamma", feature = "data")))]
//! # fn main() {}
//! ```

use std::borrow::Cow;
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
use std::fmt;

use polyoxide_venue::{KeyError, MarketKey, RawKey, VenueId};
#[cfg(any(
    feature = "clob",
    feature = "gamma",
    feature = "data",
    feature = "perps"
))]
use polyoxide_venue::{KeySpace, ProductId};

#[cfg(feature = "perps")]
use crate::perps::types::InstrumentId;

/// Polymarket's venue id, as the crate registers it.
pub const VENUE: VenueId = VenueId::from_static("polymarket");

/// The clob's product id, as the crate registers it.
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
pub const CLOB: ProductId = ProductId::from_static("clob");

/// Perps' product id, as the crate registers it.
#[cfg(feature = "perps")]
pub const PERPS: ProductId = ProductId::from_static("perps");

/// An outcome token's id: a 256-bit integer in decimal, as the clob and gamma
/// send it, with no sign and no leading zero.
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TokenId(String);

#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
impl TokenId {
    /// The most digits a token id has: 2^256 − 1 has 78.
    pub const MAX_DIGITS: usize = 78;

    /// Checks a token id.
    ///
    /// # Errors
    ///
    /// [`KeyError::InvalidId`] unless `id` is 1 to 78 decimal digits with no
    /// leading zero.
    pub fn new(id: impl Into<String>) -> Result<Self, KeyError> {
        let id = id.into();
        match check_token_id(&id) {
            Ok(()) => Ok(Self(id)),
            Err(reason) => Err(KeyError::InvalidId {
                product: format!("{VENUE}.{CLOB}"),
                id,
                reason: reason.to_owned(),
            }),
        }
    }

    /// The id, as the clob spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
impl fmt::Display for TokenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
fn check_token_id(id: &str) -> Result<(), &'static str> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("a token id is decimal digits");
    }
    if id.len() > 1 && id.starts_with('0') {
        return Err("a token id has no leading zero");
    }
    if id.len() > TokenId::MAX_DIGITS {
        return Err("a token id has at most 78 digits");
    }
    Ok(())
}

/// The clob's keys. Each names one outcome token by its [`TokenId`].
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClobKeys;

#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
impl KeySpace for ClobKeys {
    const VENUE: VenueId = VENUE;
    const PRODUCT: ProductId = CLOB;
    type Id = TokenId;

    fn parse_id(text: &str) -> Result<TokenId, String> {
        check_token_id(text)
            .map(|()| TokenId(text.to_owned()))
            .map_err(str::to_owned)
    }

    fn id_text(id: &TokenId) -> Cow<'_, str> {
        Cow::Borrowed(id.as_str())
    }
}

/// The key of the outcome token `token_id`.
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
pub fn clob(token_id: &TokenId) -> MarketKey {
    ClobKeys::key(token_id)
}

/// The outcome token `key` names.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `key` is not a `polymarket.clob` key.
#[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
pub fn token_id(key: &MarketKey) -> Result<TokenId, KeyError> {
    ClobKeys::id(key)
}

/// Perps' keys. Each names one perpetual by its [`InstrumentId`], written in
/// decimal without leading zeros.
#[cfg(feature = "perps")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PerpsKeys;

#[cfg(feature = "perps")]
impl KeySpace for PerpsKeys {
    const VENUE: VenueId = VENUE;
    const PRODUCT: ProductId = PERPS;
    type Id = InstrumentId;

    fn parse_id(text: &str) -> Result<InstrumentId, String> {
        text.parse::<u64>()
            .map(InstrumentId)
            .map_err(|err| format!("an instrument id is an unsigned integer: {err}"))
    }

    fn id_text(id: &InstrumentId) -> Cow<'_, str> {
        Cow::Owned(id.to_string())
    }
}

/// The key of the perpetual `id`.
#[cfg(feature = "perps")]
pub fn perps(id: InstrumentId) -> MarketKey {
    PerpsKeys::key(&id)
}

/// The perpetual `key` names.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `key` is not a `polymarket.perps` key.
#[cfg(feature = "perps")]
pub fn instrument_id(key: &MarketKey) -> Result<InstrumentId, KeyError> {
    PerpsKeys::id(key)
}

/// The key `raw` names, by the constructors of the Polymarket product it
/// names, among those this build enables.
///
/// # Errors
///
/// [`KeyError::UnknownProduct`] when `raw` names no product this build
/// enables, and the product's own error when its id does not parse.
pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError> {
    if raw.venue() == &VENUE {
        #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
        if raw.product() == &CLOB {
            return ClobKeys::parse(raw);
        }
        #[cfg(feature = "perps")]
        if raw.product() == &PERPS {
            return PerpsKeys::parse(raw);
        }
    }
    Err(KeyError::unknown(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> RawKey {
        RawKey::parse(text).unwrap()
    }

    #[test]
    fn the_ids_are_the_ones_the_crate_registers() {
        let declared = polyoxide_test_support::registration!();
        assert_eq!(declared.venue.as_deref(), Some(VENUE.as_str()));
        #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
        assert!(
            declared
                .products
                .iter()
                .any(|product| product == CLOB.as_str()),
            "{declared:?}"
        );
        #[cfg(feature = "perps")]
        assert!(
            declared
                .products
                .iter()
                .any(|product| product == PERPS.as_str()),
            "{declared:?}"
        );
    }

    #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
    #[test]
    fn a_token_id_is_decimal_digits_without_a_leading_zero() {
        let longest = "9".repeat(TokenId::MAX_DIGITS);
        assert!(TokenId::new(longest.as_str()).is_ok());
        assert!(TokenId::new("0").is_ok());
        for bad in ["", "0x12", "-1", "+1", "1e5", "007", " 1", &"9".repeat(79)] {
            assert!(
                matches!(TokenId::new(bad), Err(KeyError::InvalidId { .. })),
                "{bad:?}"
            );
        }
    }

    #[cfg(any(feature = "clob", feature = "gamma", feature = "data"))]
    #[test]
    fn a_clob_key_names_its_token_and_gives_it_back() {
        let token = TokenId::new(
            "71321045679252212594626385532706912750332728571942532289631379312455583992563",
        )
        .unwrap();
        let key = parse(&raw(&format!("polymarket.clob:{token}"))).unwrap();
        assert_eq!(key, clob(&token));
        assert_eq!(key.to_string(), format!("polymarket.clob:{token}"));
        assert_eq!(token_id(&key), Ok(token));
        assert!(matches!(
            parse(&raw("polymarket.clob:0x1234")),
            Err(KeyError::InvalidId { .. })
        ));
    }

    #[cfg(feature = "perps")]
    #[test]
    fn a_perps_key_names_its_instrument_without_leading_zeros() {
        let key = parse(&raw("polymarket.perps:007")).unwrap();
        assert_eq!(key, perps(InstrumentId(7)));
        assert_eq!(key.id(), "7");
        assert_eq!(instrument_id(&key), Ok(InstrumentId(7)));
        assert!(matches!(
            parse(&raw("polymarket.perps:seven")),
            Err(KeyError::InvalidId { .. })
        ));
    }

    #[test]
    fn a_product_the_build_does_not_enable_is_unknown() {
        for text in ["polymarket.rtds:btcusdt", "binance.usdm:BTCUSDT"] {
            assert!(
                matches!(parse(&raw(text)), Err(KeyError::UnknownProduct { .. })),
                "{text}"
            );
        }
    }
}
```

- [ ] **Step 2: Declare it.**

Current (`polyoxide-polymarket/src/shared/mod.rs`):

```rust
//!   ([`session_signer`]), which clob and relay share (`clob` or `relay`).
```

Replace with:

```rust
//!   ([`session_signer`]), which clob and relay share (`clob` or `relay`).
//! - [`keys`], the constructors of Polymarket's market keys: the clob's
//!   (`clob`, `gamma` or `data`) and perps' (`perps`).
```

Current (`polyoxide-polymarket/src/shared/mod.rs`):

```rust
pub mod auth;
#[cfg(any(feature = "clob", feature = "relay"))]
```

Replace with:

```rust
pub mod auth;
#[cfg(any(
    feature = "clob",
    feature = "gamma",
    feature = "data",
    feature = "perps"
))]
pub mod keys;
#[cfg(any(feature = "clob", feature = "relay"))]
```

- [ ] **Step 3: Turn on `polyoxide-venue/keys` with `clob`, `gamma`, `data` and `perps`.**

Current (`polyoxide-polymarket/Cargo.toml`):

```toml
clob = [
    "dep:polyoxide-core",
    "dep:base64",
```

Replace with:

```toml
clob = [
    "dep:polyoxide-core",
    "polyoxide-venue/keys",
    "dep:base64",
```

Current (`polyoxide-polymarket/Cargo.toml`):

```toml
gamma = [
    "dep:polyoxide-core",
    "dep:serde",
```

Replace with:

```toml
gamma = [
    "dep:polyoxide-core",
    "polyoxide-venue/keys",
    "dep:serde",
```

Current (`polyoxide-polymarket/Cargo.toml`):

```toml
    "dep:polyoxide-core",
    "dep:serde",
    "dep:serde_json",
```

Replace with:

```toml
    "dep:polyoxide-core",
    "polyoxide-venue/keys",
    "dep:serde",
    "dep:serde_json",
```

Current (`polyoxide-polymarket/Cargo.toml`):

```toml
    "dep:polyoxide-core",
    "polyoxide-venue/decimal",
```

Replace with:

```toml
    "dep:polyoxide-core",
    "polyoxide-venue/keys",
    "polyoxide-venue/decimal",
```

- [ ] **Step 4: Run the tests, in the full build and in each module's own.**

Run: `cargo test -p polyoxide-polymarket --all-features --lib shared::keys && cargo test -p polyoxide-polymarket --all-features --doc shared::keys`
Expected: PASS: five unit tests and the module doctest.

Run: `cargo test -p polyoxide-polymarket --no-default-features --features perps --lib shared::keys && cargo test -p polyoxide-polymarket --no-default-features --features gamma --lib shared::keys && for f in data relay rtds; do cargo check -p polyoxide-polymarket --no-default-features --features $f || exit 1; done`
Expected: PASS (three and four tests), and each check builds with no warning.

- [ ] **Step 5: Commit.**

```bash
git add polyoxide-polymarket/src/shared/keys.rs polyoxide-polymarket/src/shared/mod.rs polyoxide-polymarket/Cargo.toml
git commit -m "feat(polymarket): shared::keys, the clob's and perps' market keys (Story 6.2)" -- polyoxide-polymarket/src/shared/keys.rs polyoxide-polymarket/src/shared/mod.rs polyoxide-polymarket/Cargo.toml
```

### Task 1.5: `polyoxide::parse_key`

**Files:**
- Create: `polyoxide/tests/parse_key.rs`
- Modify: `polyoxide/src/lib.rs`, `polyoxide/Cargo.toml`

- [ ] **Step 1: Write the test first.**

Create `polyoxide/tests/parse_key.rs` with:

```rust
//! `polyoxide::parse_key` reads a key through the constructors of the product
//! it names, among the enabled venues.

use polyoxide::binance::usdm::{self, types::Symbol};
use polyoxide::polymarket::{perps::types::InstrumentId, shared::keys};
use polyoxide::venue::{KeyError, RawKey};

fn parse(text: &str) -> Result<polyoxide::venue::MarketKey, KeyError> {
    polyoxide::parse_key(&RawKey::parse(text).unwrap())
}

#[test]
fn each_venues_keys_come_from_its_own_constructors() {
    assert_eq!(
        parse("binance.usdm:btcusdt").unwrap(),
        usdm::key(&Symbol::new("BTCUSDT").unwrap())
    );
    assert_eq!(
        parse("polymarket.perps:007").unwrap(),
        keys::perps(InstrumentId(7))
    );
    let token = "71321045679252212594626385532706912750332728571942532289631379312455583992563";
    assert_eq!(
        parse(&format!("polymarket.clob:{token}")).unwrap(),
        keys::clob(&keys::TokenId::new(token).unwrap())
    );
}

#[test]
fn a_venue_or_product_the_build_does_not_enable_is_unknown() {
    for text in [
        "acme.spot:ABC",
        "polymarket.rtds:btcusdt",
        "binance.coinm:BTCUSD_PERP",
    ] {
        let parsed = parse(text);
        assert!(
            matches!(parsed, Err(KeyError::UnknownProduct { .. })),
            "{text}: {parsed:?}"
        );
    }
}

#[test]
fn a_product_refuses_an_id_it_cannot_read() {
    assert!(matches!(
        parse("binance.usdm:BTC/USDT"),
        Err(KeyError::InvalidId { .. })
    ));
    assert!(matches!(
        parse("polymarket.clob:0x1234"),
        Err(KeyError::InvalidId { .. })
    ));
}
```

- [ ] **Step 2: Register the test target.**

Current (`polyoxide/Cargo.toml`):

```toml
# docs.rs documents every module of every venue.
```

Replace with:

```toml
# `parse_key` across both venues' products.
[[test]]
name = "parse_key"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]

# docs.rs documents every module of every venue.
```

- [ ] **Step 3: Run it and watch it fail.**

Run: `cargo test -p polyoxide --all-features --test parse_key`
Expected: FAIL to compile: `cannot find function parse_key in crate polyoxide`.

- [ ] **Step 4: Add `parse_key` and name it in the crate docs.** It dispatches by venue to the constructors of the enabled products, and answers `UnknownProduct` for the rest.

Current (`polyoxide/src/lib.rs`):

```rust
//! - `polyoxide::prelude` re-exports a selection of the enabled modules'
```

Replace with:

```rust
//! - `polyoxide::parse_key` reads a market key, `venue.product:id`, through
//!   the constructors of whichever enabled product it names.
//! - `polyoxide::prelude` re-exports a selection of the enabled modules'
```

Current (`polyoxide/src/lib.rs`):

```rust
/// Prelude module for convenient imports
pub mod prelude {
```

Replace with:

```rust
/// The key `raw` names, made by the constructors of its product, among the
/// venues and modules this build enables: Polymarket's `clob` (with
/// `polymarket-clob`, `-gamma` or `-data`) and `perps` (`polymarket-perps`),
/// and Binance's `usdm` (`binance-usdm`).
///
/// Each product canonicalizes the id, so text read from configuration or a
/// command line gives the key the product's own constructors give.
///
/// ```
/// # #[cfg(feature = "binance-usdm")]
/// # fn main() -> Result<(), polyoxide::venue::KeyError> {
/// use polyoxide::venue::RawKey;
///
/// let btc = polyoxide::parse_key(&RawKey::parse("binance.usdm:btcusdt")?)?;
/// assert_eq!(btc.to_string(), "binance.usdm:BTCUSDT");
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "binance-usdm"))]
/// # fn main() {}
/// ```
///
/// # Errors
///
/// [`KeyError::UnknownProduct`](venue::KeyError::UnknownProduct) when no
/// enabled product has the key's venue and product, and the product's own
/// error when it does not read the id.
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-gamma",
    feature = "polymarket-data",
    feature = "polymarket-perps",
    feature = "binance-usdm"
))]
pub fn parse_key(raw: &venue::RawKey) -> Result<venue::MarketKey, venue::KeyError> {
    #[cfg(any(
        feature = "polymarket-clob",
        feature = "polymarket-gamma",
        feature = "polymarket-data",
        feature = "polymarket-perps"
    ))]
    if raw.venue() == &polymarket::shared::keys::VENUE {
        return polymarket::shared::keys::parse(raw);
    }
    #[cfg(feature = "binance-usdm")]
    if raw.venue() == &binance::usdm::key::VENUE && raw.product() == &binance::usdm::key::PRODUCT {
        return binance::usdm::key::parse(raw);
    }
    Err(venue::KeyError::unknown(raw))
}

/// Prelude module for convenient imports
pub mod prelude {
```

- [ ] **Step 5: Run the tests again.**

Run: `cargo test -p polyoxide --all-features --test parse_key && cargo test -p polyoxide --all-features --doc parse_key`
Expected: PASS: three tests and the doctest.

Run: `cargo hack check -p polyoxide --each-feature --no-dev-deps`
Expected: every feature builds, `parse_key`'s `cfg` included.

- [ ] **Step 6: Commit.**

```bash
git add polyoxide/tests/parse_key.rs polyoxide/src/lib.rs polyoxide/Cargo.toml
git commit -m "feat(umbrella): parse_key reads a key through its product's constructors (Story 6.2)" -- polyoxide/tests/parse_key.rs polyoxide/src/lib.rs polyoxide/Cargo.toml
```

### Task 1.6: CLAUDE.md and the generated regions

**Files:**
- Modify: `CLAUDE.md`, `README.md` (generated regions only)

- [ ] **Step 1: State the keys rule in CLAUDE.md.** Three hand-written passages change: the umbrella's crate root gains `parse_key`, a new **Market keys** paragraph follows **Decimal precision**, and the test-support paragraph names `registration!()`.

Current (`CLAUDE.md`):

```markdown
any `binance*` feature) and `prelude`, and nothing else. The features `polymarket` and `binance`
```

Replace with:

```markdown
any `binance*` feature), `parse_key` (under the features that build a product's keys) and
`prelude`, and nothing else. The features `polymarket` and `binance`
```

Current (`CLAUDE.md`):

```markdown
**Decimal precision** — Price/size fields use `rust_decimal::Decimal` with `serde(with = "rust_decimal::serde::str")` for string serialization.
```

Replace with:

```markdown
**Decimal precision** — Price/size fields use `rust_decimal::Decimal` with `serde(with = "rust_decimal::serde::str")` for string serialization.

**Market keys** — `polyoxide_venue::MarketKey` (feature `keys`) names one instrument as
`venue.product:id`, and has no public constructor from text or from parts: only a product's
`KeySpace` makes one, after its `parse_id` has checked and canonicalized the id, so `Eq` is
canonical equality. Each venue crate implements `KeySpace` once per product, in one place:
Binance's `usdm::key` (`UsdmKeys`, over `Symbol`, which uppercases ASCII; `usdm::key(&symbol)`,
`key::parse`, `key::symbol`), and Polymarket's `shared::keys`, whose `ClobKeys` (over `TokenId`,
1 to 78 decimal digits with no leading zero) is built with any of `clob`, `gamma` and `data`, since
gamma and data name the clob's outcomes, and whose `PerpsKeys` (over `InstrumentId`, written
without leading zeros) is built with `perps`. A clob key names one outcome token, never a market.
`RawKey::parse` splits text at its first `:` and then the part before it at its first `.`, so an
id may hold either; `RawKey` round-trips through serde as text, `MarketKey` serializes as text
only, and `polyoxide::parse_key` hands a `RawKey` to the constructors of whichever enabled
product it names. Every `KeyError` is an `InvalidRequest`. Each venue crate's
`the_ids_are_the_ones_the_crate_registers` test holds its `VENUE` and product constants to its
`[package.metadata.polyoxide]`, read with `polyoxide_test_support::registration!()`.
```

Current (`CLAUDE.md`):

```markdown
**The agreement, fixture and soak helpers exist once, in `polyoxide-test-support`.** `fixtures!("sub")` reads `tests/fixtures/sub/` of the crate that expands it (a macro, because `env!("CARGO_MANIFEST_DIR")` inside test-support names test-support). `agreement` holds `key_paths`, `assert_values_agree` (`Arrays::Zip` for perps REST and binance, whose kline drops an element on purpose; `Arrays::SameLength` for perps ws), binance's `compare`/`compare_values`, the allow-list `Ledger` (`excuse`, perps' `never_on_wire`, `stale`) over `(fixture, path, reason)` or `(path, reason)` tables, and gamma's dotted walker as `agreement::dotted`. `openapi` is the schema synthesiser and `check` (perps' `$ref`-nullable, enum, example and `OBSERVED_EXTRA` handling plus data's two-`allOf`-arms refusal), `query` (feature) reads a builder's query keys off a mock server (`keys_sent`), and its ordered key/value pairs (`pairs_sent`, which does not need the body to decode; `Case` and `assert_cases` run a table of builder calls over it for each `tests/query_setters*.rs`), and the full header set one request carries (`headers_sent`, for each HTTP module's `tests/headers*.rs`), `minute` keeps a test clear of a UTC minute boundary, and `soak` (feature) is the harnesses' `Pacer`, `percentile`, `parse_stages`, `parse_routes`, the `observe` throttle layer (the `WARN` under `polyoxide_core` that `tests/observe.rs` proves against core's client) and the two verdict rulebooks, `soak::verdict::tolerant` (data's) and `soak::verdict::strict` (perps'), which are different policies and stay apart. Each suite keeps a thin local function with its old name that states its own allow-lists and venue facts, so test names and bodies did not move; binance's live and agreement targets alias the module as `use polyoxide_test_support::agreement as common;`. The capture scripts share `scripts/capture_common.py`: `get` (a `headers(method, url)` function called per request, so signing stays in each script; a non-JSON error body is returned, not raised), `require_ok`, `write_json`, `write_raw`, `stamp`, `provenance_table`, `write_provenance` and the async `ws_session`, which imports `websockets` lazily. Run a capture script as a file (`python3 scripts/capture_x.py` or `uv run scripts/capture_x.py`), never `python3 -I`, which keeps `scripts/` off `sys.path`; Binance's, `scripts/capture_binance_usdm.py`, is a PEP 723 `uv run` script.
```

Replace with:

```markdown
**The agreement, fixture and soak helpers exist once, in `polyoxide-test-support`.** `fixtures!("sub")` reads `tests/fixtures/sub/` of the crate that expands it (a macro, because `env!("CARGO_MANIFEST_DIR")` inside test-support names test-support). `agreement` holds `key_paths`, `assert_values_agree` (`Arrays::Zip` for perps REST and binance, whose kline drops an element on purpose; `Arrays::SameLength` for perps ws), binance's `compare`/`compare_values`, the allow-list `Ledger` (`excuse`, perps' `never_on_wire`, `stale`) over `(fixture, path, reason)` or `(path, reason)` tables, and gamma's dotted walker as `agreement::dotted`. `openapi` is the schema synthesiser and `check` (perps' `$ref`-nullable, enum, example and `OBSERVED_EXTRA` handling plus data's two-`allOf`-arms refusal), `query` (feature) reads a builder's query keys off a mock server (`keys_sent`), and its ordered key/value pairs (`pairs_sent`, which does not need the body to decode; `Case` and `assert_cases` run a table of builder calls over it for each `tests/query_setters*.rs`), and the full header set one request carries (`headers_sent`, for each HTTP module's `tests/headers*.rs`), `minute` keeps a test clear of a UTC minute boundary, `registration!()` reads the ids the calling crate declares in its `[package.metadata.polyoxide]`, and `soak` (feature) is the harnesses' `Pacer`, `percentile`, `parse_stages`, `parse_routes`, the `observe` throttle layer (the `WARN` under `polyoxide_core` that `tests/observe.rs` proves against core's client) and the two verdict rulebooks, `soak::verdict::tolerant` (data's) and `soak::verdict::strict` (perps'), which are different policies and stay apart. Each suite keeps a thin local function with its old name that states its own allow-lists and venue facts, so test names and bodies did not move; binance's live and agreement targets alias the module as `use polyoxide_test_support::agreement as common;`. The capture scripts share `scripts/capture_common.py`: `get` (a `headers(method, url)` function called per request, so signing stays in each script; a non-JSON error body is returned, not raised), `require_ok`, `write_json`, `write_raw`, `stamp`, `provenance_table`, `write_provenance` and the async `ws_session`, which imports `websockets` lazily. Run a capture script as a file (`python3 scripts/capture_x.py` or `uv run scripts/capture_x.py`), never `python3 -I`, which keeps `scripts/` off `sys.path`; Binance's, `scripts/capture_binance_usdm.py`, is a PEP 723 `uv run` script.
```

- [ ] **Step 2: Regenerate.** `polyoxide-venue`'s README line changed in Task 1.1.

Run: `python3 scripts/gen_registry.py --write && python3 scripts/gen_registry.py --check`
Expected: `wrote README.md` and `wrote CLAUDE.md`, then exit 0.

- [ ] **Step 3: Run the story's checks.**

Run: `cargo fmt --all -- --check && cargo clippy -p polyoxide-venue -p polyoxide-test-support -p polyoxide-binance -p polyoxide-polymarket -p polyoxide --all-targets --all-features -- -D warnings && python3 scripts/venue_gate.py`
Expected: clean; the venue gate reports `0 not` and `0 exceptions stale`.

- [ ] **Step 4: Commit.**

```bash
git add CLAUDE.md README.md
git commit -m "docs: CLAUDE.md states the market-key rules (Story 6.2)" -- CLAUDE.md README.md
```

## Story 2: Shared records and typed extensions (Story 6.1)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-venue/src/extensions.rs`, `polyoxide-venue/src/records.rs`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/Cargo.toml`, `CLAUDE.md`, `README.md`
- **Interfaces:** exposes `polyoxide_venue::{Extensions, DuplicateExtension, Side, Level, SizeUnit, Instrument}` (feature `market`); uses Story 1's `MarketKey` and `KeySpace`.
- **Verify:** `cargo test -p polyoxide-venue --all-features && cargo check -p polyoxide-venue --no-default-features --features market`

`Instrument` is the one record that carries tick size, minimum size and `SizeUnit`; `Extensions` carries what only one venue publishes (D8, D9).

### Task 2.1: `Extensions` and the first records

**Files:**
- Create: `polyoxide-venue/src/extensions.rs`, `polyoxide-venue/src/records.rs`
- Modify: `polyoxide-venue/src/lib.rs`, `polyoxide-venue/Cargo.toml`

- [ ] **Step 1: Create `Extensions`, with the tests that prove each rule:** the derives, read-back by type, a duplicate refused and the first kept, equality by value (`equality_compares_the_values_not_just_the_types`), a deep clone, the `TypeId`s exposed, removal, and `Debug`. Its `compile_fail` doctest refuses a value type with no `PartialEq`; the prototype confirmed the error it fails with is that bound (`E0277: can't compare Uncomparable with Uncomparable`), so the doctest cannot pass for an unrelated reason.

Create `polyoxide-venue/src/extensions.rs` with:

```rust
//! Facts only one venue has, carried on a shared record without a field of
//! their own.

use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{Class, Classify};

/// The object-safe face of an extension value.
trait Extension: Any + Send + Sync {
    fn clone_box(&self) -> Box<dyn Extension>;
    fn eq_dyn(&self, other: &dyn Extension) -> bool;
    fn fmt_dyn(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T> Extension for T
where
    T: Clone + fmt::Debug + PartialEq + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn Extension> {
        Box::new(self.clone())
    }

    fn eq_dyn(&self, other: &dyn Extension) -> bool {
        other
            .as_any()
            .downcast_ref::<T>()
            .is_some_and(|other| self == other)
    }

    fn fmt_dyn(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

/// Facts only one venue has, carried on a shared record: at most one value of
/// each type, read back by its type.
///
/// Each product defines one `<Record>Ext` type per record it fills, such as a
/// quote's update id, and inserts one value of it. A value's type is
/// `Clone + Debug + PartialEq + Send + Sync + 'static`, so a record that
/// carries extensions stays `Clone`, `Debug` and `PartialEq`, and two records
/// are equal only when their extensions hold equal values of the same types.
/// There is no serde support: a value's type is known only to the crate that
/// defines it.
///
/// ```
/// use polyoxide_venue::Extensions;
///
/// #[derive(Debug, Clone, PartialEq)]
/// struct QuoteExt {
///     update_id: u64,
/// }
///
/// let mut ext = Extensions::new();
/// ext.insert(QuoteExt { update_id: 7 })?;
/// assert_eq!(ext.get::<QuoteExt>(), Some(&QuoteExt { update_id: 7 }));
/// assert!(ext.insert(QuoteExt { update_id: 8 }).is_err(), "one value per type");
/// assert_ne!(ext, Extensions::of(QuoteExt { update_id: 8 }));
/// # Ok::<(), polyoxide_venue::DuplicateExtension>(())
/// ```
///
/// A value whose type cannot be compared is refused when the program is
/// built:
///
/// ```compile_fail
/// use polyoxide_venue::Extensions;
///
/// #[derive(Debug, Clone)]
/// struct Uncomparable;
///
/// let mut ext = Extensions::new();
/// ext.insert(Uncomparable).unwrap();
/// ```
#[derive(Default)]
pub struct Extensions {
    values: BTreeMap<TypeId, Box<dyn Extension>>,
}

impl Extensions {
    /// No extensions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Extensions holding `value` alone.
    pub fn of<T>(value: T) -> Self
    where
        T: Clone + fmt::Debug + PartialEq + Send + Sync + 'static,
    {
        let mut ext = Self::new();
        ext.values.insert(TypeId::of::<T>(), Box::new(value));
        ext
    }

    /// Adds `value`.
    ///
    /// # Errors
    ///
    /// [`DuplicateExtension`] when a value of `T` is already held, which is
    /// kept unchanged.
    pub fn insert<T>(&mut self, value: T) -> Result<(), DuplicateExtension>
    where
        T: Clone + fmt::Debug + PartialEq + Send + Sync + 'static,
    {
        let id = TypeId::of::<T>();
        if self.values.contains_key(&id) {
            return Err(DuplicateExtension {
                type_name: std::any::type_name::<T>(),
            });
        }
        self.values.insert(id, Box::new(value));
        Ok(())
    }

    /// The value of `T`, when one is held.
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| value.as_ref().as_any().downcast_ref())
    }

    /// The value of `T`, to change in place, when one is held.
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.values
            .get_mut(&TypeId::of::<T>())
            .and_then(|value| value.as_mut().as_any_mut().downcast_mut())
    }

    /// Takes the value of `T` out, when one is held.
    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.values
            .remove(&TypeId::of::<T>())
            .and_then(|value| value.into_any().downcast().ok())
            .map(|value: Box<T>| *value)
    }

    /// Whether a value of `T` is held.
    pub fn contains<T: 'static>(&self) -> bool {
        self.values.contains_key(&TypeId::of::<T>())
    }

    /// The types of the values held, one each.
    pub fn type_ids(&self) -> impl Iterator<Item = TypeId> + '_ {
        self.values.keys().copied()
    }

    /// How many values are held.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no value is held.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl Clone for Extensions {
    fn clone(&self) -> Self {
        Self {
            values: self
                .values
                .iter()
                .map(|(id, value)| (*id, value.as_ref().clone_box()))
                .collect(),
        }
    }
}

impl PartialEq for Extensions {
    fn eq(&self, other: &Self) -> bool {
        self.values.len() == other.values.len()
            && self.values.iter().all(|(id, value)| {
                other
                    .values
                    .get(id)
                    .is_some_and(|theirs| value.as_ref().eq_dyn(theirs.as_ref()))
            })
    }
}

impl fmt::Debug for Extensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Value<'a>(&'a dyn Extension);

        impl fmt::Debug for Value<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt_dyn(f)
            }
        }

        f.debug_set()
            .entries(self.values.values().map(|value| Value(value.as_ref())))
            .finish()
    }
}

/// A value was inserted into [`Extensions`] that already held one of its type.
/// An [`InvalidRequest`](Class::InvalidRequest): the caller's misuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateExtension {
    /// The type both values have.
    pub type_name: &'static str,
}

impl fmt::Display for DuplicateExtension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the record already holds an extension of type {}",
            self.type_name
        )
    }
}

impl Error for DuplicateExtension {}

impl Classify for DuplicateExtension {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct QuoteExt {
        update_id: u64,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct BookExt {
        sequence: u64,
    }

    #[test]
    fn extensions_are_clone_debug_default_partial_eq_send_and_sync() {
        fn holds<T: Clone + fmt::Debug + Default + PartialEq + Send + Sync>() {}
        holds::<Extensions>();
    }

    #[test]
    fn a_value_is_read_back_by_its_type() {
        let mut ext = Extensions::new();
        assert!(ext.is_empty());
        ext.insert(QuoteExt { update_id: 7 }).unwrap();
        ext.insert(BookExt { sequence: 9 }).unwrap();
        assert_eq!(ext.len(), 2);
        assert_eq!(ext.get::<QuoteExt>(), Some(&QuoteExt { update_id: 7 }));
        assert_eq!(ext.get::<BookExt>(), Some(&BookExt { sequence: 9 }));
        assert_eq!(ext.get::<u64>(), None);
        assert!(ext.contains::<QuoteExt>());
        assert!(!ext.contains::<String>());
    }

    #[test]
    fn inserting_a_type_already_held_is_an_error_and_keeps_the_first() {
        let mut ext = Extensions::of(QuoteExt { update_id: 7 });
        let err = ext.insert(QuoteExt { update_id: 8 }).unwrap_err();
        assert!(err.type_name.ends_with("QuoteExt"), "{err}");
        assert_eq!(err.class(), Class::InvalidRequest);
        assert_eq!(ext.get::<QuoteExt>(), Some(&QuoteExt { update_id: 7 }));
        assert_eq!(ext.len(), 1);
    }

    #[test]
    fn equality_compares_the_values_not_just_the_types() {
        let a = Extensions::of(QuoteExt { update_id: 7 });
        assert_eq!(a, Extensions::of(QuoteExt { update_id: 7 }));
        assert_ne!(a, Extensions::of(QuoteExt { update_id: 8 }));
        assert_ne!(a, Extensions::of(BookExt { sequence: 7 }));
        assert_ne!(a, Extensions::new());
        assert_ne!(Extensions::new(), a);
        let mut both = a.clone();
        both.insert(BookExt { sequence: 1 }).unwrap();
        assert_ne!(a, both);
        assert_ne!(both, a);
        assert_eq!(Extensions::new(), Extensions::default());
    }

    #[test]
    fn a_clone_holds_its_own_copies() {
        let original = Extensions::of(QuoteExt { update_id: 7 });
        let mut copy = original.clone();
        assert_eq!(copy, original);
        copy.get_mut::<QuoteExt>().unwrap().update_id = 8;
        assert_eq!(original.get::<QuoteExt>(), Some(&QuoteExt { update_id: 7 }));
        assert_ne!(copy, original);
    }

    #[test]
    fn the_type_ids_held_are_exposed() {
        let mut ext = Extensions::of(QuoteExt { update_id: 7 });
        ext.insert(BookExt { sequence: 9 }).unwrap();
        let mut ids: Vec<TypeId> = ext.type_ids().collect();
        let mut expected = vec![TypeId::of::<QuoteExt>(), TypeId::of::<BookExt>()];
        ids.sort();
        expected.sort();
        assert_eq!(ids, expected);
    }

    #[test]
    fn a_removed_value_comes_back_whole() {
        let mut ext = Extensions::of(QuoteExt { update_id: 7 });
        assert_eq!(ext.remove::<QuoteExt>(), Some(QuoteExt { update_id: 7 }));
        assert_eq!(ext.remove::<QuoteExt>(), None);
        assert!(ext.is_empty());
        ext.insert(QuoteExt { update_id: 8 }).unwrap();
        assert_eq!(ext.get::<QuoteExt>(), Some(&QuoteExt { update_id: 8 }));
    }

    #[test]
    fn debug_shows_each_value() {
        let ext = Extensions::of(QuoteExt { update_id: 7 });
        assert_eq!(format!("{ext:?}"), "{QuoteExt { update_id: 7 }}");
        assert_eq!(format!("{:?}", Extensions::new()), "{}");
    }
}
```

- [ ] **Step 2: Create the records.** `two_records_differing_only_in_one_extension_value_are_unequal` is the epic's named test.

Create `polyoxide-venue/src/records.rs` with:

```rust
//! The records every venue's market data is read into.
//!
//! A record holds what venues share, as fields, and a venue's own facts in its
//! [`Extensions`]. A field a venue does not publish is an `Option`. Prices and
//! sizes are [`Decimal`]; a size is in the instrument's native quantity, which
//! [`Instrument::size_unit`] names, and an amount in the quote currency is a
//! separate `notional`.

use rust_decimal::Decimal;

use crate::{Extensions, MarketKey};

/// Which side of the book an order, or the taker of a trade, is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// Buying, against the asks.
    Buy,
    /// Selling, against the bids.
    Sell,
}

/// One price level of a book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Level {
    /// The price, in the instrument's quote currency.
    pub price: Decimal,
    /// The size resting at the price, in the instrument's native quantity.
    pub size: Decimal,
}

impl Level {
    /// A level of `size` at `price`.
    pub fn new(price: Decimal, size: Decimal) -> Self {
        Self { price, size }
    }
}

/// What an instrument's sizes count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SizeUnit {
    /// Units of the base asset, such as the coins a coin-quoted contract is
    /// written in.
    Base,
    /// Contracts, each a fixed amount of the underlying the venue defines.
    Contracts,
    /// Outcome shares of an event contract, each paying one unit of the
    /// settlement currency if its outcome happens.
    Shares,
}

/// An instrument a venue lists, and the one record that carries its trading
/// rules: tick size, minimum size and [`SizeUnit`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Instrument {
    /// The instrument's key.
    pub key: MarketKey,
    /// A name for people: the venue's symbol, or for an outcome of an event
    /// contract, its market and outcome.
    pub name: String,
    /// The asset the instrument's sizes are written in, when the venue names
    /// one.
    pub base_asset: Option<String>,
    /// The currency its prices are quoted in, when the venue names one.
    pub quote_asset: Option<String>,
    /// What its sizes count.
    pub size_unit: SizeUnit,
    /// The smallest price step, when the venue publishes a fixed one.
    pub tick_size: Option<Decimal>,
    /// The smallest order size, in native quantity.
    pub min_size: Option<Decimal>,
    /// The smallest size step, in native quantity.
    pub size_increment: Option<Decimal>,
    /// The smallest order value, in the quote currency.
    pub min_notional: Option<Decimal>,
    /// Whether the venue takes orders on it now.
    pub tradable: Option<bool>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Instrument {
    /// An instrument with its key, name and size unit, and nothing else known.
    pub fn new(key: MarketKey, name: impl Into<String>, size_unit: SizeUnit) -> Self {
        Self {
            key,
            name: name.into(),
            base_asset: None,
            quote_asset: None,
            size_unit,
            tick_size: None,
            min_size: None,
            size_increment: None,
            min_notional: None,
            tradable: None,
            extensions: Extensions::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::{KeySpace, ProductId, VenueId};

    struct Spot;

    impl KeySpace for Spot {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("spot");
        type Id = String;

        fn parse_id(text: &str) -> Result<String, String> {
            Ok(text.to_owned())
        }

        fn id_text(id: &String) -> Cow<'_, str> {
            Cow::Borrowed(id)
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct SpotInstrumentExt {
        lot: u32,
    }

    fn instrument(lot: u32) -> Instrument {
        let mut instrument = Instrument::new(Spot::key(&"ABC".to_owned()), "ABC", SizeUnit::Base);
        instrument.tick_size = Some(Decimal::new(1, 2));
        instrument
            .extensions
            .insert(SpotInstrumentExt { lot })
            .unwrap();
        instrument
    }

    #[test]
    fn records_are_clone_debug_and_partial_eq() {
        fn holds<T: Clone + std::fmt::Debug + PartialEq>() {}
        holds::<Instrument>();
        holds::<Level>();
        holds::<Side>();
        holds::<SizeUnit>();
    }

    #[test]
    fn two_records_differing_only_in_one_extension_value_are_unequal() {
        assert_eq!(instrument(1), instrument(1));
        assert_ne!(instrument(1), instrument(2));
    }

    #[test]
    fn a_new_instrument_knows_only_what_it_was_given() {
        let key = Spot::key(&"ABC".to_owned());
        let instrument = Instrument::new(key.clone(), "ABC", SizeUnit::Shares);
        assert_eq!(instrument.key, key);
        assert_eq!(instrument.size_unit, SizeUnit::Shares);
        assert_eq!(instrument.tick_size, None);
        assert_eq!(instrument.min_size, None);
        assert_eq!(instrument.size_increment, None);
        assert_eq!(instrument.min_notional, None);
        assert_eq!(instrument.tradable, None);
        assert!(instrument.extensions.is_empty());
    }

    #[test]
    fn a_level_holds_its_price_and_native_size() {
        let level = Level::new(Decimal::new(84_1255, 1), Decimal::new(14_663, 3));
        assert_eq!(level.price.to_string(), "84125.5");
        assert_eq!(level.size.to_string(), "14.663");
    }
}
```

- [ ] **Step 3: Declare both behind the `market` feature, and list `DuplicateExtension` in the `Classify` assertion.**

Current (`polyoxide-venue/src/lib.rs`):

```rust
//!   split into its parts and checked by no product yet.
```

Replace with:

```rust
//!   split into its parts and checked by no product yet.
//! - With the `market` feature, the records market data is read into
//!   (`Instrument`, with its `Level`s, `Side` and `SizeUnit`), and
//!   `Extensions`, which carries a venue's own facts on a shared record, one
//!   value per type.
```

Current (`polyoxide-venue/src/lib.rs`):

```rust
#[cfg(feature = "keys")]
mod key;
#[cfg(feature = "decimal")]
```

Replace with:

```rust
#[cfg(feature = "market")]
mod extensions;
#[cfg(feature = "keys")]
mod key;
#[cfg(feature = "decimal")]
```

Current (`polyoxide-venue/src/lib.rs`):

```rust
#[cfg(feature = "decimal")]
pub mod positional;
```

Replace with:

```rust
#[cfg(feature = "decimal")]
pub mod positional;
#[cfg(feature = "market")]
mod records;
```

Current (`polyoxide-venue/src/lib.rs`):

```rust
#[cfg(feature = "keys")]
pub use key::{KeyError, KeySpace, MarketKey, ProductId, RawKey, VenueId};
```

Replace with:

```rust
#[cfg(feature = "market")]
pub use extensions::{DuplicateExtension, Extensions};
#[cfg(feature = "keys")]
pub use key::{KeyError, KeySpace, MarketKey, ProductId, RawKey, VenueId};
#[cfg(feature = "market")]
pub use records::{Instrument, Level, Side, SizeUnit};
```

Current (`polyoxide-venue/src/lib.rs`):

```rust
    is::<UnknownVariant>();
    #[cfg(feature = "keys")]
    is::<KeyError>();
```

Replace with:

```rust
    is::<UnknownVariant>();
    #[cfg(feature = "keys")]
    is::<KeyError>();
    #[cfg(feature = "market")]
    is::<DuplicateExtension>();
```

- [ ] **Step 4: Add the `market` feature and name it in the README line.**

Current (`polyoxide-venue/Cargo.toml`):

```toml
# instrument, which serializes as its text form.
keys = ["dep:serde"]
```

Replace with:

```toml
# instrument, which serializes as its text form.
keys = ["dep:serde"]
# The records market data is read into (`Side`, `Level`, `SizeUnit`,
# `Instrument`) and `Extensions`, the venue's own facts on a record.
market = ["keys", "decimal"]
```

Current (`polyoxide-venue/Cargo.toml`):

```toml
# docs.rs documents `positional` and the keys, which only their features build.
[package.metadata.docs.rs]
features = ["decimal", "keys"]
```

Replace with:

```toml
# docs.rs documents `positional`, the keys and the records, which only their
# features build.
[package.metadata.docs.rs]
features = ["decimal", "keys", "market"]
```

Current (`polyoxide-venue/Cargo.toml`):

```toml
readme = "Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`), `UnixMillis`, and market keys (feature `keys`: `MarketKey`, `RawKey` and the `KeySpace` a venue implements per product)"
```

Replace with:

```toml
readme = "Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`), `UnixMillis`, market keys (feature `keys`: `MarketKey`, `RawKey` and the `KeySpace` a venue implements per product), and the market-data records with their typed `Extensions` (feature `market`)"
```

- [ ] **Step 5: Run the tests and every feature build.**

Run: `cargo test -p polyoxide-venue --all-features`
Expected: PASS, with `extensions::tests` (eight tests) and `records::tests` (four), and the `compile_fail` doctest counted as passed.

Run: `for f in "" keys market decimal; do cargo check -p polyoxide-venue --no-default-features ${f:+--features $f} || exit 1; done`
Expected: all four build with no warning.

- [ ] **Step 6: Commit.**

```bash
git add polyoxide-venue/src/extensions.rs polyoxide-venue/src/records.rs polyoxide-venue/src/lib.rs polyoxide-venue/Cargo.toml
git commit -m "feat(venue): the shared records and their typed Extensions (Story 6.1)" -- polyoxide-venue/src/extensions.rs polyoxide-venue/src/records.rs polyoxide-venue/src/lib.rs polyoxide-venue/Cargo.toml
```

### Task 2.2: CLAUDE.md and the generated regions

**Files:**
- Modify: `CLAUDE.md`, `README.md` (generated regions only)

- [ ] **Step 1: Add the **Market-data records** paragraph after **Market keys**.**

Current (`CLAUDE.md`):

```markdown
`[package.metadata.polyoxide]`, read with `polyoxide_test_support::registration!()`.
```

Replace with:

```markdown
`[package.metadata.polyoxide]`, read with `polyoxide_test_support::registration!()`.

**Market-data records** — `polyoxide-venue`'s `market` feature holds the records market data is
read into: `Instrument`, the one record that carries tick size, minimum size and `SizeUnit`
(`Base`, `Contracts` or `Shares`); `Level`, a price and a size, which is a value inside a record
rather than a record; and `Side { Buy, Sell }`. A size is the instrument's native quantity, a
quote-currency amount is a separate `notional`, and a field a venue does not publish is an
`Option`. Records derive `Clone`, `Debug` and `PartialEq`, are `#[non_exhaustive]` with a `new`
that takes their required fields, and carry `Extensions`: at most one value per type, each
`Clone + Debug + PartialEq + Send + Sync + 'static`, compared by value, with no serde, and
`type_ids()` to list them. Inserting a type already held is a `DuplicateExtension`
(`InvalidRequest`), and `Extensions::of(value)` starts one with a single value. A product's own
facts go in its one `<Record>Ext` per record type, never in a new field on the shared record.
```

- [ ] **Step 2: Regenerate and check.**

Run: `python3 scripts/gen_registry.py --write && python3 scripts/gen_registry.py --check && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-venue --all-features && python3 scripts/venue_gate.py`
Expected: `wrote README.md`, `wrote CLAUDE.md`, exit 0, a clean doc build, and no venue-gate hit.

- [ ] **Step 3: Commit.**

```bash
git add CLAUDE.md README.md
git commit -m "docs: CLAUDE.md states the record and Extensions rules (Story 6.1)" -- CLAUDE.md README.md
```

## Story 3: The market-data traits, implemented for Binance USDⓈ-M (Story 6.3)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-venue/src/records.rs`, `polyoxide-venue/src/market.rs`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/Cargo.toml`, `polyoxide-venue/README.md`, `polyoxide-test-support/src/tag.rs`, `polyoxide-test-support/src/lib.rs`, `polyoxide-test-support/tests/tags.rs`, `polyoxide-binance/src/usdm/venue.rs`, `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/src/usdm/README.md`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/venue_usdm.rs`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md`, `CLAUDE.md`, `README.md`, `Cargo.lock`
- **Interfaces:** exposes `polyoxide_venue::{MarketData, Instruments, Trades, Candles, Funding, EventGroups}` and their `Dyn…` wrappers, `polyoxide_venue::{Quote, Book, Trade, TradeId, Candle, CandleInterval, CandleQuery, FundingRate, EventGroup, EventOutcome, NotListed}`; `polyoxide_test_support::{ClassifiedResultExt, fail_classified}`; `polyoxide_binance::usdm::venue::{InstrumentExt, QuoteExt, BookExt, TradeExt, CandleExt, FundingExt}` and the five trait impls on `Usdm`. Uses Story 1's keys and Story 2's records.
- **Verify:** `cargo test -p polyoxide-venue -p polyoxide-test-support --all-features && cargo test -p polyoxide-binance --all-features --test venue_usdm && cargo test -p polyoxide-binance --all-features --lib usdm::venue`

Each trait has `Send + Sync` supertraits, methods returning `impl Future<Output = Result<_, ClassifiedError>> + Send`, and a `#[dynosaur::dynosaur(pub Dyn<Trait> = dyn(box) <Trait>)]` wrapper, as core's hooks do (AD-5). No method returns `Unsupported`: a product that cannot do something does not implement the trait (AD-6, D1).

### Task 3.1: The traits and the records they return

**Files:**
- Create: `polyoxide-venue/src/market.rs`
- Modify: `polyoxide-venue/src/records.rs`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/Cargo.toml`, `polyoxide-venue/README.md`

- [ ] **Step 1: Add the trait-named records to `records.rs`:** `Quote`, `Book`, `TradeId`, `Trade`, `CandleInterval`, `CandleQuery`, `Candle`, `FundingRate`, `EventGroup`, `EventOutcome` and `NotListed`, with their tests.

Current (`polyoxide-venue/src/records.rs`):

```rust
use rust_decimal::Decimal;

use crate::{Extensions, MarketKey};
```

Replace with:

```rust
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use rust_decimal::Decimal;

use crate::{Class, Classify, Extensions, MarketKey, UnixMillis};
```

Current (`polyoxide-venue/src/records.rs`):

```rust
#[cfg(test)]
mod tests {
    use std::borrow::Cow;
```

Replace with:

```rust
/// An instrument's best bid and ask at one moment.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Quote {
    /// The instrument's key.
    pub key: MarketKey,
    /// The highest bid, when there is one.
    pub bid: Option<Level>,
    /// The lowest ask, when there is one.
    pub ask: Option<Level>,
    /// When the venue took it.
    pub time: Option<UnixMillis>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Quote {
    /// A quote for `key` with neither side known.
    pub fn new(key: MarketKey) -> Self {
        Self {
            key,
            bid: None,
            ask: None,
            time: None,
            extensions: Extensions::new(),
        }
    }
}

/// An instrument's order book at one moment.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Book {
    /// The instrument's key.
    pub key: MarketKey,
    /// Bids, best (highest price) first.
    pub bids: Vec<Level>,
    /// Asks, best (lowest price) first.
    pub asks: Vec<Level>,
    /// When the venue took it.
    pub time: Option<UnixMillis>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Book {
    /// An empty book for `key`.
    pub fn new(key: MarketKey) -> Self {
        Self {
            key,
            bids: Vec::new(),
            asks: Vec::new(),
            time: None,
            extensions: Extensions::new(),
        }
    }
}

/// A trade's id at its venue. Opaque: compare it, never parse it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TradeId(Arc<str>);

impl TradeId {
    /// The id `id`.
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }

    /// The id, as the venue wrote it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TradeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One public trade.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Trade {
    /// The instrument's key.
    pub key: MarketKey,
    /// The venue's id for the trade, when it gives one.
    pub id: Option<TradeId>,
    /// The price.
    pub price: Decimal,
    /// The size, in native quantity.
    pub size: Decimal,
    /// The amount, in the quote currency, when the venue publishes it.
    pub notional: Option<Decimal>,
    /// The taker's side, when the venue says.
    pub taker_side: Option<Side>,
    /// When it traded.
    pub time: UnixMillis,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Trade {
    /// A trade of `size` at `price`, at `time`.
    pub fn new(key: MarketKey, price: Decimal, size: Decimal, time: UnixMillis) -> Self {
        Self {
            key,
            id: None,
            price,
            size,
            notional: None,
            taker_side: None,
            time,
            extensions: Extensions::new(),
        }
    }
}

/// How long a candle spans. Each venue that serves candles serves every
/// width here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum CandleInterval {
    /// One minute.
    Minute1,
    /// Five minutes.
    Minute5,
    /// Fifteen minutes.
    Minute15,
    /// One hour.
    Hour1,
    /// Four hours.
    Hour4,
    /// One day.
    Day1,
    /// One week.
    Week1,
}

impl CandleInterval {
    /// Every width, shortest first.
    pub const ALL: &'static [CandleInterval] = &[
        Self::Minute1,
        Self::Minute5,
        Self::Minute15,
        Self::Hour1,
        Self::Hour4,
        Self::Day1,
        Self::Week1,
    ];

    /// How long the width is.
    pub fn duration(self) -> Duration {
        let minutes = match self {
            Self::Minute1 => 1,
            Self::Minute5 => 5,
            Self::Minute15 => 15,
            Self::Hour1 => 60,
            Self::Hour4 => 240,
            Self::Day1 => 1_440,
            Self::Week1 => 10_080,
        };
        Duration::from_secs(minutes * 60)
    }
}

/// Which candles to read: their width and the open times they span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CandleQuery {
    /// The candles' width.
    pub interval: CandleInterval,
    /// The earliest open time.
    pub start: UnixMillis,
    /// The latest open time; now, when `None`.
    pub end: Option<UnixMillis>,
}

impl CandleQuery {
    /// Candles of `interval` from `start` to now.
    pub fn new(interval: CandleInterval, start: UnixMillis) -> Self {
        Self {
            interval,
            start,
            end: None,
        }
    }

    /// Ends the span at `end`.
    pub fn end(mut self, end: UnixMillis) -> Self {
        self.end = Some(end);
        self
    }
}

/// One candle. A venue that samples one price per interval gives only
/// [`close`](Self::close).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Candle {
    /// The instrument's key.
    pub key: MarketKey,
    /// The candle's width.
    pub interval: CandleInterval,
    /// When it opens.
    pub open_time: UnixMillis,
    /// The first price.
    pub open: Option<Decimal>,
    /// The highest price.
    pub high: Option<Decimal>,
    /// The lowest price.
    pub low: Option<Decimal>,
    /// The last price.
    pub close: Decimal,
    /// The size traded, in native quantity.
    pub volume: Option<Decimal>,
    /// The amount traded, in the quote currency.
    pub notional: Option<Decimal>,
    /// How many trades.
    pub trades: Option<u64>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Candle {
    /// A candle of `interval` opening at `open_time` that closed at `close`.
    pub fn new(
        key: MarketKey,
        interval: CandleInterval,
        open_time: UnixMillis,
        close: Decimal,
    ) -> Self {
        Self {
            key,
            interval,
            open_time,
            open: None,
            high: None,
            low: None,
            close,
            volume: None,
            notional: None,
            trades: None,
            extensions: Extensions::new(),
        }
    }
}

/// A perpetual's funding rate: the one now accruing, or one already settled.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FundingRate {
    /// The instrument's key.
    pub key: MarketKey,
    /// The rate, a fraction of position value per funding period.
    pub rate: Decimal,
    /// When the venue sampled the rate now accruing, or when a settled one
    /// settled.
    pub time: UnixMillis,
    /// When the rate now accruing next settles; `None` for a settled rate.
    pub next_funding: Option<UnixMillis>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl FundingRate {
    /// A rate of `rate` at `time`.
    pub fn new(key: MarketKey, rate: Decimal, time: UnixMillis) -> Self {
        Self {
            key,
            rate,
            time,
            next_funding: None,
            extensions: Extensions::new(),
        }
    }
}

/// An event and every outcome it groups, across its markets.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct EventGroup {
    /// The venue's id for the event.
    pub id: String,
    /// The event's title, when it has one.
    pub title: Option<String>,
    /// Its outcomes, market by market.
    pub outcomes: Vec<EventOutcome>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl EventGroup {
    /// The event `id`, with no outcomes yet.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: None,
            outcomes: Vec::new(),
            extensions: Extensions::new(),
        }
    }
}

/// One outcome of an event: a key of its own, in one of the event's markets.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct EventOutcome {
    /// The outcome's key.
    pub key: MarketKey,
    /// The venue's id for the market the outcome is in.
    pub market: String,
    /// The market's title, when it has one.
    pub market_title: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub label: String,
}

impl EventOutcome {
    /// The outcome `label` of `market`, keyed `key`.
    pub fn new(key: MarketKey, market: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key,
            market: market.into(),
            market_title: None,
            label: label.into(),
        }
    }
}

/// The venue lists no instrument by the key. A
/// [`VenueRefusal`](Class::VenueRefusal), as the venue's own answer for an
/// unknown instrument is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotListed {
    /// The key asked for.
    pub key: MarketKey,
}

impl fmt::Display for NotListed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not listed", self.key)
    }
}

impl Error for NotListed {}

impl Classify for NotListed {
    fn class(&self) -> Class {
        Class::VenueRefusal { code: None }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
```

Current (`polyoxide-venue/src/records.rs`):

```rust
        holds::<Level>();
        holds::<Side>();
        holds::<SizeUnit>();
```

Replace with:

```rust
        holds::<Level>();
        holds::<Side>();
        holds::<SizeUnit>();
        holds::<Quote>();
        holds::<Book>();
        holds::<Trade>();
        holds::<Candle>();
        holds::<FundingRate>();
        holds::<EventGroup>();
        holds::<EventOutcome>();
    }

    #[test]
    fn each_record_carries_extensions_into_its_equality() {
        #[derive(Debug, Clone, PartialEq)]
        struct Venue(u8);

        let key = Spot::key(&"ABC".to_owned());
        let time = UnixMillis(1);
        let mut quote = Quote::new(key.clone());
        quote.extensions.insert(Venue(1)).unwrap();
        let mut other = Quote::new(key.clone());
        other.extensions.insert(Venue(2)).unwrap();
        assert_ne!(quote, other);

        let mut trade = Trade::new(key.clone(), Decimal::ONE, Decimal::ONE, time);
        let plain = trade.clone();
        trade.extensions.insert(Venue(1)).unwrap();
        assert_ne!(trade, plain);

        let mut candle = Candle::new(key.clone(), CandleInterval::Hour1, time, Decimal::ONE);
        let plain = candle.clone();
        candle.extensions.insert(Venue(1)).unwrap();
        assert_ne!(candle, plain);

        let mut rate = FundingRate::new(key.clone(), Decimal::ONE, time);
        let plain = rate.clone();
        rate.extensions.insert(Venue(1)).unwrap();
        assert_ne!(rate, plain);

        let mut book = Book::new(key);
        let plain = book.clone();
        book.extensions.insert(Venue(1)).unwrap();
        assert_ne!(book, plain);

        let mut group = EventGroup::new("1");
        let plain = group.clone();
        group.extensions.insert(Venue(1)).unwrap();
        assert_ne!(group, plain);
    }

    #[test]
    fn each_candle_interval_has_its_duration_in_order() {
        let minutes: Vec<u64> = CandleInterval::ALL
            .iter()
            .map(|interval| interval.duration().as_secs() / 60)
            .collect();
        assert_eq!(minutes, [1, 5, 15, 60, 240, 1_440, 10_080]);
    }

    #[test]
    fn a_candle_query_ends_now_unless_told() {
        let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(5));
        assert_eq!(query.end, None);
        assert_eq!(query.end(UnixMillis(9)).end, Some(UnixMillis(9)));
    }

    #[test]
    fn a_trade_id_is_opaque_text() {
        let id = TradeId::new("8353866920580050");
        assert_eq!(id.as_str(), "8353866920580050");
        assert_eq!(id.to_string(), "8353866920580050");
        assert_eq!(id, TradeId::new(String::from("8353866920580050")));
    }

    #[test]
    fn an_unlisted_key_is_a_venue_refusal_that_names_it() {
        let err = NotListed {
            key: Spot::key(&"ABC".to_owned()),
        };
        assert_eq!(err.to_string(), "acme.spot:ABC is not listed");
        assert_eq!(err.class(), Class::VenueRefusal { code: None });
        assert!(!err.is_retriable());
```

- [ ] **Step 2: Create the traits.** The test venue in `market.rs`'s tests implements every trait and holds each through its `Dyn` wrapper as `Arc<Dyn…<'static>>`; it is polled with `Waker::noop()` (stable since 1.85, under the MSRV), since it never waits.

Create `polyoxide-venue/src/market.rs` with:

```rust
//! The traits a venue's products implement, so market data reads the same on
//! every venue.
//!
//! Every product implements [`MarketData`]. What only some products offer is a
//! separate trait, implemented only where offered: [`Instruments`] (the whole
//! listing, where one request or a few return it), [`Trades`], [`Candles`],
//! [`Funding`] and [`EventGroups`]. No method answers "unsupported": a product
//! that cannot do something does not implement its trait.
//!
//! Each method returns a [`ClassifiedError`], built with `?` from the module's
//! own error; the native clients keep their typed errors. A key of another
//! product is a [`KeyError`](crate::KeyError), an
//! [`InvalidRequest`](crate::Class::InvalidRequest).
//!
//! Each trait has a `Dyn` wrapper, so one value can be held as, for example,
//! `Arc<DynMarketData<'static>>` whichever venue it reads:
//!
//! ```
//! use std::sync::Arc;
//!
//! use polyoxide_venue::{
//!     Book, ClassifiedError, DynMarketData, Instrument, MarketData, MarketKey, Quote,
//! };
//!
//! struct Closed;
//!
//! impl MarketData for Closed {
//!     async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
//!         Err(polyoxide_venue::NotListed { key: key.clone() }.into())
//!     }
//!
//!     async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
//!         Ok(Quote::new(key.clone()))
//!     }
//!
//!     async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
//!         Ok(Book::new(key.clone()))
//!     }
//! }
//!
//! let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Closed);
//! # let _ = venue;
//! ```

use std::future::Future;

use crate::{
    Book, Candle, CandleQuery, ClassifiedError, EventGroup, FundingRate, Instrument, MarketKey,
    Quote, Trade, UnixMillis,
};

/// What every product implements: its instruments, quotes and books.
#[dynosaur::dynosaur(pub DynMarketData = dyn(box) MarketData)]
pub trait MarketData: Send + Sync {
    /// The instrument `key` names.
    fn instrument(
        &self,
        key: &MarketKey,
    ) -> impl Future<Output = Result<Instrument, ClassifiedError>> + Send;

    /// The best bid and ask now.
    fn quote(&self, key: &MarketKey)
        -> impl Future<Output = Result<Quote, ClassifiedError>> + Send;

    /// The order book now, as deep as the venue serves it by default.
    fn book(&self, key: &MarketKey) -> impl Future<Output = Result<Book, ClassifiedError>> + Send;
}

/// Every instrument a product lists, where one request or a few return them.
#[dynosaur::dynosaur(pub DynInstruments = dyn(box) Instruments)]
pub trait Instruments: Send + Sync {
    /// Every instrument the product lists.
    fn instruments(&self) -> impl Future<Output = Result<Vec<Instrument>, ClassifiedError>> + Send;
}

/// Public trades.
#[dynosaur::dynosaur(pub DynTrades = dyn(box) Trades)]
pub trait Trades: Send + Sync {
    /// Recent public trades, oldest first: as many as the venue returns in
    /// one request.
    fn trades(
        &self,
        key: &MarketKey,
    ) -> impl Future<Output = Result<Vec<Trade>, ClassifiedError>> + Send;
}

/// Candles.
#[dynosaur::dynosaur(pub DynCandles = dyn(box) Candles)]
pub trait Candles: Send + Sync {
    /// Candles opening between `query.start` and `query.end`, oldest first:
    /// as many as the venue returns in one request. Read on from the last
    /// candle's open time for more.
    fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> impl Future<Output = Result<Vec<Candle>, ClassifiedError>> + Send;
}

/// A perpetual's funding.
#[dynosaur::dynosaur(pub DynFunding = dyn(box) Funding)]
pub trait Funding: Send + Sync {
    /// The rate now accruing, and when it next settles.
    fn funding_rate(
        &self,
        key: &MarketKey,
    ) -> impl Future<Output = Result<FundingRate, ClassifiedError>> + Send;

    /// Settled rates from `start`, oldest first: as many as the venue returns
    /// in one request.
    fn funding_history(
        &self,
        key: &MarketKey,
        start: UnixMillis,
    ) -> impl Future<Output = Result<Vec<FundingRate>, ClassifiedError>> + Send;
}

/// Events that group an event-contract venue's outcomes.
#[dynosaur::dynosaur(pub DynEventGroups = dyn(box) EventGroups)]
pub trait EventGroups: Send + Sync {
    /// The event `key`'s outcome belongs to, with every outcome it groups.
    fn event_group(
        &self,
        key: &MarketKey,
    ) -> impl Future<Output = Result<EventGroup, ClassifiedError>> + Send;
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::sync::Arc;

    use rust_decimal::Decimal;

    use super::*;
    use crate::{
        CandleInterval, Class, EventOutcome, KeySpace, Level, NotListed, ProductId, SizeUnit,
        VenueId,
    };

    struct Spot;

    impl KeySpace for Spot {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("spot");
        type Id = String;

        fn parse_id(text: &str) -> Result<String, String> {
            Ok(text.to_owned())
        }

        fn id_text(id: &String) -> Cow<'_, str> {
            Cow::Borrowed(id)
        }
    }

    /// One instrument, `acme.spot:ABC`, with a one-level book.
    struct Acme;

    impl Acme {
        fn check(key: &MarketKey) -> Result<String, ClassifiedError> {
            let id = Spot::id(key)?;
            if id != "ABC" {
                return Err(NotListed { key: key.clone() }.into());
            }
            Ok(id)
        }
    }

    impl MarketData for Acme {
        async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
            let id = Self::check(key)?;
            Ok(Instrument::new(key.clone(), id, SizeUnit::Base))
        }

        async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
            Self::check(key)?;
            let mut quote = Quote::new(key.clone());
            quote.bid = Some(Level::new(Decimal::ONE, Decimal::TWO));
            Ok(quote)
        }

        async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
            Self::check(key)?;
            let mut book = Book::new(key.clone());
            book.bids.push(Level::new(Decimal::ONE, Decimal::TWO));
            Ok(book)
        }
    }

    impl Instruments for Acme {
        async fn instruments(&self) -> Result<Vec<Instrument>, ClassifiedError> {
            let key = Spot::key(&"ABC".to_owned());
            Ok(vec![self.instrument(&key).await?])
        }
    }

    impl Trades for Acme {
        async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
            Self::check(key)?;
            Ok(vec![Trade::new(
                key.clone(),
                Decimal::ONE,
                Decimal::ONE,
                UnixMillis(1),
            )])
        }
    }

    impl Candles for Acme {
        async fn candles(
            &self,
            key: &MarketKey,
            query: &CandleQuery,
        ) -> Result<Vec<Candle>, ClassifiedError> {
            Self::check(key)?;
            Ok(vec![Candle::new(
                key.clone(),
                query.interval,
                query.start,
                Decimal::ONE,
            )])
        }
    }

    impl Funding for Acme {
        async fn funding_rate(&self, key: &MarketKey) -> Result<FundingRate, ClassifiedError> {
            Self::check(key)?;
            Ok(FundingRate::new(key.clone(), Decimal::ONE, UnixMillis(1)))
        }

        async fn funding_history(
            &self,
            key: &MarketKey,
            start: UnixMillis,
        ) -> Result<Vec<FundingRate>, ClassifiedError> {
            Self::check(key)?;
            Ok(vec![FundingRate::new(key.clone(), Decimal::ONE, start)])
        }
    }

    impl EventGroups for Acme {
        async fn event_group(&self, key: &MarketKey) -> Result<EventGroup, ClassifiedError> {
            Self::check(key)?;
            let mut group = EventGroup::new("E");
            group
                .outcomes
                .push(EventOutcome::new(key.clone(), "M", "Yes"));
            Ok(group)
        }
    }

    fn abc() -> MarketKey {
        Spot::key(&"ABC".to_owned())
    }

    /// Polls a future that never waits to completion.
    fn ready<F: Future>(future: F) -> F::Output {
        use std::pin::pin;
        use std::task::{Context, Poll, Waker};

        let mut future = pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("the test venue never waits"),
        }
    }

    #[test]
    fn every_trait_is_send_and_sync_and_held_through_its_dyn_wrapper() {
        fn send_sync<T: Send + Sync + ?Sized>() {}
        send_sync::<DynMarketData<'static>>();
        send_sync::<DynInstruments<'static>>();
        send_sync::<DynTrades<'static>>();
        send_sync::<DynCandles<'static>>();
        send_sync::<DynFunding<'static>>();
        send_sync::<DynEventGroups<'static>>();

        let market: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Acme);
        let listing: Arc<DynInstruments<'static>> = DynInstruments::new_arc(Acme);
        let trades: Arc<DynTrades<'static>> = DynTrades::new_arc(Acme);
        let candles: Arc<DynCandles<'static>> = DynCandles::new_arc(Acme);
        let funding: Arc<DynFunding<'static>> = DynFunding::new_arc(Acme);
        let groups: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(Acme);

        let key = abc();
        assert_eq!(
            ready(market.quote(&key)).unwrap().bid,
            Some(Level::new(Decimal::ONE, Decimal::TWO))
        );
        assert_eq!(ready(market.book(&key)).unwrap().bids.len(), 1);
        assert_eq!(ready(market.instrument(&key)).unwrap().key, key);
        assert_eq!(ready(listing.instruments()).unwrap().len(), 1);
        assert_eq!(ready(trades.trades(&key)).unwrap().len(), 1);
        let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(7));
        assert_eq!(
            ready(candles.candles(&key, &query)).unwrap()[0].open_time,
            UnixMillis(7)
        );
        assert!(ready(funding.funding_rate(&key)).is_ok());
        assert_eq!(
            ready(funding.funding_history(&key, UnixMillis(3))).unwrap()[0].time,
            UnixMillis(3)
        );
        assert_eq!(
            ready(groups.event_group(&key)).unwrap().outcomes[0].key,
            key
        );
    }

    #[test]
    fn a_key_of_another_product_is_an_invalid_request_and_an_unknown_one_a_refusal() {
        let market: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Acme);
        struct Other;
        impl KeySpace for Other {
            const VENUE: VenueId = VenueId::from_static("other");
            const PRODUCT: ProductId = ProductId::from_static("spot");
            type Id = String;
            fn parse_id(text: &str) -> Result<String, String> {
                Ok(text.to_owned())
            }
            fn id_text(id: &String) -> Cow<'_, str> {
                Cow::Borrowed(id)
            }
        }
        let foreign = Other::key(&"ABC".to_owned());
        let err = ready(market.quote(&foreign)).unwrap_err();
        assert_eq!(err.class, Class::InvalidRequest);

        let unknown = Spot::key(&"XYZ".to_owned());
        let err = ready(market.book(&unknown)).unwrap_err();
        assert_eq!(err.class, Class::VenueRefusal { code: None });
        assert!(err.source.downcast_ref::<NotListed>().is_some());
    }
}
```

- [ ] **Step 3: Declare the module, export the traits and records, and list `NotListed` in the `Classify` assertion.**

Current (`polyoxide-venue/src/lib.rs`):

```rust
//! - With the `market` feature, the records market data is read into
//!   (`Instrument`, with its `Level`s, `Side` and `SizeUnit`), and
//!   `Extensions`, which carries a venue's own facts on a shared record, one
//!   value per type.
```

Replace with:

```rust
//! - With the `market` feature, the traits a venue's products implement so
//!   market data reads the same on every venue: `MarketData`, which every
//!   product implements, and the capability traits `Instruments`, `Trades`,
//!   `Candles`, `Funding` and `EventGroups`, each with a `Dyn` wrapper. They
//!   return the records market data is read into (`Instrument`, `Quote`,
//!   `Book`, `Trade`, `Candle`, `FundingRate` and `EventGroup`), each carrying
//!   `Extensions`, a venue's own facts on a shared record, one value per type.
```

Current (`polyoxide-venue/src/lib.rs`):

```rust
#[cfg(feature = "decimal")]
pub mod positional;
```

Replace with:

```rust
#[cfg(feature = "market")]
mod market;
#[cfg(feature = "decimal")]
pub mod positional;
```

Current (`polyoxide-venue/src/lib.rs`):

```rust
pub use records::{Instrument, Level, Side, SizeUnit};
```

Replace with:

```rust
pub use market::{
    Candles, DynCandles, DynEventGroups, DynFunding, DynInstruments, DynMarketData, DynTrades,
    EventGroups, Funding, Instruments, MarketData, Trades,
};
#[cfg(feature = "market")]
pub use records::{
    Book, Candle, CandleInterval, CandleQuery, EventGroup, EventOutcome, FundingRate, Instrument,
    Level, NotListed, Quote, Side, SizeUnit, Trade, TradeId,
};
```

Current (`polyoxide-venue/src/lib.rs`):

```rust
    #[cfg(feature = "market")]
    is::<DuplicateExtension>();
```

Replace with:

```rust
    #[cfg(feature = "market")]
    is::<DuplicateExtension>();
    #[cfg(feature = "market")]
    is::<NotListed>();
```

- [ ] **Step 4: Give `market` dynosaur, and say so on docs.rs and in the README line.**

Current (`polyoxide-venue/Cargo.toml`):

```toml
rust_decimal = { workspace = true, optional = true }
```

Replace with:

```toml
# The `Dyn` wrappers of the market-data traits (`market`), as core's hooks
# have theirs.
dynosaur = { workspace = true, optional = true }
rust_decimal = { workspace = true, optional = true }
```

Current (`polyoxide-venue/Cargo.toml`):

```toml
# The records market data is read into (`Side`, `Level`, `SizeUnit`,
# `Instrument`) and `Extensions`, the venue's own facts on a record.
market = ["keys", "decimal"]
```

Replace with:

```toml
# The market-data traits (`MarketData` and the capability traits, each with
# its `Dyn` wrapper), the records they return, and `Extensions`, the venue's
# own facts on a record.
market = ["keys", "decimal", "dep:dynosaur"]
```

Current (`polyoxide-venue/Cargo.toml`):

```toml
readme = "Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`), `UnixMillis`, market keys (feature `keys`: `MarketKey`, `RawKey` and the `KeySpace` a venue implements per product), and the market-data records with their typed `Extensions` (feature `market`)"
```

Replace with:

```toml
readme = "Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`), `UnixMillis`, market keys (feature `keys`: `MarketKey`, `RawKey` and the `KeySpace` a venue implements per product), and the market-data traits with the records they return and their typed `Extensions` (feature `market`)"
```

- [ ] **Step 5: Name both features in the crate README.**

Current (`polyoxide-venue/README.md`):

```markdown
feature, the positional decimal serde of rows sent as bare arrays.
```

Replace with:

```markdown
feature, the positional decimal serde of rows sent as bare arrays.

Behind the `keys` feature, `MarketKey` names one instrument as
`venue.product:id`, canonical because only a product's `KeySpace` makes one.
Behind the `market` feature, the market-data traits (`MarketData`, which every
venue product implements, and the capability traits `Instruments`, `Trades`,
`Candles`, `Funding` and `EventGroups`, each with a `Dyn` wrapper) return the
shared records, each carrying a venue's own facts in its `Extensions`.
```

- [ ] **Step 6: Run the tests, the MSRV build and the doc build.**

Run: `cargo test -p polyoxide-venue --all-features`
Expected: PASS, with `market::tests::every_trait_is_send_and_sync_and_held_through_its_dyn_wrapper`, `market::tests::a_key_of_another_product_is_an_invalid_request_and_an_unknown_one_a_refusal` and the new `records::tests`.

Run: `msrv-cargo check -p polyoxide-venue --all-features && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-venue --all-features`
Expected: both clean. dynosaur failing either would trigger AD-5's fallback to `async-trait`, which is a question, not a developer's call.

- [ ] **Step 7: Commit.**

```bash
git add polyoxide-venue/src/market.rs polyoxide-venue/src/records.rs polyoxide-venue/src/lib.rs polyoxide-venue/Cargo.toml polyoxide-venue/README.md Cargo.lock
git commit -m "feat(venue): the market-data traits, with the records they return (Story 6.3)" -- polyoxide-venue/src/market.rs polyoxide-venue/src/records.rs polyoxide-venue/src/lib.rs polyoxide-venue/Cargo.toml polyoxide-venue/README.md Cargo.lock
```

### Task 3.2: Live tests tag a `ClassifiedError`

A trait returns `ClassifiedError`, which deliberately does not implement `Classify`, so `ResultExt::or_fail` cannot unwrap it, and a blanket impl covering both would overlap (E0119). This adds a second trait whose method is also `or_fail`: method resolution picks the impl whose bound holds, which `both_or_fails_are_in_scope_at_once` proves.

**Files:**
- Modify: `polyoxide-test-support/src/tag.rs`, `polyoxide-test-support/src/lib.rs`, `polyoxide-test-support/tests/tags.rs`

- [ ] **Step 1: Write the tests first.** Two new children and three tests go beside the existing ones; no existing item changes.

Current (`polyoxide-test-support/tests/tags.rs`):

```rust
use polyoxide_test_support::{environmental, fail, tag_for, transient, ResultExt, Tag};
use polyoxide_venue::{Class, Classify};
```

Replace with:

```rust
use polyoxide_test_support::{
    environmental, fail, fail_classified, tag_for, transient, ClassifiedResultExt, ResultExt, Tag,
};
use polyoxide_venue::{Class, ClassifiedError, Classify};
```

Current (`polyoxide-test-support/tests/tags.rs`):

```rust
        assert!(caught.is_err());
        panic!("an untagged panic");
```

Replace with:

```rust
        assert!(caught.is_err());
        panic!("an untagged panic");
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_or_fail_classified() {
    if is_child() {
        let classified: Result<(), ClassifiedError> = failed().map_err(ClassifiedError::from);
        classified.or_fail("read the book");
    }
}

#[test]
#[ignore = "a child process of the tests below"]
fn child_fail_classified() {
    if is_child() {
        let err = ClassifiedError::from(failed().unwrap_err());
        fail_classified("read the book", &err);
```

Current (`polyoxide-test-support/tests/tags.rs`):

```rust
    assert!(!stderr.contains("polyoxide-class="), "{stderr}");
}
```

Replace with:

```rust
    assert!(!stderr.contains("polyoxide-class="), "{stderr}");
}

#[test]
fn a_classified_error_prints_every_class_as_its_tag() {
    for child_name in ["child_or_fail_classified", "child_fail_classified"] {
        for (name, fault, tag) in CLASSES {
            let child = run_child(child_name, &described(name, fault), &[]);
            assert!(
                !child.passed,
                "{child_name}, {name}, fault {fault}: the child did not fail"
            );
            assert_eq!(
                child.tags(),
                [format!("polyoxide-class={tag}")],
                "{child_name}, {name}, fault {fault}:\n{}",
                child.stderr
            );
        }
    }
}

#[test]
fn a_classified_error_panics_with_its_context_at_the_callers_line() {
    let child = run_child("child_or_fail_classified", &described("decode", true), &[]);
    assert!(
        child
            .first_message()
            .starts_with("read the book: ClassifiedError"),
        "{}",
        child.stderr
    );
    let report = child.lines()[child.line_of("panicked at")];
    assert!(report.contains("tests/tags.rs:"), "{report}");
}

#[test]
fn both_or_fails_are_in_scope_at_once() {
    let native: Result<u8, Failed> = Ok(1);
    let classified: Result<u8, ClassifiedError> = Ok(2);
    assert_eq!(
        native.or_fail("native") + classified.or_fail("classified"),
        3
    );
}
```

- [ ] **Step 2: Run them and watch them fail.**

Run: `cargo test -p polyoxide-test-support --test tags`
Expected: FAIL to compile: `fail_classified` and `ClassifiedResultExt` are not in `polyoxide_test_support`.

- [ ] **Step 3: Add `ClassifiedResultExt` and `fail_classified`.**

Current (`polyoxide-test-support/src/tag.rs`):

```rust
use polyoxide_venue::{Class, Classify};

/// The prefix of the tag line, which is this followed by the tag and nothing
```

Replace with:

```rust
use polyoxide_venue::{Class, ClassifiedError, Classify};

/// The prefix of the tag line, which is this followed by the tag and nothing
```

Current (`polyoxide-test-support/src/tag.rs`):

```rust
/// Fails the test because the world, not the code, is why it cannot run, such
```

Replace with:

```rust
/// Unwraps a venue trait's `Result` in a live test, tagging the failure by its
/// error's class.
///
/// A venue trait returns [`ClassifiedError`], which does not implement
/// [`Classify`] (its own docs say why), so [`ResultExt`] cannot reach it. This
/// trait's `or_fail` does for it what `ResultExt::or_fail` does for a module's
/// error, and both may be in scope at once: each covers only its own `Result`.
pub trait ClassifiedResultExt<T> {
    /// The `Ok` value. On an `Err`, fails the test as [`fail_classified`]
    /// does. The panic is reported at the caller's line.
    fn or_fail(self, ctx: &str) -> T;
}

impl<T> ClassifiedResultExt<T> for Result<T, ClassifiedError> {
    #[track_caller]
    fn or_fail(self, ctx: &str) -> T {
        match self {
            Ok(value) => value,
            Err(err) => fail_classified(ctx, &err),
        }
    }
}

/// Fails the test with a [`ClassifiedError`] a test holds. Prints
/// `polyoxide-class=<tag>` to stderr, with the tag [`tag_for`] gives the
/// error's class and fault flag as the error recorded them, then panics with
/// `"{ctx}: {err:?}"`, as [`fail`] does. The panic is reported at the caller's
/// line.
#[track_caller]
pub fn fail_classified(ctx: &str, err: &ClassifiedError) -> ! {
    raise(
        tag_for(&err.class, err.is_fault()),
        format_args!("{ctx}: {err:?}"),
    )
}

/// Fails the test because the world, not the code, is why it cannot run, such
```

- [ ] **Step 4: Export them and name them in the crate docs.**

Current (`polyoxide-test-support/src/lib.rs`):

```rust
//! - [`environmental`] and [`transient`] fail a test for a reason no error value
```

Replace with:

```rust
//! - [`ClassifiedResultExt::or_fail`] and [`fail_classified`] do the same for
//!   the [`ClassifiedError`](polyoxide_venue::ClassifiedError) a venue trait
//!   returns, which does not implement `Classify`.
//! - [`environmental`] and [`transient`] fail a test for a reason no error value
```

Current (`polyoxide-test-support/src/lib.rs`):

```rust
pub use tag::{environmental, fail, tag_for, transient, ResultExt, Tag};
```

Replace with:

```rust
pub use tag::{
    environmental, fail, fail_classified, tag_for, transient, ClassifiedResultExt, ResultExt, Tag,
};
```

- [ ] **Step 5: Run the tests again.**

Run: `cargo test -p polyoxide-test-support --test tags`
Expected: PASS: 14 tests and 10 ignored children, among them `a_classified_error_prints_every_class_as_its_tag` (every class, through both entry points, prints the tag `tag_for` gives) and `both_or_fails_are_in_scope_at_once`.

- [ ] **Step 6: Commit.**

```bash
git add polyoxide-test-support/src/tag.rs polyoxide-test-support/src/lib.rs polyoxide-test-support/tests/tags.rs
git commit -m "test(support): or_fail and fail_classified tag a ClassifiedError (Story 6.3)" -- polyoxide-test-support/src/tag.rs polyoxide-test-support/src/lib.rs polyoxide-test-support/tests/tags.rs
```

### Task 3.3: USDⓈ-M through the traits

**Files:**
- Create: `polyoxide-binance/src/usdm/venue.rs`, `polyoxide-binance/tests/venue_usdm.rs`
- Modify: `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/usdm/README.md`

- [ ] **Step 1: Write the fixture test first.** Each test serves a captured fixture from `tests/fixtures/usdm/rest/`, decodes it through the native client and through the trait held as its `Dyn` wrapper, and compares the two field by field: `a_dyn_market_data_holding_usdm_returns_the_native_quote` is the epic's criterion.

Create `polyoxide-binance/tests/venue_usdm.rs` with:

```rust
//! USDⓈ-M through the venue-neutral traits, each held as its `Dyn` wrapper,
//! against mock servers serving the captured fixtures: every method returns
//! the values the native client decodes from the same body.

use std::borrow::Cow;
use std::sync::Arc;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_binance::usdm::{
    self,
    types::{DepthLimit, Filter, Interval, Symbol},
    venue::{BookExt, CandleExt, FundingExt, InstrumentExt, QuoteExt, TradeExt},
    Usdm, UsdmError,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Class, DynCandles, DynFunding, DynInstruments,
    DynMarketData, DynTrades, Funding, Instruments, KeySpace, Level, MarketData, MarketKey,
    NotListed, ProductId, Side, SizeUnit, TradeId, Trades, UnixMillis, VenueId,
};
use serde_json::Value;

fn client(server: &ServerGuard) -> Usdm {
    Usdm::builder().base_url(server.url()).build().unwrap()
}

fn fixture(name: &str) -> String {
    fixtures!("usdm/rest").text(name)
}

fn btc_symbol() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

fn btc() -> MarketKey {
    usdm::key(&btc_symbol())
}

/// A route answering `body` `hits` times, matched on its whole query string.
async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str, hits: usize) -> Mock {
    let query = if query.is_empty() {
        Matcher::Missing
    } else {
        Matcher::Exact(query.to_owned())
    };
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_header("x-mbx-used-weight-1m", "1")
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

#[tokio::test]
async fn a_dyn_market_data_holding_usdm_returns_the_native_quote() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/depth",
        "symbol=BTCUSDT&limit=5",
        &fixture("depth"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .depth(&btc_symbol())
        .limit(DepthLimit::Five)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let quote = venue.quote(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(quote.key, btc());
    assert_eq!(
        quote.bid,
        Some(Level::new(native.bids[0].price, native.bids[0].quantity))
    );
    assert_eq!(
        quote.ask,
        Some(Level::new(native.asks[0].price, native.asks[0].quantity))
    );
    assert_eq!(quote.time, Some(UnixMillis(native.transaction_time)));
    let ext = quote.extensions.get::<QuoteExt>().unwrap();
    assert_eq!(ext.last_update_id, Some(native.last_update_id));
    assert_eq!(ext.event_time, Some(UnixMillis(native.event_time)));
}

#[tokio::test]
async fn book_returns_every_level_of_the_native_depth_best_first() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/depth",
        "symbol=BTCUSDT",
        &fixture("depth"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.market().depth(&btc_symbol()).send().await.unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let book = venue.book(&btc()).await.unwrap();
    mock.assert_async().await;

    let bids: Vec<Level> = native
        .bids
        .iter()
        .map(|l| Level::new(l.price, l.quantity))
        .collect();
    let asks: Vec<Level> = native
        .asks
        .iter()
        .map(|l| Level::new(l.price, l.quantity))
        .collect();
    assert_eq!(book.bids, bids);
    assert_eq!(book.asks, asks);
    assert!(
        book.bids.windows(2).all(|w| w[0].price > w[1].price),
        "bids best first"
    );
    assert!(
        book.asks.windows(2).all(|w| w[0].price < w[1].price),
        "asks best first"
    );
    assert_eq!(book.time, Some(UnixMillis(native.transaction_time)));
    let ext = book.extensions.get::<BookExt>().unwrap();
    assert_eq!(ext.last_update_id, Some(native.last_update_id));
}

#[tokio::test]
async fn instrument_reads_its_contract_from_exchange_info() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/exchangeInfo",
        "",
        &fixture("exchange_info"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.exchange().exchange_info().send().await.unwrap();
    let row = native
        .symbols
        .iter()
        .find(|row| row.symbol == "BTCUSDT")
        .unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let instrument = venue.instrument(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(instrument.key, btc());
    assert_eq!(instrument.name, "BTCUSDT");
    assert_eq!(instrument.size_unit, SizeUnit::Base);
    assert_eq!(
        instrument.base_asset.as_deref(),
        Some(row.base_asset.as_str())
    );
    assert_eq!(
        instrument.quote_asset.as_deref(),
        Some(row.quote_asset.as_str())
    );
    assert_eq!(instrument.tradable, Some(true));
    for filter in &row.filters {
        match filter {
            Filter::PriceFilter { tick_size, .. } => {
                assert_eq!(instrument.tick_size, Some(*tick_size))
            }
            Filter::LotSize {
                min_qty, step_size, ..
            } => {
                assert_eq!(instrument.min_size, Some(*min_qty));
                assert_eq!(instrument.size_increment, Some(*step_size));
            }
            Filter::MinNotional { notional, .. } => {
                assert_eq!(instrument.min_notional, Some(*notional))
            }
            _ => {}
        }
    }
    assert!(instrument.tick_size.is_some() && instrument.min_size.is_some());
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.contract_type.as_ref(), Some(&row.contract_type));
    assert_eq!(ext.status.as_ref(), Some(&row.status));
    assert_eq!(ext.filters.as_ref(), Some(&row.filters));
    assert_eq!(ext.delivery_date, Some(UnixMillis(row.delivery_date)));
}

#[tokio::test]
async fn a_contract_exchange_info_does_not_list_is_a_venue_refusal() {
    let mut server = Server::new_async().await;
    let _mock = route(
        &mut server,
        "/fapi/v1/exchangeInfo",
        "",
        &fixture("exchange_info"),
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let missing = usdm::key(&Symbol::new("NOTLISTEDUSDT").unwrap());
    let err = venue.instrument(&missing).await.unwrap_err();
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert_eq!(
        err.source.downcast_ref::<NotListed>(),
        Some(&NotListed { key: missing })
    );
}

#[tokio::test]
async fn instruments_lists_every_contract_with_its_status() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/exchangeInfo",
        "",
        &fixture("exchange_info"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.exchange().exchange_info().send().await.unwrap();

    let venue: Arc<DynInstruments<'static>> = DynInstruments::new_arc(client);
    let instruments = venue.instruments().await.unwrap();
    mock.assert_async().await;

    let keys: Vec<String> = instruments.iter().map(|i| i.key.to_string()).collect();
    let expected: Vec<String> = native
        .symbols
        .iter()
        .map(|row| format!("binance.usdm:{}", row.symbol))
        .collect();
    assert_eq!(keys, expected);
    for (instrument, row) in instruments.iter().zip(&native.symbols) {
        assert_eq!(
            instrument.tradable,
            Some(row.status.as_str() == "TRADING"),
            "{}",
            row.symbol
        );
    }
    assert!(
        instruments.iter().any(|i| i.tradable == Some(false)),
        "the fixture holds a SETTLING contract"
    );
}

#[tokio::test]
async fn trades_are_the_native_aggregate_trades_with_the_takers_side() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/aggTrades",
        "symbol=BTCUSDT",
        &fixture("agg_trades"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .agg_trades(&btc_symbol())
        .send()
        .await
        .unwrap();

    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client);
    let trades = venue.trades(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(trades.len(), native.len());
    for (trade, row) in trades.iter().zip(&native) {
        assert_eq!(trade.key, btc());
        assert_eq!(trade.id, Some(TradeId::new(row.id.to_string())));
        assert_eq!(trade.price, row.price);
        assert_eq!(trade.size, row.quantity);
        assert_eq!(trade.notional, None);
        assert_eq!(
            trade.taker_side,
            Some(if row.is_buyer_maker {
                Side::Sell
            } else {
                Side::Buy
            })
        );
        assert_eq!(trade.time, UnixMillis(row.time));
        let ext = trade.extensions.get::<TradeExt>().unwrap();
        assert_eq!(ext.first_trade_id, Some(row.first_trade_id));
        assert_eq!(ext.last_trade_id, Some(row.last_trade_id));
        assert_eq!(ext.normal_size, Some(row.normal_quantity));
    }
    assert!(
        trades.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}

#[tokio::test]
async fn candles_are_the_native_klines_from_start() {
    let mut server = Server::new_async().await;
    let query = "symbol=BTCUSDT&interval=1h&startTime=1791380000000&endTime=1791390000000";
    let mock = route(&mut server, "/fapi/v1/klines", query, &fixture("klines"), 2).await;
    let client = client(&server);
    let native = client
        .market()
        .klines(&btc_symbol(), Interval::H1)
        .start_time(1_791_380_000_000)
        .end_time(1_791_390_000_000)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_791_380_000_000))
        .end(UnixMillis(1_791_390_000_000));
    let candles = venue.candles(&btc(), &request).await.unwrap();
    mock.assert_async().await;

    assert_eq!(candles.len(), native.len());
    for (candle, row) in candles.iter().zip(&native) {
        assert_eq!(candle.interval, CandleInterval::Hour1);
        assert_eq!(candle.open_time, UnixMillis(row.open_time));
        assert_eq!(candle.open, Some(row.open));
        assert_eq!(candle.high, Some(row.high));
        assert_eq!(candle.low, Some(row.low));
        assert_eq!(candle.close, row.close);
        assert_eq!(candle.volume, Some(row.volume));
        assert_eq!(candle.notional, Some(row.quote_volume));
        assert_eq!(candle.trades, Some(row.trade_count));
        let ext = candle.extensions.get::<CandleExt>().unwrap();
        assert_eq!(ext.close_time, Some(UnixMillis(row.close_time)));
        assert_eq!(ext.taker_buy_volume, Some(row.taker_buy_base_volume));
        assert_eq!(ext.taker_buy_notional, Some(row.taker_buy_quote_volume));
    }
}

#[tokio::test]
async fn funding_rate_is_the_premium_index_now_accruing() {
    let mut server = Server::new_async().await;
    let all: Vec<Value> = serde_json::from_str(&fixture("premium_index")).unwrap();
    let body = all
        .iter()
        .find(|row| row["symbol"] == "BTCUSDT")
        .unwrap()
        .to_string();
    let mock = route(
        &mut server,
        "/fapi/v1/premiumIndex",
        "symbol=BTCUSDT",
        &body,
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .premium_index(&btc_symbol())
        .send()
        .await
        .unwrap();

    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let rate = venue.funding_rate(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(rate.rate, native.last_funding_rate);
    assert_eq!(rate.time, UnixMillis(native.time));
    assert_eq!(
        rate.next_funding,
        Some(UnixMillis(native.next_funding_time))
    );
    let ext = rate.extensions.get::<FundingExt>().unwrap();
    assert_eq!(ext.mark_price, Some(native.mark_price));
    assert_eq!(ext.index_price, Some(native.index_price));
    assert_eq!(
        ext.estimated_settle_price,
        Some(native.estimated_settle_price)
    );
    assert_eq!(ext.interest_rate, Some(native.interest_rate));
}

#[tokio::test]
async fn funding_history_is_the_native_funding_rates_from_start() {
    let mut server = Server::new_async().await;
    let query = "symbol=BTCUSDT&startTime=1791200000000";
    let mock = route(
        &mut server,
        "/fapi/v1/fundingRate",
        query,
        &fixture("funding_rate"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .funding_rate()
        .symbol(&btc_symbol())
        .start_time(1_791_200_000_000)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let history = venue
        .funding_history(&btc(), UnixMillis(1_791_200_000_000))
        .await
        .unwrap();
    mock.assert_async().await;

    assert_eq!(history.len(), native.len());
    for (rate, row) in history.iter().zip(&native) {
        assert_eq!(rate.rate, row.funding_rate);
        assert_eq!(rate.time, UnixMillis(row.funding_time));
        assert_eq!(rate.next_funding, None, "a settled rate");
        assert_eq!(
            rate.extensions.get::<FundingExt>().unwrap().mark_price,
            row.mark_price
        );
    }
    assert!(
        history.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}

#[tokio::test]
async fn a_venue_error_reaches_the_caller_classified_with_its_native_error_as_source() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/fapi/v1/depth")
        .match_query(Matcher::Any)
        .with_status(400)
        .with_body(r#"{"code":-1121,"msg":"Invalid symbol."}"#)
        .create_async()
        .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue.book(&btc()).await.unwrap_err();
    assert_eq!(
        err.class,
        Class::VenueRefusal {
            code: Some("-1121".into())
        }
    );
    assert!(err.source.downcast_ref::<UsdmError>().is_some());
}

/// A product of another venue, to hand USDⓈ-M a key it does not own.
struct Elsewhere;

impl KeySpace for Elsewhere {
    const VENUE: VenueId = VenueId::from_static("elsewhere");
    const PRODUCT: ProductId = ProductId::from_static("spot");
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_owned())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

#[tokio::test]
async fn a_key_of_another_product_is_refused_before_anything_is_sent() {
    let server = Server::new_async().await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let foreign = Elsewhere::key(&"BTCUSDT".to_owned());
    for err in [
        venue.quote(&foreign).await.unwrap_err(),
        venue.book(&foreign).await.unwrap_err(),
        venue.instrument(&foreign).await.unwrap_err(),
    ] {
        assert_eq!(err.class, Class::InvalidRequest, "{err}");
    }
}
```

- [ ] **Step 2: Register it, and turn on `polyoxide-venue/market` with `usdm` in place of `keys`.**

Current (`polyoxide-binance/Cargo.toml`):

```toml
    "polyoxide-venue/keys",
    "dep:rust_decimal",
```

Replace with:

```toml
    "polyoxide-venue/market",
    "dep:rust_decimal",
```

Current (`polyoxide-binance/Cargo.toml`):

```toml
name = "wire_agreement_usdm"
required-features = ["usdm"]
```

Replace with:

```toml
name = "venue_usdm"
required-features = ["usdm"]

[[test]]
name = "wire_agreement_usdm"
required-features = ["usdm"]
```

- [ ] **Step 3: Run it and watch it fail.**

Run: `cargo test -p polyoxide-binance --all-features --test venue_usdm`
Expected: FAIL to compile: no module `venue` in `usdm`.

- [ ] **Step 4: Implement the traits on `Usdm`.** The quote is a 5-level `depth` and the book an unlimited one (D12); a row whose symbol no key can carry is a `Decode` failure.

Create `polyoxide-binance/src/usdm/venue.rs` with:

```rust
//! USDⓈ-M through the venue-neutral market-data traits.
//!
//! [`Usdm`] implements [`MarketData`], [`Instruments`], [`Trades`], [`Candles`]
//! and [`Funding`] over its own routes, keyed `binance.usdm:<SYMBOL>` by the
//! constructors in `usdm::key`.
//! What Binance alone publishes rides in this module's `<Record>Ext` types,
//! one per record, in each record's [`Extensions`](polyoxide_venue::Extensions).
//!
//! | Method | Route | Weight |
//! | --- | --- | --- |
//! | `instrument`, `instruments` | `exchangeInfo` | 1 |
//! | `quote` | `depth`, 5 levels | 2 |
//! | `book` | `depth`, no limit (500 levels) | 1 |
//! | `trades` | `aggTrades` (the last 500) | 20 |
//! | `candles` | `klines`, from `start` (up to 500) | 5 |
//! | `funding_rate` | `premiumIndex` | 1 |
//! | `funding_history` | `fundingRate`, from `start` (up to 100) | funding bucket |
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use polyoxide_binance::usdm::{self, types::Symbol, Usdm};
//! use polyoxide_venue::{DynMarketData, MarketData};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Usdm::new()?);
//! let quote = venue.quote(&usdm::key(&Symbol::new("BTCUSDT")?)).await?;
//! println!("bid {:?} ask {:?}", quote.bid, quote.ask);
//! # Ok(())
//! # }
//! ```

use polyoxide_venue::{
    Book, Candle, CandleInterval, CandleQuery, Candles, Class, ClassifiedError, Funding,
    FundingRate, Instrument, Instruments, Level, MarketData, MarketKey, NotListed, Quote, Side,
    SizeUnit, Trade, TradeId, Trades, UnixMillis,
};
use rust_decimal::Decimal;

use crate::usdm::{
    key,
    types::{self, ContractType, DepthLimit, Filter, Interval, Symbol, SymbolInfo, SymbolStatus},
    Usdm,
};

/// Binance's facts about a contract, beside [`Instrument`]'s fields.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct InstrumentExt {
    /// Perpetual, TradFi perpetual or a delivery contract.
    pub contract_type: Option<ContractType>,
    /// Trading, settling (delisted) and so on.
    pub status: Option<SymbolStatus>,
    /// The underlying pair, `BTCUSDT` for `BTCUSDT_261225`.
    pub pair: Option<String>,
    /// The margin asset.
    pub margin_asset: Option<String>,
    /// Delivery time; far in the future for a perpetual.
    pub delivery_date: Option<UnixMillis>,
    /// Listing time.
    pub onboard_date: Option<UnixMillis>,
    /// Every order filter, the ones [`Instrument`]'s fields come from
    /// included.
    pub filters: Option<Vec<Filter>>,
}

/// Binance's facts about a quote, beside [`Quote`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct QuoteExt {
    /// The book update id of the snapshot.
    pub last_update_id: Option<u64>,
    /// When the message was produced.
    pub event_time: Option<UnixMillis>,
}

/// Binance's facts about a book, beside [`Book`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BookExt {
    /// The book update id of the snapshot, to align it with the depth stream.
    pub last_update_id: Option<u64>,
    /// When the message was produced.
    pub event_time: Option<UnixMillis>,
}

/// Binance's facts about an aggregate trade, beside [`Trade`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct TradeExt {
    /// The first trade merged into it.
    pub first_trade_id: Option<u64>,
    /// The last trade merged into it.
    pub last_trade_id: Option<u64>,
    /// The size without trades against RPI (Retail Price Improvement) orders.
    pub normal_size: Option<Decimal>,
}

/// Binance's facts about a candle, beside [`Candle`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct CandleExt {
    /// When the candle closes, inclusive.
    pub close_time: Option<UnixMillis>,
    /// The size takers bought.
    pub taker_buy_volume: Option<Decimal>,
    /// The quote-currency amount takers bought.
    pub taker_buy_notional: Option<Decimal>,
}

/// Binance's facts about a funding rate, beside [`FundingRate`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct FundingExt {
    /// The mark price: now, for the rate accruing, and at settlement for a
    /// settled one (absent before about 2022).
    pub mark_price: Option<Decimal>,
    /// The index price now, for the rate accruing.
    pub index_price: Option<Decimal>,
    /// The estimated settle price, meaningful only in the hour before a
    /// settlement.
    pub estimated_settle_price: Option<Decimal>,
    /// The interest rate, for the rate accruing.
    pub interest_rate: Option<Decimal>,
}

fn level(level: &types::Level) -> Level {
    Level::new(level.price, level.quantity)
}

/// The instrument a contract's row describes.
fn instrument(info: &SymbolInfo) -> Result<Instrument, ClassifiedError> {
    // A row whose symbol no key can carry is a response this client cannot read.
    let symbol = Symbol::new(info.symbol.as_str())
        .map_err(|err| ClassifiedError::new(Class::Decode, err))?;
    let mut instrument = Instrument::new(key::key(&symbol), info.symbol.clone(), SizeUnit::Base);
    instrument.base_asset = Some(info.base_asset.clone());
    instrument.quote_asset = Some(info.quote_asset.clone());
    instrument.tradable = Some(info.status == SymbolStatus::Trading);
    for filter in &info.filters {
        match filter {
            Filter::PriceFilter { tick_size, .. } => instrument.tick_size = Some(*tick_size),
            Filter::LotSize {
                min_qty, step_size, ..
            } => {
                instrument.min_size = Some(*min_qty);
                instrument.size_increment = Some(*step_size);
            }
            Filter::MinNotional { notional } => instrument.min_notional = Some(*notional),
            _ => {}
        }
    }
    instrument.extensions = polyoxide_venue::Extensions::of(InstrumentExt {
        contract_type: Some(info.contract_type.clone()),
        status: Some(info.status.clone()),
        pair: Some(info.pair.clone()),
        margin_asset: Some(info.margin_asset.clone()),
        delivery_date: Some(UnixMillis(info.delivery_date)),
        onboard_date: Some(UnixMillis(info.onboard_date)),
        filters: Some(info.filters.clone()),
    });
    Ok(instrument)
}

/// The kline width a candle interval is.
fn interval(interval: CandleInterval) -> Result<Interval, ClassifiedError> {
    Ok(match interval {
        CandleInterval::Minute1 => Interval::M1,
        CandleInterval::Minute5 => Interval::M5,
        CandleInterval::Minute15 => Interval::M15,
        CandleInterval::Hour1 => Interval::H1,
        CandleInterval::Hour4 => Interval::H4,
        CandleInterval::Day1 => Interval::D1,
        CandleInterval::Week1 => Interval::W1,
        other => {
            return Err(ClassifiedError::new(
                Class::InvalidRequest,
                format!("USDⓈ-M serves no {other:?} klines"),
            ))
        }
    })
}

impl MarketData for Usdm {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let info = self.exchange().exchange_info().send().await?;
        match info
            .symbols
            .iter()
            .find(|row| row.symbol == symbol.as_str())
        {
            Some(row) => instrument(row),
            None => Err(NotListed { key: key.clone() }.into()),
        }
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let depth = self
            .market()
            .depth(&symbol)
            .limit(DepthLimit::Five)
            .send()
            .await?;
        let mut quote = Quote::new(key.clone());
        quote.bid = depth.bids.first().map(level);
        quote.ask = depth.asks.first().map(level);
        quote.time = Some(UnixMillis(depth.transaction_time));
        quote.extensions = polyoxide_venue::Extensions::of(QuoteExt {
            last_update_id: Some(depth.last_update_id),
            event_time: Some(UnixMillis(depth.event_time)),
        });
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let depth = self.market().depth(&symbol).send().await?;
        let mut book = Book::new(key.clone());
        book.bids = depth.bids.iter().map(level).collect();
        book.asks = depth.asks.iter().map(level).collect();
        book.time = Some(UnixMillis(depth.transaction_time));
        book.extensions = polyoxide_venue::Extensions::of(BookExt {
            last_update_id: Some(depth.last_update_id),
            event_time: Some(UnixMillis(depth.event_time)),
        });
        Ok(book)
    }
}

impl Instruments for Usdm {
    async fn instruments(&self) -> Result<Vec<Instrument>, ClassifiedError> {
        let info = self.exchange().exchange_info().send().await?;
        info.symbols.iter().map(instrument).collect()
    }
}

impl Trades for Usdm {
    async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let rows = self.market().agg_trades(&symbol).send().await?;
        Ok(rows
            .iter()
            .map(|row| {
                let mut trade =
                    Trade::new(key.clone(), row.price, row.quantity, UnixMillis(row.time));
                trade.id = Some(TradeId::new(row.id.to_string()));
                // The buyer made the market, so the taker sold.
                trade.taker_side = Some(if row.is_buyer_maker {
                    Side::Sell
                } else {
                    Side::Buy
                });
                trade.extensions = polyoxide_venue::Extensions::of(TradeExt {
                    first_trade_id: Some(row.first_trade_id),
                    last_trade_id: Some(row.last_trade_id),
                    normal_size: Some(row.normal_quantity),
                });
                trade
            })
            .collect())
    }
}

impl Candles for Usdm {
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let mut request = self
            .market()
            .klines(&symbol, interval(query.interval)?)
            .start_time(query.start.0);
        if let Some(end) = query.end {
            request = request.end_time(end.0);
        }
        let rows = request.send().await?;
        Ok(rows
            .iter()
            .map(|row| {
                let mut candle = Candle::new(
                    key.clone(),
                    query.interval,
                    UnixMillis(row.open_time),
                    row.close,
                );
                candle.open = Some(row.open);
                candle.high = Some(row.high);
                candle.low = Some(row.low);
                candle.volume = Some(row.volume);
                candle.notional = Some(row.quote_volume);
                candle.trades = Some(row.trade_count);
                candle.extensions = polyoxide_venue::Extensions::of(CandleExt {
                    close_time: Some(UnixMillis(row.close_time)),
                    taker_buy_volume: Some(row.taker_buy_base_volume),
                    taker_buy_notional: Some(row.taker_buy_quote_volume),
                });
                candle
            })
            .collect())
    }
}

impl Funding for Usdm {
    async fn funding_rate(&self, key: &MarketKey) -> Result<FundingRate, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let index = self.market().premium_index(&symbol).send().await?;
        let mut rate =
            FundingRate::new(key.clone(), index.last_funding_rate, UnixMillis(index.time));
        // `0` is a contract with no funding scheduled.
        rate.next_funding =
            (index.next_funding_time > 0).then_some(UnixMillis(index.next_funding_time));
        rate.extensions = polyoxide_venue::Extensions::of(FundingExt {
            mark_price: Some(index.mark_price),
            index_price: Some(index.index_price),
            estimated_settle_price: Some(index.estimated_settle_price),
            interest_rate: Some(index.interest_rate),
        });
        Ok(rate)
    }

    async fn funding_history(
        &self,
        key: &MarketKey,
        start: UnixMillis,
    ) -> Result<Vec<FundingRate>, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let rows = self
            .market()
            .funding_rate()
            .symbol(&symbol)
            .start_time(start.0)
            .send()
            .await?;
        Ok(rows
            .iter()
            .map(|row| {
                let mut rate =
                    FundingRate::new(key.clone(), row.funding_rate, UnixMillis(row.funding_time));
                rate.extensions = polyoxide_venue::Extensions::of(FundingExt {
                    mark_price: row.mark_price,
                    ..FundingExt::default()
                });
                rate
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_candle_interval_is_a_kline_width() {
        let widths: Vec<&str> = CandleInterval::ALL
            .iter()
            .map(|each| interval(*each).unwrap().as_str())
            .collect();
        assert_eq!(widths, ["1m", "5m", "15m", "1h", "4h", "1d", "1w"]);
    }

    #[test]
    fn a_row_whose_symbol_no_key_carries_is_a_decode_failure() {
        let info: types::ExchangeInfo = serde_json::from_str(include_str!(
            "../../tests/fixtures/usdm/rest/exchange_info.json"
        ))
        .unwrap();
        let mut row = info.symbols[0].clone();
        row.symbol = "BTC/USDT".to_owned();
        let err = instrument(&row).unwrap_err();
        assert_eq!(err.class, Class::Decode);
    }
}
```

- [ ] **Step 5: Declare the module.**

Current (`polyoxide-binance/src/usdm/mod.rs`):

```rust
pub mod types;
pub mod weight;
#[cfg(feature = "usdm-ws")]
```

Replace with:

```rust
pub mod types;
pub mod venue;
pub mod weight;
#[cfg(feature = "usdm-ws")]
```

- [ ] **Step 6: Run the tests again.**

Run: `cargo test -p polyoxide-binance --all-features --test venue_usdm && cargo test -p polyoxide-binance --all-features --lib usdm::venue && cargo test -p polyoxide-binance --all-features --doc`
Expected: PASS: 11 fixture tests, 2 unit tests, and the doctests.

- [ ] **Step 7: Add the README section.**

Current (`polyoxide-binance/src/usdm/README.md`):

```markdown
only the funding routes carries no weight and can keep the default.

## Streaming
```

Replace with:

```markdown
only the funding routes carries no weight and can keep the default.

## Through the venue-neutral traits

`Usdm` implements `polyoxide-venue`'s `MarketData`, `Instruments`, `Trades`,
`Candles` and `Funding`, keyed `binance.usdm:<SYMBOL>` (`usdm::key`), so code
written against those traits reads USDⓈ-M as it reads any other venue. What
only Binance publishes rides in `usdm::venue`'s `<Record>Ext` types.
The `usdm::venue` module's documentation has an example.

## Streaming
```

- [ ] **Step 8: Commit.**

```bash
git add polyoxide-binance/src/usdm/venue.rs polyoxide-binance/tests/venue_usdm.rs polyoxide-binance/src/usdm/mod.rs polyoxide-binance/Cargo.toml polyoxide-binance/src/usdm/README.md
git commit -m "feat(binance): USDⓈ-M through the market-data traits (Story 6.3)" -- polyoxide-binance/src/usdm/venue.rs polyoxide-binance/tests/venue_usdm.rs polyoxide-binance/src/usdm/mod.rs polyoxide-binance/Cargo.toml polyoxide-binance/src/usdm/README.md
```

### Task 3.4: Amendment A6-1, CLAUDE.md and the generated regions

**Files:**
- Create: `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md`
- Modify: `CLAUDE.md`, `README.md` (generated regions only)

- [ ] **Step 1: Record amendment A6-1 (D1).**

Create `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md` with:

```markdown
# Proposed spine amendments: Epic 6

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A6-1: `MarketData` reads one instrument by key; listing them all is the `Instruments` capability (amends AD-6)

- **Proposed by:** the user, 2026-10-16, while Epic 6 was planned (`docs/plans/2026-10-16-epic-6-venue-neutral-market-data.md`).
- **Current rule (AD-6, Story 6.3):** every venue product implements `MarketData` (instruments, quote, book).
- **Proposed rule:**
  - `MarketData` is `instrument(&MarketKey)`, `quote(&MarketKey)` and `book(&MarketKey)`, and every venue product implements it.
  - Listing every instrument a product has is a capability trait, `Instruments` (`instruments()`, with its `DynInstruments` wrapper), implemented only by a product that lists them in one request or a few: Binance USDⓈ-M (`exchangeInfo`) and Polymarket perps (`/v1/info/instruments`).
  - The Polymarket CLOB implements `MarketData` but not `Instruments`.
- **Why:**
  - **The CLOB's listing is not one call.** On 2026-10-16, `/sampling-markets` alone ran past 12 pages of 1,000 open markets, and `/markets` cursors ran past market id 5,000,000. A base trait every product must implement would make the generic call a dozen or more heavy requests on one venue and one on the others.
  - **AD-6's own rule fits.** An ability only some products offer cheaply is a capability trait, and no method returns `Unsupported`.
- **Follow-up in the code:** Story 6.3 ships the traits in this shape; Stories 6.3 and 6.4 implement `Instruments` for usdm and perps.
```

- [ ] **Step 2: Add the **Market-data traits** paragraph after **Market-data records**, and name `ClassifiedResultExt` and `fail_classified` under **Tags decide.****

Current (`CLAUDE.md`):

```markdown
**Wire enums** — Story 3.9 declared the wire enums of gamma (`polyoxide-polymarket/src/gamma/types.rs`), data v2 (`src/data/v2/types/common.rs` there), perps (`src/perps/types.rs` in `polyoxide-polymarket`), Binance (`usdm/types.rs`) and relay (`src/relay/types.rs` and `session_signers.rs` in `polyoxide-polymarket`) with `polyoxide-venue`'s macros: `wire_enum!` for a closed set the client sends (serde renames, `ALL`, `as_str(self) -> &'static str`, `Display`, and a `FromStr` whose error is `polyoxide_venue::UnknownVariant`, `"v" is not a valid X`, classed `InvalidRequest`), and `open_enum!` for a set the venue extends (`#[non_exhaustive]`, an `Other(String)` that keeps an unknown spelling verbatim, `ALL`, `as_str`, `from_wire`, an infallible `FromStr`, and string serde). Both take `$vis enum Name { Variant => "wire", .. }` with attributes on the enum and each variant; neither adds `#[non_exhaustive]` to a closed set, so data's pass it themselves. The expansions name `::serde`, so the calling crate depends on serde, and `wire_enum!`, which derives `::serde::Serialize` and `::serde::Deserialize`, also needs serde's `derive` feature; `open_enum!` implements both by hand and does not. They emit no specta code, since a `cfg(feature = "specta")` in an exported macro is evaluated in every calling crate: data's closed enums pass `#[cfg_attr(feature = "specta", derive(specta::Type))]`, and gamma and data give their open enums a `String` shape with one `polyoxide_venue::specta_as_string!(..)`. Perps and Binance keep separate `Interval` enums (D16). Other one-spelling-per-variant enums are still written by hand: clob's `OrderSide` (`src/clob/types.rs`) and `SortPosition`, `MultiMarketOrderBy` and `UserRewardMarketOrderBy` (`src/clob/api/rewards.rs`), relay's `WalletType`, Binance's `StreamPath` (`usdm/ws/mod.rs`), gamma's `ParentEntityType`, and data's v1 enums in `data/types.rs`, among them the `#[serde(other)] Unknown` ones (`TradeSide`, `ActivityType`, `ApprovalFeature`, `ApprovalStandard`, `ComboStatus`, `ComboLegStatus`).
```

Replace with:

```markdown
**Market-data traits** — the `market` feature also holds the traits a venue's products implement
so market data reads the same on every venue (AD-5, AD-6, amendment A6-1): `MarketData`
(`instrument(key)`, `quote(key)`, `book(key)`), which every product implements, and the
capability traits `Instruments` (the whole listing, only where one request or a few return it),
`Trades`, `Candles`, `Funding` and `EventGroups`, implemented only where offered; no method
answers "unsupported". Each has `Send + Sync` supertraits, methods returning
`impl Future<Output = Result<_, ClassifiedError>> + Send`, and a dynosaur wrapper
(`DynMarketData` and so on), held as `Arc<Dyn…<'static>>`; an implementation writes `async fn`
and converts its module's error with `?`, so the native clients keep their typed errors. A key of
another product is a `KeyError` (`InvalidRequest`), sent nowhere, and an instrument the venue
does not list is `NotListed` (`VenueRefusal`). The records they return are `Quote`, `Book` (bids
highest first, asks lowest first), `Trade` (its `TradeId` an opaque `Arc<str>`), `Candle` (a
venue that samples one price per interval gives only `close`), `FundingRate` (the rate accruing
has a `next_funding`, a settled one none) and `EventGroup` with its `EventOutcome`s; every list a
trait returns is oldest first, one request's worth. `CandleQuery` names a `CandleInterval` (one
minute to one week, each width every candle venue serves) and a span. Binance's `usdm::venue`
implements `MarketData`, `Instruments`, `Trades`, `Candles` and `Funding` on `Usdm`, with
`InstrumentExt`, `QuoteExt`, `BookExt`, `TradeExt`, `CandleExt` and `FundingExt`; its quote is a
5-level `depth` (weight 2) and its book an unlimited one (500 levels, weight 1), and
`tests/venue_usdm.rs` holds each method to the native client's values on the captured fixtures.
A live test unwraps a trait's `Result<_, ClassifiedError>` with test-support's
`ClassifiedResultExt::or_fail` (or `fail_classified`), since `ClassifiedError` does not implement
`Classify`; `ResultExt::or_fail` and it may be in scope together.

**Wire enums** — Story 3.9 declared the wire enums of gamma (`polyoxide-polymarket/src/gamma/types.rs`), data v2 (`src/data/v2/types/common.rs` there), perps (`src/perps/types.rs` in `polyoxide-polymarket`), Binance (`usdm/types.rs`) and relay (`src/relay/types.rs` and `session_signers.rs` in `polyoxide-polymarket`) with `polyoxide-venue`'s macros: `wire_enum!` for a closed set the client sends (serde renames, `ALL`, `as_str(self) -> &'static str`, `Display`, and a `FromStr` whose error is `polyoxide_venue::UnknownVariant`, `"v" is not a valid X`, classed `InvalidRequest`), and `open_enum!` for a set the venue extends (`#[non_exhaustive]`, an `Other(String)` that keeps an unknown spelling verbatim, `ALL`, `as_str`, `from_wire`, an infallible `FromStr`, and string serde). Both take `$vis enum Name { Variant => "wire", .. }` with attributes on the enum and each variant; neither adds `#[non_exhaustive]` to a closed set, so data's pass it themselves. The expansions name `::serde`, so the calling crate depends on serde, and `wire_enum!`, which derives `::serde::Serialize` and `::serde::Deserialize`, also needs serde's `derive` feature; `open_enum!` implements both by hand and does not. They emit no specta code, since a `cfg(feature = "specta")` in an exported macro is evaluated in every calling crate: data's closed enums pass `#[cfg_attr(feature = "specta", derive(specta::Type))]`, and gamma and data give their open enums a `String` shape with one `polyoxide_venue::specta_as_string!(..)`. Perps and Binance keep separate `Interval` enums (D16). Other one-spelling-per-variant enums are still written by hand: clob's `OrderSide` (`src/clob/types.rs`) and `SortPosition`, `MultiMarketOrderBy` and `UserRewardMarketOrderBy` (`src/clob/api/rewards.rs`), relay's `WalletType`, Binance's `StreamPath` (`usdm/ws/mod.rs`), gamma's `ParentEntityType`, and data's v1 enums in `data/types.rs`, among them the `#[serde(other)] Unknown` ones (`TradeSide`, `ActivityType`, `ApprovalFeature`, `ApprovalStandard`, `ComboStatus`, `ComboLegStatus`).
```

Current (`CLAUDE.md`):

```markdown
**Tags decide.** A live test that fails through `polyoxide-test-support` prints `polyoxide-class=<tag>` alone on a stderr line just before it panics, and that line is all `classify_failures.py` reads. Only the tag just above the log's final `panicked at` report decides, since one anywhere else belongs to a panic the test caught or a spawned task raised; a `real` or unknown tag anywhere makes the failure `real`, and so does no tag at all. `.or_fail(ctx)` (`ResultExt`), and `fail(ctx, &err)` for an error in a match arm or a source chain, take the tag from the error's `Class` and `is_fault()` (`Network`, `Unavailable` and `RateLimited` are `transient`; `Restricted` is `environmental` when it is not a fault, such as a 451 region block, and `real` when it is, such as a 418 ban the client earned; every other class is `real`), the credential loaders (`load_env`, `optional_env`, `keychain`) count an empty value as absent and fail through `Missing::or_auth_gated()` as `auth-gated`, and `environmental(reason)` and `transient(reason)` tag what no error value carries. A foreign error is wrapped in the venue's own error first (`ApiError::Network`, `UsdmWsError::from(WsError::from(..))`, `RtdsError::from(WsError::from(..))`, `ClobWsError::from(WsError::from(..))`, `SportsError::Transport { source: WsError::from(..) }`), and so is a raw socket's close frame, so it takes its tag from the socket table.
```

Replace with:

```markdown
**Tags decide.** A live test that fails through `polyoxide-test-support` prints `polyoxide-class=<tag>` alone on a stderr line just before it panics, and that line is all `classify_failures.py` reads. Only the tag just above the log's final `panicked at` report decides, since one anywhere else belongs to a panic the test caught or a spawned task raised; a `real` or unknown tag anywhere makes the failure `real`, and so does no tag at all. `.or_fail(ctx)` (`ResultExt`, and `ClassifiedResultExt` for a venue trait's `ClassifiedError`), and `fail(ctx, &err)` (`fail_classified` for a `ClassifiedError`) for an error in a match arm or a source chain, take the tag from the error's `Class` and `is_fault()` (`Network`, `Unavailable` and `RateLimited` are `transient`; `Restricted` is `environmental` when it is not a fault, such as a 451 region block, and `real` when it is, such as a 418 ban the client earned; every other class is `real`), the credential loaders (`load_env`, `optional_env`, `keychain`) count an empty value as absent and fail through `Missing::or_auth_gated()` as `auth-gated`, and `environmental(reason)` and `transient(reason)` tag what no error value carries. A foreign error is wrapped in the venue's own error first (`ApiError::Network`, `UsdmWsError::from(WsError::from(..))`, `RtdsError::from(WsError::from(..))`, `ClobWsError::from(WsError::from(..))`, `SportsError::Transport { source: WsError::from(..) }`), and so is a raw socket's close frame, so it takes its tag from the socket table.
```

- [ ] **Step 3: Regenerate, and run the story's checks.**

Run: `python3 scripts/gen_registry.py --write && python3 scripts/gen_registry.py --check && cargo fmt --all -- --check && cargo clippy -p polyoxide-venue -p polyoxide-test-support -p polyoxide-binance -p polyoxide --all-targets --all-features -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-venue -p polyoxide-test-support -p polyoxide-binance --all-features && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py`
Expected: every step clean.

- [ ] **Step 4: Commit.**

```bash
git add _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md CLAUDE.md README.md
git commit -m "docs: amendment A6-1 and CLAUDE.md's market-data trait rules (Story 6.3)" -- _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md CLAUDE.md README.md
```

## Story 4: Polymarket perps through the traits (Story 6.4)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-polymarket/src/perps/venue.rs`, `polyoxide-polymarket/src/perps/mod.rs`, `polyoxide-polymarket/src/perps/README.md`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/tests/venue_perps.rs`, `CLAUDE.md`
- **Interfaces:** exposes `polyoxide_polymarket::perps::venue::{InstrumentExt, BookExt, TradeExt, FundingExt}` and `MarketData`, `Instruments`, `Trades`, `Candles` and `Funding` on `Perps`. Uses Story 1's `shared::keys::{perps, instrument_id}` and Story 3's traits and records.
- **Verify:** `cargo test -p polyoxide-polymarket --no-default-features --features perps --test venue_perps && cargo test -p polyoxide-polymarket --no-default-features --features perps --lib perps::venue`

Instrument ids map to keys through perps' constructor, and fee tiers, risk tiers and price bounds ride in `InstrumentExt` (D13).

### Task 4.1: Perps through the traits

**Files:**
- Create: `polyoxide-polymarket/src/perps/venue.rs`, `polyoxide-polymarket/tests/venue_perps.rs`
- Modify: `polyoxide-polymarket/src/perps/mod.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/src/perps/README.md`

- [ ] **Step 1: Write the fixture test first.** Each test serves `tests/fixtures/perps/` and holds the trait's record to the native client's decode of the same body. Instrument 9 is an equity, the one category the captured fee schedule lists, and instrument 1 an index, which it does not.

Create `polyoxide-polymarket/tests/venue_perps.rs` with:

```rust
//! Perps through the venue-neutral traits, each held as its `Dyn` wrapper,
//! against mock servers serving the captured fixtures: every method returns
//! the values the native client decodes from the same body.

use std::sync::Arc;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_polymarket::perps::{
    types::{InstrumentId, Interval},
    venue::{BookExt, FundingExt, InstrumentExt, TradeExt},
    Perps,
};
use polyoxide_polymarket::shared::keys;
use polyoxide_test_support::fixtures;
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Class, DynCandles, DynFunding, DynInstruments,
    DynMarketData, DynTrades, Funding, Instruments, Level, MarketData, MarketKey, NotListed, Side,
    SizeUnit, TradeId, Trades, UnixMillis,
};
use rust_decimal::Decimal;
use serde_json::Value;

fn client(server: &ServerGuard) -> Perps {
    Perps::builder().base_url(server.url()).build().unwrap()
}

fn fixture(name: &str) -> String {
    fixtures!("perps").text(name)
}

fn key(id: u64) -> MarketKey {
    keys::perps(InstrumentId(id))
}

/// The captured instrument rows whose id is `id`, as the host answers a
/// filtered request.
fn instruments_with(id: u64) -> String {
    let rows: Vec<Value> = serde_json::from_str(&fixture("instruments")).unwrap();
    Value::Array(
        rows.into_iter()
            .filter(|row| row["instrument_id"] == id)
            .collect(),
    )
    .to_string()
}

/// A route answering `body` `hits` times, matched on its whole query string.
async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str, hits: usize) -> Mock {
    let query = if query.is_empty() {
        Matcher::Missing
    } else {
        Matcher::Exact(query.to_owned())
    };
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

#[tokio::test]
async fn a_dyn_market_data_holding_perps_returns_the_native_bbo() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/v1/info/bbo",
        "instrument_id=1",
        &fixture("bbo"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .bbo()
        .instrument_id(InstrumentId(1))
        .send()
        .await
        .unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let quote = venue.quote(&key(1)).await.unwrap();
    mock.assert_async().await;

    assert_eq!(quote.key, key(1));
    assert_eq!(
        quote.bid,
        Some(Level::new(native[0].bid_price, native[0].bid_quantity))
    );
    assert_eq!(
        quote.ask,
        Some(Level::new(native[0].ask_price, native[0].ask_quantity))
    );
    assert_eq!(quote.time, Some(UnixMillis(native[0].timestamp)));
}

#[tokio::test]
async fn an_instrument_the_host_sends_no_bbo_for_has_an_empty_quote() {
    let mut server = Server::new_async().await;
    let _mock = route(&mut server, "/v1/info/bbo", "instrument_id=999", "[]", 1).await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let quote = venue.quote(&key(999)).await.unwrap();
    assert_eq!((quote.bid, quote.ask, quote.time), (None, None, None));
}

#[tokio::test]
async fn book_is_the_native_book_best_first() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/v1/info/book",
        "instrument_id=1",
        &fixture("book"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.market().book(InstrumentId(1)).send().await.unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let book = venue.book(&key(1)).await.unwrap();
    mock.assert_async().await;

    let bids: Vec<Level> = native
        .bids
        .iter()
        .map(|l| Level::new(l.price, l.quantity))
        .collect();
    let asks: Vec<Level> = native
        .asks
        .iter()
        .map(|l| Level::new(l.price, l.quantity))
        .collect();
    assert_eq!(book.bids, bids);
    assert_eq!(book.asks, asks);
    assert!(
        book.bids.windows(2).all(|w| w[0].price > w[1].price),
        "bids best first"
    );
    assert!(
        book.asks.windows(2).all(|w| w[0].price < w[1].price),
        "asks best first"
    );
    assert_eq!(book.time, Some(UnixMillis(native.timestamp)));
    assert_eq!(
        book.extensions.get::<BookExt>().unwrap().sequence,
        Some(native.sequence)
    );
}

#[tokio::test]
async fn instrument_carries_the_rows_rules_and_its_categorys_fee_tiers() {
    let mut server = Server::new_async().await;
    // Instrument 9 is an equity, the one category the captured schedule has.
    let rows = route(
        &mut server,
        "/v1/info/instruments",
        "instrument_id=9",
        &instruments_with(9),
        2,
    )
    .await;
    let fees = route(&mut server, "/v1/info/fees", "", &fixture("fees"), 2).await;
    let client = client(&server);
    let native = client
        .exchange()
        .instruments()
        .instrument_id(InstrumentId(9))
        .send()
        .await
        .unwrap();
    let schedule = client.exchange().fees().send().await.unwrap();
    let row = &native[0];
    let entry = &schedule.fee_schedule[0];

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let instrument = venue.instrument(&key(9)).await.unwrap();
    rows.assert_async().await;
    fees.assert_async().await;

    assert_eq!(instrument.key, key(9));
    assert_eq!(instrument.name, row.symbol);
    assert_eq!(instrument.size_unit, SizeUnit::Contracts);
    assert_eq!(
        instrument.base_asset.as_deref(),
        Some(row.base_asset.as_str())
    );
    assert_eq!(
        instrument.quote_asset.as_deref(),
        Some(row.quote_asset.as_str())
    );
    assert_eq!(
        instrument.tick_size, None,
        "prices keep significant figures, not a tick"
    );
    assert_eq!(
        instrument.size_increment,
        Some(Decimal::new(1, row.quantity_decimals))
    );
    assert_eq!(instrument.min_notional, Some(row.min_notional));
    assert_eq!(
        instrument.tradable,
        row.close_only.map(|close_only| !close_only)
    );
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.price_bounds, Some(row.price_bounds));
    assert_eq!(ext.risk_tiers.as_ref(), Some(&row.risk_tiers));
    assert_eq!(ext.max_leverage, Some(row.max_leverage));
    assert_eq!(ext.price_decimals, Some(row.price_decimals));
    assert_eq!(entry.category, row.category);
    assert_eq!(ext.fee_tiers.as_ref(), Some(&entry.tiers));
    assert_eq!(ext.taker_fee_rate, Some(entry.taker_fee_rate));
    assert_eq!(ext.maker_fee_rate, Some(entry.maker_fee_rate));
}

#[tokio::test]
async fn an_instrument_whose_category_has_no_fee_schedule_has_no_fee_tiers() {
    let mut server = Server::new_async().await;
    // Instrument 1 is an index, which the captured schedule does not list.
    let _rows = route(
        &mut server,
        "/v1/info/instruments",
        "instrument_id=1",
        &instruments_with(1),
        1,
    )
    .await;
    let _fees = route(&mut server, "/v1/info/fees", "", &fixture("fees"), 1).await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let instrument = venue.instrument(&key(1)).await.unwrap();
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.fee_tiers, None);
    assert_eq!(ext.taker_fee_rate, None);
}

#[tokio::test]
async fn an_instrument_the_host_does_not_list_is_a_venue_refusal() {
    let mut server = Server::new_async().await;
    let _rows = route(
        &mut server,
        "/v1/info/instruments",
        "instrument_id=999",
        "[]",
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue.instrument(&key(999)).await.unwrap_err();
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert_eq!(
        err.source.downcast_ref::<NotListed>(),
        Some(&NotListed { key: key(999) })
    );
}

#[tokio::test]
async fn instruments_lists_every_row_once() {
    let mut server = Server::new_async().await;
    let rows = route(
        &mut server,
        "/v1/info/instruments",
        "",
        &fixture("instruments"),
        2,
    )
    .await;
    let fees = route(&mut server, "/v1/info/fees", "", &fixture("fees"), 1).await;
    let client = client(&server);
    let native = client.exchange().instruments().send().await.unwrap();

    let venue: Arc<DynInstruments<'static>> = DynInstruments::new_arc(client);
    let instruments = venue.instruments().await.unwrap();
    rows.assert_async().await;
    fees.assert_async().await;

    let keys: Vec<MarketKey> = instruments.iter().map(|i| i.key.clone()).collect();
    let expected: Vec<MarketKey> = native
        .iter()
        .map(|row| keys::perps(row.instrument_id))
        .collect();
    assert_eq!(keys, expected);
    assert_eq!(instruments.len(), 88);
}

#[tokio::test]
async fn trades_are_the_native_trades_oldest_first() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/v1/info/trades",
        "instrument_id=1",
        &fixture("trades"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .trades(InstrumentId(1))
        .send()
        .await
        .unwrap();

    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client);
    let trades = venue.trades(&key(1)).await.unwrap();
    mock.assert_async().await;

    assert_eq!(trades.len(), native.data.len());
    for (trade, row) in trades.iter().zip(native.data.iter().rev()) {
        assert_eq!(trade.id, Some(TradeId::new(row.trade_id.to_string())));
        assert_eq!(trade.price, row.price);
        assert_eq!(trade.size, row.quantity);
        assert_eq!(trade.time, UnixMillis(row.timestamp));
        assert_eq!(
            trade.taker_side,
            Some(match row.side.as_str() {
                "long" => Side::Buy,
                _ => Side::Sell,
            })
        );
        let ext = trade.extensions.get::<TradeExt>().unwrap();
        assert_eq!(ext.hash.as_deref(), Some(row.hash.as_str()));
        assert_eq!(ext.settlement, row.settlement);
    }
    assert!(
        trades.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}

#[tokio::test]
async fn candles_are_the_native_klines_from_start() {
    let mut server = Server::new_async().await;
    let query =
        "instrument_id=1&interval=1h&start_timestamp=1790675505663&end_timestamp=1790761905663";
    let mock = route(&mut server, "/v1/info/klines", query, &fixture("klines"), 2).await;
    let client = client(&server);
    let native = client
        .market()
        .klines(InstrumentId(1), Interval::H1, 1_790_675_505_663)
        .end(1_790_761_905_663)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_790_675_505_663))
        .end(UnixMillis(1_790_761_905_663));
    let candles = venue.candles(&key(1), &request).await.unwrap();
    mock.assert_async().await;

    assert_eq!(candles.len(), native.data.len());
    for (candle, row) in candles.iter().zip(&native.data) {
        assert_eq!(candle.open_time, UnixMillis(row.open_time));
        assert_eq!(candle.open, Some(row.open));
        assert_eq!(candle.high, Some(row.high));
        assert_eq!(candle.low, Some(row.low));
        assert_eq!(candle.close, row.close);
        assert_eq!(candle.volume, Some(row.volume));
        assert_eq!(candle.trades, Some(row.trades));
        assert_eq!(candle.notional, None);
    }
}

#[tokio::test]
async fn funding_rate_is_the_instruments_ticker() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/v1/info/tickers",
        "instrument_id=1",
        &fixture("tickers"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .tickers()
        .instrument_id(InstrumentId(1))
        .send()
        .await
        .unwrap();
    let row = native
        .iter()
        .find(|row| row.instrument_id == InstrumentId(1))
        .unwrap();
    assert!(native.len() > 1, "the host sends every instrument");

    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let rate = venue.funding_rate(&key(1)).await.unwrap();
    mock.assert_async().await;

    assert_eq!(rate.rate, row.funding_rate);
    assert_eq!(rate.time, UnixMillis(row.timestamp));
    assert_eq!(rate.next_funding, Some(UnixMillis(row.next_funding)));
    let ext = rate.extensions.get::<FundingExt>().unwrap();
    assert_eq!(ext.mark_price, Some(row.mark_price));
    assert_eq!(ext.index_price, Some(row.index_price));
    assert_eq!(ext.open_interest, Some(row.open_interest));
}

#[tokio::test]
async fn funding_history_is_the_native_history_oldest_first() {
    let mut server = Server::new_async().await;
    let query = "instrument_id=1&start_timestamp=1790675505663";
    let mock = route(
        &mut server,
        "/v1/info/funding",
        query,
        &fixture("funding"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .funding(InstrumentId(1))
        .start(1_790_675_505_663)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynFunding<'static>> = DynFunding::new_arc(client);
    let history = venue
        .funding_history(&key(1), UnixMillis(1_790_675_505_663))
        .await
        .unwrap();
    mock.assert_async().await;

    assert_eq!(history.len(), native.data.len());
    for (rate, row) in history.iter().zip(native.data.iter().rev()) {
        assert_eq!(rate.rate, row.funding_rate);
        assert_eq!(rate.time, UnixMillis(row.timestamp));
        assert_eq!(rate.next_funding, None);
    }
    assert!(
        history.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}
```

- [ ] **Step 2: Register it, and turn on `polyoxide-venue/market` with `perps` in place of `keys`.**

Current (`polyoxide-polymarket/Cargo.toml`):

```toml
    "polyoxide-venue/keys",
    "polyoxide-venue/decimal",
```

Replace with:

```toml
    "polyoxide-venue/market",
    "polyoxide-venue/decimal",
```

Current (`polyoxide-polymarket/Cargo.toml`):

```toml
name = "wire_agreement_perps"
required-features = ["perps"]
```

Replace with:

```toml
name = "venue_perps"
required-features = ["perps"]

[[test]]
name = "wire_agreement_perps"
required-features = ["perps"]
```

- [ ] **Step 3: Run it and watch it fail.**

Run: `cargo test -p polyoxide-polymarket --no-default-features --features perps --test venue_perps`
Expected: FAIL to compile: no module `venue` in `perps`.

- [ ] **Step 4: Implement the traits on `Perps`.** `trades` and `funding_history` reverse the host's newest-first pages; `funding_rate` filters `tickers`, which the host sends for every instrument.

Create `polyoxide-polymarket/src/perps/venue.rs` with:

```rust
//! Perps through the venue-neutral market-data traits.
//!
//! [`Perps`] implements [`MarketData`], [`Instruments`], [`Trades`],
//! [`Candles`] and [`Funding`] over its public `/v1/info/*` routes, keyed
//! `polymarket.perps:<instrument id>` by the constructors in
//! `shared::keys`. What only perps publishes (fee tiers, risk tiers, price
//! bounds and the rest) rides in this module's `<Record>Ext` types, one per
//! record, in each record's [`Extensions`].
//!
//! | Method | Routes |
//! | --- | --- |
//! | `instrument`, `instruments` | `/v1/info/instruments`, then `/v1/info/fees` |
//! | `quote` | `/v1/info/bbo` |
//! | `book` | `/v1/info/book` (100 levels, the host's default) |
//! | `trades` | `/v1/info/trades` |
//! | `candles` | `/v1/info/klines`, from `start` (up to 1000) |
//! | `funding_rate` | `/v1/info/tickers`, every instrument, filtered here |
//! | `funding_history` | `/v1/info/funding`, from `start` |
//!
//! Perps prices have no fixed tick: a non-integer price keeps at most five
//! significant figures, so [`Instrument::tick_size`] is `None` and the
//! decimal places are in [`InstrumentExt`]. Sizes are contracts.
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use polyoxide_polymarket::perps::{types::InstrumentId, Perps};
//! use polyoxide_polymarket::shared::keys;
//! use polyoxide_venue::{DynMarketData, MarketData};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Perps::new()?);
//! let quote = venue.quote(&keys::perps(InstrumentId(1))).await?;
//! println!("bid {:?} ask {:?}", quote.bid, quote.ask);
//! # Ok(())
//! # }
//! ```

use polyoxide_venue::{
    Book, Candle, CandleInterval, CandleQuery, Candles, Class, ClassifiedError, Extensions,
    Funding, FundingRate, Instrument, Instruments, Level, MarketData, MarketKey, NotListed, Quote,
    Side, SizeUnit, Trade, TradeId, Trades, UnixMillis,
};
use rust_decimal::Decimal;

use crate::perps::{
    api::exchange::{FeeTier, FeesInfo, Instrument as PerpsInstrument, RiskTier},
    types::{self, InstrumentCategory, InstrumentType, Interval},
    Perps,
};
use crate::shared::keys;

/// Perps' facts about an instrument, beside [`Instrument`]'s fields.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct InstrumentExt {
    /// Instrument type.
    pub instrument_type: Option<InstrumentType>,
    /// Category.
    pub category: Option<InstrumentCategory>,
    /// The symbol first-party interfaces show, when it differs.
    pub display_symbol: Option<String>,
    /// Whether new positions are refused.
    pub close_only: Option<bool>,
    /// Whether only isolated margin is supported.
    pub isolated_only: Option<bool>,
    /// Decimal places of a price. A non-integer price also keeps at most five
    /// significant figures.
    pub price_decimals: Option<u32>,
    /// Decimal places of a quantity.
    pub quantity_decimals: Option<u32>,
    /// How far an order's price may stray from the mark, as a fraction.
    pub price_bounds: Option<Decimal>,
    /// Liquidation fee rate.
    pub liquidation_fee: Option<Decimal>,
    /// Most open orders.
    pub max_order_count: Option<u32>,
    /// Largest market-order notional, in USD.
    pub max_market_notional: Option<Decimal>,
    /// Largest limit-order notional, in USD.
    pub max_limit_notional: Option<Decimal>,
    /// Highest leverage.
    pub max_leverage: Option<u32>,
    /// Leverage caps by position size.
    pub risk_tiers: Option<Vec<RiskTier>>,
    /// Funding interval, such as `1h`.
    pub funding_interval: Option<String>,
    /// Base taker fee rate of the instrument's type and category, when the
    /// fee schedule has one.
    pub taker_fee_rate: Option<Decimal>,
    /// Base maker fee rate, likewise.
    pub maker_fee_rate: Option<Decimal>,
    /// The fee schedule's volume tiers for the instrument's type and
    /// category.
    pub fee_tiers: Option<Vec<FeeTier>>,
}

/// Perps' facts about a book, beside [`Book`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BookExt {
    /// The book's sequence number.
    pub sequence: Option<u64>,
}

/// Perps' facts about a trade, beside [`Trade`]'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct TradeExt {
    /// The transaction hash.
    pub hash: Option<String>,
    /// Whether it was a settlement trade. Undocumented.
    pub settlement: Option<bool>,
}

/// Perps' facts about the funding rate now accruing, beside
/// [`FundingRate`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct FundingExt {
    /// Mark price.
    pub mark_price: Option<Decimal>,
    /// Index price.
    pub index_price: Option<Decimal>,
    /// Last traded price.
    pub last_price: Option<Decimal>,
    /// Mid price.
    pub mid_price: Option<Decimal>,
    /// Open interest, in contracts.
    pub open_interest: Option<Decimal>,
}

/// The instrument a row describes, with its type and category's fees.
fn instrument(row: &PerpsInstrument, fees: &FeesInfo) -> Instrument {
    let mut instrument = Instrument::new(
        keys::perps(row.instrument_id),
        row.symbol.clone(),
        SizeUnit::Contracts,
    );
    instrument.base_asset = Some(row.base_asset.clone());
    instrument.quote_asset = Some(row.quote_asset.clone());
    // `None` past the 28 places a decimal holds, which no instrument uses.
    instrument.size_increment = Decimal::try_new(1, row.quantity_decimals).ok();
    instrument.min_notional = Some(row.min_notional);
    instrument.tradable = row.close_only.map(|close_only| !close_only);
    let schedule = fees.fee_schedule.iter().find(|entry| {
        entry.instrument_type == row.instrument_type && entry.category == row.category
    });
    instrument.extensions = Extensions::of(InstrumentExt {
        instrument_type: Some(row.instrument_type),
        category: Some(row.category),
        display_symbol: row.display_symbol.clone(),
        close_only: row.close_only,
        isolated_only: Some(row.isolated_only),
        price_decimals: Some(row.price_decimals),
        quantity_decimals: Some(row.quantity_decimals),
        price_bounds: Some(row.price_bounds),
        liquidation_fee: Some(row.liquidation_fee),
        max_order_count: Some(row.max_order_count),
        max_market_notional: Some(row.max_market_notional),
        max_limit_notional: Some(row.max_limit_notional),
        max_leverage: Some(row.max_leverage),
        risk_tiers: Some(row.risk_tiers.clone()),
        funding_interval: Some(row.funding_interval.clone()),
        taker_fee_rate: schedule.map(|entry| entry.taker_fee_rate),
        maker_fee_rate: schedule.map(|entry| entry.maker_fee_rate),
        fee_tiers: schedule.map(|entry| entry.tiers.clone()),
    });
    instrument
}

/// A best price and its quantity, or no level when the quantity is zero.
fn side(price: Decimal, quantity: Decimal) -> Option<Level> {
    (!quantity.is_zero()).then_some(Level::new(price, quantity))
}

fn level(level: &types::Level) -> Level {
    Level::new(level.price, level.quantity)
}

/// The kline width a candle interval is.
fn interval(interval: CandleInterval) -> Result<Interval, ClassifiedError> {
    Ok(match interval {
        CandleInterval::Minute1 => Interval::M1,
        CandleInterval::Minute5 => Interval::M5,
        CandleInterval::Minute15 => Interval::M15,
        CandleInterval::Hour1 => Interval::H1,
        CandleInterval::Hour4 => Interval::H4,
        CandleInterval::Day1 => Interval::D1,
        CandleInterval::Week1 => Interval::W1,
        other => {
            return Err(ClassifiedError::new(
                Class::InvalidRequest,
                format!("perps serves no {other:?} klines"),
            ))
        }
    })
}

impl MarketData for Perps {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let rows = self
            .exchange()
            .instruments()
            .instrument_id(id)
            .send()
            .await?;
        let Some(row) = rows.iter().find(|row| row.instrument_id == id) else {
            return Err(NotListed { key: key.clone() }.into());
        };
        let fees = self.exchange().fees().send().await?;
        Ok(instrument(row, &fees))
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let rows = self.market().bbo().instrument_id(id).send().await?;
        let mut quote = Quote::new(key.clone());
        // An instrument the host does not know has no row, as its book is empty.
        if let Some(row) = rows.iter().find(|row| row.instrument_id == id) {
            quote.bid = side(row.bid_price, row.bid_quantity);
            quote.ask = side(row.ask_price, row.ask_quantity);
            quote.time = Some(UnixMillis(row.timestamp));
        }
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let native = self.market().book(id).send().await?;
        let mut book = Book::new(key.clone());
        book.bids = native.bids.iter().map(level).collect();
        book.asks = native.asks.iter().map(level).collect();
        book.time = Some(UnixMillis(native.timestamp));
        book.extensions = Extensions::of(BookExt {
            sequence: Some(native.sequence),
        });
        Ok(book)
    }
}

impl Instruments for Perps {
    async fn instruments(&self) -> Result<Vec<Instrument>, ClassifiedError> {
        let rows = self.exchange().instruments().send().await?;
        let fees = self.exchange().fees().send().await?;
        Ok(rows.iter().map(|row| instrument(row, &fees)).collect())
    }
}

impl Trades for Perps {
    async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let page = self.market().trades(id).send().await?;
        // The host sends the newest first.
        Ok(page
            .data
            .iter()
            .rev()
            .map(|row| {
                let mut trade = Trade::new(
                    key.clone(),
                    row.price,
                    row.quantity,
                    UnixMillis(row.timestamp),
                );
                trade.id = Some(TradeId::new(row.trade_id.to_string()));
                trade.taker_side = Some(match row.side {
                    types::Side::Long => Side::Buy,
                    types::Side::Short => Side::Sell,
                });
                trade.extensions = Extensions::of(TradeExt {
                    hash: Some(row.hash.clone()),
                    settlement: row.settlement,
                });
                trade
            })
            .collect())
    }
}

impl Candles for Perps {
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let mut request = self
            .market()
            .klines(id, interval(query.interval)?, query.start.0);
        if let Some(end) = query.end {
            request = request.end(end.0);
        }
        let page = request.send().await?;
        Ok(page
            .data
            .iter()
            .map(|row| {
                let mut candle = Candle::new(
                    key.clone(),
                    query.interval,
                    UnixMillis(row.open_time),
                    row.close,
                );
                candle.open = Some(row.open);
                candle.high = Some(row.high);
                candle.low = Some(row.low);
                candle.volume = Some(row.volume);
                candle.trades = Some(row.trades);
                candle
            })
            .collect())
    }
}

impl Funding for Perps {
    async fn funding_rate(&self, key: &MarketKey) -> Result<FundingRate, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        // The host ignores the filter and sends every instrument.
        let rows = self.market().tickers().instrument_id(id).send().await?;
        let Some(row) = rows.iter().find(|row| row.instrument_id == id) else {
            return Err(NotListed { key: key.clone() }.into());
        };
        let mut rate = FundingRate::new(key.clone(), row.funding_rate, UnixMillis(row.timestamp));
        rate.next_funding = Some(UnixMillis(row.next_funding));
        rate.extensions = Extensions::of(FundingExt {
            mark_price: Some(row.mark_price),
            index_price: Some(row.index_price),
            last_price: Some(row.last_price),
            mid_price: Some(row.mid_price),
            open_interest: Some(row.open_interest),
        });
        Ok(rate)
    }

    async fn funding_history(
        &self,
        key: &MarketKey,
        start: UnixMillis,
    ) -> Result<Vec<FundingRate>, ClassifiedError> {
        let id = keys::instrument_id(key)?;
        let page = self.market().funding(id).start(start.0).send().await?;
        // The host sends the newest first.
        Ok(page
            .data
            .iter()
            .rev()
            .map(|row| FundingRate::new(key.clone(), row.funding_rate, UnixMillis(row.timestamp)))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_candle_interval_is_a_kline_width() {
        let widths: Vec<&str> = CandleInterval::ALL
            .iter()
            .map(|each| interval(*each).unwrap().as_str())
            .collect();
        assert_eq!(widths, ["1m", "5m", "15m", "1h", "4h", "1d", "1w"]);
    }

    #[test]
    fn a_side_with_no_quantity_is_no_level() {
        assert_eq!(side(Decimal::ONE, Decimal::ZERO), None);
        assert_eq!(
            side(Decimal::ONE, Decimal::TWO),
            Some(Level::new(Decimal::ONE, Decimal::TWO))
        );
    }
}
```

- [ ] **Step 5: Declare the module.**

Current (`polyoxide-polymarket/src/perps/mod.rs`):

```rust
pub mod types;
#[cfg(feature = "perps-ws")]
pub mod ws;
```

Replace with:

```rust
pub mod types;
pub mod venue;
#[cfg(feature = "perps-ws")]
pub mod ws;
```

- [ ] **Step 6: Run the tests again, in perps' own build and the full one.**

Run: `cargo test -p polyoxide-polymarket --no-default-features --features perps --test venue_perps && cargo test -p polyoxide-polymarket --no-default-features --features perps --lib perps::venue && cargo test -p polyoxide-polymarket --all-features --test venue_perps && cargo check -p polyoxide-polymarket --no-default-features --features perps-ws`
Expected: PASS: 11 fixture tests (a few seconds: perps' limiter paces them) and 2 unit tests; `perps-ws` builds with no warning.

- [ ] **Step 7: Add the README section.**

Current (`polyoxide-polymarket/src/perps/README.md`):

```markdown
## Streaming

With the `perps-ws` feature, the public channels stream over one connection.
```

Replace with:

```markdown
## Through the venue-neutral traits

`Perps` implements `polyoxide-venue`'s `MarketData`, `Instruments`, `Trades`,
`Candles` and `Funding`, keyed `polymarket.perps:<instrument id>`
(`shared::keys::perps`), so code written against those traits reads perps as it
reads Binance. Fee tiers, risk tiers, price bounds and perps' other facts ride
in `perps::venue`'s `<Record>Ext` types.
The `perps::venue` module's documentation has an example.

## Streaming

With the `perps-ws` feature, the public channels stream over one connection.
```

- [ ] **Step 8: Commit.**

```bash
git add polyoxide-polymarket/src/perps/venue.rs polyoxide-polymarket/tests/venue_perps.rs polyoxide-polymarket/src/perps/mod.rs polyoxide-polymarket/Cargo.toml polyoxide-polymarket/src/perps/README.md
git commit -m "feat(perps): perps through the market-data traits (Story 6.4)" -- polyoxide-polymarket/src/perps/venue.rs polyoxide-polymarket/tests/venue_perps.rs polyoxide-polymarket/src/perps/mod.rs polyoxide-polymarket/Cargo.toml polyoxide-polymarket/src/perps/README.md
```

### Task 4.2: CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Say what `perps::venue` does under **Perps public routes**.**

Current (`CLAUDE.md`):

```markdown
Three test files of `polyoxide-polymarket` hold it in place on the pattern of Data v2:
```

Replace with:

```markdown
`perps::venue` implements `polyoxide-venue`'s `MarketData`, `Instruments`, `Trades`, `Candles` and
`Funding` on `Perps` (Story 6.4), keyed by `shared::keys::perps`: an instrument is its
`/v1/info/instruments` row plus its type and category's `/v1/info/fees` entry (two requests),
with no `tick_size`, since a non-integer price keeps five significant figures, and its fee
tiers, risk tiers, price bounds and decimals in `InstrumentExt`; a quote is the `bbo` row, a side
of zero quantity none and an instrument with no row an empty quote; the funding rate is the
instrument's row of `tickers`, which the host sends for every instrument; `trades` and
`funding_history` reverse the host's newest-first pages. `tests/venue_perps.rs` holds each method
to the native client's values on the captured fixtures.
Three test files of `polyoxide-polymarket` hold it in place on the pattern of Data v2:
```

- [ ] **Step 2: Run the story's checks.**

Run: `cargo fmt --all -- --check && cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-polymarket --all-features && python3 scripts/gen_registry.py --check`
Expected: all clean.

- [ ] **Step 3: Commit.**

```bash
git commit -m "docs: CLAUDE.md says what perps::venue implements (Story 6.4)" -- CLAUDE.md
```

## Story 5: The Polymarket CLOB through the traits (Story 6.5)

- **Developer:** intermediate-dev
- **Files:** `scripts/capture_polymarket_clob.py`, `polyoxide-polymarket/tests/fixtures/clob/venue/PROVENANCE.md`, `polyoxide-polymarket/tests/fixtures/clob/venue/book.json`, `polyoxide-polymarket/tests/fixtures/clob/venue/markets_by_token.json`, `polyoxide-polymarket/tests/fixtures/clob/venue/market.json`, `polyoxide-polymarket/tests/fixtures/clob/venue/prices_history.json`, `polyoxide-polymarket/tests/fixtures/clob/venue/trades.json`, `polyoxide-polymarket/tests/fixtures/clob/venue/gamma_markets.json`, `polyoxide-polymarket/tests/fixtures/clob/venue/gamma_event.json`, `polyoxide-polymarket/src/clob/error.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/client.rs`, `polyoxide-polymarket/src/clob/venue.rs`, `polyoxide-polymarket/src/clob/mod.rs`, `polyoxide-polymarket/src/clob/README.md`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/tests/venue_clob.rs`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `CLAUDE.md`
- **Interfaces:** exposes `ClobBuilder::data` and `ClobError::Data` (`data`); `polyoxide_polymarket::clob::venue::{InstrumentExt, QuoteExt, BookExt}`, with `TradeExt` (`data`) and `EventGroupExt` (`gamma`); `MarketData` and `Candles` on `Clob`, `Trades` with `data` and `EventGroups` with `gamma`. Uses Story 1's `shared::keys::{clob, token_id, TokenId}` and Story 3's traits and records.
- **Verify:** `cargo test -p polyoxide-polymarket --all-features --test venue_clob --test mock_clob --lib && for f in clob clob,data clob,gamma; do cargo clippy -p polyoxide-polymarket --no-default-features --features $f --all-targets -- -D warnings || exit 1; done`

Each key names one outcome token, and its book is bids and asks in that outcome's own price terms (AD-7). `Trades` comes through the Data API (D2), and `EventGroups` through Gamma (D14). Adding `ClobError::Data` moves lines `docs/MUTANTS.md` cites, so Task 5.3 moves the ledger and proves its three clob rows again.

### Task 5.1: Capture the fixtures

The venue tests serve bodies from three hosts for one live outcome token. The fixtures are captured, not given: the tests compare the trait's records with the native clients' decode of whatever was captured, and the script only keeps a token that has what the tests read (two levels on each side of its book, public trades of it and of another outcome, a price history, and a Gamma event).

**Files:**
- Create: `scripts/capture_polymarket_clob.py`, and the fixtures it writes under `polyoxide-polymarket/tests/fixtures/clob/venue/`

- [ ] **Step 1: Create the capture script.**

Create `scripts/capture_polymarket_clob.py` with:

```python
#!/usr/bin/env python3
"""Capture the CLOB, Data and Gamma responses the clob module's venue tests
serve, for one live outcome token.

The token is chosen live: the first outcome of the first open neg-risk market
on /sampling-markets whose book has at least two levels on each side, whose
market has public trades of that outcome and of another, and whose Gamma row
names its event. Requests are spaced out. Lists are trimmed, as noted in
PROVENANCE.md; every key of every object kept is kept whole.

Usage:
    python3 scripts/capture_polymarket_clob.py polyoxide-polymarket/tests/fixtures/clob/venue

Writes `<name>.json` per capture plus `PROVENANCE.md` listing each URL and the
capture time. Re-run to refresh; review the diff before committing.
"""
import sys
import time
from pathlib import Path

import capture_common

CLOB = "https://clob.polymarket.com"
DATA = "https://data-api.polymarket.com"
GAMMA = "https://gamma-api.polymarket.com"
PAUSE_SECONDS = 0.5
BOOK_LEVELS = 10
EVENT_MARKETS = 3
TRADES_KEPT = 30
HISTORY_SECONDS = 3 * 24 * 60 * 60


def get(url, **params):
    return capture_common.get(url, params=params, pause=PAUSE_SECONDS)


def candidate(market):
    """The first outcome token of an open neg-risk market with a book, or None."""
    if not (market.get("neg_risk") and market.get("accepting_orders") and market.get("enable_order_book")):
        return None
    tokens = [t for t in market.get("tokens") or [] if t.get("token_id")]
    return tokens[0]["token_id"] if tokens else None


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    captured = []

    def keep(name, reply, body, note):
        capture_common.write_json(out / f"{name}.json", body)
        captured.append((name, reply.url, note))

    sampling = capture_common.require_ok(get(f"{CLOB}/sampling-markets"), "sampling-markets")
    chosen = None
    for market in sampling["data"]:
        token = candidate(market)
        if token is None:
            continue
        book = get(f"{CLOB}/book", token_id=token)
        if book.status != 200 or len(book.body.get("bids", [])) < 2 or len(book.body.get("asks", [])) < 2:
            continue
        trades = get(f"{DATA}/trades", market=market["condition_id"], takerOnly="true")
        rows = trades.body if trades.status == 200 and isinstance(trades.body, list) else []
        if not any(r.get("asset") == token for r in rows) or not any(r.get("asset") != token for r in rows):
            continue
        gamma = get(f"{GAMMA}/markets", clob_token_ids=token)
        if gamma.status != 200 or not gamma.body or not gamma.body[0].get("events"):
            continue
        chosen = (market, token, book, trades, rows, gamma)
        break
    if chosen is None:
        sys.exit("no open neg-risk market on the first sampling page has a two-sided book, trades of two outcomes and an event")
    market, token, book, trades, rows, gamma = chosen
    condition = market["condition_id"]

    by_token = get(f"{CLOB}/markets-by-token/{token}")
    keep("markets_by_token", by_token, capture_common.require_ok(by_token, "markets-by-token"), "as fetched")
    detail = get(f"{CLOB}/markets/{condition}")
    keep("market", detail, capture_common.require_ok(detail, "market"), "as fetched")

    # The CLOB sends each side worst first, so the levels nearest the touch are last.
    body = dict(book.body)
    body["bids"] = body["bids"][-BOOK_LEVELS:]
    body["asks"] = body["asks"][-BOOK_LEVELS:]
    keep("book", book, body, f"each side to its {BOOK_LEVELS} levels nearest the touch, in wire order")

    now = int(time.time())
    history = get(f"{CLOB}/prices-history", market=token, fidelity=60, startTs=now - HISTORY_SECONDS, endTs=now)
    points = capture_common.require_ok(history, "prices-history")
    if not points.get("history"):
        sys.exit(f"{history.url}: no price history")
    keep("prices_history", history, points, "as fetched")

    kept, ours, theirs = [], 0, 0
    for row in rows:
        if len(kept) >= TRADES_KEPT and ours >= 3 and theirs >= 1:
            break
        kept.append(row)
        ours += row.get("asset") == token
        theirs += row.get("asset") != token
    keep("trades", trades, kept, f"the first {len(kept)} rows, newest first, {ours} of the token and {theirs} of another")

    keep("gamma_markets", gamma, gamma.body, "as fetched")
    event_id = gamma.body[0]["events"][0]["id"]
    event = get(f"{GAMMA}/events/{event_id}")
    event_body = dict(capture_common.require_ok(event, "event"))
    markets = event_body.get("markets") or []
    ours_first = [m for m in markets if token in (m.get("clobTokenIds") or "")]
    others = [m for m in markets if token not in (m.get("clobTokenIds") or "")]
    event_body["markets"] = (ours_first + others)[:EVENT_MARKETS]
    keep("gamma_event", event, event_body, f"`markets` to the token's market and the next {EVENT_MARKETS - 1}")

    stamp = capture_common.stamp()
    lines = [
        "# CLOB venue fixtures",
        "",
        f"Captured {stamp} by `scripts/capture_polymarket_clob.py` from the live hosts, for the",
        f"outcome token `{token}` of the market `{condition}`. The clob module's venue tests",
        "(`tests/venue_clob.rs`) serve them from one mock server. Lists are trimmed as noted.",
        "",
        *capture_common.provenance_table(
            ("Fixture", "Request", "Kept"),
            [(f"`{name}.json`", f"`{url}`", note) for name, url, note in captured]),
    ]
    capture_common.write_provenance(out, lines)
    print(f"captured {len(captured)} fixtures to {out}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run it.** Run it as a file, never with `python3 -I` (CLAUDE.md: that keeps `scripts/` off `sys.path`).

Run: `python3 scripts/capture_polymarket_clob.py polyoxide-polymarket/tests/fixtures/clob/venue`
Expected: `captured 7 fixtures to polyoxide-polymarket/tests/fixtures/clob/venue`, and a `PROVENANCE.md` naming the token, its market and each URL. If it exits with "no open neg-risk market on the first sampling page …", re-run it later; do not loosen its checks.

- [ ] **Step 3: Commit.**

```bash
git add scripts/capture_polymarket_clob.py polyoxide-polymarket/tests/fixtures/clob/venue
git commit -m "test(clob): capture the CLOB, Data and Gamma bodies the venue tests serve (Story 6.5)" -- scripts/capture_polymarket_clob.py polyoxide-polymarket/tests/fixtures/clob/venue
```

### Task 5.2: The CLOB through the traits

**Files:**
- Create: `polyoxide-polymarket/src/clob/venue.rs`, `polyoxide-polymarket/tests/venue_clob.rs`
- Modify: `polyoxide-polymarket/src/clob/error.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/client.rs`, `polyoxide-polymarket/src/clob/mod.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/src/clob/README.md`

- [ ] **Step 1: Write the fixture test first.** One mock server stands in for all three hosts. `book_holds_every_native_level_best_first` also asserts the CLOB sent its bids worst first, so the sort is proven needed; `event_group_holds_every_v1_outcome_of_the_tokens_event` keeps only `v1` markets, as the trait does.

Create `polyoxide-polymarket/tests/venue_clob.rs` with:

```rust
//! The CLOB through the venue-neutral traits, each held as its `Dyn` wrapper,
//! against one mock server serving the captured CLOB, Data and Gamma
//! fixtures: every method returns the values the native clients decode from
//! the same bodies.

use std::str::FromStr;
use std::sync::Arc;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_polymarket::clob::{
    venue::{BookExt, EventGroupExt, InstrumentExt, QuoteExt, TradeExt},
    Clob, ClobBuilder, PricesHistoryQuery,
};
use polyoxide_polymarket::data::{types::TradeSide, DataApi};
use polyoxide_polymarket::gamma::{types::ProtocolVersion, Gamma};
use polyoxide_polymarket::shared::keys::{self, TokenId};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Class, DynCandles, DynEventGroups, DynMarketData,
    DynTrades, EventGroups, Level, MarketData, MarketKey, Side, SizeUnit, Trades, UnixMillis,
};
use rust_decimal::Decimal;
use serde_json::Value;

/// One client of each host, all pointed at the one mock server.
fn client(server: &ServerGuard) -> Clob {
    ClobBuilder::new()
        .base_url(server.url())
        .gamma(Gamma::builder().base_url(server.url()).build().unwrap())
        .data(DataApi::builder().base_url(server.url()).build().unwrap())
        .build()
        .unwrap()
}

fn fixture(name: &str) -> String {
    fixtures!("clob/venue").text(name)
}

fn json(name: &str) -> Value {
    fixtures!("clob/venue").json(name)
}

/// The captured outcome token.
fn token() -> String {
    json("book")["asset_id"].as_str().unwrap().to_owned()
}

/// The captured token's market.
fn condition() -> String {
    json("markets_by_token")["condition_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn key() -> MarketKey {
    keys::clob(&TokenId::new(token()).unwrap())
}

/// A number as the trait reads it: through its shortest decimal spelling.
fn decimal(value: f64) -> Decimal {
    Decimal::from_str(&value.to_string()).unwrap()
}

/// A route answering `body` `hits` times, matched on its whole query string.
async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str, hits: usize) -> Mock {
    let query = if query.is_empty() {
        Matcher::Missing
    } else {
        Matcher::Exact(query.to_owned())
    };
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

#[tokio::test]
async fn a_dyn_market_data_holding_the_clob_returns_the_books_best_levels() {
    let mut server = Server::new_async().await;
    let query = format!("token_id={}", token());
    let mock = route(&mut server, "/book", &query, &fixture("book"), 2).await;
    let client = client(&server);
    let native = client.markets().order_book(token()).send().await.unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let quote = venue.quote(&key()).await.unwrap();
    mock.assert_async().await;

    let best_bid = native.bids.iter().max_by_key(|l| l.price).unwrap();
    let best_ask = native.asks.iter().min_by_key(|l| l.price).unwrap();
    assert_eq!(quote.key, key());
    assert_eq!(quote.bid, Some(Level::new(best_bid.price, best_bid.size)));
    assert_eq!(quote.ask, Some(Level::new(best_ask.price, best_ask.size)));
    assert!(
        quote.bid.unwrap().price < quote.ask.unwrap().price,
        "an uncrossed book"
    );
    assert_eq!(
        quote.time,
        Some(UnixMillis(native.timestamp.parse().unwrap()))
    );
    assert_eq!(
        quote.extensions.get::<QuoteExt>().unwrap().last_trade_price,
        native
            .last_trade_price
            .as_deref()
            .map(|p| Decimal::from_str(p).unwrap())
    );
}

#[tokio::test]
async fn book_holds_every_native_level_best_first() {
    let mut server = Server::new_async().await;
    let query = format!("token_id={}", token());
    let mock = route(&mut server, "/book", &query, &fixture("book"), 2).await;
    let client = client(&server);
    let native = client.markets().order_book(token()).send().await.unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let book = venue.book(&key()).await.unwrap();
    mock.assert_async().await;

    let mut bids: Vec<Level> = native
        .bids
        .iter()
        .map(|l| Level::new(l.price, l.size))
        .collect();
    let mut asks: Vec<Level> = native
        .asks
        .iter()
        .map(|l| Level::new(l.price, l.size))
        .collect();
    bids.sort_by_key(|level| std::cmp::Reverse(level.price));
    asks.sort_by_key(|level| level.price);
    assert!(
        bids.len() >= 2 && asks.len() >= 2,
        "the capture keeps two levels a side"
    );
    assert_ne!(
        native.bids[0].price, bids[0].price,
        "the CLOB sends bids worst first, so the trait must reorder them"
    );
    assert_eq!(book.bids, bids);
    assert_eq!(book.asks, asks);
    let ext = book.extensions.get::<BookExt>().unwrap();
    assert_eq!(ext.condition_id.as_deref(), Some(native.market.as_str()));
    assert_eq!(ext.hash.as_deref(), Some(native.hash.as_str()));
    assert_eq!(ext.neg_risk, native.neg_risk);
    assert_eq!(
        ext.tick_size,
        native
            .tick_size
            .as_deref()
            .map(|t| Decimal::from_str(t).unwrap())
    );
}

#[tokio::test]
async fn instrument_is_the_outcome_of_its_market() {
    let mut server = Server::new_async().await;
    let by_token = route(
        &mut server,
        &format!("/markets-by-token/{}", token()),
        "",
        &fixture("markets_by_token"),
        1,
    )
    .await;
    let market_route = route(
        &mut server,
        &format!("/markets/{}", condition()),
        "",
        &fixture("market"),
        2,
    )
    .await;
    let client = client(&server);
    let market = client.markets().get(condition()).send().await.unwrap();
    let outcome = market
        .tokens
        .iter()
        .find(|t| t.token_id.as_deref() == Some(token().as_str()))
        .unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let instrument = venue.instrument(&key()).await.unwrap();
    by_token.assert_async().await;
    market_route.assert_async().await;

    assert_eq!(instrument.key, key());
    assert_eq!(instrument.size_unit, SizeUnit::Shares);
    assert!(
        instrument.name.contains(&outcome.outcome),
        "{}",
        instrument.name
    );
    assert!(
        instrument
            .name
            .contains(market.question.as_deref().unwrap()),
        "{}",
        instrument.name
    );
    assert_eq!(instrument.tick_size, market.minimum_tick_size.map(decimal));
    assert_eq!(instrument.min_size, market.minimum_order_size.map(decimal));
    assert_eq!(instrument.tradable, market.accepting_orders);
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(
        ext.condition_id.as_deref(),
        Some(market.condition_id.as_str())
    );
    assert_eq!(ext.outcome.as_deref(), Some(outcome.outcome.as_str()));
    assert_eq!(ext.neg_risk, market.neg_risk);
    assert_eq!(ext.closed, Some(market.closed));
}

#[tokio::test]
async fn a_token_the_clob_does_not_know_is_its_404() {
    let mut server = Server::new_async().await;
    let unknown = "1";
    let _mock = server
        .mock("GET", format!("/markets-by-token/{unknown}").as_str())
        .with_status(404)
        .with_body(r#"{"error":"market not found"}"#)
        .create_async()
        .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue
        .instrument(&keys::clob(&TokenId::new(unknown).unwrap()))
        .await
        .unwrap_err();
    assert!(
        matches!(err.class, Class::VenueRefusal { .. }),
        "{:?}",
        err.class
    );
}

#[tokio::test]
async fn candles_are_the_native_price_history_one_price_an_interval() {
    let mut server = Server::new_async().await;
    let (start, end) = (1_791_902_391_u64, 1_792_161_591_u64);
    let query = format!("market={}&fidelity=60&startTs={start}&endTs={end}", token());
    let mock = route(
        &mut server,
        "/prices-history",
        &query,
        &fixture("prices_history"),
        2,
    )
    .await;
    let client = client(&server);
    let params = PricesHistoryQuery {
        interval: None,
        fidelity: Some(60),
        start_ts: Some(start as i64),
        end_ts: Some(end as i64),
    };
    let native = client
        .markets()
        .prices_history_with(token(), &params)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(start * 1_000))
        .end(UnixMillis(end * 1_000));
    let candles = venue.candles(&key(), &request).await.unwrap();
    mock.assert_async().await;

    assert_eq!(candles.len(), native.history.len());
    assert!(!candles.is_empty());
    for (candle, point) in candles.iter().zip(&native.history) {
        assert_eq!(candle.interval, CandleInterval::Hour1);
        assert_eq!(candle.open_time, UnixMillis(point.timestamp as u64 * 1_000));
        assert_eq!(candle.close, decimal(point.price));
        assert_eq!((candle.open, candle.high, candle.low), (None, None, None));
        assert_eq!(candle.volume, None);
    }
}

#[tokio::test]
async fn trades_are_the_data_apis_public_trades_of_the_token_oldest_first() {
    let mut server = Server::new_async().await;
    let by_token = route(
        &mut server,
        &format!("/markets-by-token/{}", token()),
        "",
        &fixture("markets_by_token"),
        1,
    )
    .await;
    let query = format!("market={}&takerOnly=true", condition());
    let trades_route = route(&mut server, "/trades", &query, &fixture("trades"), 2).await;
    let client = client(&server);
    let native = DataApi::builder()
        .base_url(server.url())
        .build()
        .unwrap()
        .trades()
        .list()
        .market([condition()])
        .taker_only(true)
        .send()
        .await
        .unwrap();
    let expected: Vec<_> = native
        .iter()
        .rev()
        .filter(|row| row.asset == token())
        .collect();
    assert!(
        native.iter().any(|row| row.asset != token()),
        "the capture holds another outcome's trades, which the trait leaves out"
    );

    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client);
    let trades = venue.trades(&key()).await.unwrap();
    by_token.assert_async().await;
    trades_route.assert_async().await;

    assert_eq!(trades.len(), expected.len());
    for (trade, row) in trades.iter().zip(expected) {
        assert_eq!(trade.key, key());
        assert_eq!(trade.id, None, "the Data API gives a trade no id");
        assert_eq!(trade.price, decimal(row.price));
        assert_eq!(trade.size, decimal(row.size));
        assert_eq!(trade.time, UnixMillis(row.timestamp as u64 * 1_000));
        assert_eq!(
            trade.taker_side,
            match row.side {
                TradeSide::Buy => Some(Side::Buy),
                TradeSide::Sell => Some(Side::Sell),
                _ => None,
            }
        );
        let ext = trade.extensions.get::<TradeExt>().unwrap();
        assert_eq!(ext.transaction_hash, row.transaction_hash);
        assert_eq!(ext.outcome.as_deref(), Some(row.outcome.as_str()));
    }
    assert!(
        trades.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}

#[tokio::test]
async fn event_group_holds_every_v1_outcome_of_the_tokens_event() {
    let mut server = Server::new_async().await;
    let markets_query = format!("clob_token_ids={}", token());
    let markets = route(
        &mut server,
        "/markets",
        &markets_query,
        &fixture("gamma_markets"),
        1,
    )
    .await;
    let event_id = json("gamma_markets")[0]["events"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let event_route = route(
        &mut server,
        &format!("/events/{event_id}"),
        "",
        &fixture("gamma_event"),
        2,
    )
    .await;
    let client = client(&server);
    let event = Gamma::builder()
        .base_url(server.url())
        .build()
        .unwrap()
        .events()
        .get(event_id.clone())
        .send()
        .await
        .unwrap();

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client);
    let group = venue.event_group(&key()).await.unwrap();
    markets.assert_async().await;
    event_route.assert_async().await;

    assert_eq!(group.id, event.id);
    assert_eq!(group.title, event.title);
    let mut expected = Vec::new();
    // Only a v1 market's outcome ids are the CLOB's tokens.
    for market in event
        .markets
        .iter()
        .filter(|market| market.version == Some(ProtocolVersion::V1))
    {
        let tokens: Vec<String> =
            serde_json::from_str(market.clob_token_ids.as_deref().unwrap()).unwrap();
        let labels: Vec<String> =
            serde_json::from_str(market.outcomes.as_deref().unwrap()).unwrap();
        for (token, label) in tokens.iter().zip(&labels) {
            expected.push((
                keys::clob(&TokenId::new(token.as_str()).unwrap()),
                market.condition_id.clone(),
                label.clone(),
                market.group_item_title.clone(),
            ));
        }
    }
    let actual: Vec<_> = group
        .outcomes
        .iter()
        .map(|o| {
            (
                o.key.clone(),
                o.market.clone(),
                o.label.clone(),
                o.market_title.clone(),
            )
        })
        .collect();
    assert_eq!(actual, expected);
    assert!(
        group.outcomes.iter().any(|o| o.key == key()),
        "the token's own outcome"
    );
    let ext = group.extensions.get::<EventGroupExt>().unwrap();
    assert_eq!(ext.slug, event.slug);
    assert_eq!(ext.neg_risk, event.neg_risk);
}
```

- [ ] **Step 2: Register it (it needs `clob`, `gamma` and `data`), and turn on `polyoxide-venue/market` with `clob` in place of `keys`.**

Current (`polyoxide-polymarket/Cargo.toml`):

```toml
    "dep:polyoxide-core",
    "polyoxide-venue/keys",
    "dep:base64",
```

Replace with:

```toml
    "dep:polyoxide-core",
    "polyoxide-venue/market",
    "dep:base64",
```

Current (`polyoxide-polymarket/Cargo.toml`):

```toml
# The relay module's suites (Story 5.4).
[[test]]
```

Replace with:

```toml
# The clob's traits read Data's public trades and Gamma's events too.
[[test]]
name = "venue_clob"
required-features = ["clob", "gamma", "data"]

# The relay module's suites (Story 5.4).
[[test]]
```

- [ ] **Step 3: Run it and watch it fail.**

Run: `cargo test -p polyoxide-polymarket --all-features --test venue_clob`
Expected: FAIL to compile: no module `venue` in `clob`, and no method `data` on `ClobBuilder`.

- [ ] **Step 4: Add `ClobError::Data`, classed as the Data error, as `ClobError::Gamma` is classed as Gamma's.**

Current (`polyoxide-polymarket/src/clob/error.rs`):

```rust
    #[error("Gamma error: {0}")]
    Gamma(crate::gamma::GammaError),
```

Replace with:

```rust
    #[error("Gamma error: {0}")]
    Gamma(crate::gamma::GammaError),

    /// The Data client clob reads public trades through could not be built.
    /// Its class is the Data error's.
    #[cfg(feature = "data")]
    #[error("Data error: {0}")]
    Data(crate::data::DataError),
```

Current (`polyoxide-polymarket/src/clob/error.rs`):

```rust
/// says. `Api`, `Gamma` and `BurstCapacityExceeded` delegate, local signing
```

Replace with:

```rust
/// says. `Api`, `Gamma`, `Data` and `BurstCapacityExceeded` delegate, local signing
```

Current (`polyoxide-polymarket/src/clob/error.rs`):

```rust
            Self::Crypto(_) | Self::Alloy(_) => Class::InvalidRequest,
```

Replace with:

```rust
            #[cfg(feature = "data")]
            Self::Data(err) => err.class(),
            Self::Crypto(_) | Self::Alloy(_) => Class::InvalidRequest,
```

Current (`polyoxide-polymarket/src/clob/error.rs`):

```rust
            Self::FakUnmatched { .. } | Self::FokUnfilled { .. } => false,
```

Replace with:

```rust
            #[cfg(feature = "data")]
            Self::Data(err) => err.is_fault(),
            Self::FakUnmatched { .. } | Self::FokUnfilled { .. } => false,
```

Current (`polyoxide-polymarket/src/clob/error.rs`):

```rust
            Self::BurstCapacityExceeded(err) => Classify::retry_after(err),
```

Replace with:

```rust
            #[cfg(feature = "data")]
            Self::Data(err) => Classify::retry_after(err),
            Self::BurstCapacityExceeded(err) => Classify::retry_after(err),
```

- [ ] **Step 5: Embed the Data client in `Clob`, as Gamma's is (D2).**

Current (`polyoxide-polymarket/src/clob/client.rs`):

```rust
#[cfg(feature = "gamma")]
use crate::gamma::Gamma;
```

Replace with:

```rust
#[cfg(feature = "data")]
use crate::data::DataApi;
#[cfg(feature = "gamma")]
use crate::gamma::Gamma;
```

Current (`polyoxide-polymarket/src/clob/client.rs`):

```rust
    #[cfg(feature = "gamma")]
    pub(crate) gamma: Gamma,
```

Replace with:

```rust
    #[cfg(feature = "gamma")]
    pub(crate) gamma: Gamma,
    /// The Data client the `Trades` implementation reads public trades
    /// through, since the CLOB serves none.
    #[cfg(feature = "data")]
    pub(crate) data: DataApi,
```

Current (`polyoxide-polymarket/src/clob/client.rs`):

```rust
    #[cfg(feature = "gamma")]
    gamma: Option<Gamma>,
```

Replace with:

```rust
    #[cfg(feature = "gamma")]
    gamma: Option<Gamma>,
    #[cfg(feature = "data")]
    data: Option<DataApi>,
```

Current (`polyoxide-polymarket/src/clob/client.rs`):

```rust
            account: None,
            #[cfg(feature = "gamma")]
            gamma: None,
```

Replace with:

```rust
            account: None,
            #[cfg(feature = "gamma")]
            gamma: None,
            #[cfg(feature = "data")]
            data: None,
```

Current (`polyoxide-polymarket/src/clob/client.rs`):

```rust
    /// Build the CLOB client
    pub fn build(self) -> Result<Clob, ClobError> {
```

Replace with:

```rust
    /// Set the Data client the `Trades` implementation reads public trades
    /// through. Without one, `build` makes one with this builder's timeout
    /// and pool size.
    #[cfg(feature = "data")]
    pub fn data(mut self, data: DataApi) -> Self {
        self.data = Some(data);
        self
    }

    /// Build the CLOB client
    pub fn build(self) -> Result<Clob, ClobError> {
```

Current (`polyoxide-polymarket/src/clob/client.rs`):

```rust
        Ok(Clob {
            http_client,
            chain_id: self.chain.chain_id(),
```

Replace with:

```rust
        #[cfg(feature = "data")]
        let data = if let Some(data) = self.data {
            data
        } else {
            crate::data::DataApi::builder()
                .timeout_ms(self.config.timeout_ms)
                .pool_size(self.config.pool_size)
                .build()
                .map_err(ClobError::Data)?
        };

        Ok(Clob {
            http_client,
            chain_id: self.chain.chain_id(),
```

Current (`polyoxide-polymarket/src/clob/client.rs`):

```rust
            signer_limiter,
            #[cfg(feature = "gamma")]
            gamma,
```

Replace with:

```rust
            signer_limiter,
            #[cfg(feature = "gamma")]
            gamma,
            #[cfg(feature = "data")]
            data,
```

- [ ] **Step 6: Have the unified client build its Data client first and hand it to clob.** Each base URL still fails as its own `BuildError` variant.

Current (`polyoxide-polymarket/src/client.rs`):

```rust
        // Build CLOB client
        let mut clob_builder = ClobBuilder::new().with_account(self.account);
```

Replace with:

```rust
        // Build Data API client
        let mut data_builder = DataApiBuilder::default();

        if let Some(url) = self.data_base_url {
            data_builder = data_builder.base_url(url);
        }
        if let Some(timeout) = self.timeout_ms {
            data_builder = data_builder.timeout_ms(timeout);
        }

        let data = data_builder.build()?;

        // Build CLOB client, which reads public trades through the Data client
        let mut clob_builder = ClobBuilder::new()
            .with_account(self.account)
            .data(data.clone());
```

Current (`polyoxide-polymarket/src/client.rs`):

```rust
        let clob = clob_builder.build()?;

        // Build Data API client
        let mut data_builder = DataApiBuilder::default();

        if let Some(url) = self.data_base_url {
            data_builder = data_builder.base_url(url);
        }
        if let Some(timeout) = self.timeout_ms {
            data_builder = data_builder.timeout_ms(timeout);
        }

        let data = data_builder.build()?;
```

Replace with:

```rust
        let clob = clob_builder.build()?;
```

- [ ] **Step 7: Implement the traits on `Clob`.**

Create `polyoxide-polymarket/src/clob/venue.rs` with:

```rust
//! The CLOB through the venue-neutral market-data traits.
//!
//! Each key names one outcome token, `polymarket.clob:<token id>`, made by
//! the constructors in `shared::keys`, and an outcome's book is bids and asks
//! in that outcome's own price terms. [`Clob`] implements [`MarketData`] and
//! [`Candles`] with `clob` alone; [`Trades`](polyoxide_venue::Trades) with
//! `data` as well, since the CLOB serves no public trades and the Data API
//! does; and [`EventGroups`](polyoxide_venue::EventGroups) with `gamma` as
//! well, which groups outcomes into events. What only the CLOB publishes (neg
//! risk, the book's tick size and the rest) rides in this module's
//! `<Record>Ext` types, one per record, in each record's [`Extensions`].
//!
//! | Method | Routes |
//! | --- | --- |
//! | `instrument` | `/markets-by-token/{token}`, then `/markets/{condition}` |
//! | `quote`, `book` | `/book` |
//! | `candles` | `/prices-history`, one price per interval |
//! | `trades` (`data`) | `/markets-by-token/{token}`, then the Data API's `/trades` |
//! | `event_group` (`gamma`) | Gamma's `/markets?clob_token_ids=`, then `/events/{id}` |
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use polyoxide_polymarket::clob::Clob;
//! use polyoxide_polymarket::shared::keys::{self, TokenId};
//! use polyoxide_venue::{DynMarketData, MarketData};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Clob::public());
//! let token = TokenId::new("71321045679252212594626385532706912750332728571942532289631379312455583992563")?;
//! let quote = venue.quote(&keys::clob(&token)).await?;
//! println!("bid {:?} ask {:?}", quote.bid, quote.ask);
//! # Ok(())
//! # }
//! ```

use std::cmp::Reverse;

use polyoxide_venue::{
    Book, Candle, CandleInterval, CandleQuery, Candles, Class, ClassifiedError, Extensions,
    Instrument, Level, MarketData, MarketKey, NotListed, Quote, SizeUnit, UnixMillis,
};
use rust_decimal::Decimal;

use crate::clob::{
    api::markets::{Market, MarketToken, OrderBook, OrderLevel, PricesHistoryQuery},
    Clob,
};
use crate::shared::keys;

/// The CLOB's facts about an outcome, beside [`Instrument`]'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct InstrumentExt {
    /// The condition id of the market the outcome is in.
    pub condition_id: Option<String>,
    /// The market's question.
    pub question: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// Whether the market is in a negative-risk event.
    pub neg_risk: Option<bool>,
    /// The negative-risk event's id.
    pub neg_risk_market_id: Option<String>,
    /// Whether the market is active.
    pub active: Option<bool>,
    /// Whether the market is closed.
    pub closed: Option<bool>,
    /// When the market ends, as the CLOB writes it.
    pub end_date_iso: Option<String>,
    /// Whether the outcome won, once the market resolved.
    pub winner: Option<bool>,
}

/// The CLOB's facts about a quote, beside [`Quote`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct QuoteExt {
    /// The last traded price.
    pub last_trade_price: Option<Decimal>,
}

/// The CLOB's facts about a book, beside [`Book`]'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BookExt {
    /// The condition id of the market the outcome is in.
    pub condition_id: Option<String>,
    /// The book's hash.
    pub hash: Option<String>,
    /// The tick size now in force, which the CLOB changes as the price
    /// nears 0 or 1.
    pub tick_size: Option<Decimal>,
    /// The smallest order size, in shares.
    pub min_order_size: Option<Decimal>,
    /// Whether the market is in a negative-risk event.
    pub neg_risk: Option<bool>,
    /// The last traded price.
    pub last_trade_price: Option<Decimal>,
}

/// A decimal the CLOB sent as a JSON number, read through its shortest
/// decimal spelling, so `0.001` is `0.001` and not the nearest binary value.
fn decimal(value: f64) -> Option<Decimal> {
    value.to_string().parse().ok()
}

/// A decimal the CLOB sent as text.
fn text_decimal(value: Option<&str>) -> Option<Decimal> {
    value.and_then(|text| text.parse().ok())
}

fn decode(what: &str) -> ClassifiedError {
    ClassifiedError::new(Class::Decode, format!("the CLOB sent {what}"))
}

fn level(level: &OrderLevel) -> Level {
    Level::new(level.price, level.size)
}

/// A book's sides best first. The CLOB sends each side worst first.
fn sides(book: &OrderBook) -> (Vec<Level>, Vec<Level>) {
    let mut bids: Vec<Level> = book.bids.iter().map(level).collect();
    let mut asks: Vec<Level> = book.asks.iter().map(level).collect();
    bids.sort_by_key(|level| Reverse(level.price));
    asks.sort_by_key(|level| level.price);
    (bids, asks)
}

fn book_time(book: &OrderBook) -> Option<UnixMillis> {
    book.timestamp.parse().ok().map(UnixMillis)
}

/// The instrument an outcome of a market is.
fn instrument(key: &MarketKey, market: &Market, outcome: &MarketToken) -> Instrument {
    let name = match &market.question {
        Some(question) => format!("{question} — {}", outcome.outcome),
        None => outcome.outcome.clone(),
    };
    let mut instrument = Instrument::new(key.clone(), name, SizeUnit::Shares);
    instrument.tick_size = market.minimum_tick_size.and_then(decimal);
    instrument.min_size = market.minimum_order_size.and_then(decimal);
    instrument.tradable = market.accepting_orders;
    instrument.extensions = Extensions::of(InstrumentExt {
        condition_id: Some(market.condition_id.clone()),
        question: market.question.clone(),
        outcome: Some(outcome.outcome.clone()),
        neg_risk: market.neg_risk,
        neg_risk_market_id: market.neg_risk_market_id.clone(),
        active: Some(market.active),
        closed: Some(market.closed),
        end_date_iso: market.end_date_iso.clone(),
        winner: outcome.winner,
    });
    instrument
}

/// The `/prices-history` fidelity, in minutes, of a candle interval.
fn fidelity(interval: CandleInterval) -> Result<i32, ClassifiedError> {
    Ok(match interval {
        CandleInterval::Minute1 => 1,
        CandleInterval::Minute5 => 5,
        CandleInterval::Minute15 => 15,
        CandleInterval::Hour1 => 60,
        CandleInterval::Hour4 => 240,
        CandleInterval::Day1 => 1_440,
        CandleInterval::Week1 => 10_080,
        other => {
            return Err(ClassifiedError::new(
                Class::InvalidRequest,
                format!("the CLOB serves no {other:?} price history"),
            ))
        }
    })
}

/// Unix seconds, from milliseconds, rounded down.
fn seconds(time: UnixMillis) -> i64 {
    i64::try_from(time.0 / 1_000).unwrap_or(i64::MAX)
}

impl MarketData for Clob {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let token = keys::token_id(key)?;
        let found = self
            .markets()
            .market_by_token(token.as_str())
            .send()
            .await?;
        let market = self.markets().get(found.condition_id).send().await?;
        let Some(outcome) = market
            .tokens
            .iter()
            .find(|outcome| outcome.token_id.as_deref() == Some(token.as_str()))
        else {
            return Err(NotListed { key: key.clone() }.into());
        };
        Ok(instrument(key, &market, outcome))
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        let token = keys::token_id(key)?;
        let book = self.markets().order_book(token.as_str()).send().await?;
        let (bids, asks) = sides(&book);
        let mut quote = Quote::new(key.clone());
        quote.bid = bids.first().copied();
        quote.ask = asks.first().copied();
        quote.time = book_time(&book);
        quote.extensions = Extensions::of(QuoteExt {
            last_trade_price: text_decimal(book.last_trade_price.as_deref()),
        });
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        let token = keys::token_id(key)?;
        let native = self.markets().order_book(token.as_str()).send().await?;
        let (bids, asks) = sides(&native);
        let mut book = Book::new(key.clone());
        book.bids = bids;
        book.asks = asks;
        book.time = book_time(&native);
        book.extensions = Extensions::of(BookExt {
            condition_id: Some(native.market.clone()),
            hash: Some(native.hash.clone()),
            tick_size: text_decimal(native.tick_size.as_deref()),
            min_order_size: text_decimal(native.min_order_size.as_deref()),
            neg_risk: native.neg_risk,
            last_trade_price: text_decimal(native.last_trade_price.as_deref()),
        });
        Ok(book)
    }
}

impl Candles for Clob {
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        let token = keys::token_id(key)?;
        let params = PricesHistoryQuery {
            interval: None,
            fidelity: Some(fidelity(query.interval)?),
            start_ts: Some(seconds(query.start)),
            end_ts: Some(seconds(query.end.unwrap_or_else(UnixMillis::now))),
        };
        let history = self
            .markets()
            .prices_history_with(token.as_str(), &params)
            .send()
            .await?;
        history
            .history
            .iter()
            .map(|point| {
                let open_time = u64::try_from(point.timestamp)
                    .ok()
                    .and_then(|seconds| seconds.checked_mul(1_000))
                    .ok_or_else(|| decode("a price point before 1970"))?;
                let close =
                    decimal(point.price).ok_or_else(|| decode("a price that is not a decimal"))?;
                Ok(Candle::new(
                    key.clone(),
                    query.interval,
                    UnixMillis(open_time),
                    close,
                ))
            })
            .collect()
    }
}

#[cfg(feature = "data")]
mod trades {
    use polyoxide_venue::{
        ClassifiedError, Extensions, MarketKey, Side, Trade, Trades, UnixMillis,
    };

    use super::{decimal, decode};
    use crate::clob::Clob;
    use crate::data::types::TradeSide;
    use crate::shared::keys;

    /// The CLOB's facts about a public trade, beside [`Trade`]'s fields.
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    #[non_exhaustive]
    pub struct TradeExt {
        /// The settling transaction's hash.
        pub transaction_hash: Option<String>,
        /// The taker's proxy wallet.
        pub proxy_wallet: Option<String>,
        /// The outcome's label, such as `Yes`.
        pub outcome: Option<String>,
    }

    impl Trades for Clob {
        async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
            let token = keys::token_id(key)?;
            let found = self
                .markets()
                .market_by_token(token.as_str())
                .send()
                .await?;
            let rows = self
                .data
                .trades()
                .list()
                .market([found.condition_id])
                .taker_only(true)
                .send()
                .await?;
            // The Data API sends the newest first, and both outcomes' trades.
            rows.iter()
                .rev()
                .filter(|row| row.asset == token.as_str())
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
                        TradeSide::Unknown => None,
                    };
                    trade.extensions = Extensions::of(TradeExt {
                        transaction_hash: row.transaction_hash.clone(),
                        proxy_wallet: Some(row.proxy_wallet.clone()),
                        outcome: Some(row.outcome.clone()),
                    });
                    Ok(trade)
                })
                .collect()
        }
    }
}

#[cfg(feature = "data")]
pub use trades::TradeExt;

#[cfg(feature = "gamma")]
mod events {
    use polyoxide_core::{QueryBuilder, Request};
    use polyoxide_venue::{
        ClassifiedError, EventGroup, EventGroups, EventOutcome, Extensions, MarketKey, NotListed,
    };
    use serde::Deserialize;

    use super::decode;
    use crate::clob::Clob;
    use crate::gamma::{types::ProtocolVersion, GammaError};
    use crate::shared::keys::{self, TokenId};

    /// The CLOB's facts about an event, beside [`EventGroup`]'s fields.
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    #[non_exhaustive]
    pub struct EventGroupExt {
        /// The event's slug.
        pub slug: Option<String>,
        /// Whether the event is negative-risk: its markets' Yes outcomes are
        /// mutually exclusive.
        pub neg_risk: Option<bool>,
        /// The negative-risk event's id.
        pub neg_risk_market_id: Option<String>,
    }

    /// The events a Gamma market row names. Gamma's `Market` does not model
    /// them, so this reads them alone.
    #[derive(Deserialize)]
    struct MarketEvents {
        #[serde(default)]
        events: Vec<EventId>,
    }

    #[derive(Deserialize)]
    struct EventId {
        id: String,
    }

    /// A JSON-encoded array of strings, as Gamma sends a market's outcomes
    /// and token ids.
    fn strings(json: Option<&str>, what: &str) -> Result<Vec<String>, ClassifiedError> {
        json.map_or(Ok(Vec::new()), |json| {
            serde_json::from_str(json).map_err(|_| decode(what))
        })
    }

    impl EventGroups for Clob {
        async fn event_group(&self, key: &MarketKey) -> Result<EventGroup, ClassifiedError> {
            let token = keys::token_id(key)?;
            let markets = Request::<Vec<MarketEvents>, GammaError>::new(
                self.gamma.http_client.clone(),
                "/markets",
            )
            .query("clob_token_ids", token.as_str())
            .send()
            .await?;
            let Some(event_id) = markets
                .iter()
                .flat_map(|market| &market.events)
                .map(|event| event.id.clone())
                .next()
            else {
                return Err(NotListed { key: key.clone() }.into());
            };
            let event = self.gamma.events().get(event_id).send().await?;
            let mut group = EventGroup::new(event.id.clone());
            group.title = event.title.clone();
            for market in &event.markets {
                // Only a v1 market's outcomes are the CLOB's tokens.
                if market.version != Some(ProtocolVersion::V1) {
                    continue;
                }
                let tokens = strings(
                    market.clob_token_ids.as_deref(),
                    "token ids that are not a JSON array of strings",
                )?;
                let labels = strings(
                    market.outcomes.as_deref(),
                    "outcomes that are not a JSON array of strings",
                )?;
                if tokens.len() != labels.len() {
                    return Err(decode(
                        "a market whose token ids and outcomes differ in number",
                    ));
                }
                for (token, label) in tokens.into_iter().zip(labels) {
                    let token = TokenId::new(token)?;
                    let mut outcome =
                        EventOutcome::new(keys::clob(&token), market.condition_id.clone(), label);
                    outcome.market_title = market
                        .group_item_title
                        .clone()
                        .or_else(|| Some(market.question.clone()));
                    group.outcomes.push(outcome);
                }
            }
            group.extensions = Extensions::of(EventGroupExt {
                slug: event.slug.clone(),
                neg_risk: event.neg_risk,
                neg_risk_market_id: event.neg_risk_market_id.clone(),
            });
            Ok(group)
        }
    }
}

#[cfg(feature = "gamma")]
pub use events::EventGroupExt;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::keys::TokenId;

    #[test]
    fn every_candle_interval_is_a_fidelity_in_minutes() {
        let minutes: Vec<i32> = CandleInterval::ALL
            .iter()
            .map(|each| fidelity(*each).unwrap())
            .collect();
        assert_eq!(minutes, [1, 5, 15, 60, 240, 1_440, 10_080]);
    }

    #[test]
    fn a_number_is_read_through_its_shortest_spelling() {
        assert_eq!(decimal(0.001), Some(Decimal::new(1, 3)));
        assert_eq!(decimal(5.0), Some(Decimal::from(5)));
        assert_eq!(decimal(0.155), Some(Decimal::new(155, 3)));
        assert_eq!(decimal(f64::NAN), None);
    }

    #[test]
    fn a_book_is_sorted_best_first_whatever_order_it_came_in() {
        let book: OrderBook = serde_json::from_value(serde_json::json!({
            "market": "0x1", "asset_id": "1", "timestamp": "1792161583225", "hash": "h",
            "bids": [{"price": "0.13", "size": "1"}, {"price": "0.15", "size": "2"}, {"price": "0.14", "size": "3"}],
            "asks": [{"price": "0.18", "size": "1"}, {"price": "0.16", "size": "2"}, {"price": "0.17", "size": "3"}],
        }))
        .unwrap();
        let (bids, asks) = sides(&book);
        let bid_prices: Vec<String> = bids.iter().map(|l| l.price.to_string()).collect();
        let ask_prices: Vec<String> = asks.iter().map(|l| l.price.to_string()).collect();
        assert_eq!(bid_prices, ["0.15", "0.14", "0.13"]);
        assert_eq!(ask_prices, ["0.16", "0.17", "0.18"]);
        assert_eq!(book_time(&book), Some(UnixMillis(1_792_161_583_225)));
    }

    #[test]
    fn a_token_id_this_module_reads_is_a_clob_key() {
        let token = TokenId::new(
            "54533043819946592547517511176940999955633860128497669742211153063842200957669",
        )
        .unwrap();
        assert_eq!(keys::token_id(&keys::clob(&token)), Ok(token));
    }
}
```

- [ ] **Step 8: Declare the module.**

Current (`polyoxide-polymarket/src/clob/mod.rs`):

```rust
pub mod types;
pub mod utils;

#[cfg(feature = "clob-ws")]
```

Replace with:

```rust
pub mod types;
pub mod utils;
pub mod venue;

#[cfg(feature = "clob-ws")]
```

- [ ] **Step 9: Run the tests again, and lint each feature combination the gates use.**

Run: `cargo test -p polyoxide-polymarket --all-features --test venue_clob --test mock_clob --lib`
Expected: PASS: 7 fixture tests, `mock_clob` unchanged, and `clob::venue::tests` (4) among the unit tests.

Run: `for f in clob clob,data clob,gamma clob,gamma,data; do cargo clippy -p polyoxide-polymarket --no-default-features --features $f --all-targets -- -D warnings || exit 1; done && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-polymarket --all-features`
Expected: clean in all four builds; `Trades` and `EventGroups` compile only where their features are on.

- [ ] **Step 10: Add the README section.**

Current (`polyoxide-polymarket/src/clob/README.md`):

```markdown
## License

Licensed under either of [MIT](../../../LICENSE-MIT) or [Apache-2.0](../../../LICENSE-APACHE) at your option.
```

Replace with:

```markdown
## Through the venue-neutral traits

`Clob` implements `polyoxide-venue`'s `MarketData` and `Candles` (from
`/prices-history`, one price per interval), and with the `data` feature
`Trades`, since the CLOB serves no public trades and the Data API does, and with
`gamma` `EventGroups`. A key names one outcome token,
`polymarket.clob:<token id>` (`shared::keys::clob`), and its book is bids and
asks in that outcome's own price terms, best first. Neg risk, the book's tick
size and the CLOB's other facts ride in `clob::venue`'s `<Record>Ext` types.
The `clob::venue` module's documentation has an example.

## License

Licensed under either of [MIT](../../../LICENSE-MIT) or [Apache-2.0](../../../LICENSE-APACHE) at your option.
```

- [ ] **Step 11: Commit, and note the commit's short sha for Task 5.3.**

```bash
git add polyoxide-polymarket/src/clob/venue.rs polyoxide-polymarket/tests/venue_clob.rs polyoxide-polymarket/src/clob/error.rs polyoxide-polymarket/src/clob/client.rs polyoxide-polymarket/src/client.rs polyoxide-polymarket/src/clob/mod.rs polyoxide-polymarket/Cargo.toml polyoxide-polymarket/src/clob/README.md
git commit -m "feat(clob): the CLOB through the market-data traits, its trades through Data (Story 6.5)" -- polyoxide-polymarket/src/clob/venue.rs polyoxide-polymarket/tests/venue_clob.rs polyoxide-polymarket/src/clob/error.rs polyoxide-polymarket/src/clob/client.rs polyoxide-polymarket/src/client.rs polyoxide-polymarket/src/clob/mod.rs polyoxide-polymarket/Cargo.toml polyoxide-polymarket/src/clob/README.md
git log -1 --format=%h
```

### Task 5.3: Move the mutants ledger and prove its clob rows again

`ClobError::Data` and its three `Classify` arms moved the rule lines of rows "(e)", "(e), case" and "(e), only a 400" 6 lines down and their `error.rs` tests 12 down; their `mock_clob.rs` tests did not move. CLAUDE.md: "Move one of those lines or tests and the ledger moves with it, re-proved."

**Files:**
- Modify: `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`

- [ ] **Step 1: Confirm the moved lines hold the cited code.**

Run: `for n in 103 106 133 356 368 376 389 400 431 470; do printf '%s: ' $n; sed -n "${n}p" polyoxide-polymarket/src/clob/error.rs; done`
Expected, in order: `let m = message.to_ascii_lowercase();`, `if m.contains("fak order") && (…`, `ApiError::Response(response) if response.status.as_u16() == 400 => {`, then the `fn` lines of `test_classify_recognizes_verbatim_venue_messages`, `test_classify_preserves_message_verbatim`, `test_classify_is_case_insensitive`, `test_classify_tolerates_curly_apostrophe_in_fok_message`, `test_classify_does_not_capture_neighbouring_400s`, `test_classify_requires_both_tokens` and `the_decode_splits_out_a_kill_only_on_a_400`. If any differs, stop and ask: the ledger below assumes these lines.

- [ ] **Step 2: Prove each row's mutant.** Each mutation is applied, its tests run (`--no-fail-fast`, so `mock_clob` runs after a failing unit test), and the mutation undone by the inverse edit. Every named test must fail under its mutant and pass once the file is restored.

```bash
python3 - <<'EOF'
import subprocess
PATH = "polyoxide-polymarket/src/clob/error.rs"
ROWS = [
    ("(e)", 106, 'm.contains("fak order") &&', 'm.contains("fak order") ||',
     ["test_classify_requires_both_tokens"]),
    ("(e), case", 103, "message.to_ascii_lowercase()", "message.to_string()",
     ["test_classify_recognizes_verbatim_venue_messages", "test_classify_preserves_message_verbatim",
      "test_classify_is_case_insensitive", "test_classify_tolerates_curly_apostrophe_in_fok_message",
      "fak_unmatched_maps_to_typed_error_not_generic_validation",
      "fok_unfilled_maps_to_typed_error_not_generic_validation"]),
    ("(e), only a 400", 133, " if response.status.as_u16() == 400 =>", " =>",
     ["the_decode_splits_out_a_kill_only_on_a_400", "fak_prose_on_non_400_status_is_not_reclassified"]),
]

def swap(line, old, new):
    lines = open(PATH).read().split("\n")
    assert lines[line - 1].count(old) == 1, (line, lines[line - 1])
    lines[line - 1] = lines[line - 1].replace(old, new)
    open(PATH, "w").write("\n".join(lines))

def run(tests):
    out = subprocess.run(
        ["cargo", "test", "-p", "polyoxide-polymarket", "--all-features", "--no-fail-fast",
         "--lib", "--test", "mock_clob", "--", *tests],
        capture_output=True, text=True).stdout
    return {t: (f"{t} ... FAILED" in out) for t in tests}, {t: (f"{t} ... ok" in out) for t in tests}

for name, line, old, new, tests in ROWS:
    swap(line, old, new)
    failed, _ = run(tests)
    swap(line, new, old)
    _, passed = run(tests)
    print(name, "fails under its mutant:", all(failed.values()), "| passes restored:", all(passed.values()))
    assert all(failed.values()) and all(passed.values()), (name, failed, passed)
EOF
git diff --exit-code -- polyoxide-polymarket/src/clob/error.rs
```

Expected: three lines, each `fails under its mutant: True | passes restored: True`, and `git diff --exit-code` clean.

- [ ] **Step 3: Move the rows and record the proof.** In the replacement for the last paragraph before `## The rules`, write the date of Step 2 for `<DATE>` and Task 5.2's short sha for `<SHA>`.

Current (`docs/MUTANTS.md`):

```markdown
`polyoxide-ws`, Binance and `polyoxide-polymarket`, where perps' and rtds' tests of one name
both failed.
```

Replace with:

```markdown
`polyoxide-ws`, Binance and `polyoxide-polymarket`, where perps' and rtds' tests of one name
both failed.
Story 6.5's `ClobError::Data`, a new variant with its three `Classify` arms, moved the rule
lines of rows "(e)", "(e), case" and "(e), only a 400" 6 lines down and their `error.rs` tests
12 down, with the note on `test_classify_does_not_capture_neighbouring_400s`; their
`mock_clob.rs` tests did not move. The three rows were proved again on <DATE> on top of
`<SHA>`, by running each row's tests with its mutant in place, then without it, through
`cargo test -p polyoxide-polymarket --all-features --no-fail-fast --lib --test mock_clob`.
```

Current (`docs/MUTANTS.md`):

```markdown
| (e) `classify_order_kill` needs both the order kind and the kill token | `polyoxide-polymarket/src/clob/error.rs:100` | `&&` → `\|\|` after `m.contains("fak order")` | `polyoxide-polymarket/src/clob/error.rs:419` `test_classify_requires_both_tokens` |
| (e), case | `polyoxide-polymarket/src/clob/error.rs:97`, the message is lowercased before matching | `let m = message.to_string();` | `polyoxide-polymarket/src/clob/error.rs:344` `test_classify_recognizes_verbatim_venue_messages`; `polyoxide-polymarket/src/clob/error.rs:356` `test_classify_preserves_message_verbatim`; `polyoxide-polymarket/src/clob/error.rs:364` `test_classify_is_case_insensitive`; `polyoxide-polymarket/src/clob/error.rs:377` `test_classify_tolerates_curly_apostrophe_in_fok_message`; `polyoxide-polymarket/tests/mock_clob.rs:3387` `fak_unmatched_maps_to_typed_error_not_generic_validation`; `polyoxide-polymarket/tests/mock_clob.rs:3421` `fok_unfilled_maps_to_typed_error_not_generic_validation` |
| (e), only a 400 is classified | `polyoxide-polymarket/src/clob/error.rs:127`, clob's decode, `From<ApiError>`, classifies an `ApiError::Response` only when its status is 400 | Drop the arm's `if response.status.as_u16() == 400` guard, so every response's message is classified | `polyoxide-polymarket/tests/mock_clob.rs:3485` `fak_prose_on_non_400_status_is_not_reclassified`; `polyoxide-polymarket/src/clob/error.rs:458` `the_decode_splits_out_a_kill_only_on_a_400` |
```

Replace with:

```markdown
| (e) `classify_order_kill` needs both the order kind and the kill token | `polyoxide-polymarket/src/clob/error.rs:106` | `&&` → `\|\|` after `m.contains("fak order")` | `polyoxide-polymarket/src/clob/error.rs:431` `test_classify_requires_both_tokens` |
| (e), case | `polyoxide-polymarket/src/clob/error.rs:103`, the message is lowercased before matching | `let m = message.to_string();` | `polyoxide-polymarket/src/clob/error.rs:356` `test_classify_recognizes_verbatim_venue_messages`; `polyoxide-polymarket/src/clob/error.rs:368` `test_classify_preserves_message_verbatim`; `polyoxide-polymarket/src/clob/error.rs:376` `test_classify_is_case_insensitive`; `polyoxide-polymarket/src/clob/error.rs:389` `test_classify_tolerates_curly_apostrophe_in_fok_message`; `polyoxide-polymarket/tests/mock_clob.rs:3387` `fak_unmatched_maps_to_typed_error_not_generic_validation`; `polyoxide-polymarket/tests/mock_clob.rs:3421` `fok_unfilled_maps_to_typed_error_not_generic_validation` |
| (e), only a 400 is classified | `polyoxide-polymarket/src/clob/error.rs:133`, clob's decode, `From<ApiError>`, classifies an `ApiError::Response` only when its status is 400 | Drop the arm's `if response.status.as_u16() == 400` guard, so every response's message is classified | `polyoxide-polymarket/tests/mock_clob.rs:3485` `fak_prose_on_non_400_status_is_not_reclassified`; `polyoxide-polymarket/src/clob/error.rs:470` `the_decode_splits_out_a_kill_only_on_a_400` |
```

Current (`docs/MUTANTS.md`):

```markdown
`polyoxide-polymarket/src/clob/error.rs:388` `test_classify_does_not_capture_neighbouring_400s` fails
```

Replace with:

```markdown
`polyoxide-polymarket/src/clob/error.rs:400` `test_classify_does_not_capture_neighbouring_400s` fails
```

- [ ] **Step 4: Move the ledger test's citations.**

Current (`.github/scripts/tests/test_mutants_ledger.py`):

```python
    ("polyoxide-polymarket/src/clob/error.rs", 97): "let m = message.to_ascii_lowercase();",
    ("polyoxide-polymarket/src/clob/error.rs", 100): 'if m.contains("fak order") && (m.contains("no match")',
    ("polyoxide-polymarket/src/clob/error.rs", 127): "ApiError::Response(response) if response.status.as_u16() == 400 => {",
    ("polyoxide-polymarket/src/clob/error.rs", 344): "fn test_classify_recognizes_verbatim_venue_messages(",
    ("polyoxide-polymarket/src/clob/error.rs", 356): "fn test_classify_preserves_message_verbatim(",
    ("polyoxide-polymarket/src/clob/error.rs", 364): "fn test_classify_is_case_insensitive(",
    ("polyoxide-polymarket/src/clob/error.rs", 377): "fn test_classify_tolerates_curly_apostrophe_in_fok_message(",
    ("polyoxide-polymarket/src/clob/error.rs", 388): "fn test_classify_does_not_capture_neighbouring_400s(",
    ("polyoxide-polymarket/src/clob/error.rs", 419): "fn test_classify_requires_both_tokens(",
    ("polyoxide-polymarket/src/clob/error.rs", 458): "fn the_decode_splits_out_a_kill_only_on_a_400(",
```

Replace with:

```python
    ("polyoxide-polymarket/src/clob/error.rs", 103): "let m = message.to_ascii_lowercase();",
    ("polyoxide-polymarket/src/clob/error.rs", 106): 'if m.contains("fak order") && (m.contains("no match")',
    ("polyoxide-polymarket/src/clob/error.rs", 133): "ApiError::Response(response) if response.status.as_u16() == 400 => {",
    ("polyoxide-polymarket/src/clob/error.rs", 356): "fn test_classify_recognizes_verbatim_venue_messages(",
    ("polyoxide-polymarket/src/clob/error.rs", 368): "fn test_classify_preserves_message_verbatim(",
    ("polyoxide-polymarket/src/clob/error.rs", 376): "fn test_classify_is_case_insensitive(",
    ("polyoxide-polymarket/src/clob/error.rs", 389): "fn test_classify_tolerates_curly_apostrophe_in_fok_message(",
    ("polyoxide-polymarket/src/clob/error.rs", 400): "fn test_classify_does_not_capture_neighbouring_400s(",
    ("polyoxide-polymarket/src/clob/error.rs", 431): "fn test_classify_requires_both_tokens(",
    ("polyoxide-polymarket/src/clob/error.rs", 470): "fn the_decode_splits_out_a_kill_only_on_a_400(",
```

- [ ] **Step 5: Run the ledger's test.**

Run: `cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q`
Expected: PASS, with no cited line failing.

- [ ] **Step 6: Commit.**

```bash
git add docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py
git commit -m "docs(mutants): move rows (e) after ClobError::Data and prove them again (Story 6.5)" -- docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py
```

### Task 5.4: CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Name clob's `data` knob and `ClobError::Data`, and add **The CLOB through the traits** after **Market-data traits**.**

Current (`CLAUDE.md`):

```markdown
**Builder pattern** — All clients use builders: `ClobBuilder::new()`, `Clob::builder(private_key, credentials)`, `Gamma::builder()`, `DataApi::builder()`, `Perps::builder()`, `Usdm::builder()`, `RelayClient::default_builder()`, `Polymarket::builder(account)`. Each venue builder holds a `polyoxide_core::ClientConfig` (base URL, timeout, pool size, retry config, concurrency, and the client's default concurrency: relay 2; gamma, data, perps and Binance 4; clob 8) and gets its five transport setters, `base_url`, `timeout_ms`, `pool_size`, `with_retry_config` and `max_concurrent`, from `polyoxide_core::client_config_setters!`. Its `build` starts from `ClientConfig::http_builder()` and adds the throttle and retry policy the venue installs; the config sets no `gzip`. A venue's own knobs stay on its builder: data's `pnl_base_url` and `rankings_base_url`, perps' `with_rate_limiter`, clob's `chain`, `signature_type`, `builder_code`, `gamma` (built only with the `gamma` feature, which `clob` does not imply) and `with_account`, relay's `url`, `chain_id`, account, auth and wallet setters, and Binance's `weight_budget`. Relay keeps `url(&str) -> Result`, which checks the URL at once, beside `base_url`, and its `build` adds the trailing slash either way, so a path prefix is kept. The unified client, `polyoxide_polymarket::Polymarket` (built with `clob`, `gamma` and `data`; Story 5.7 moved it out of the umbrella, which re-exports it as `polyoxide::polymarket::Polymarket` and from its prelude), builds one CLOB, Gamma and Data client from one `Account`; `Polymarket::new` and `PolymarketBuilder::build` return a `BuildError` whose `Clob`, `Gamma` and `Data` variants wrap the error of the client that could not be built and classify as it does. `PolymarketError` is gone.
```

Replace with:

```markdown
**Builder pattern** — All clients use builders: `ClobBuilder::new()`, `Clob::builder(private_key, credentials)`, `Gamma::builder()`, `DataApi::builder()`, `Perps::builder()`, `Usdm::builder()`, `RelayClient::default_builder()`, `Polymarket::builder(account)`. Each venue builder holds a `polyoxide_core::ClientConfig` (base URL, timeout, pool size, retry config, concurrency, and the client's default concurrency: relay 2; gamma, data, perps and Binance 4; clob 8) and gets its five transport setters, `base_url`, `timeout_ms`, `pool_size`, `with_retry_config` and `max_concurrent`, from `polyoxide_core::client_config_setters!`. Its `build` starts from `ClientConfig::http_builder()` and adds the throttle and retry policy the venue installs; the config sets no `gzip`. A venue's own knobs stay on its builder: data's `pnl_base_url` and `rankings_base_url`, perps' `with_rate_limiter`, clob's `chain`, `signature_type`, `builder_code`, `gamma` (built only with the `gamma` feature, which `clob` does not imply), `data` (built only with the `data` feature, for the `Trades` the CLOB reads through the Data API; `build` makes one with the builder's timeout and pool size when none is given, and the unified `Polymarket` client hands clob its own) and `with_account`, relay's `url`, `chain_id`, account, auth and wallet setters, and Binance's `weight_budget`. Relay keeps `url(&str) -> Result`, which checks the URL at once, beside `base_url`, and its `build` adds the trailing slash either way, so a path prefix is kept. The unified client, `polyoxide_polymarket::Polymarket` (built with `clob`, `gamma` and `data`; Story 5.7 moved it out of the umbrella, which re-exports it as `polyoxide::polymarket::Polymarket` and from its prelude), builds one CLOB, Gamma and Data client from one `Account`; `Polymarket::new` and `PolymarketBuilder::build` return a `BuildError` whose `Clob`, `Gamma` and `Data` variants wrap the error of the client that could not be built and classify as it does. `PolymarketError` is gone.
```

Current (`CLAUDE.md`):

```markdown
**Every error implements `polyoxide_venue::Classify`** — inner and auxiliary types included (`V2Error`, `VenueError`, `BurstCapacityExceeded`, `Refused`, `ParseTickSizeError`, `polyoxide-venue`'s `UnknownVariant`, `InvalidSymbol`, `InvalidStreamName`). `class()` is one of the eight `Class` variants of AD-15, decided by the status before the body; `is_fault()` is false only where the venue answered as designed (the FAK and FOK kills, and a 451 region block wherever a status decides the class); `retry_after()` is the wait the server asked for. The class decides retriability for callers: `Network`, `Unavailable` and `RateLimited` are retriable. `ApiError::Response` is classed by its status (any status outside 400–599 a `Decode`, such as a ping's 2xx whose body says it is not ok), with a `429`'s `Retry-After` as its wait and every status's `Retry-After` as `retry_after()`; `ApiError::Validation` and `Sign` are `InvalidRequest`. No error has an inherent `is_retriable` or `retry_after`, the socket errors included: data's v2 `retryable` flag is surfaced on `V2Error`, not obeyed, and Binance's `418`, `451` and firewall `403` are `Restricted` (D14), not retriable. A failure of clob's Gamma dependency is `ClobError::Gamma`, classed as the Gamma error, and a reqwest error that broke off mid-body is `Network` though reqwest labels it a decode error. Each crate's `lib.rs` lists its error types in a `const _` assertion, gated like the types (`polyoxide-venue`'s names the trait `crate::Classify`), so a missing impl fails the build, and `.github/scripts/tests/test_classify_coverage.py` fails when a public error type is missing from that list, or when a type with a `WsError` field sits in a file that does not expand the socket table's macro, or that file does not call the three functions the macro defines. Besides venue's `ClassifiedError`, which must not implement the trait, the sweep exempts `polyoxide-ws`'s `WsError`: venue does not depend on ws (AD-2) and ws does not depend on venue, so neither can implement the trait for it, and in a venue crate both the trait and the type are foreign (the orphan rule), so each venue error that wraps it classifies it through `polyoxide_venue::impl_ws_classification!`, a macro the venue crate expands. `polyoxide-venue` also holds the status and close-code maps and the one `Retry-After` parser, `parse_retry_after`, which every `Retry-After` read goes through with its caller's clamp: core's `max_backoff_ms` for the wait the loop sleeps, none for core's `ErrorResponse::retry_after`, the wait an error surfaces, which perps' `VenueError` and data's `V2Error` copy, and Binance's 3 days for every arm of its decode, a core `Response` it keeps included. A zero is no wait anywhere (DRIFT R4). No crate matches over `tungstenite::Error`: every socket error is on the one socket table, `polyoxide_venue::impl_ws_classification!()` (Story 4.3), perps (Story 4.6), Binance (Story 4.7), rtds (Story 4.8), clob (Story 4.9) and sports (Story 4.10), `PerpsWsError::Connection`, `UsdmWsError::Connect`, `RtdsError::Connection`, `ClobWsError::Connection` and `SportsError::{Connect, Transport}` wrapping `polyoxide_ws::WsError` (Binance and sports map the kit's server close and connect timeout to their own `Closed` and `ConnectTimeout`, and Binance keeps its handshake `403` → `Restricted` row ahead of the table, D14; clob maps a timed-out connect step to its own `ConnectTimeout { url, timeout }`, and a host that resolves to no address is an I/O failure, `Network`), so a 451 refusal is each one's one transport non-fault. Each recovery retries a handshake refused with 408, 425, 429 or 5xx through `polyoxide_ws::retrying_can_fix` (DRIFT R1). The table: it expands, in the invoking module, to private `ws_class`, `ws_is_fault` and `ws_retry_after` over `polyoxide_ws::WsError`'s `Kind`, which a venue's `Classify` impl calls on its `WsError` arm. No lint reports one the venue never calls (rustc does not warn about dead code expanded from another crate's macro), so `test_classify_coverage.py` is the guard: a file that expands the macro must call all three in code, and a type with a `WsError` field must sit in a file that expands it, the file the fence checks. A refused upgrade follows `class_for_handshake_status` with its `Retry-After` read unclamped, a server close `class_for_close_code`, and only a 451 refusal is not a fault; every refusal's `Retry-After`, whatever its status, is the error's `retry_after()`, as HTTP's `ErrorResponse` surfaces it, though only a 429's class carries it (Story 4.10); `polyoxide-venue/tests/ws_classification.rs` covers every row, and that `polyoxide_ws::retrying_can_fix`, given venue's status rule, agrees with the class on every refused status.
```

Replace with:

```markdown
**Every error implements `polyoxide_venue::Classify`** — inner and auxiliary types included (`V2Error`, `VenueError`, `BurstCapacityExceeded`, `Refused`, `ParseTickSizeError`, `polyoxide-venue`'s `UnknownVariant`, `InvalidSymbol`, `InvalidStreamName`). `class()` is one of the eight `Class` variants of AD-15, decided by the status before the body; `is_fault()` is false only where the venue answered as designed (the FAK and FOK kills, and a 451 region block wherever a status decides the class); `retry_after()` is the wait the server asked for. The class decides retriability for callers: `Network`, `Unavailable` and `RateLimited` are retriable. `ApiError::Response` is classed by its status (any status outside 400–599 a `Decode`, such as a ping's 2xx whose body says it is not ok), with a `429`'s `Retry-After` as its wait and every status's `Retry-After` as `retry_after()`; `ApiError::Validation` and `Sign` are `InvalidRequest`. No error has an inherent `is_retriable` or `retry_after`, the socket errors included: data's v2 `retryable` flag is surfaced on `V2Error`, not obeyed, and Binance's `418`, `451` and firewall `403` are `Restricted` (D14), not retriable. A failure of clob's Gamma dependency is `ClobError::Gamma`, classed as the Gamma error, and a Data client clob could not build is `ClobError::Data`, classed as the Data error, and a reqwest error that broke off mid-body is `Network` though reqwest labels it a decode error. Each crate's `lib.rs` lists its error types in a `const _` assertion, gated like the types (`polyoxide-venue`'s names the trait `crate::Classify`), so a missing impl fails the build, and `.github/scripts/tests/test_classify_coverage.py` fails when a public error type is missing from that list, or when a type with a `WsError` field sits in a file that does not expand the socket table's macro, or that file does not call the three functions the macro defines. Besides venue's `ClassifiedError`, which must not implement the trait, the sweep exempts `polyoxide-ws`'s `WsError`: venue does not depend on ws (AD-2) and ws does not depend on venue, so neither can implement the trait for it, and in a venue crate both the trait and the type are foreign (the orphan rule), so each venue error that wraps it classifies it through `polyoxide_venue::impl_ws_classification!`, a macro the venue crate expands. `polyoxide-venue` also holds the status and close-code maps and the one `Retry-After` parser, `parse_retry_after`, which every `Retry-After` read goes through with its caller's clamp: core's `max_backoff_ms` for the wait the loop sleeps, none for core's `ErrorResponse::retry_after`, the wait an error surfaces, which perps' `VenueError` and data's `V2Error` copy, and Binance's 3 days for every arm of its decode, a core `Response` it keeps included. A zero is no wait anywhere (DRIFT R4). No crate matches over `tungstenite::Error`: every socket error is on the one socket table, `polyoxide_venue::impl_ws_classification!()` (Story 4.3), perps (Story 4.6), Binance (Story 4.7), rtds (Story 4.8), clob (Story 4.9) and sports (Story 4.10), `PerpsWsError::Connection`, `UsdmWsError::Connect`, `RtdsError::Connection`, `ClobWsError::Connection` and `SportsError::{Connect, Transport}` wrapping `polyoxide_ws::WsError` (Binance and sports map the kit's server close and connect timeout to their own `Closed` and `ConnectTimeout`, and Binance keeps its handshake `403` → `Restricted` row ahead of the table, D14; clob maps a timed-out connect step to its own `ConnectTimeout { url, timeout }`, and a host that resolves to no address is an I/O failure, `Network`), so a 451 refusal is each one's one transport non-fault. Each recovery retries a handshake refused with 408, 425, 429 or 5xx through `polyoxide_ws::retrying_can_fix` (DRIFT R1). The table: it expands, in the invoking module, to private `ws_class`, `ws_is_fault` and `ws_retry_after` over `polyoxide_ws::WsError`'s `Kind`, which a venue's `Classify` impl calls on its `WsError` arm. No lint reports one the venue never calls (rustc does not warn about dead code expanded from another crate's macro), so `test_classify_coverage.py` is the guard: a file that expands the macro must call all three in code, and a type with a `WsError` field must sit in a file that expands it, the file the fence checks. A refused upgrade follows `class_for_handshake_status` with its `Retry-After` read unclamped, a server close `class_for_close_code`, and only a 451 refusal is not a fault; every refusal's `Retry-After`, whatever its status, is the error's `retry_after()`, as HTTP's `ErrorResponse` surfaces it, though only a 429's class carries it (Story 4.10); `polyoxide-venue/tests/ws_classification.rs` covers every row, and that `polyoxide_ws::retrying_can_fix`, given venue's status rule, agrees with the class on every refused status.
```

Current (`CLAUDE.md`):

```markdown
`Classify`; `ResultExt::or_fail` and it may be in scope together.
```

Replace with:

```markdown
`Classify`; `ResultExt::or_fail` and it may be in scope together.

**The CLOB through the traits** — `clob::venue` (Story 6.5) implements `MarketData` and `Candles`
on `Clob` with `clob` alone, `Trades` with `data` as well and `EventGroups` with `gamma` as well
(`all(clob, data)` and `all(clob, gamma)`), keyed by `shared::keys::clob`: a key names one outcome
token (AD-7). `instrument` resolves the token's market with `/markets-by-token/{token}`, then reads
`/markets/{condition}` (a token the CLOB does not know is its `404`, a `VenueRefusal`); `quote`
and `book` read `/book`, whose sides the CLOB sends worst first, so both are sorted best first;
`candles` read `/prices-history`, one price per interval as `close`, the `CandleInterval` sent as
`fidelity` minutes; `trades` read the Data API's taker-only `/trades` for the token's market and
keep the token's rows, oldest first, with no trade id (the Data API gives none); and `event_group`
reads the event id from Gamma's `/markets?clob_token_ids=` (a private decode, since Gamma's
`Market` does not model its `events`), then `/events/{id}`, keeping only `v1` markets' outcomes.
The CLOB does not implement `Instruments` (amendment A6-1). A number the CLOB or Data API sends as
a float is read through its shortest decimal spelling. `tests/venue_clob.rs` holds each method to
the native clients' values on `tests/fixtures/clob/venue/`, captured for one live neg-risk outcome
by `scripts/capture_polymarket_clob.py`.
```

- [ ] **Step 2: Run the story's checks.**

Run: `cargo fmt --all -- --check && cargo clippy -p polyoxide-polymarket -p polyoxide --all-targets --all-features -- -D warnings && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py`
Expected: all clean.

- [ ] **Step 3: Commit.**

```bash
git commit -m "docs: CLAUDE.md says what clob::venue implements and where its trades come from (Story 6.5)" -- CLAUDE.md
```

## Story 6: One generic function across venues (Story 6.6)

- **Developer:** intermediate-dev
- **Files:** `polyoxide/examples/venue_neutral.rs`, `polyoxide/tests/venue_neutral.rs`, `polyoxide/tests/live_venue.rs`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/README.md`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md`, `docs/s3-migration.md`, `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `.github/workflows/nightly-behavioral.yml`, `Cargo.lock`
- **Interfaces:** exposes the umbrella example's `best_bid_ask(&DynMarketData<'_>, &MarketKey)`, `classify<E: Classify + ?Sized>`, `classify_classified` and `Handling`; the traits on `polyoxide::venue` under `polymarket-clob`, `polymarket-perps` and `binance-usdm`; the nightly job `live-polyoxide-venue`. Uses every earlier story.
- **Verify:** `cargo test -p polyoxide --all-features --test venue_neutral --test parse_key && cargo test -p polyoxide --all-features --doc && python3 scripts/gen_registry.py --check`

The example is written once against `polyoxide::venue`'s `MarketData` and `Classify`, and one suite, `tests/venue_neutral.rs`, runs both functions unchanged against each module: that is CAP-5's and CAP-4's success criterion. The classifier reads a module's native error through `Classify`, and a trait's `ClassifiedError` through the same table, and the suite asserts both reach the same `Handling` for each module and status.

### Task 6.1: The example and its two suites

**Files:**
- Create: `polyoxide/examples/venue_neutral.rs`, `polyoxide/tests/venue_neutral.rs`, `polyoxide/tests/live_venue.rs`
- Modify: `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/README.md`

- [ ] **Step 1: Write the mock suite first.** It includes the example by `#[path]`, runs `best_bid_ask` against a mock server for each module (the CLOB's sides sent worst first), and runs one classifier over each module's native error and its trait error for a 429 with `Retry-After`, a 503, a 401, a 451 and a 400, with no retries so a refused request answers at once.

Create `polyoxide/tests/venue_neutral.rs` with:

```rust
//! The example's `best_bid_ask` and `classify`, written once, run unchanged
//! against mock servers for the clob, perps and usdm modules: CAP-5's and
//! CAP-4's success criteria.

#[allow(dead_code)]
#[path = "../examples/venue_neutral.rs"]
mod example;

use std::sync::Arc;
use std::time::Duration;

use example::{best_bid_ask, classify, classify_classified, Handling};
use mockito::{Matcher, Server, ServerGuard};
use polyoxide::binance::usdm::{
    self,
    types::{DepthLimit, Symbol},
    Usdm,
};
use polyoxide::polymarket::clob::{Clob, ClobBuilder};
use polyoxide::polymarket::perps::{types::InstrumentId, Perps};
use polyoxide::polymarket::shared::keys::{self, TokenId};
use polyoxide::venue::{DynMarketData, Level, MarketKey};
use polyoxide_core::RetryConfig;
use rust_decimal::Decimal;

const TOKEN: &str = "54533043819946592547517511176940999955633860128497669742211153063842200957669";

/// A module's client, its key, and the route and body its quote reads.
struct Module {
    name: &'static str,
    path: &'static str,
    query: String,
    body: &'static str,
}

fn modules() -> [Module; 3] {
    [
        Module {
            name: "usdm",
            path: "/fapi/v1/depth",
            query: "symbol=BTCUSDT&limit=5".to_owned(),
            body: r#"{"lastUpdateId":1,"E":1791383684399,"T":1791383684397,"bids":[["83120.70","4.483"],["83120.60","0.002"]],"asks":[["83120.80","3.638"],["83120.90","0.001"]]}"#,
        },
        Module {
            name: "perps",
            path: "/v1/info/bbo",
            query: "instrument_id=1".to_owned(),
            body: r#"[{"instrument_id":1,"bid_price":"7681.6","bid_quantity":"1.03204","ask_price":"7681.7","ask_quantity":"19.90711","timestamp":1790761918305}]"#,
        },
        Module {
            name: "clob",
            path: "/book",
            query: format!("token_id={TOKEN}"),
            // Each side worst first, as the CLOB sends it.
            body: r#"{"market":"0x0f49","asset_id":"5453","timestamp":"1792161583225","hash":"h","bids":[{"price":"0.13","size":"10"},{"price":"0.15","size":"20"}],"asks":[{"price":"0.18","size":"10"},{"price":"0.16","size":"20"}],"min_order_size":"5","tick_size":"0.001","neg_risk":true,"last_trade_price":"0.15"}"#,
        },
    ]
}

/// No retries, so a refused request answers at once.
fn once() -> RetryConfig {
    RetryConfig {
        max_retries: 0,
        ..RetryConfig::default()
    }
}

fn usdm_client(server: &ServerGuard) -> Usdm {
    Usdm::builder()
        .base_url(server.url())
        .with_retry_config(once())
        .build()
        .unwrap()
}

fn perps_client(server: &ServerGuard) -> Perps {
    Perps::builder()
        .base_url(server.url())
        .with_retry_config(once())
        .build()
        .unwrap()
}

fn clob_client(server: &ServerGuard) -> Clob {
    ClobBuilder::new()
        .base_url(server.url())
        .with_retry_config(once())
        .build()
        .unwrap()
}

/// The module's client as a `DynMarketData`, and the key its quote reads.
fn venue(name: &str, server: &ServerGuard) -> (Arc<DynMarketData<'static>>, MarketKey) {
    match name {
        "usdm" => (
            DynMarketData::new_arc(usdm_client(server)),
            usdm::key(&Symbol::new("BTCUSDT").unwrap()),
        ),
        "perps" => (
            DynMarketData::new_arc(perps_client(server)),
            keys::perps(InstrumentId(1)),
        ),
        "clob" => (
            DynMarketData::new_arc(clob_client(server)),
            keys::clob(&TokenId::new(TOKEN).unwrap()),
        ),
        other => panic!("no module {other}"),
    }
}

fn level(price: &str, size: &str) -> Level {
    Level::new(
        price.parse::<Decimal>().unwrap(),
        size.parse::<Decimal>().unwrap(),
    )
}

#[tokio::test]
async fn best_bid_ask_reads_each_module_unchanged() {
    let expected = [
        (
            "usdm",
            level("83120.70", "4.483"),
            level("83120.80", "3.638"),
        ),
        (
            "perps",
            level("7681.6", "1.03204"),
            level("7681.7", "19.90711"),
        ),
        ("clob", level("0.15", "20"), level("0.16", "20")),
    ];
    for (module, (name, bid, ask)) in modules().into_iter().zip(expected) {
        assert_eq!(module.name, name);
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", module.path)
            .match_query(Matcher::Exact(module.query.clone()))
            .with_status(200)
            .with_body(module.body)
            .create_async()
            .await;
        let (venue, key) = venue(name, &server);
        let quote = best_bid_ask(&venue, &key).await.unwrap();
        mock.assert_async().await;
        assert_eq!(quote, (Some(bid), Some(ask)), "{name}");
    }
}

/// Fails the module's own quote route with `status`, through its native
/// client, and returns how the classifier reads that error.
async fn native(name: &str, server: &ServerGuard) -> Handling {
    match name {
        "usdm" => {
            let err = usdm_client(server)
                .market()
                .depth(&Symbol::new("BTCUSDT").unwrap())
                .limit(DepthLimit::Five)
                .send()
                .await
                .unwrap_err();
            classify(&err)
        }
        "perps" => {
            let err = perps_client(server)
                .market()
                .bbo()
                .instrument_id(InstrumentId(1))
                .send()
                .await
                .unwrap_err();
            classify(&err)
        }
        "clob" => {
            let err = clob_client(server)
                .markets()
                .order_book(TOKEN)
                .send()
                .await
                .unwrap_err();
            classify(&err)
        }
        other => panic!("no module {other}"),
    }
}

#[tokio::test]
async fn one_classifier_reads_each_modules_errors_natively_and_through_the_traits() {
    let cases = [
        (
            429,
            Some("1"),
            Handling::Backoff(Some(Duration::from_secs(1))),
        ),
        (503, None, Handling::Retry),
        (401, None, Handling::Reauthenticate),
        (451, None, Handling::Restricted { alert: false }),
        (400, None, Handling::Rejected { alert: true }),
    ];
    for (status, retry_after, expected) in cases {
        for module in modules() {
            let mut server = Server::new_async().await;
            let mut mock = server
                .mock("GET", module.path)
                .match_query(Matcher::Any)
                .with_status(status)
                .with_body(r#"{"code":-1,"msg":"refused","error":"refused"}"#);
            if let Some(wait) = retry_after {
                mock = mock.with_header("retry-after", wait);
            }
            let _mock = mock.create_async().await;

            assert_eq!(
                native(module.name, &server).await,
                expected,
                "{} {status}, native",
                module.name
            );
            let (venue, key) = venue(module.name, &server);
            let err = best_bid_ask(&venue, &key).await.unwrap_err();
            assert_eq!(
                classify_classified(&err),
                expected,
                "{} {status}, through the traits",
                module.name
            );
        }
    }
}

#[test]
fn the_traits_are_on_polyoxide_venue() {
    fn market_data<T: polyoxide::venue::MarketData>() {}
    fn trades<T: polyoxide::venue::Trades>() {}
    fn candles<T: polyoxide::venue::Candles>() {}
    fn funding<T: polyoxide::venue::Funding>() {}
    fn instruments<T: polyoxide::venue::Instruments>() {}
    market_data::<Usdm>();
    market_data::<Perps>();
    market_data::<Clob>();
    trades::<Usdm>();
    candles::<Clob>();
    funding::<Perps>();
    instruments::<Usdm>();
}
```

- [ ] **Step 2: Write the live suite.** It fails through `or_fail`: a native client's `Result` through `ResultExt`, a trait's through `ClassifiedResultExt`. A market with nothing that takes orders is `environmental`, and a US runner's 451 from Binance is `Restricted` and not a fault, so it tags `environmental` too.

Create `polyoxide/tests/live_venue.rs` with:

```rust
//! The example's `best_bid_ask` against the live hosts: the same function
//! reads Binance USDⓈ-M, Polymarket perps and the Polymarket CLOB.

#[allow(dead_code)]
#[path = "../examples/venue_neutral.rs"]
mod example;

use std::sync::Arc;

use example::best_bid_ask;
use polyoxide::binance::usdm::{self, types::Symbol, Usdm};
use polyoxide::polymarket::clob::Clob;
use polyoxide::polymarket::perps::Perps;
use polyoxide::polymarket::shared::keys::{self, TokenId};
use polyoxide::venue::{DynInstruments, DynMarketData, Instruments, Level, MarketKey};
use polyoxide_test_support::{environmental, ClassifiedResultExt, ResultExt};

/// Reads `key`'s best bid and ask through `venue`, and checks the two do not
/// cross.
async fn read(
    venue: Arc<DynMarketData<'static>>,
    key: &MarketKey,
) -> (Option<Level>, Option<Level>) {
    let (bid, ask) = best_bid_ask(&venue, key)
        .await
        .or_fail("read the best bid and ask");
    if let (Some(bid), Some(ask)) = (bid, ask) {
        assert!(
            bid.price < ask.price,
            "{key}: bid {bid:?} crosses ask {ask:?}"
        );
    }
    (bid, ask)
}

#[tokio::test]
#[ignore = "hits the live hosts"]
async fn best_bid_ask_reads_binance_usdm() {
    let venue = DynMarketData::new_arc(Usdm::new().or_fail("build the usdm client"));
    let key = usdm::key(&Symbol::new("BTCUSDT").or_fail("name BTCUSDT"));
    let (bid, ask) = read(venue, &key).await;
    assert!(
        bid.is_some() && ask.is_some(),
        "BTCUSDT always has both sides"
    );
}

#[tokio::test]
#[ignore = "hits the live hosts"]
async fn best_bid_ask_reads_polymarket_perps() {
    let perps = Perps::new().or_fail("build the perps client");
    let listing: Arc<DynInstruments<'static>> = DynInstruments::new_arc(perps.clone());
    let instruments = listing.instruments().await.or_fail("list the instruments");
    let Some(instrument) = instruments
        .iter()
        .find(|instrument| instrument.tradable != Some(false))
    else {
        environmental("perps lists no instrument that takes orders");
    };
    read(DynMarketData::new_arc(perps), &instrument.key).await;
}

#[tokio::test]
#[ignore = "hits the live hosts"]
async fn best_bid_ask_reads_the_polymarket_clob() {
    let clob = Clob::public();
    let open = clob
        .markets()
        .sampling()
        .send()
        .await
        .or_fail("list the sampling markets");
    let Some(token) = open
        .data
        .iter()
        .filter(|market| market.accepting_orders == Some(true))
        .flat_map(|market| &market.tokens)
        .find_map(|token| token.token_id.clone())
    else {
        environmental("no market on the first sampling page takes orders");
    };
    let key = keys::clob(&TokenId::new(token).or_fail("read the token id"));
    read(DynMarketData::new_arc(clob), &key).await;
}
```

- [ ] **Step 3: Register the example, the two suites and the live target, add the dev-dependencies (the two workspace crates path-only, so cargo strips them on publish), and turn on `polyoxide-venue/market` or `keys` with each module.**

Current (`polyoxide/Cargo.toml`):

```toml
# through, and a `-ws` one turns its module's on as well. `keychain` is an
# extra across venues: each venue that stores credentials adds its own
# `?/keychain`.
[features]
default = ["polymarket"]
polymarket = ["polymarket-clob", "polymarket-gamma", "polymarket-data"]
polymarket-clob = ["dep:polyoxide-polymarket", "polyoxide-polymarket/clob"]
polymarket-clob-ws = ["polymarket-clob", "polyoxide-polymarket/clob-ws"]
polymarket-gamma = ["dep:polyoxide-polymarket", "polyoxide-polymarket/gamma"]
polymarket-data = ["dep:polyoxide-polymarket", "polyoxide-polymarket/data"]
polymarket-relay = ["dep:polyoxide-polymarket", "polyoxide-polymarket/relay"]
polymarket-perps = ["dep:polyoxide-polymarket", "polyoxide-polymarket/perps"]
```

Replace with:

```toml
# through, and a `-ws` one turns its module's on as well. A module that
# implements the market-data traits turns on `polyoxide-venue/market`, and one
# that only names keys `polyoxide-venue/keys`, so `polyoxide::venue` carries
# what its modules use (Story 6.6). `keychain` is an extra across venues: each
# venue that stores credentials adds its own `?/keychain`.
[features]
default = ["polymarket"]
polymarket = ["polymarket-clob", "polymarket-gamma", "polymarket-data"]
polymarket-clob = ["dep:polyoxide-polymarket", "polyoxide-polymarket/clob", "polyoxide-venue/market"]
polymarket-clob-ws = ["polymarket-clob", "polyoxide-polymarket/clob-ws"]
polymarket-gamma = ["dep:polyoxide-polymarket", "polyoxide-polymarket/gamma", "polyoxide-venue/keys"]
polymarket-data = ["dep:polyoxide-polymarket", "polyoxide-polymarket/data", "polyoxide-venue/keys"]
polymarket-relay = ["dep:polyoxide-polymarket", "polyoxide-polymarket/relay"]
polymarket-perps = ["dep:polyoxide-polymarket", "polyoxide-polymarket/perps", "polyoxide-venue/market"]
```

Current (`polyoxide/Cargo.toml`):

```toml
binance-usdm = ["dep:polyoxide-binance", "polyoxide-binance/usdm"]
```

Replace with:

```toml
binance-usdm = ["dep:polyoxide-binance", "polyoxide-binance/usdm", "polyoxide-venue/market"]
```

Current (`polyoxide/Cargo.toml`):

```toml
# Needed only so the WebSocket README doctests can bring `StreamExt` into scope.
futures-util = "0.3"
```

Replace with:

```toml
# Needed only so the WebSocket README doctests can bring `StreamExt` into scope.
futures-util = "0.3"
# `tests/venue_neutral.rs`: mock servers, prices, and a retry schedule of none.
mockito = { workspace = true }
rust_decimal = { workspace = true }
# Path-only, so cargo strips them on publish: the retry schedule's type, and the
# live suite's failure tags.
polyoxide-core = { path = "../polyoxide-core" }
polyoxide-test-support = { path = "../polyoxide-test-support" }

# The example's generic code and its two suites use one module of each kind:
# the CLOB, perps and USDⓈ-M.
[[example]]
name = "venue_neutral"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]

[[test]]
name = "live_venue"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]
```

Current (`polyoxide/Cargo.toml`):

```toml
name = "parse_key"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]
```

Replace with:

```toml
name = "parse_key"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]

[[test]]
name = "venue_neutral"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]
```

Current (`polyoxide/Cargo.toml`):

```toml
readme = "The multi-venue umbrella: `polyoxide::polymarket` and `polyoxide::binance` behind features named for each venue and module, `polyoxide::venue` and a `prelude`"
```

Replace with:

```toml
readme = "The multi-venue umbrella: `polyoxide::polymarket` and `polyoxide::binance` behind features named for each venue and module, `polyoxide::venue` with the market-data traits, `polyoxide::parse_key` and a `prelude`"

[package.metadata.polyoxide.live.live_venue]
suite = "venue"
timeout = 15
features = ["polymarket-clob", "polymarket-perps", "binance-usdm"]
secrets = []
```

- [ ] **Step 4: Run the mock suite and watch it fail.**

Run: `cargo test -p polyoxide --all-features --test venue_neutral`
Expected: FAIL: `examples/venue_neutral.rs`, which the example target and the `#[path]` include name, does not exist yet.

- [ ] **Step 5: Create the example.** Its `main` reads one live key of each venue; the suites ignore it.

Create `polyoxide/examples/venue_neutral.rs` with:

```rust
//! One function reads any venue's best bid and ask, and one classifier reads
//! any venue's error. Written once, against `polyoxide::venue`'s
//! `MarketData` and `Classify`, they run unchanged on the Polymarket CLOB,
//! Polymarket perps and Binance USDⓈ-M; adding a venue adds no code here.
//!
//! ```text
//! cargo run -p polyoxide --example venue_neutral \
//!     --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm
//! ```
//!
//! `tests/venue_neutral.rs` runs both functions against mock servers for each
//! module, and `tests/live_venue.rs` runs `best_bid_ask` against the live hosts.

use std::sync::Arc;
use std::time::Duration;

use polyoxide::binance::usdm::{self, types::Symbol, Usdm};
use polyoxide::polymarket::clob::Clob;
use polyoxide::polymarket::perps::{types::InstrumentId, Perps};
use polyoxide::polymarket::shared::keys::{self, TokenId};
use polyoxide::venue::{
    Class, ClassifiedError, Classify, DynMarketData, Level, MarketData, MarketKey,
};

/// The best bid and ask of `key`, whichever venue `venue` reads.
pub async fn best_bid_ask(
    venue: &DynMarketData<'_>,
    key: &MarketKey,
) -> Result<(Option<Level>, Option<Level>), ClassifiedError> {
    let quote = venue.quote(key).await?;
    Ok((quote.bid, quote.ask))
}

/// What a consumer does about an error, decided from its class alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handling {
    /// Slow down, for the wait the venue asked for when it gave one.
    Backoff(Option<Duration>),
    /// Try again: no answer came, or the venue could not answer now.
    Retry,
    /// The credentials are missing, wrong or not allowed.
    Reauthenticate,
    /// The venue will not serve this caller. Alert only on a fault.
    Restricted {
        /// Whether the error is a fault.
        alert: bool,
    },
    /// The request was wrong, or the venue refused it. Alert only on a fault.
    Rejected {
        /// Whether the error is a fault.
        alert: bool,
    },
}

/// Reads any module's error, whichever venue raised it, through `Classify`.
pub fn classify<E: Classify + ?Sized>(err: &E) -> Handling {
    handling(&err.class(), err.is_fault(), err.retry_after())
}

/// Reads the error a venue trait returns, which carries what `Classify`
/// reported.
pub fn classify_classified(err: &ClassifiedError) -> Handling {
    handling(&err.class, err.is_fault(), err.retry_after())
}

fn handling(class: &Class, fault: bool, retry_after: Option<Duration>) -> Handling {
    match class {
        Class::RateLimited { .. } => Handling::Backoff(retry_after),
        Class::Network | Class::Unavailable { .. } => Handling::Retry,
        Class::Unauthorized => Handling::Reauthenticate,
        Class::Restricted => Handling::Restricted { alert: fault },
        _ => Handling::Rejected { alert: fault },
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let clob = Clob::public();
    let open = clob.markets().sampling().send().await?;
    let token = open
        .data
        .iter()
        .filter(|market| market.accepting_orders == Some(true))
        .flat_map(|market| &market.tokens)
        .find_map(|token| token.token_id.clone())
        .ok_or("no open market on the CLOB's first sampling page")?;

    let venues: [(Arc<DynMarketData<'static>>, MarketKey); 3] = [
        (
            DynMarketData::new_arc(Usdm::new()?),
            usdm::key(&Symbol::new("BTCUSDT")?),
        ),
        (
            DynMarketData::new_arc(Perps::new()?),
            keys::perps(InstrumentId(1)),
        ),
        (
            DynMarketData::new_arc(clob),
            keys::clob(&TokenId::new(token)?),
        ),
    ];
    for (venue, key) in &venues {
        match best_bid_ask(venue, key).await {
            Ok((bid, ask)) => println!("{key}: bid {bid:?}, ask {ask:?}"),
            Err(err) => println!("{key}: {err} ({:?})", classify_classified(&err)),
        }
    }
    Ok(())
}
```

- [ ] **Step 6: Pin the traits and `parse_key` to the features that carry them, and point the crate docs at the example.** The two new items are `const _` pins like the file's others; `plan_premises.py` flags `_` as already defined, which is expected for anonymous consts.

Current (`polyoxide/src/lib.rs`):

```rust
//!   on.
//! - `polyoxide::polymarket` is `polyoxide-polymarket`, under `polymarket`
```

Replace with:

```rust
//!   on. With any module that implements them (`polymarket-clob`,
//!   `polymarket-perps`, `binance-usdm`) it also holds the market-data traits,
//!   `MarketData` and its capabilities, which read every venue the same way:
//!   `examples/venue_neutral.rs` reads the best bid and ask of the CLOB, perps
//!   and USDⓈ-M with one function.
//! - `polyoxide::polymarket` is `polyoxide-polymarket`, under `polymarket`
```

Current (`polyoxide/src/lib.rs`):

```rust
// The same for each of the prelude's `cfg` blocks, gated on the feature it is
```

Replace with:

```rust
// The market-data traits are on `polyoxide::venue` with every module that
// implements them, and `parse_key` with every module that names keys.
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-perps",
    feature = "binance-usdm"
))]
const _: fn(&crate::venue::DynMarketData<'static>) = |_| {};
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-gamma",
    feature = "polymarket-data",
    feature = "polymarket-perps",
    feature = "binance-usdm"
))]
const _: fn(&crate::venue::RawKey) -> Result<crate::venue::MarketKey, crate::venue::KeyError> =
    crate::parse_key;

// The same for each of the prelude's `cfg` blocks, gated on the feature it is
```

- [ ] **Step 7: Name the traits, `parse_key` and the example in the umbrella's README.**

Current (`polyoxide/README.md`):

```markdown
| `polyoxide::venue` | `polyoxide-venue`: the vocabulary every venue shares, such as the error classes and `Classify` | always on |
| `polyoxide::polymarket` | `polyoxide-polymarket`: [Polymarket](https://polymarket.com)'s CLOB, Gamma, Data, Relay, Perps, RTDS and Sports modules, and the unified `Polymarket` client | `polymarket` and `polymarket-<module>` |
| `polyoxide::binance` | `polyoxide-binance`: Binance's USDⓈ-M futures market data and streams | `binance` and `binance-<module>` |
| `polyoxide::prelude` | A selection of the enabled modules' clients, errors and stream types | |
```

Replace with:

```markdown
| `polyoxide::venue` | `polyoxide-venue`: the vocabulary every venue shares, such as the error classes and `Classify`, and, with any module that implements them, the market-data traits (`MarketData` and its capabilities) | always on |
| `polyoxide::parse_key` | Reads a market key, `venue.product:id`, through the constructors of the enabled product it names | any module with keys |
| `polyoxide::polymarket` | `polyoxide-polymarket`: [Polymarket](https://polymarket.com)'s CLOB, Gamma, Data, Relay, Perps, RTDS and Sports modules, and the unified `Polymarket` client | `polymarket` and `polymarket-<module>` |
| `polyoxide::binance` | `polyoxide-binance`: Binance's USDⓈ-M futures market data and streams | `binance` and `binance-<module>` |
| `polyoxide::prelude` | A selection of the enabled modules' clients, errors and stream types | |

Code written against `polyoxide::venue`'s traits reads every venue the same way:
[`examples/venue_neutral.rs`](examples/venue_neutral.rs) reads the best bid and ask of the
Polymarket CLOB, Polymarket perps and Binance USDⓈ-M with one function, and their errors with one
classifier.
```

- [ ] **Step 8: Run the mock suite again, every feature build, and the live suite once.**

Run: `cargo test -p polyoxide --all-features --test venue_neutral --test parse_key && cargo test -p polyoxide --all-features --doc && cargo hack check -p polyoxide --each-feature --no-dev-deps`
Expected: PASS: 3 and 3 tests, the doctests, and every feature building.

Run: `cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm --test live_venue -- --ignored`
Expected: 3 passed against the live hosts on 2026-10-16. A failure tagged `environmental` (a 451 from Binance, or no market taking orders) is the world, not the code: report it and go on.

- [ ] **Step 9: Commit.**

```bash
git add polyoxide/examples/venue_neutral.rs polyoxide/tests/venue_neutral.rs polyoxide/tests/live_venue.rs polyoxide/Cargo.toml polyoxide/src/lib.rs polyoxide/README.md Cargo.lock
git commit -m "feat(umbrella): one best_bid_ask and one classifier across the CLOB, perps and USDⓈ-M (Story 6.6)" -- polyoxide/examples/venue_neutral.rs polyoxide/tests/venue_neutral.rs polyoxide/tests/live_venue.rs polyoxide/Cargo.toml polyoxide/src/lib.rs polyoxide/README.md Cargo.lock
```

### Task 6.2: Amendment A6-2, the S3 note, CLAUDE.md and the generated regions

**Files:**
- Create: `docs/s3-migration.md`
- Modify: `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md`, `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `.github/workflows/nightly-behavioral.yml` (the last three's generated regions only)

- [ ] **Step 1: Record the example's link as amendment A6-2.**

Current (`_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md`):

```markdown
- **Follow-up in the code:** Story 6.3 ships the traits in this shape; Stories 6.3 and 6.4 implement `Instruments` for usdm and perps.
```

Replace with:

```markdown
- **Follow-up in the code:** Story 6.3 ships the traits in this shape; Stories 6.3 and 6.4 implement `Instruments` for usdm and perps.

## A6-2: the guide links the venue-neutral example (amends the guide's "Types and keys" and CAP-5's and CAP-4's success criteria)

- **Proposed by:** Story 6.6, as its acceptance criteria ask.
- **Current rule:** CAP-5's success criterion is "one generic function, such as best bid/ask for a key, runs unchanged against each" product, and CAP-4's is "a consumer classifier like prader's `perps_err` is written once, generic over the interface". The guide names neither the function nor where it lives.
- **Proposed rule:** the guide's "Types and keys" section gains one bullet: "**One function, every venue (CAP-4, CAP-5).** [`polyoxide/examples/venue_neutral.rs`](../polyoxide/examples/venue_neutral.rs) reads the best bid and ask of the Polymarket CLOB, Polymarket perps and Binance USDⓈ-M with one `best_bid_ask(&DynMarketData, &MarketKey)`, and reads each module's errors with one classifier written against `Classify`. `polyoxide/tests/venue_neutral.rs` runs both against mock servers for each module, and `tests/live_venue.rs` runs `best_bid_ask` against the live hosts nightly. A new venue whose products implement the traits needs no change to it."
- **Why:** AD-21 makes example links spine content, and the guide is regenerated from the spine. The example is the evidence both success criteria name.
- **Follow-up:** none in the code. The session that merges this file into the spine (through `bmad-architecture`, with A2-1 to A5-1 and A6-1) adds the bullet and regenerates `docs/ARCHITECTURE.md` with `python3 scripts/gen_registry.py --write`.
```

- [ ] **Step 2: Write Epic 6's consumer-impact note (D17).**

Create `docs/s3-migration.md` with:

```markdown
# polyoxide in stage S3: what each release adds

Stage S3 of the multi-venue restructure is for additions (AD-16): the venue-neutral traits, a
supervised clob socket, and the Kalshi skeleton. Its removals, which are few, are listed in
[`s3-removals.md`](s3-removals.md). This page holds the consumer-impact note of each S3 piece of
work, for the CHANGELOG section of the first release that ships it.

## Epic 6: venue-neutral market data

**Consumer impact: additive.** Nothing is removed or renamed, and no signature changes; code
that builds on 0.40 builds unchanged.

- `polyoxide-venue` gains two features. `keys`: `MarketKey`, written `venue.product:id`, which only
  a product's `KeySpace` makes, so two keys are equal exactly when they name the same instrument;
  `RawKey`, text split into its parts; `VenueId`, `ProductId` and `KeyError`. `market`: the
  records (`Instrument`, `Quote`, `Book`, `Level`, `Trade`, `Candle`, `FundingRate`,
  `EventGroup`), `Extensions` for each venue's own facts, `NotListed`, and the traits
  `MarketData` (`instrument`, `quote`, `book`) and `Instruments`, `Trades`, `Candles`,
  `Funding` and `EventGroups`, each with a `Dyn` wrapper. Every trait method returns
  `ClassifiedError`.
- `polyoxide-binance`: `usdm::key` (and `usdm::key(&symbol)`), and `usdm::venue`, whose
  `Usdm` implements `MarketData`, `Instruments`, `Trades`, `Candles` and `Funding`.
- `polyoxide-polymarket`: `shared::keys` (`TokenId`, `ClobKeys`, `PerpsKeys`); `perps::venue`,
  whose `Perps` implements the same five; `clob::venue`, whose `Clob` implements `MarketData` and
  `Candles`, `Trades` with `data`, and `EventGroups` with `gamma`.
- `ClobBuilder::data` and `ClobError::Data` are new (`ClobError` is `#[non_exhaustive]`). With
  the `data` feature, which `polyoxide-polymarket`'s defaults include, `ClobBuilder::build` also
  builds a Data client for `Trades`, with the builder's timeout and pool size; it sends nothing
  until a trait method asks. The unified `Polymarket` client hands clob its own Data client.
- The umbrella: `polyoxide::parse_key`, the traits on `polyoxide::venue` with any module that
  implements them, and `examples/venue_neutral.rs`, one `best_bid_ask` and one error classifier
  that run unchanged on the CLOB, perps and USDⓈ-M.

No dependency tree changes: `market` builds only rust_decimal, serde and dynosaur, which every
module that turns it on already builds, and the socket-only modules turn on neither feature.
```

- [ ] **Step 3: Name the umbrella's market-data features, the example and the live suite in CLAUDE.md.**

Current (`CLAUDE.md`):

```markdown
`?/keychain`; there is no umbrella `specta`. Each passthrough names one type of its module through
```

Replace with:

```markdown
`?/keychain`; there is no umbrella `specta`. A module that implements the market-data traits
(`polymarket-clob`, `polymarket-perps`, `binance-usdm`) also turns on `polyoxide-venue/market`, and
one that only names keys (`polymarket-gamma`, `polymarket-data`) `polyoxide-venue/keys`, so
`polyoxide::venue` carries the traits with every module that implements them, which a `const _`
pins. `examples/venue_neutral.rs` reads the best bid and ask of the CLOB, perps and USDⓈ-M with one
`best_bid_ask(&DynMarketData, &MarketKey)` and their errors with one classifier generic over
`Classify` (CAP-5 and CAP-4); `tests/venue_neutral.rs` includes it by `#[path]` and runs both against
mock servers for each module, and `tests/live_venue.rs` runs `best_bid_ask` against the live hosts.
Each passthrough names one type of its module through
```

Current (`CLAUDE.md`):

```markdown
The CLI has live integration tests in `tests/live_api.rs`, and each module of a venue crate in `tests/live_<module>[_<suite>].rs` (`polyoxide-polymarket`'s `live_clob`, `live_clob_ws`, `live_clob_session_keys`, `live_gamma`, `live_data`, `live_relay`, `live_perps`, `live_perps_ws`, `live_rtds` and `live_sports`, and `polyoxide-binance`'s `live_usdm` and `live_usdm_ws`), gated with `#[ignore]` so CI skips them. They hit the real upstream APIs. Run with `-- --ignored` flag.
```

Replace with:

```markdown
The CLI has live integration tests in `tests/live_api.rs`, the umbrella in `tests/live_venue.rs` (the venue-neutral example against each venue), and each module of a venue crate in `tests/live_<module>[_<suite>].rs` (`polyoxide-polymarket`'s `live_clob`, `live_clob_ws`, `live_clob_session_keys`, `live_gamma`, `live_data`, `live_relay`, `live_perps`, `live_perps_ws`, `live_rtds` and `live_sports`, and `polyoxide-binance`'s `live_usdm` and `live_usdm_ws`), gated with `#[ignore]` so CI skips them. They hit the real upstream APIs. Run with `-- --ignored` flag.
```

- [ ] **Step 4: Regenerate.** The umbrella's README line and its new live target change four files' regions.

Run: `python3 scripts/gen_registry.py --write && python3 scripts/gen_registry.py --check`
Expected: `wrote README.md`, `wrote CLAUDE.md`, `wrote SELF-HEALING.md`, `wrote .github/workflows/nightly-behavioral.yml`, then exit 0. The workflow gains the job `live-polyoxide-venue` with `--no-default-features --features binance-usdm,polymarket-clob,polymarket-perps --test live_venue`, and the aggregate job needs it.

- [ ] **Step 5: Run the CI scripts' suite, which holds the live registration, the allowlists, the classify coverage and the ledger.**

Run: `cd .github/scripts && uv sync && uv run pytest tests/ -q`
Expected: every test passes. `test_dep_allowlists.py` in particular passes with no regenerated file (D4).

- [ ] **Step 6: Commit.**

```bash
git add docs/s3-migration.md _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md CLAUDE.md README.md SELF-HEALING.md .github/workflows/nightly-behavioral.yml
git commit -m "docs: amendment A6-2, Epic 6's consumer-impact note and the nightly venue suite (Story 6.6)" -- docs/s3-migration.md _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md CLAUDE.md README.md SELF-HEALING.md .github/workflows/nightly-behavioral.yml
```

## After the run (the owner)

- **Merge the amendments.** One `bmad-architecture` session merges A2-1, A3-1 to A3-5, A4-1 to A4-4 (correcting A4-2, per its deferred-work entry), A5-1, A6-1 and A6-2 into the spine and its guide, then runs `python3 scripts/gen_registry.py --write`, which regenerates `docs/ARCHITECTURE.md`. Story 6.6's criterion "regenerated once that amendment is merged" is met then.
- **Ship the note.** The version-bump commit on `main` that first releases this work writes its `CHANGELOG.md` section with Epic 6's note from `docs/s3-migration.md`: the work is additive.
- **Watch the first nightly** of `live-polyoxide-venue`. From GitHub's US runners Binance answers 451, which tags `environmental`, as `live_usdm` does today.
