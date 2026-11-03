# Proposed spine amendments: Binance spot

Recorded per AD-21 from `docs/superpowers/specs/2026-11-02-binance-spot-design.md`. One session later merges this file into the spine, after ADR 0001's amendments (`adr-0001.md`), and regenerates the guide.

## BSPOT-1: `ProductClass::Spot` (amends ADR1-1)

- **Proposed by:** the owner, 2026-11-02, in brainstorming (spec §2, the "Product class" row).
- **Current rule:** ADR1-1 makes `polyoxide-venue`'s `ProductClass` (feature `keys`) `#[non_exhaustive]`, with `EventContract` and `PerpetualFuture`, and adds a variant only when a product of that class lands. Each `KeySpace` declares `const CLASS: ProductClass`, and a crate registers it in `classes = { <product> = "<class>" }`.
- **Proposed rule:**
  - `ProductClass` gains `Spot`, after `PerpetualFuture`, with `as_str()` `"spot"`: buying and selling the asset itself; a price is unbounded, a position is a balance of the base asset and so never negative, and there is no funding.
  - Binance spot is the class's first product: `polyoxide-binance` registers `products = ["usdm", "spot"]` and `classes = { usdm = "perpetual-future", spot = "spot" }`, and `spot::key::SpotKeys` declares `CLASS = ProductClass::Spot`.
  - The foundation crate names no venue: the variant's docs and its tests use made-up instruments.
- **Why:**
  - **Spot is a class of its own.** A perpetual future has funding and a signed position; a spot product has neither, so a consumer that reads a key's class without a request must tell them apart (ADR1-1's reason).
  - **The rule's condition is met.** ADR1-1 adds a variant only when a product of that class lands, and Binance spot lands with this slice.
- **Follow-up in the code:** Story 3 of `docs/plans/2026-11-02-binance-spot-rest.md` adds the variant; Story 6 registers the class and the key space. The merge session edits ADR1-1's text and the guide's list of classes.

## BSPOT-2: the `SpotMarket` bundle (amends ADR1-2)

- **Proposed by:** the owner, 2026-11-02, in brainstorming (spec §2, the "Product class" row, and §5.3).
- **Current rule:** ADR1-2 makes a class bundle name `MarketData` plus the capabilities that every product of the class offers and that mean nothing outside it (`PerpetualMarket: MarketData + Funding`, `EventContractMarket: MarketData + EventGroups`), each with a blanket impl and no `Dyn` wrapper, and has each venue crate pin its products to their bundles in a `const _`.
- **Proposed rule:**
  - `polyoxide-venue` (feature `market`) holds `pub trait SpotMarket: MarketData {}` with `impl<T: MarketData> SpotMarket for T {}`, re-exported at the crate root beside the other two bundles, with no `Dyn` wrapper.
  - The bundle adds nothing to `MarketData`: no capability is defined by spot alone. `Instruments`, `Trades` and `Candles` are shared with other classes and stay out, as ADR1-2 says they do.
  - `polyoxide-binance`'s `lib.rs` pins `spot::Spot` to `SpotMarket` in a `const _`, beside `usdm::Usdm`'s pin to `PerpetualMarket`.
- **Why:**
  - **Every class has one bound.** A function generic over a class takes one bound instead of a list each consumer writes and lets drift, and the bundle names only what defines its class. Nothing beyond `MarketData` defines spot, so the bundle is that alone.
  - **It has one implementer today.** ADR1-2 justifies a bundle by two implementers; this one holds no capability that needs the justification, and a capability that spot and a second spot product both offer and no other class does joins it under ADR1-2's promotion rule, citing both products.
- **Follow-up in the code:** Story 3 of `docs/plans/2026-11-02-binance-spot-rest.md` adds the trait and its re-export; Story 6 adds the pin in `polyoxide-binance/src/lib.rs`. The merge session edits ADR1-2's text and the guide's list of bundles.
