# Venue landscape

The traits, the throttle interface and the socket blocks must accommodate every column below.
Kalshi facts are from docs.kalshi.com, read on 2026-10-08. The others are from the code and
CLAUDE.md.

## Per-venue facts that bend the abstractions

| Concern | Polymarket | Binance USDⓈ-M | Kalshi |
|---|---|---|---|
| Product classes | event contracts (CLOB); perpetual futures (perps host) | perpetual futures | event contracts; perpetual futures (margin product, `GET /account/limits/perps`) |
| REST auth | L1 EIP-712 `ClobAuth`; L2 HMAC-SHA256 of `ts+method+path[+body]`, url-safe base64; order signing EIP-712 (types 0–3); perps `POLYMARKET-PROXY`/`-SECRET` headers | none in scope (public market data only) | RSA-PSS/SHA-256 (Ed25519 keys also accepted) of `timestamp_ms + METHOD + path-without-query`; headers `KALSHI-ACCESS-KEY`, `-SIGNATURE`, `-TIMESTAMP` |
| Socket auth | clob user channel: credentials in the subscribe frame; all others credential-free | none | API key at the handshake, **even for market data**; signs `timestamp + "GET" + "/trade-api/ws/v2"` |
| Limiter model | Cloudflare window quotas per path and per IP (`RateLimiter`, no `allow_burst`, a tenth reserved); per-signer token buckets with batch costs (`signer_limit`, keeps `allow_burst`, tier from the `Poly-RateLimit-Tier` header) | per-IP request weight per UTC minute, 2400 published, 2160 used; weight depends on route and parameters; funding routes have a separate bucket; 418 ban after continued 429s | per-account token buckets, separate read and write, `refill_rate` and `bucket_capacity` per tier (Basic … Prestige) read from `GET /account/limits`; routes cost tokens (30 on one route, 10 per order in a batch); per-route costs are served by public `/account/endpoint_costs` (default 10); write buckets are per shard; a 429 carries no `Retry-After` |
| Book model | CLOB market channel: `book` snapshot, then `price_change` deltas; one token id per outcome, each with bids and asks | whole top-N partial-depth snapshots, no local book | `orderbook_snapshot`, then `orderbook_delta` with a sequence number; levels are **yes bids and no bids**, with no asks |
| Money and size | `Decimal` strings | `Decimal` strings | `*_dollars` fixed-point strings to 4 dp; `*_fp` fixed-point counts (fractional contracts); integer cents removed 2026-03-12; some markets have sub-penny ticks |
| Ids | token ids, condition ids, perps `InstrumentId(u64)` | `Symbol(String)`, case-folded, Unicode allowed; trade ids exceed `u32` | market tickers (`HIGHNY-24JAN01-T60`), UUID order and trade ids |
| Order shape | GTC/GTD/FOK/FAK; FAK/FOK kill outcomes arrive as HTTP 400 and are typed non-retriable | not in scope | `CreateOrderV2Request` addresses the YES side only (a NO order converts, spine AD-7): side bid/ask, fixed-point count and price, `time_in_force` fill_or_kill/good_till_canceled/immediate_or_cancel, `self_trade_prevention_type`, `post_only`, `reduce_only`, `client_order_id`, `subaccount`, `order_group_id`; batched create |
| Published spec | OpenAPI and AsyncAPI mirrored in `docs/specs/` and drift-checked nightly (some hosts excluded) | none; the live suites are the drift detector | OpenAPI at `docs.kalshi.com/openapi.yaml` (3.34.0 as of 2026-10-08); fits the mirror and nightly-schema pattern |
| Sandbox | none; live trading tests use real funds and are auth-gated | none needed (public) | demo host `external-api.demo.kalshi.co/trade-api/v2`, so a live trading round trip can run without real funds |

## Polymarket code in `polyoxide-core` today

CAP-7's grep test requires all of this to leave the venue-neutral foundation. It moves into
`polyoxide-polymarket` in S2, where every host module that needs it can share it. In S1 its
hooks live in a `polymarket` module inside `polyoxide-core` (spine AD-16).

| Item | Where |
|---|---|
| Package description and keywords | `polyoxide-core/Cargo.toml:9-10`, `src/lib.rs:3,5` |
| `Signer`, `Base64Format`, `create_message` (L2/builder HMAC, `ts+method+path+body`, base64 only) | `src/auth.rs`, `create_message` at `:103`; used by clob and relay |
| `DepositWalletRole`, `SessionSignerScope` (`CLOB`, `COMBOSRFQ`, `ALL`) | `src/session_signer.rs` |
| `SignerLimiter`, `Tier`, `TradingRequest`, `Poly-RateLimit-*` headers | `src/signer_limit.rs` (735 lines) |
| `RateLimiter::{clob,gamma,data,relay,perps}_default` tables (~245 lines) and ~1,100 lines of `documented_*_limits` tests; `RateSpec` documented "as published by Polymarket" | `src/rate_limit.rs:26, 448-666, 732-1842` |
| `425 Too Early` as the matching-engine signal in `should_retry` and `is_retriable` | `src/client.rs:105,132`; `src/error.rs:81-93` |
| `from_status_and_body` reads the `error`/`message` keys and maps 401/403 to `Authentication` (Binance overrides it: its 403 is the WAF) | `src/error.rs:57-68` |

Already venue-neutral: the limiter engine (`rate_limit.rs:1-447`), `Request`, `RetryConfig`,
`macros.rs`, `truncate_for_log`, `keychain.rs`.

## Consumer evidence (prader-rs)

prader-rs already does at the consumer layer what CAP-4, CAP-5 and CAP-6 move into
polyoxide. Its shapes are prior art, not requirements.

- **Venue-neutral perps types** (`prader_core::perps`):
  - `PerpKey` is venue-tagged (`polymarket:6`, `binance:BTCUSDT`).
  - `Option` covers fields a venue does not publish: `max_leverage`, `open_interest`, `mid_price`.
  - `PerpVenueFacts` is a tagged enum for fields only one venue has.
  - Trade ids are opaque `String`.
- **Feed core and per-venue adapter trait** (`prader-app-core/src/perps/`):
  - The venue-independent core holds ref-counted membership, staleness fan-out and batching.
  - The adapter supplies stream names, connect/subscribe/unsubscribe reported as Up/Down, projection, and a silence threshold.
- **Per-crate error classification.** `perps_err` maps each polyoxide error enum to `RateLimited{retry_after}`, `Unauthorized`, `UpstreamUnavailable`, `InvalidRequest` or `Network`. It is re-read at every polyoxide bump because the enums are `#[non_exhaustive]`.
- **Trading seam** (`prader-mm/src/venue/mod.rs`): `Venue::submit(Place | Cancel | CancelAll)`.
  - `Ok` means accepted for transmission.
  - Acks, fills and cancels arrive on an unbounded event channel. It is unbounded because dropping a fill desyncs inventory.
  - `reconcile()` returns the resting quotes.
  - Only `ClobVenue` (Polymarket) implements it live.
- **Never a path or git dependency.** prader depends on polyoxide only through crates.io releases; one path dependency broke its CI on 2026-09-07.
- **Roadmap.** prader's perps roadmap names Hyperliquid and Bybit next. polyoxide's next venue is Kalshi (user, 2026-10-08).
