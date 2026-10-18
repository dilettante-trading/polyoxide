# polyoxide-kalshi

A Rust client library for Kalshi's event-contract exchange, the third venue of the polyoxide workspace after Polymarket and Binance.

This crate is a walking skeleton and it is not published. It exists to show that a venue lands by the architecture guide's recipe: a crate of its own, registered in its manifest, touching the workspace only in the few places the recipe names.

It has one module, `predictions`, behind the `predictions` feature, which is on by default. The module covers Kalshi's Predictions exchange on `external-api.kalshi.com`, and on the demo host `external-api.demo.kalshi.co`. Its documentation says what it offers.

The feature `predictions-ws`, which is off by default and turns on `predictions`, adds the module's order-book socket, `predictions::ws`, on the polyoxide socket kit. It streams a snapshot of each subscribed market's book and then its changes, in YES pricing, and reconnects after a drop, a stall or a gap. Kalshi refuses a socket handshake that carries no API-key headers, public data included, so the socket needs `Credentials`, where the REST reads of public data need none.
