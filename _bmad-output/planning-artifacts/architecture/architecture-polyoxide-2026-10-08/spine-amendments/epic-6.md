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

## A6-2: the guide links the venue-neutral example (amends the guide's "Types and keys" and CAP-5's and CAP-4's success criteria)

- **Proposed by:** Story 6.6, as its acceptance criteria ask.
- **Current rule:** CAP-5's success criterion is "one generic function, such as best bid/ask for a key, runs unchanged against each" product, and CAP-4's is "a consumer classifier like prader's `perps_err` is written once, generic over the interface". The guide names neither the function nor where it lives.
- **Proposed rule:** the guide's "Types and keys" section gains one bullet: "**One function, every venue (CAP-4, CAP-5).** [`polyoxide/examples/venue_neutral.rs`](../polyoxide/examples/venue_neutral.rs) reads the best bid and ask of the Polymarket CLOB, Polymarket perps and Binance USDⓈ-M with one `best_bid_ask(&DynMarketData, &MarketKey)`, and reads each module's errors with one classifier written against `Classify`. `polyoxide/tests/venue_neutral.rs` runs both against mock servers for each module, and `tests/live_venue.rs` runs `best_bid_ask` against the live hosts nightly. A new venue whose products implement the traits needs no change to it."
- **Why:** AD-21 makes example links spine content, and the guide is regenerated from the spine. The example is the evidence both success criteria name.
- **Follow-up:** none in the code. The session that merges this file into the spine (through `bmad-architecture`, with A2-1 to A5-1 and A6-1) adds the bullet and regenerates `docs/ARCHITECTURE.md` with `python3 scripts/gen_registry.py --write`.
