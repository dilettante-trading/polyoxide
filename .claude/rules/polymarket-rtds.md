---
paths:
  - "polyoxide-polymarket/src/rtds/**"
  - "polyoxide-polymarket/tests/*rtds*.rs"
  - "polyoxide-cli/src/commands/polymarket/rtds.rs"
  - "docs/specs/rtds/**"
---

# Polymarket RTDS

The `rtds` module covers `wss://ws-live-data.polymarket.com`, which multiplexes many topics over
one connection under an `action`/`subscriptions` envelope, unlike the CLOB channels, which are one
channel per connection. Its guide is `src/rtds/mod.rs`'s module docs; it has no README
(`test_module_readmes.py`'s `WITHOUT_README`).

- The feature builds `polyoxide-venue`, which depends on nothing by default (its `decimal`
  feature, which perps, Binance and, through `market`, clob enable, adds rust_decimal and serde
  for `positional`), and the socket kit, and no core, `reqwest` or `alloy`;
  `test_dep_allowlists.py` holds `@rtds` to that. So a
  credential-free price feed does not pull in `alloy`:
  `polyoxide-polymarket --no-default-features --features clob-ws` builds 254 crates against core's
  126 (unique names in `cargo tree -e normal`).
- Two tiers: `Rtds` is a bare `Stream`; `RtdsBuilder`/`SupervisedRtds` adds keep-alive, a
  staleness watchdog and reconnect-with-resubscribe. Each connect goes through the kit's `connect`,
  bounded by a connect timeout, 10 s by default (`RtdsBuilder::connect_timeout`, DRIFT R2), and
  the bare tier reads through the kit's `Connection` (`Mode::Data`), which answers the server's
  close with the RFC 6455 close reply (DRIFT R3) and logs it under `polyoxide_ws`.
- The documented 5-second `PING` is neither required nor answered, so staleness, not the
  heartbeat, is the only liveness signal. Its pump pings only on a quiet tick: do not copy that
  shape for a host that requires pings.

## `full_accuracy_value` does not mean the same thing on every topic

It is E18 fixed-point on the three Chainlink topics and a **plain decimal** on `crypto_prices`
(Binance). Two frames captured a second apart both report BTC at ≈$79,697 with byte-identical
payload keys, differing only in that scale, so the two spot payloads are separate types and the
scale is never a runtime decision. A test asserting only "the value is a positive Decimal" passes
on both and proves nothing; `the_two_spot_topics_do_not_share_a_scale` in `src/rtds/event.rs`
holds this up, and it has been observed failing in both directions.

## Four behaviours the docs omit (`docs/specs/rtds/OBSERVED.md`)

- A filter with one stray space delivers the backfill and then goes silent forever with no error,
  which is why `filters` is built by `serde_json` and never accepted as a caller string.
- One unrecognised topic returns zero frames for every topic in the same batch.
- A Chainlink-spot snapshot arrives labelled with the Binance topic, so
  `correct_mislabelled_spot_snapshot` keys on the symbol format: taking the label at face value
  files Chainlink prices under Binance, silently.
- A backfill is sent only for a symbol-filtered subscription; an unfiltered one never receives
  one, so it cannot re-initialise from a snapshot after a reconnect.

`docs/specs/rtds/` is modelled on captured frames and excluded from `nightly-schema.yml`: upstream
publishes nothing to diff it against.
