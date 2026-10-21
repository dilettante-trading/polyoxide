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
