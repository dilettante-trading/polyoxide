# Kalshi order-book socket fixtures

Captured 2026-10-18T15:15:57Z by `scripts/capture_kalshi_predictions_ws.py` from `wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2` (the demo host), with the owner's demo key signing the handshake through `scripts/capture_kalshi_account.py`'s `signed_headers`. No key appears in a fixture: the handshake's headers are not frames.

Market `KXBTCD-26OCT2317-T81999.99`; `KXTRUMPAPPROVE-26OCT19-E39.1` was added to the subscription. The exchange: a `subscribe` (`orderbook_delta`, `use_yes_price: true`), up to 60 s of frames (at most 100 deltas), an `update_subscription` `add_markets`, an `unsubscribe`, and a `subscribe` that names no market.

| Fixture | What it is |
|---------|------------|
| `subscribed.json` | the answer to the `subscribe` |
| `snapshot.json` | the first `orderbook_snapshot` of the subscription |
| `deltas.json` | 44 `orderbook_delta` frames, as an array, in wire order |
| `ok.json` | the `ok` answering `update_subscription` with `add_markets` |
| `unsubscribed.json` | the `unsubscribed` answering `unsubscribe` |
| `error.json` | the `error` answering a `subscribe` that names no market |

<!-- epic9:begin -->
## Epic 9 captures

Captured 2026-10-21T10:35:57Z by `scripts/capture_kalshi_predictions_ws.py --epic9` from `wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2` (the demo host), the handshake signed as above. A `subscribe` for `orderbook_delta`, `ticker` and `trade` on `KXRAINSHARD2-26OCT22-NYC` and `KXATPCHALLENGERMATCH-26OCT21PDAJAR-PDA` (`use_yes_price: true`), up to 180 s for a `ticker` and a `trade` frame, then the commands of spec §3 in order. `KXATPCHALLENGERMATCH-26OCT21PDAJAR-JAR` was added by the two-`sid` update and `KXGOLDD-26OCT2217-T4083` was never subscribed.

| Fixture | What it is |
|---------|------------|
| `subscribed_ticker.json` | the `subscribed` answer for the `ticker` channel, from the three-channel `subscribe` |
| `subscribed_trade.json` | the `subscribed` answer for the `trade` channel, from the same `subscribe` |
| `ticker.json` | the first `ticker` frame of the session |
| `trade.json` | the first `trade` frame of the session |
| `snapshot_answer.json` | the `orderbook_snapshot` answering `get_snapshot` for `first`, with its `id`, `sid` and `seq` |
| `deltas_after_snapshot.json` | 100 `orderbook_delta` frames on that `sid` after it, as an array, in wire order (every market of the `sid`, not only `first`) |
| `snapshot_outside.json` | the answer to `get_snapshot` for `KXGOLDD-26OCT2217-T4083`, a ticker outside the subscription: an `orderbook_snapshot` carrying the command's `id`, as the host sent it |
| `error_snapshot_ticker_sid.json` | the `error` answering `get_snapshot` on the `ticker` channel's `sid` |
| `error_snapshot_trade_sid.json` | the `error` answering `get_snapshot` on the `trade` channel's `sid`, which carries no `id` |
| `multi_sid_answer.json` | the answer to one `update_subscription` naming two `sids` (an array when there is more than one frame) |
<!-- epic9:end -->
