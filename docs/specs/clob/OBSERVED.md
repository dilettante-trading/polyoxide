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
