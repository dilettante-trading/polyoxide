# polyoxide-kalshi

A Rust client library for Kalshi's event-contract exchange, the third venue of the polyoxide workspace after Polymarket and Binance.

This crate is a walking skeleton and it is not published. It exists to show that a venue lands by the architecture guide's recipe: a crate of its own, registered in its manifest, touching the workspace only in the few places the recipe names.

It has one module, `predictions`, behind the `predictions` feature, which is on by default. The module covers Kalshi's Predictions exchange on `external-api.kalshi.com`, and on the demo host `external-api.demo.kalshi.co`. Its documentation says what it offers.
