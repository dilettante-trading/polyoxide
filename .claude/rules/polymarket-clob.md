---
paths:
  - "polyoxide-polymarket/src/clob/**"
  - "polyoxide-polymarket/tests/*clob*.rs"
  - "polyoxide-polymarket/tests/fixtures/clob/**"
  - "polyoxide-polymarket/examples/{test_order,get_nonce,twap_stream}.rs"
  - "scripts/capture_polymarket_clob.py"
  - "docs/specs/clob/**"
---

# Polymarket CLOB

Namespaces: `clob.markets()`, `orders()`, `account_api()`, `health()`, `auth()`, `rewards()`,
`public_rewards()`, `notifications()`. `rewards()` requires an `Account`; `public_rewards()`
exposes the subset that is public upstream (`/rewards/markets/current`,
`/rewards/markets/{condition_id}`, `/rewards/markets/multi`, `/rebates/current`) without one.
Tests use `Clob::public()` for unauthenticated endpoints.

`clob` does not turn on `gamma`. With both, the CLOB client looks a proxy wallet up through Gamma;
without `gamma`, a proxy order with no funder is refused, an arm that only a build without `gamma`
compiles (CI's features job runs it, `ci-gates.md`). Sockets: `polymarket-clob-ws.md`. Rate
limits and the per-signer buckets: `rate-limits.md`.

## Two auth layers, three signing schemes

Managed through the `Account` type in `src/clob/account/`. Don't conflate them: they use different
EIP-712 domains and are verified by different parties.

| Scheme | Used for | Shape |
|---|---|---|
| **L1** | creating/deriving API credentials (`/auth/api-key`, `/auth/derive-api-key`) | EIP-712 `ClobAuth`, domain `ClobAuthDomain` v1, **no `verifyingContract`** |
| **L2** | everything else authenticated: orders, balances, trades | HMAC-SHA256 over `timestamp + method + path [+ body]`, url-safe base64 |
| **Order signing** | the signed order payload itself, posted under L2 | EIP-712 `Order`, domain `Polymarket CTF Exchange` v2, **with** `verifyingContract` |

The two EIP-712 domains are unrelated: order signing needs a verifying contract, L1 auth must not
have one. `docs/specs/clob/auth.md` has both type strings; `src/clob/core/eip712.rs` pins them
against golden vectors from `py-clob-client`. Signature type 3 (Deposit Wallets) and session keys:
`session-keys.md`.

The module signs exchange domain `"2"` only, which is correct for Gamma `v1` markets; Protocol V2
markets sign for ExchangeV3 (domain `"3"`) and are not supported (#51, `polymarket-gamma.md`).

## Order kill outcomes are not faults

Polymarket returns HTTP 400 both for genuine faults and for the defined kill outcomes of
marketable orders, so `ClobError` splits the latter out as `FakUnmatched` (FAK matched nothing)
and `FokUnfilled` (FOK could not fill in full). They are deterministic and never retriable.
`classify_order_kill` in `src/clob/error.rs` classifies them by the venue's message body, the only
signal available, since the venue ships no error code. Upstream's error catalogue is
https://docs.polymarket.com/resources/error-codes; `docs/specs/clob/openapi.yaml` omits these rows.

## Heartbeats arm a dead man's switch

`account_api().heartbeat()` (`POST /heartbeats`) and `heartbeat_v1(id)` (`POST /v1/heartbeats`)
each send one heartbeat and schedule none. The first accepted send on either route arms the API
key's cancellation deadline: 10 seconds without another accepted send queues a cancel of every
open order the key owns, and nothing disarms it (`docs/specs/clob/account.md`). `heartbeat_v1`
sends `""` first, then each ID a response returns; a 400 with the route's `{error_msg,
heartbeat_id}` body is `ClobError::HeartbeatIdMismatch { expected }`, a `VenueRefusal` that is
not a fault, recognised by that body's shape. `live_heartbeat` arms the deadline of the key it
runs under, so every order that key has resting is cancelled some 10 to 15 seconds later.

## `clob::venue`: the market-data traits

`MarketData` and `Candles` on `Clob` with `clob` alone, `Trades` with `data` as well and
`EventGroups` with `gamma` as well (`all(clob, data)`, `all(clob, gamma)`), keyed by
`shared::keys::clob`: a key names one outcome token (AD-7). No `Instruments` (amendment A6-1).

- `instrument` resolves the token's market with `/markets-by-token/{token}`, then reads
  `/markets/{condition}` (a token the CLOB does not know is its `404`, a `VenueRefusal`).
- `quote` and `book` read `/book` (a listed token with no book is also its `404`), whose sides the
  CLOB sends worst first, so both are sorted best first.
- `candles` read `/prices-history`, one price per interval as `close`, the `CandleInterval` sent as
  `fidelity` minutes, from `start` rounded up to the second and no further than 14 days on, since a
  longer window is a `400` (`docs/specs/clob/OBSERVED.md`).
- `trades` read the Data API's taker-only `/trades` for the token's market and keep the token's
  rows, oldest first, with no trade id (the Data API gives none).
- `event_group` reads the event id from Gamma's `/markets?clob_token_ids=` (a private decode,
  since Gamma's `Market` does not model its `events`) with `closed=any`, since Gamma leaves closed
  markets out unless asked (DRIFT R13), then `/events/{id}`, keeping only `v1` markets' outcomes.
- A number the CLOB or Data API sends as a float is read through its shortest decimal spelling.
- `tests/venue_clob.rs` holds each method to the native clients' values on
  `tests/fixtures/clob/venue/`, captured for one live neg-risk outcome by
  `scripts/capture_polymarket_clob.py`.

## `clob::venue`: `Trading` (amendment A7-3)

Built with `clob-ws` and `data` together: `events()` runs on the supervised user socket, which is
`clob-ws`'s, and `positions` reads the Data API. `clob-ws` turns on `polyoxide-venue/trading`, and
so does the umbrella's `polymarket-clob-ws`; the gate is in the crate's `notes`, so the generated
crate list shows it.

- Every method needs an `Account` and fails with the CLOB's own "Account required" error
  (`InvalidRequest`) without one.
- The address that trades is the client's signature type's (`ClobBuilder::signature_type`, by
  default the account target's): `place` signs as it, and `positions` and `events()` resolve their
  maker address from it with no funder, as `balances` reads with it. So a client built for a proxy
  wallet places, lists positions and follows fills as that wallet (found through Gamma; without
  `gamma`, the native refusal of a proxy with no funder).
- `place` supports every account the native order routes sign for, a Deposit Wallet's (signature
  type 3) included: the order's id is its EIP-712 hash under its exchange's domain, computed before
  signing for every signature type, and a Deposit Wallet's order is signed as its ERC-7739
  envelope.
- `place` refuses before it sends anything, as `InvalidRequest`: a key of another product; an
  extension other than `ClobOrderOptions` (`neg_risk`, `tick_size` and `expiration`; no fee terms,
  since a V2 order signs no fee rate; not `Eq`, since `TickSize` is not); reduce-only; `post_only`
  or an expiration with anything but good-til-cancelled; a client order id an order still holds; a
  price off the tick size (the options', else `GET /tick-size`) or outside 0 to 1; a size over two
  decimals or under 0.01. It then builds the order with the native `create_order`, signs it and
  posts it.
- Outcomes. A FAK or FOK kill, matched on the native `ClobError` variants before any conversion
  (AD-20), is `Ok` with `Killed { reason }`. A 200 whose `success` is false is a `VenueRefusal`.
  `live` and `delayed` are `Open`. `matched` is read back from `GET /data/order/{id}` for its
  `size_matched`: `Filled`; a short IOC `Killed { Unmatched }` with its `filled`; a short FOK
  `Cancelled` with its `filled` (D22; the 400 kill stays `Killed { Unfillable }` with nothing
  filled); a short resting order `Open`. A failed read-back leaves it `Open` with a WARN, since an
  `Err` would invite a duplicate.
- `cancel` is `Ok` only for an id among `canceled`. `cancel_all(Some(key))` resolves the token's
  condition through `/markets-by-token` and sends both it and the token, which the route requires.
- `open_orders` pages `/data/orders` to the `LTE=` sentinel and refuses a repeated cursor.
  `positions` is Data v1 `/positions` for the maker address with `sizeThreshold=0`, in pages of
  500 up to Data's offset of 10,000, past which it is a `VenueRefusal`. `balances` is one pUSD
  balance with six decimals and each allowance in `BalanceExt`, an unlimited approval being
  `Decimal::MAX`.

### The client-order-id ledger (D21)

The CLOB stores no client order id, so a `Clob` keeps an in-process ledger of the ones it placed
orders with, by the order's id, shared by its clones and its events streams, bounded at 10,000
entries and lost when the process ends. An entry has two parts: the order's client id, and the
reuse guard that refuses a second order with the same id.

- `place` drops both when the post is refused, and keeps both after a `Network`, `Unavailable` or
  `Decode` failure (D18; the last includes a 2xx `POST /order` whose body does not decode, since
  the order may be on the venue).
- `place` of an order that ended at once (`Killed`, `Filled`, `Cancelled`), `cancel`,
  `cancel_all`, and an ack of a `Cancelled` or `Filled` order drop only the guard, so the id may be
  placed again at once while later acks and fills still carry it.
- The bound evicts the oldest entry, and its guard only while the guard still names that order.

### `events()`

It opens its own socket when first polled (`ClobBuilder::user_socket` sets its URL and timing)
and yields:

- a leading `Resync`; a `Resync` after every reconnect, and for a `FAILED` trade (the venue voided
  a fill the stream reported, and the trait has no event that takes one back);
- one `Fill` per order of the account's in a `MATCHED` trade: the taker's when the trade's `owner`
  is the account's API key and no maker entry is the account's, so a self-trade yields only its
  maker fills (D17), then each maker entry at its maker address, a maker on the trade's own token
  taking the opposite side;
- an ack per order message. A placement's `created` and a fill's `time` are the user channel's
  `timestamp`, read as seconds below 10^11 and as milliseconds otherwise (D23), since the mirror's
  schema says milliseconds but its examples are seconds.

`MINED`, `CONFIRMED` and `RETRYING` yield nothing; an order message does not carry the time in
force; text that does not parse is an `Err` item that skips its message. `tests/trading_clob.rs`
drives all of it on mock servers and the kit's scripted server.
