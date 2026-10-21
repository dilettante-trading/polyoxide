---
status: proposed
date: 2026-10-21
---

# Product classes are a tag on the product and a bundle over one capability core

## Context and Problem Statement

polyoxide is a multi-venue SDK. Binance USDⓈ-M and Kalshi have shipped beside Polymarket, and
Kalshi's crate is published and re-exported by the umbrella as `polyoxide::kalshi` (Epic 9, A9-1).
The long-term aim is generic interfaces for two kinds of market: prediction markets
(Polymarket, Kalshi) and asset exchanges (Binance, Bybit). The question is where that split
belongs: between venues, between two trait trees, or somewhere else. The current design has one
base trait, `MarketData`, and optional capability traits (AD-6, A6-1, A9-2), and it has no notion
of a market's kind. The spec's glossary already names two **product classes**: event contracts
(Polymarket CLOB, Kalshi) and perpetual futures (Polymarket perps, Binance USDⓈ-M, Kalshi margin).

## Decision Drivers

* **Venues straddle classes.** Polymarket has `clob` (event contracts) and `perps` (perpetual
  futures) in one crate. Kalshi lists event contracts, which `polyoxide-kalshi`'s `predictions`
  implements, and a margin perps product that no crate implements yet (`venue-landscape.md`). Only
  Binance, in its current scope, has one class.
* **Most of the surface is shared.** `MarketData`, `Instruments`, `Trades`, `Candles` and `Trading`
  have the same signatures for both classes. `examples/venue_neutral.rs` reads the best bid and
  ask of the CLOB, perps, USDⓈ-M and Kalshi's `predictions` through one
  `best_bid_ask(&DynMarketData, &MarketKey)`. AD-6
  exists to prevent "a generic function that cannot take both product classes".
* **The classes do differ in meaning:**
  * An event-contract price is bounded by 0 and 1, and a share pays one unit if its outcome
    happens (`SizeUnit::Shares`).
  * A key names an outcome, and buying NO is buying the NO outcome's key (AD-7).
  * `Position::size` is never negative for an outcome but is signed for a perpetual.
  * Funding, mark price and margin exist only for perpetuals. Events and settlement exist only for
    event contracts.
* **No runtime "unsupported" (AD-6).** An ability is either a trait a product implements or it is
  absent.
* **Evidence before abstraction.** Venue-only facts live in `<Record>Ext` (AD-19) until a shared
  shape is proven.
* **Dyn shape (AD-5).** The venue traits return `impl Future` and are held as `Arc<Dyn…<'static>>`
  through dynosaur, one wrapper per trait.
* **Consumers want class-generic code.** A consumer such as prader-rs wants to write a funding
  scan across every perpetual product, or an event comparison across Polymarket and Kalshi, with
  one bound and no list of venues.

## Considered Options

* Split by venue: a prediction-market interface and an exchange interface
* Split by product class: two parallel trait trees
* One capability core, with the product class tagged on each product and class bundles over it
* Capabilities only, with no product class (the status quo)

## Decision Outcome

Chosen option: "One capability core, with the product class tagged on each product and class
bundles over it". It is the only option that keeps one generic function working across both
classes (AD-6) and lets a venue such as Polymarket or Kalshi list both classes without special
cases. It also gives consumers a name for each class's contract. The class's meaning is stated on
the shared records, and class-only abilities stay additive capabilities.

The decision has four parts:

1. **The class is a tag on the product, not the venue.** `polyoxide-venue` (feature `keys`) gains
   `ProductClass`, a `#[non_exhaustive]` enum with `EventContract` and `PerpetualFuture`. A variant
   is added only when a product of that class lands. Each `KeySpace` declares
   `const CLASS: ProductClass` beside `VENUE` and `PRODUCT`. Each product registers its class in
   `[package.metadata.polyoxide]` (AD-13), and the existing
   `the_ids_are_the_ones_the_crate_registers` test holds the const to the metadata. The key stamps
   the class when its product constructs it, so `MarketKey::class()` answers for every record that
   carries a key. The class depends only on venue and product, so the text form, `Eq` and `Hash`
   do not change.
2. **Class abilities stay capability traits.** What only one class has is a capability trait that
   only products of that class implement: `EventGroups` today, and later settlement for event
   contracts, or mark price and open interest for perpetuals. No trait tree is forked.
3. **A class bundle names what defines the class.** It holds `MarketData` plus the capabilities that
   every product of the class offers and that mean nothing outside it. Shared optional abilities
   (`Instruments`, `Trades`, `Candles`) stay out, so a bundle never has to shrink. `Candles` shows
   why: every product that offers it now serves a different set of widths, read from
   `Candles::intervals()` (A9-2), so even a shared capability is not uniform. Bundles are
   generic-only, like `Protocol` (AD-5), each with a blanket impl:

   ```rust
   // polyoxide-venue, feature `market`
   // Polymarket perps and Binance USDⓈ-M
   pub trait PerpetualMarket: MarketData + Funding {}
   impl<T: MarketData + Funding> PerpetualMarket for T {}

   // the Polymarket CLOB (with `gamma`) and Kalshi's `predictions`
   pub trait EventContractMarket: MarketData + EventGroups {}
   impl<T: MarketData + EventGroups> EventContractMarket for T {}
   ```

4. **Promotion needs two products of the class.** A class fact joins a shared record, a capability
   trait or a bundle only when two products of that class supply it. Until then it rides in the
   product's `<Record>Ext` (AD-19). Both classes have two today: Polymarket perps and Binance
   USDⓈ-M agree on `Funding`, and the Polymarket CLOB and Kalshi's `predictions` agree on
   `EventGroups`.

### Consequences

* Good, because one generic function still serves every product, and a class-generic one needs a
  single bound.
* Good, because a venue that lists both classes needs nothing special: each product declares its
  own class.
* Good, because a consumer can read a key's class without a request. `SizeUnit` cannot stand in
  for it: `Base` and `Contracts` do not separate a perpetual from a spot or dated future.
* Good, because class abilities arrive the same way every capability has (AD-6, A6-1), with no
  `Unsupported`.
* Bad, because `ProductClass` is a closed enum in the foundation. A new class (spot, dated futures,
  options) edits `polyoxide-venue`. That is deliberate, since a class fixes what shared records
  mean, but it is the one foundation edit that AD-4's open venue ids avoid.
* Bad, because bundles are generic-only. A consumer that holds trait objects holds one
  `Arc<Dyn…<'static>>` per capability, or a struct of them, since dynosaur makes one wrapper per
  trait.
* Bad, because shared records carry class-dependent meanings, such as the sign of
  `Position::size` and the price's bounds. Each record's docs must state them per class, and code
  that mixes classes must read `class()` before it interprets a value.
* Bad, because adding a capability to a bundle is a public API change that the semver and removals
  gates will see. A defining capability should join its bundle in the same release that adds it.
* Neutral, because both bundles have their two implementers today, so they can land together in
  one release.

### Confirmation

* Each venue crate's registration test asserts every `KeySpace::CLASS` against
  `[package.metadata.polyoxide]`, as it does today for venue and product ids.
* A `const _` beside each crate's `Classify` assertions pins every product of a class to its bundle,
  gated as the impls are: `Perps: PerpetualMarket` under `perps`, `Usdm: PerpetualMarket` under
  `usdm`, `Clob: EventContractMarket` under `all(clob, gamma)`, and
  `Predictions: EventContractMarket` under Kalshi's `predictions`. A product that drops a defining
  capability then fails the build.
* A test in the umbrella, beside `tests/parse_key.rs`, checks that `MarketKey::class()` equals the
  constructing `KeySpace::CLASS` for every product its `full` feature builds: `polymarket.clob`, `polymarket.perps`,
  `binance.usdm` and `kalshi.predictions`.
* In review, a new field on a shared record, or a new member of a bundle, cites the two products of
  the class that supply it. Otherwise it goes in `<Record>Ext` (AD-19).

## Pros and Cons of the Options

### Split by venue: a prediction-market interface and an exchange interface

Each venue implements one interface according to what kind of venue it is.

* Good, because it matches how venues market themselves.
* Bad, because Polymarket and Kalshi both list both classes. Each would have to implement both
  interfaces, or the split would hide half of what it offers.
* Bad, because a funding scan across Polymarket perps and Binance USDⓈ-M would span both
  interfaces.
* Bad, because the shared surface would be written twice.

### Split by product class: two parallel trait trees

Separate trees for each class, such as `OutcomeMarketData` and `OutcomeTrading`, and
`PerpMarketData` and `PerpTrading`, with their own records.

* Good, because class meanings can live in types, such as a bounded probability price or an
  unsigned outcome size.
* Bad, because `MarketData`, `Trades`, `Candles`, `Trading`, `TradingEvents` and their records would
  be duplicated with identical signatures.
* Bad, because `best_bid_ask` and every other cross-class function would need two code paths, which
  is what AD-6 prevents.
* Bad, because every capability shared by both classes would need a home in one tree or a third
  shared one, which is the chosen option with extra steps.

### One capability core, with the product class tagged on each product and class bundles over it

Described under Decision Outcome.

* Good, because it keeps AD-6's one base and its capabilities, and adds only a tag and two trait
  aliases.
* Good, because the promotion rule lets bundles and records grow from evidence, as `<Record>Ext`
  already does.
* Neutral, because class meanings are documented, not typed. A consumer that wants a probability
  newtype can build one on `class()`.
* Bad, because of the closed enum and the generic-only bundles (see Consequences).

### Capabilities only, with no product class (the status quo)

* Good, because it adds nothing new, and it already works for four products.
* Bad, because no key or record says what it is. `SizeUnit` (one `instrument` request away)
  identifies event contracts only.
* Bad, because there is no name for what every product of a class offers. Consumers write their
  own bound lists, which drift.
* Bad, because there is no stated rule for when a class fact joins the shared core, so the first
  venue to need one decides alone.

## More Information

**Relation to the spine.** If accepted, this ADR becomes an amendment to AD-6 (product classes and
bundles) and AD-4/AD-13 (the class is registered with the product). It enters the spine as a
`spine-amendments/` entry merged through `bmad-architecture`, and `docs/ARCHITECTURE.md` is
regenerated with `python3 scripts/gen_registry.py --write`. Epic 9's A9-2 also amends AD-6
(`Candles::intervals()` and the refusal of an unserved width), and A8-1..A8-11 and A9-1..A9-5 are
still awaiting their spine merge. This amendment is written against AD-6 as A9-2 leaves it, and
merges after those.

**Facts the event-contract class must hold**, from `venue-landscape.md` and
`kalshi-trading-mapping.md`:

* Kalshi's book has YES bids and NO bids and no asks. AD-7 already converts deterministically: a NO
  bid at p is a YES ask at 1 − p. `Book` keeps one outcome's bids and asks for both venues.
* Kalshi prices are `*_dollars` fixed-point strings with up to 4 decimals; integer cents were
  removed on 2026-03-12. Some markets tick below a cent. Polymarket prices are decimals between 0
  and 1. Both are already a price in the settlement currency per share, so no unit conversion
  belongs in the shared record.
* Kalshi groups series → event → market, and Polymarket's Gamma also has series. `EventGroup` is
  one level, event → outcomes. A series capability is a candidate under the promotion rule, not
  part of this decision.

**Candidate capabilities, not decided here.** Each would be added under the promotion rule:

* Settlement for event contracts: market status, the winning outcome and its payout. Both
  products now read it natively: Polymarket through Gamma's resolution fields and the market
  channel's `market_resolved`, and Kalshi through its `Market`'s `settled` status, `result`,
  `settlement_value_dollars` and `settlement_timer_seconds`
  (`polyoxide-kalshi/src/predictions/types/markets.rs`). So it meets the promotion rule's
  two-product bar, but its shared shape is not decided here.
* Mark and index price for perpetuals. These already qualify: both perpetual products carry
  `mark_price` and `index_price` in their `FundingExt` (`polyoxide-polymarket/src/perps/venue.rs`,
  `polyoxide-binance/src/usdm/venue.rs`). Open interest does not yet: only Polymarket perps'
  `FundingExt` carries it, though Binance serves `/fapi/v1/openInterest`.
* Margin, leverage and liquidation on perpetual positions. This is deferred with perpetual
  trading, since neither perps product has signed routes yet (spine, Deferred).

**Related:** AD-4 (canonical keys), AD-5 (trait shape), AD-6, A6-1 and A9-2 (one base, many
capabilities, and the widths a `Candles` product serves), AD-7 (an event-contract key names an
outcome), AD-13 (registration in Cargo metadata), AD-19 (typed extensions), A9-1 (Kalshi
published and in the umbrella), the Epic 8 Kalshi walking-skeleton design (product id
`predictions`), and the Epic 9 design
(`docs/superpowers/specs/2026-10-19-epic-9-kalshi-published-design.md`).
