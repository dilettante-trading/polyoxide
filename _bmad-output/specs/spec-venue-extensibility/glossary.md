# Glossary

| Term | Meaning in this spec |
|---|---|
| **Venue** | An exchange or market operator with its own accounts, auth and rate limits: Polymarket, Binance, Kalshi. |
| **Host** | One API surface of a venue on its own base URL. Polymarket has seven: clob, gamma, data, relay, rtds, sports, perps. |
| **Product class** | The kind of instrument traded, which fixes what a market-data or trading trait can promise. Two classes today: **event contracts** (Polymarket CLOB, Kalshi) and **perpetual futures** (Polymarket perps, Binance USDⓈ-M, Kalshi margin). |
| **Foundation** | The venue-neutral shared code (HTTP path, throttle interface, cooldown, error classification, socket blocks, test support). It contains no venue identifiers. |
| **Native client** | A venue's own typed client exposing every route it implements. The traits sit on top of it and do not replace it. |
| **Venue-options slot** | A typed place in a normalized request or record for fields only one venue has, so normalization loses nothing. Realised as that product's `<Record>Ext` in the record's `Extensions` map (spine AD-19). |
| **Hold** | A client-wide wait that every request on a throttle serves, set by a 429 or a venue ban; today's code calls it a cooldown. It is throttle state, shared by clients that share a throttle (spine AD-9, AD-23). |
| **IDENTICAL** | Copies whose bodies match except for comments. |
| **PARAMETRIC** | Copies of one algorithm that differ only in constants or types. |
| **DFR** | Divergent for a stated reason. The difference carries behaviour and stays per-venue (`divergences.md`). |
| **DRIFT** | A difference with no recorded reason. It needs a decision before its copies merge (`divergences.md`). |
| **Drift-detector pattern** | The per-host set that keeps the code honest against the live host: `OBSERVED.md`, captured fixtures with `PROVENANCE.md`, a capture script, wire- and spec-agreement tests, and a live no-unmodelled-keys test. |
| **Registration source** | The one place a new venue or crate is declared. Every other list derives from it or is checked against it. |
| **Stage** | One of the restructure's three release steps (spine AD-16): **S1** internals (shared code; only consolidated paths break), **S2** renames (every other public rename, built in one integration session), **S3+** additions (traits, clob supervision, the Kalshi skeleton). |
| **Walking skeleton** | The minimal Kalshi crate that proves extensibility: exchange status, one market-data trait implementation, one supervised socket with a signed handshake against the demo host. |
