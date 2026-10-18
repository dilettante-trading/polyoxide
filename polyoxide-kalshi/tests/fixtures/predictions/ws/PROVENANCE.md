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
