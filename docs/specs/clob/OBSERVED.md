# CLOB: what the host does that the published docs do not say

`openapi.yaml` and the `.md` pages in this directory are mirrors of upstream's
published documents and must stay byte-faithful; this file records what the
live host was observed to do beyond them.

## `GET /prices-history` refuses a window longer than 14 days

Probed 2026-10-16 (GET only), `market=<token>&startTs=&endTs=&fidelity=`. A
window of 30, 90 or 365 days is refused at fidelity 1 and at 1440 with HTTP
`400` and `{"error":"invalid filters: 'startTs' and 'endTs' interval is too long"}`.
Windows of 3, 7 and 14 days are served, ascending and anchored at `startTs`,
with no truncation: 14 days at fidelity 1 is 20108 points, at 1440 it is 15.
The limit between 14 and 30 days was not bisected. At fidelity 1 some points
repeat a timestamp.

The venue-neutral `candles` (`polyoxide-polymarket/src/clob/venue.rs`) therefore
sends `endTs` no later than `startTs` plus 14 days, and the caller reads on from
the last candle's open time.

## `POST /prices`, `/midpoints` and `/spreads` answer a map of strings

Probed 2026-10-21 (#70). Each answers a JSON object keyed by token id, as
`openapi.yaml` says, but its values are strings where the spec types the
`/prices` values as `number`:

```
POST /prices     [{"token_id": T, "side": "BUY"}]       → {"T": {"BUY": "0.43"}}
POST /midpoints  [{"token_id": T1}, {"token_id": T2}]   → {"T1": "0.565", "T2": "0.435"}
POST /spreads    [{"token_id": T1}, {"token_id": T2}]   → {"T1": "0.01", "T2": "0.01"}
```

In `/prices` a side with no orders is `"0"` (BUY) or `"1"` (SELL), and a token
with no order book is absent from the map. `/prices` takes at most 500 params:
250 and 500 answer `200`, 1,000 answers `400` with
`{"error":"Payload exceeds the limit"}`. The limit was not bisected, and was not
probed on `/midpoints` or `/spreads`. 200 calls of `/prices` every 15 s at 8 in
flight ran for 30 minutes with every call answering `200` and no `429`.

`Markets::{prices, midpoints, spreads}` decode a map whose values are either a
string or a number. `POST /last-trades-prices` does answer an array.

## Browser transport (2026-11)

Probed with `Origin: https://prader.example` on 2026-11-02, and checked nightly by
`polyoxide-wasm-smoke`'s `browser_live` (the `live-wasm-browser` job).

- `clob.polymarket.com` answers `Access-Control-Allow-Origin: *`, and the preflight for
  `POST /order` with the `POLY_*` headers is allowed (`Access-Control-Max-Age` 30 days).
- It sets no `Access-Control-Expose-Headers`, so a browser cannot read `Retry-After` from it:
  `ErrorResponse::retry_after` is `None` there, though the server sent one.
- `ws-subscriptions-clob.polymarket.com` upgrades (101) on `/ws/market` and `/ws/user`. A browser
  sends no protocol pings and shows none, so the text `PING` is the only keepalive it has. The
  market path closes 1008 `invalid subscription payload` on a first frame that is not a
  subscription (measured 2026-11-02 with `PING`), so the nightly job opens with the empty
  subscription `{"assets_ids":[],"type":"market"}`, which the host keeps open, and holds the
  socket until the first data frame or 20 s.

## The Deposit Wallet routes are routed in production (2026-11-06)

`openapi.yaml` (#91) says `DELETE /cancel-wallet-orders` "currently returns an nginx 404
page until load-balancer routing is enabled". Probed 2026-11-06 with unauthenticated `GET`s
only (no cancel was sent): `GET /cancel-wallet-orders` and `GET /v1/gateway/cancel-wallet-orders`
both answer `405` with no body, so both paths now reach the application rather than nginx;
whether the production feature flag is on was not probed. `GET /data/wallet-open-orders`
answers `401 {"error":"Unauthorized/Invalid api key"}`, a JSON answer from the application.
`Orders::wallet_open_orders`, `Readonly::wallet_open_orders`, `cancel_wallet_orders` and
`cancel_all_wallet_orders` implement the two routes from the mirror alone; neither has been
called with credentials or a read-only key.

## Market socket: `initial_dump` governs only the first subscription frame

Probed 2026-11-06 (#92), unauthenticated, on `wss://ws-subscriptions-clob.polymarket.com/ws/market`,
each case on a fresh socket and run twice on different tokens. The mirror's
`SubscriptionRequest.initial_dump` now says "Subsequent subscriptions ignore this"; the host
goes further than that wording:

| First frame (`{"type":"market",…}`) | Then | Book snapshot |
|---|---|---|
| `[a1]`, `initial_dump: false` | — | none for `a1` |
| `[a1]`, `initial_dump: false` | `{"operation":"subscribe","assets_ids":[a2]}` | `a2`, ~157 ms |
| `[a1]`, `initial_dump: false` | the same update with `"initial_dump": false` | yes, ~157 ms |
| `[a1]`, default | the same update with `"initial_dump": false` | yes, ~160 ms |
| `[]`, `initial_dump: false` | update subscribe `[a1]` | `a1`, ~157 ms |
| `[a1]`, `initial_dump: false` | unsubscribe `a1`, then subscribe `a1` | `a1`, ~157 ms |
| `[a1]`, `initial_dump: false` | a second full frame `[a1, a2]`, `initial_dump: true` | none |
| `[a1]`, `initial_dump: true` | a second full frame `[a2]`, `initial_dump: false` | none (and `a1`'s ~160 ms) |

- The flag is honoured only in a connection's first subscription frame.
- Every `subscribe` update sends a book for each newly added asset, whatever the first frame's
  `initial_dump` and whatever `initial_dump` the update carries, which `SubscriptionRequestUpdate`
  does not list anyway.
- A second full subscription frame on a live socket changes nothing and sends no book either way
  (as found 2026-09-09: it is not additive).

So the client's `MarketSubscriptionOptions::initial_dump` applies to the subscribe frame a socket
opens with, which the supervised market socket replays as each fresh connection's first frame; a
later membership change cannot turn the snapshot off.
