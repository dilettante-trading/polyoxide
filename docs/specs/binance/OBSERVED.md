# Binance USDⓈ-M: what the host does

Measured against `fapi.binance.com` and `fstream.binance.com` on 2026-10-07 unless an
entry says otherwise. The weights and the window name their method; the other entries are
single requests made while writing the design spec,
`docs/superpowers/specs/2026-10-07-polyoxide-binance-design.md`.

## Request weight

Method: the rise in `X-MBX-USED-WEIGHT-1M` across back-to-back requests inside one
minute. `polyoxide-binance/examples/weight_probe.rs` re-measures every weighted row; the
`klines` edges it skips (99, 499, 999, 1500), `aggTrades` at limits 100 and 1000, and
the refusals below were probed on 2026-10-07, with `docs/specs/binance/probes/probe_rest.py`
and single requests.

| Route | Measured 2026-10-07 | The page says |
|---|---|---|
| `ping`, `time`, `exchangeInfo`, `openInterest` | 1 | 1 |
| `ticker/24hr` | 1 with `symbol`, 40 without | the same |
| `premiumIndex` | 1 with `symbol`, 10 without | the same |
| `klines` | 1 up to 100, 2 up to 500, 5 up to 1000, 10 above; **5 without `limit`** | under 100 → 1, 100–499 → 2, 500–1000 → 5, over 1000 → 10 |
| `aggTrades` | 20 at limits 1, 100 and 1000 | 20 |
| `depth` | 2 at 5, 10, 20, 50; 5 at 100; 10 at 500; 20 at 1000; **1 without `limit`** | 2 / 5 / 10 / 20 by limit |
| `fundingInfo`, `fundingRate` | no header | share 500 per 5 minutes per IP |

`klines` was measured at every band edge (99, 100, 101, 499, 500, 501, 999, 1000, 1001,
1500), twice: each band is inclusive at the top, which puts the page one off at the 100
and 500 edges; the 1000 edge agrees. A request without `limit` returns 500 rows for 5
where an explicit `limit=500` costs 2.
`depth` without `limit` returns 500 levels for 1 where an explicit `limit=500` costs 10.
A request refused with `400` still costs weight. An unknown symbol costs its route's
weight: 1 on `premiumIndex` and 20 on `aggTrades`. A `klines` limit of 1501 costs 10,
and a `depth` limit of 7 costs 1.

## The weight window

The UTC clock minute. On 2026-10-07 a `ping` every 3 s read 10 at 08:34:59.5 and 1 at
08:35:02.9, then 16 at 08:35:57.3 and 1 at 08:36:00.9. A sliding 60-second window would
have read 10 or more, not 1, at 08:35:02.9.

## Errors

- An unknown symbol: `400 {"code":-1121,"msg":"Invalid symbol."}`.
- A `depth` limit outside 5, 10, 20, 50, 100, 500, 1000: `400 {"code":-4021,"msg":"7 is not valid depth limit"}`.
- A `klines` limit above 1500: `400 {"code":-1130,"msg":"Data sent for parameter 'limit' is not valid."}`.
- A `klines` interval of `1s`, which the docs list: `400 {"code":-1120,"msg":"Invalid interval."}`.
- `aggTrades` older than 48 hours: `400 {"code":-4166,"msg":"Search window is restricted to recent 2 days only."}`.
  The page documents the 48 hours but not the code.
- `418`, `429`, `451` and `403` were not provoked from a test IP; the client classifies
  them by status.

## Response shapes the page does not give

- `exchangeInfo` has a top-level `futuresType`, `"U_MARGINED"`.
- `fundingInfo`'s `updateTime` is `null` on 57 of 805 rows, `BTCUSDT` among them.
- `fundingInfo` also lists COIN-M perpetuals (`BTCUSD_PERP`, `ETHUSD_PERP`, …) that
  `exchangeInfo` on this host does not, which is one reason row symbols are `String`.
- `fundingRate`'s `markPrice` is `""` for funding events through at least 2022-01-01
  (`startTime=1568102400000` and `startTime=1640995200000` both answered `""`; the
  cutoff was not located).
- `ticker/24hr` without `symbol` lists only `TRADING` contracts (789 of 924);
  `premiumIndex` lists 927 rows.
- The REST host accepts a lowercase symbol (`premiumIndex?symbol=btcusdt`) and answers
  with `BTCUSDT`. No listed symbol has a lowercase ASCII letter.
- Quarterly symbols carry an underscore (`BTCUSDT_261225`). Of the 924 symbols in
  `exchangeInfo` on 2026-10-07 no other symbol used a character other than a letter or
  a digit; five were Chinese-character symbols; the longest was 17 characters.
- `underlyingType` took nine values: `COIN`, `EQUITY`, `HK_EQUITY`, `COMMODITY`,
  `KR_EQUITY`, `PREMARKET`, `INDEX`, `CN_EQUITY`, `FX`. The docs list none.
- `aggTrades` rows carry `nq`, documented as the quantity without trades involving RPI
  orders.

## What the page says and the host did not refuse

- `aggTrades` with both `startTime` and `endTime` "must span less than an hour": a
  two-hour window was answered `200` on 2026-10-07.

## Streams

Measured on `fstream.binance.com` on 2026-10-07 with `probes/probe_ws.py`,
`probe_ws2.py` and `probe_ws_ping.py`, and from full frames read while planning.

| Rule | Documented | Measured on `/market` |
|---|---|---|
| Streams per connection | 1024 | 1024 accepted. The 1025th is answered `{"error":{"code":4,"msg":"Too many subscriptions"}}`, then the server closes with 1008 "Invalid request", losing all 1024 |
| Incoming messages | 10 per second | 15 back-to-back requests all answered; of 40 sent back to back, 15 were answered, then a close with 1008 "Too many requests" |
| Acknowledgement | | Every `SUBSCRIBE` is acknowledged: an unknown symbol, a stream type that does not exist, an uppercase symbol. `LIST_SUBSCRIPTIONS` echoes them. An `UNSUBSCRIBE` of a name never subscribed is acknowledged |
| Case | Symbols in stream names are lowercase | `BTCUSDT@aggTrade` is acknowledged and delivers nothing; `btcusdt@aggTrade` delivers |
| Server ping | Every 3 minutes; no pong for 10 minutes disconnects | Pings at 140 s and 320 s on a quiet connection, payload a millisecond timestamp |
| Client ping | | Answered in about 0.3 s |
| Request size | | 200 names in one `SUBSCRIBE` acknowledged |
| Non-ASCII symbols | | Raw UTF-8 in `SUBSCRIBE` accepted; the stream name is echoed exactly |
| Depth speed | `<s>@depth<levels>` for 250 ms, or with `@100ms` or `@500ms` | `@250ms` is acknowledged and delivers nothing, at every level; the bare name pushes every 250 ms. Every kline interval from `1m` to `1M` delivers |
| Paths | `/public` for depth and book tickers, `/market` for the rest | The legacy `/stream` served depth but sent nothing for `!markPrice@arr@1s` |
| Connection lifetime | 24 hours | Not measured; the supervised tier rotates at 23 h 50 min |

So an acknowledgement proves nothing about a stream's validity, and a name that will never
deliver looks like a quiet one. `StreamName` builds every name, and both tiers enforce the
cap, 200 names per request and one request per 200 ms before sending.

Payloads:

- `!markPrice@arr@1s` sends two frames a second (745 and 217 rows). A row with no funding
  scheduled (51 of 745, delisted contracts) sends `T: 0` and `r: "0.00000000"`. `ap` is
  the "mark price moving average", equal to `p` on every row seen.
- `!ticker@arr` sends only the symbols that changed: 226 rows in one frame.
- `st`, "(After CM migration) Symbol type: 1 = UM, 2 = CM", is on every payload but the
  kline event. COIN-M rows arrive on this USDⓈ-M host: 30 of 745 mark-price rows, and
  `AAVEUSD_PERP` in `!ticker@arr`.
- The kline's `B` is documented as "Ignore" and is not modelled.
