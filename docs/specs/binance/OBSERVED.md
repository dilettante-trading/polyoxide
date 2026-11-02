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
`probe_ws2.py` and `probe_ws_ping.py`, and from full frames read while planning. The depth
speed, non-ASCII and legacy-path rows come from single live probes with no stored script.
The limits were measured on `/market`; `/public` is assumed to share them.

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

## Spot

Measured against `api.binance.com`, and `data-api.binance.vision` where a section says so, with `docs/specs/binance/probes/probe_rest.py --product spot`, and the streams on `stream.binance.com` with `docs/specs/binance/probes/probe_ws.py --product spot`; the design is `docs/superpowers/specs/2026-11-02-binance-spot-design.md`. Each section names the day it was measured. A later finding goes directly above the last line of this file, under a heading of its own.

### REST weights

Measured 2026-11-02 on `api.binance.com`: the rise in `X-MBX-USED-WEIGHT-1M` across back-to-back requests inside one UTC minute, after a `time` call for the baseline. Each row is measured over distinct URLs (symbols and limits vary within a row), so a cached answer cannot stand in for a repeat. `Result` compares every URL with the design spec's section 3.1. `docs/specs/binance/probes/probe_rest.py --product spot --section weights` re-runs it.

| Row | Requests | Status | Measured | Spec §3.1 | Result |
|---|---|---|---|---|---|
| ping | `/api/v3/ping`; `/api/v3/ping` | 200, 200 | 1, 1 | 1 | agrees |
| time | `/api/v3/time`; `/api/v3/time` | 200, 200 | 1, 1 | 1 | agrees |
| exchangeInfo, with symbol | `/api/v3/exchangeInfo?symbol=BTCUSDT`; `/api/v3/exchangeInfo?symbol=ETHUSDT` | 200, 200 | 20, 20 | 20 | agrees |
| exchangeInfo, without symbol | `/api/v3/exchangeInfo`; `/api/v3/exchangeInfo?showPermissionSets=false` | 200, 200 | 20, 20 | 20 | agrees |
| depth, no limit | `/api/v3/depth?symbol=BTCUSDT`; `/api/v3/depth?symbol=ETHUSDT`; `/api/v3/depth?symbol=BNBUSDT` | 200, 200, 200 | 5, 5, 5 | 5 | agrees |
| depth, limit 1 to 100 | `/api/v3/depth?symbol=BTCUSDT&limit=1`; `/api/v3/depth?symbol=ETHUSDT&limit=7`; `/api/v3/depth?symbol=BNBUSDT&limit=100` | 200, 200, 200 | 5, 5, 5 | 5 | agrees |
| depth, limit 101 to 500 | `/api/v3/depth?symbol=BTCUSDT&limit=101`; `/api/v3/depth?symbol=ETHUSDT&limit=300`; `/api/v3/depth?symbol=BNBUSDT&limit=500` | 200, 200, 200 | 25, 25, 25 | 25 | agrees |
| depth, limit 501 to 1000 | `/api/v3/depth?symbol=BTCUSDT&limit=501`; `/api/v3/depth?symbol=ETHUSDT&limit=750`; `/api/v3/depth?symbol=BNBUSDT&limit=1000` | 200, 200, 200 | 50, 50, 50 | 50 | agrees |
| depth, limit above 1000 | `/api/v3/depth?symbol=BTCUSDT&limit=1001`; `/api/v3/depth?symbol=ETHUSDT&limit=5000`; `/api/v3/depth?symbol=BNBUSDT&limit=5001` | 200, 200, 200 | 250, 250, 250 | 250 | agrees |
| trades, no limit | `/api/v3/trades?symbol=BTCUSDT`; `/api/v3/trades?symbol=ETHUSDT`; `/api/v3/trades?symbol=BNBUSDT` | 200, 200, 200 | 25, 25, 25 | 25 | agrees |
| trades, with limit | `/api/v3/trades?symbol=BTCUSDT&limit=1`; `/api/v3/trades?symbol=ETHUSDT&limit=3`; `/api/v3/trades?symbol=BNBUSDT&limit=500` | 200, 200, 200 | 25, 25, 25 | 25 | agrees |
| aggTrades, no limit | `/api/v3/aggTrades?symbol=BTCUSDT`; `/api/v3/aggTrades?symbol=ETHUSDT`; `/api/v3/aggTrades?symbol=BNBUSDT` | 200, 200, 200 | 4, 4, 4 | 4 | agrees |
| aggTrades, with limit | `/api/v3/aggTrades?symbol=BTCUSDT&limit=1`; `/api/v3/aggTrades?symbol=ETHUSDT&limit=3`; `/api/v3/aggTrades?symbol=BNBUSDT&limit=500` | 200, 200, 200 | 4, 4, 4 | 4 | agrees |
| klines, no limit | `/api/v3/klines?symbol=BTCUSDT&interval=1m`; `/api/v3/klines?symbol=ETHUSDT&interval=1m`; `/api/v3/klines?symbol=BNBUSDT&interval=1m` | 200, 200, 200 | 2, 2, 2 | 2 | agrees |
| klines, with limit | `/api/v3/klines?symbol=BTCUSDT&interval=1m&limit=1`; `/api/v3/klines?symbol=ETHUSDT&interval=1m&limit=2`; `/api/v3/klines?symbol=BNBUSDT&interval=1m&limit=1000` | 200, 200, 200 | 2, 2, 2 | 2 | agrees |
| avgPrice | `/api/v3/avgPrice?symbol=BTCUSDT`; `/api/v3/avgPrice?symbol=ETHUSDT`; `/api/v3/avgPrice?symbol=BNBUSDT` | 200, 200, 200 | 2, 2, 2 | 2 | agrees |
| ticker/24hr, one symbol | `/api/v3/ticker/24hr?symbol=BTCUSDT`; `/api/v3/ticker/24hr?symbol=ETHUSDT`; `/api/v3/ticker/24hr?symbol=BNBUSDT` | 200, 200, 200 | 2, 2, 2 | 2 | agrees |
| ticker/24hr, all | `/api/v3/ticker/24hr`; `/api/v3/ticker/24hr?type=MINI` | 200, 200 | 80, 80 | 80 | agrees |
| ticker/price, one symbol | `/api/v3/ticker/price?symbol=BTCUSDT`; `/api/v3/ticker/price?symbol=ETHUSDT`; `/api/v3/ticker/price?symbol=BNBUSDT` | 200, 200, 200 | 2, 2, 2 | 2 | agrees |
| ticker/price, all | `/api/v3/ticker/price`; `/api/v3/ticker/price?symbolStatus=TRADING` | 200, 200 | 4, 4 | 4 | agrees |
| ticker/bookTicker, one symbol | `/api/v3/ticker/bookTicker?symbol=BTCUSDT`; `/api/v3/ticker/bookTicker?symbol=ETHUSDT`; `/api/v3/ticker/bookTicker?symbol=BNBUSDT` | 200, 200, 200 | 2, 2, 2 | 2 | agrees |
| ticker/bookTicker, all | `/api/v3/ticker/bookTicker`; `/api/v3/ticker/bookTicker?symbolStatus=TRADING` | 200, 200 | 4, 4 | 4 | agrees |

`exchangeInfo`'s `rateLimits`: REQUEST_WEIGHT 6000 per 1 minute; ORDERS 100 per 10 second; ORDERS 200000 per 1 day; RAW_REQUESTS 300000 per 5 minute.

### Limits past a route's maximum

Measured 2026-11-02 on `api.binance.com`. `Weight` is the rise in `X-MBX-USED-WEIGHT-1M` over a `time` call before it; a refused request still shows what it cost. `docs/specs/binance/probes/probe_rest.py --product spot --section limits` re-runs it.

| Request | Status | Returned | Weight | Spec §3.1 says | Result |
|---|---|---|---|---|---|
| `/api/v3/klines?symbol=BTCUSDT&interval=1m&limit=1001` | 200 | 1000 rows | 2 | 200, 1000 rows | agrees |
| `/api/v3/trades?symbol=BTCUSDT&limit=1001` | 200 | 1000 rows | 25 | 200, 1000 rows | agrees |
| `/api/v3/aggTrades?symbol=BTCUSDT&limit=1001` | 200 | 1000 rows | 4 | 200, 1000 rows | agrees |
| `/api/v3/depth?symbol=BTCUSDT&limit=5001` | 200 | 5000 bids, 5000 asks | 250 | 200, 5000 bids, 5000 asks | agrees |
| `/api/v3/depth?symbol=ETHUSDT&limit=7` | 200 | 7 bids, 7 asks | 5 | 200, 7 bids, 7 asks | agrees |
| `/api/v3/aggTrades?symbol=BTCUSDT&startTime=1793595517393&endTime=1793602717393` | 200 | 500 rows | 4 | 200, 500 rows | agrees |
| `/api/v3/depth?symbol=NOPEUSDT` | 400 | code -1121 | 5 | 400, code -1121 | agrees |

### The weight window

Measured 2026-11-02 on `api.binance.com`: a `ping` every 3 s from 12 s before a UTC minute boundary, 22 readings of `X-MBX-USED-WEIGHT-1M`. A sliding 60-second window would not fall to the weight of the last few calls at the boundary. `docs/specs/binance/probes/probe_rest.py --product spot --section window` re-runs it.

```text
07:58:48.0 used=1800
07:58:51.0 used=1801
07:58:54.0 used=1802
07:58:57.0 used=1803
07:59:00.0 used=1
07:59:03.0 used=2
07:59:06.0 used=3
07:59:09.0 used=4
07:59:12.0 used=5
07:59:15.0 used=6
07:59:18.0 used=7
07:59:21.0 used=8
07:59:24.0 used=9
07:59:27.0 used=10
07:59:30.0 used=11
07:59:33.0 used=12
07:59:36.0 used=13
07:59:39.0 used=14
07:59:42.0 used=15
07:59:45.0 used=16
07:59:48.0 used=17
07:59:51.0 used=18
```

The window is the UTC clock minute: the reading fell from 1803 at 07:58:57.0 to 1 at 07:59:00.0, across a minute boundary.

### One counter across hosts

Measured 2026-11-02: alternating calls on `api.binance.com` (`exchangeInfo?symbol=`, weight 20) and `data-api.binance.vision` (`ticker/price?symbol=`, weight 2), each read against the other host's last value, one symbol per pair so no URL repeats. If the counter is shared, a reading is the other host's last reading plus its own call's weight (`If shared`); if not, it is the same host's last reading plus its own (`If separate`). `docs/specs/binance/probes/probe_rest.py --product spot --section counter` re-runs it.

| # | Host | Request | Weight | Reading | If shared | If separate |
|---|---|---|---|---|---|---|
| 1 | `api.binance.com` | `/api/v3/exchangeInfo?symbol=BTCUSDT` | 20 | 20 | - | - |
| 2 | `data-api.binance.vision` | `/api/v3/ticker/price?symbol=BTCUSDT` | 2 | 22 | 22 | - |
| 3 | `api.binance.com` | `/api/v3/exchangeInfo?symbol=ETHUSDT` | 20 | 42 | 42 | 40 |
| 4 | `data-api.binance.vision` | `/api/v3/ticker/price?symbol=ETHUSDT` | 2 | 44 | 44 | 24 |
| 5 | `api.binance.com` | `/api/v3/exchangeInfo?symbol=BNBUSDT` | 20 | 64 | 64 | 62 |
| 6 | `data-api.binance.vision` | `/api/v3/ticker/price?symbol=BNBUSDT` | 2 | 66 | 66 | 46 |
| 7 | `api.binance.com` | `/api/v3/exchangeInfo?symbol=SOLUSDT` | 20 | 86 | 86 | 84 |
| 8 | `data-api.binance.vision` | `/api/v3/ticker/price?symbol=SOLUSDT` | 2 | 88 | 88 | 68 |

One counter: each host's reading is the other host's last reading plus its own call's weight.

### Stream limits

Measured 2026-11-02 on `stream.binance.com:9443/stream` with `docs/specs/binance/probes/probe_ws.py <wsprobe_dir> --product spot`, one connection per section; the five sections below give each measurement whole. The probe's `burst` section was not run: the `pace` section's connection was the run's first disconnect, and the design spec's section 10 allows one.

| Rule | Documented | Measured |
|---|---|---|
| Names per request | Not documented | 1024 names (23,598 bytes) in one `SUBSCRIBE` acknowledged and held, as were 200 and 500 and their `UNSUBSCRIBE`s |
| Streams per connection | 1024 | 1024 held; a 1025th was not sent |
| Acknowledgement | | Every `SUBSCRIBE` is acknowledged `{"result":null,"id":…}`: an unknown symbol, an unknown stream type, an uppercase symbol. `LIST_SUBSCRIPTIONS` echoes each name as sent. An `UNSUBSCRIBE` of a name never subscribed is acknowledged |
| Case | Symbols in stream names are lowercase | `BTCUSDT@trade` is acknowledged and delivers nothing; `btcusdt@aggTrade` delivers |
| Incoming messages | 5 a second, pings, pongs and requests counted; more disconnects | A request every 250 ms, with one client ping and one pong, was closed 16.2 s in with 1008 "Too many requests": 65 requests sent, 64 answered, at most 6 messages in any one second |
| Server ping | Every 20 s; no pong within a minute disconnects | Every 20.0 s, the first 7.9 s after the connection opened, so not timed from the open; the payload is a millisecond timestamp. The missed-pong cutoff was not measured: it costs a disconnect |
| Client ping | | Answered in about 0.3 s, its payload echoed |
| `!ticker@arr` | No longer on the streams page | Acknowledged; nothing in 20 s |
| Partial depth | 1000 ms bare, 100 ms with `@100ms` | As documented, at 5, 10 and 20 levels |
| Envelope | `{"stream": …, "data": …}` | `stream` then `data`, on every frame |
| Connection lifetime | 24 hours | Not measured |

So, as on USDⓈ-M, an acknowledgement proves nothing about a stream's validity, and a name that will never deliver looks like a quiet one. A request every 250 ms does not stay under the limit once pongs and pings are counted; the probe did not record which second crossed it (`pace` now prints the messages sent in the 1.5 s before a close).

### Stream answers

Measured 2026-11-02 on `stream.binance.com:9443/stream`: one connection, a message a second. `docs/specs/binance/probes/probe_ws.py <wsprobe_dir> --product spot --section answers` re-runs it.

| Request | Answer | Frames delivered on the connection |
|---|---|---|
| `SUBSCRIBE ["notasymbolusdt@aggTrade"]` (an unknown symbol) | `{"result":null,"id":1}` | none |
| `SUBSCRIBE ["btcusdt@nonsense"]` (an unknown stream type) | `{"result":null,"id":2}` | none |
| `SUBSCRIBE ["BTCUSDT@trade"]` (an uppercase symbol) | `{"result":null,"id":3}` | none |
| `SUBSCRIBE ["btcusdt@aggTrade"]` (a lowercase symbol) | `{"result":null,"id":4}` | 114 as `btcusdt@aggTrade` |
| `LIST_SUBSCRIPTIONS` | `{"result":["notasymbolusdt@aggTrade","btcusdt@nonsense","BTCUSDT@trade","btcusdt@aggTrade"],"id":5}` | - |
| `UNSUBSCRIBE ["ethusdt@aggTrade"]` (a name never subscribed) | `{"result":null,"id":6}` | none |

The connection lived 16 s. Every stream that delivered: `btcusdt@aggTrade` (114).
A client ping was answered with a pong in 0.316 s, its payload echoed.

### Names per request

Measured 2026-11-02 on `stream.binance.com:9443/stream`: one connection, a message a second, names of the form `zz0000usdt@aggTrade`; `Held` is the length of `LIST_SUBSCRIPTIONS`' result after the request. `docs/specs/binance/probes/probe_ws.py <wsprobe_dir> --product spot --section request` re-runs it.

| Request | Frame | Answer | Held |
|---|---|---|---|
| `SUBSCRIBE` of 200 names | 4646 bytes | `{"result":null,"id":100}` | 200 |
| `UNSUBSCRIBE` of 200 names | 4648 bytes | `{"result":null,"id":102}` | 0 |
| `SUBSCRIBE` of 500 names | 11546 bytes | `{"result":null,"id":104}` | 500 |
| `UNSUBSCRIBE` of 500 names | 11548 bytes | `{"result":null,"id":106}` | 0 |
| `SUBSCRIBE` of 1024 names | 23598 bytes | `{"result":null,"id":108}` | 1024 |

### Pings

Measured 2026-11-02 on `stream.binance.com:9443/stream`: one connection carrying `btcusdt@avgPrice` for 65 s, every server ping ponged with its payload. `docs/specs/binance/probes/probe_ws.py <wsprobe_dir> --product spot --section pings` re-runs it.

- Server pings at 7.9 s, 27.9 s, 47.9 s after the connection opened; gaps 20.0 s, 19.9 s.
- Their payloads: '1793619947374', '1793619967418', '1793619987363'.
- A client ping was answered in 0.300 s.
- 59 `avgPrice` frames arrived in 65 s.

### Frames

Measured 2026-11-02 on `stream.binance.com:9443/stream`: one `SUBSCRIBE` of the 15 names below, answered `{"result":null,"id":1}`, then 20 s of frames. `Gap` is the median time between a stream's frames. Every envelope's keys: `stream`, `data`. `docs/specs/binance/probes/probe_ws.py <wsprobe_dir> --product spot --section frames` re-runs it.

| Name | Frames | Gap | `data` |
|---|---|---|---|
| `btcusdt@aggTrade` | 70 | 172 ms | object: `e`:str `E`:int `s`:str `a`:int `p`:str `q`:str `f`:int `l`:int `T`:int `m`:bool `M`:bool |
| `btcusdt@trade` | 214 | 0 ms | object: `e`:str `E`:int `s`:str `t`:int `p`:str `q`:str `T`:int `m`:bool `M`:bool |
| `btcusdt@kline_1m` | 10 | 1995 ms | object: `e`:str `E`:int `s`:str `k`:object |
| `btcusdt@miniTicker` | 20 | 1000 ms | object: `e`:str `E`:int `s`:str `c`:str `o`:str `h`:str `l`:str `v`:str `q`:str |
| `!miniTicker@arr` | 20 | 1000 ms | array of 56 rows; a row: `e`:str `E`:int `s`:str `c`:str `o`:str `h`:str `l`:str `v`:str `q`:str |
| `btcusdt@ticker` | 20 | 1000 ms | object: `e`:str `E`:int `s`:str `p`:str `P`:str `w`:str `x`:str `c`:str `Q`:str `b`:str `B`:str `a`:str `A`:str `o`:str `h`:str `l`:str `v`:str `q`:str `O`:int `C`:int `F`:int `L`:int `n`:int |
| `btcusdt@bookTicker` | 709 | 1 ms | object: `u`:int `s`:str `b`:str `B`:str `a`:str `A`:str |
| `btcusdt@depth5` | 20 | 1000 ms | object: `lastUpdateId`:int `bids`:list `asks`:list (`bids` 5 levels, `asks` 5 levels) |
| `btcusdt@depth10` | 20 | 1000 ms | object: `lastUpdateId`:int `bids`:list `asks`:list (`bids` 10 levels, `asks` 10 levels) |
| `btcusdt@depth20` | 20 | 1000 ms | object: `lastUpdateId`:int `bids`:list `asks`:list (`bids` 20 levels, `asks` 20 levels) |
| `btcusdt@depth5@100ms` | 200 | 100 ms | object: `lastUpdateId`:int `bids`:list `asks`:list (`bids` 5 levels, `asks` 5 levels) |
| `btcusdt@depth10@100ms` | 200 | 100 ms | object: `lastUpdateId`:int `bids`:list `asks`:list (`bids` 10 levels, `asks` 10 levels) |
| `btcusdt@depth20@100ms` | 200 | 100 ms | object: `lastUpdateId`:int `bids`:list `asks`:list (`bids` 20 levels, `asks` 20 levels) |
| `btcusdt@avgPrice` | 20 | 1000 ms | object: `e`:str `E`:int `s`:str `i`:str `w`:str `T`:int |
| `!ticker@arr` | 0 | - | nothing |

### Pace

Measured 2026-11-02 on `stream.binance.com:9443/stream`: one connection, alternating `SUBSCRIBE` and `UNSUBSCRIBE` of `btcusdt@avgPrice` every 250 ms for 45 s, a client ping every 20 s from 10 s in, every server ping ponged. `docs/specs/binance/probes/probe_ws.py <wsprobe_dir> --product spot --section pace` re-runs it and is disconnected at this pace: run it only with `--pace-ms` to confirm a slower one.

- Sent: 65 requests, 1 client ping, 1 pong; at most 6 messages in any one second.
- Answered: 64 of 65 requests; 1 of 1 client ping.
- The server closed it at 18.23 s with 1008 'Too many requests'.

<!-- spot findings go above this line -->
