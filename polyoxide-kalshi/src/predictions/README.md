# Predictions

The module of `polyoxide-kalshi` for Kalshi's Predictions exchange, on `external-api.kalshi.com` and, for keys made there, on the demo host `external-api.demo.kalshi.co`. The crate is a walking skeleton and it is not published. Kalshi's other exchange, Margin, is not covered, and the name `margin` is left free for it.

A market is binary, and its two outcomes trade as two books, so a market key names one outcome: `kalshi.predictions:<TICKER>:yes` or `kalshi.predictions:<TICKER>:no`. Tickers are case-sensitive and uppercase, so the key space uppercases the ticker it is given.

Every request runs on the send loop of `polyoxide-core` and costs tokens from a Read throttle, sized at Kalshi's Basic tier, which no tier falls below. The module's documentation says what it offers, and `docs/specs/kalshi/OBSERVED.md` records what the host does.
