# Predictions

The module of `polyoxide-kalshi` for Kalshi's Predictions exchange, on `external-api.kalshi.com` and, for keys made there, on the demo host `external-api.demo.kalshi.co`. The crate is a walking skeleton and it is not published. Kalshi's other exchange, Margin, is not covered, and the name `margin` is left free for it.

A market is binary, and its two outcomes trade as two books, so a market key names one outcome: `kalshi.predictions:<TICKER>:yes` or `kalshi.predictions:<TICKER>:no`. Tickers are case-sensitive and uppercase, so the key space uppercases the ticker it is given.

Every request runs on the send loop of `polyoxide-core` and costs tokens from a Read throttle, sized at Kalshi's Basic tier, which no tier falls below. The module's documentation says what it offers, and `docs/specs/kalshi/OBSERVED.md` records what the host does.

With the `predictions-ws` feature, which is off by default, the module also streams the order book of the markets it is given, as `predictions::ws`, on the socket kit and not on the send loop. Every price is in YES pricing, so a NO bid is reported as the YES ask it is. Kalshi refuses a socket handshake that carries no API-key headers, public data included, so the socket is built from `Credentials` and signs each connection attempt afresh, where the REST reads of public data need none. The demo host's socket takes the demo key, and keys do not cross environments.
