# RTDS: observed behaviour

Polymarket publishes no AsyncAPI for `wss://ws-live-data.polymarket.com`, and
its prose documentation disagrees with the server in six places. This file
records what the host actually does, observed on 2026-09-05 across more than
seven probes run over two sessions — more than the seven the initial design
spec counted, since the resubscribe behaviour below was checked separately
afterward. `asyncapi-live-data.json` beside it is modelled on captured frames,
not on any upstream document, so it must never be added to `nightly-schema.yml`
— there is nothing to diff it against.

## Contradictions with the published documentation

| # | Documented | Observed |
|---|---|---|
| 1 | `full_accuracy_value` is "the exact signed E18 fixed-point value" | E18 on the three Chainlink topics; a **plain decimal** on `crypto_prices` |
| 2 | TWAP: "no snapshot, history, or replay" | A backfill *is* sent — but **only for a symbol-filtered subscription**. Unfiltered subscriptions to `crypto_prices`, `crypto_prices_chainlink` and `crypto_prices_twap_thirty` each received zero `subscribe` frames across 100-152 updates. Filtered, the length varies: 50-59 points on the Chainlink topics across six captures (50, 53, 55, 57, 59, 59), 120 on Binance. Do not treat the count as fixed |
| 3 | Binance filter is `"btcusdt,ethusdt"` | Yields zero frames. The working form is `{"symbol":"btcusdt"}` |
| 4 | Symbols must be lowercase | `{"symbol":"BTC/USD"}` works; matching is case-insensitive |
| 5 | Envelope is `{topic,type,timestamp,payload}` | `update` frames carry an undocumented `connection_id`; `subscribe` frames do not |
| 6 | Chainlink supports 4 symbols; Binance 4 | 8 Chainlink live, 6 Binance |

## Undocumented behaviour

**The whitespace trap.** `{"symbol": "btc/usd"}` — one space — still delivers
the subscribe backfill and then never sends another update, with no error. The
failure is indistinguishable from an idle feed. `polyoxide-rtds` never accepts a
caller-supplied filter string for this reason.

**Batch poisoning.** One unrecognised topic in a subscription array returns zero
frames for **every** topic in that array, answering only with
`{"body":{"message":"leger GetTopics error: … not found"},"statusCode":401}`.
The `401` is not meaningful — the body describes a not-found.

**`PING` is not load-bearing, and there is no `PONG`.** With both the
application `PING` and the protocol-level ping disabled, a subscription ran 240
seconds and 224 frames without interruption. The only non-JSON text frame RTDS
ever sent was a single empty string at connect. Liveness must be inferred from
update staleness; nothing comes back from a ping.

**A Chainlink-spot snapshot is mislabelled with the Binance topic.** Captured
on a connection subscribed to `crypto_prices_chainlink` and nothing else: its
update frames are labelled `crypto_prices_chainlink`, but its snapshot frame
is labelled `crypto_prices`. Both spot topics' backfills come back under that
one label, and the symbol format is the only discriminator — `btc/usd` versus
`btcusdt`. TWAP snapshots are unaffected. `polyoxide-rtds` corrects this in
`correct_mislabelled_spot_snapshot`; taking the label at face value files
every Chainlink-spot backfill under Binance, silently, since both spot
snapshots are display-only and the points parse either way.

**A second `subscribe` frame on an open connection is accepted.** Verified
four times across two sessions: a connection subscribed to
`crypto_prices_twap_thirty` was sent a second subscribe frame adding
`crypto_prices_chainlink`, and both topics then produced updates on the same
socket. No rejection envelope was returned. `Rtds::subscribe_more` relies on
this.

## Symbols observed

- Chainlink (`btc/usd` form): `btc`, `eth`, `sol`, `xrp`, `bnb`, `doge`, `hype`, `zec`
- Binance (`btcusdt` form): `btc`, `eth`, `sol`, `xrp`, `bnb`, `doge`

Symbol sets moved beyond the documented four within one observation window, so
symbols are modelled as `String` rather than an enum.
