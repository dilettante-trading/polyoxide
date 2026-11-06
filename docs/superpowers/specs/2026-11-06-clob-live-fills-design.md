# CLOB live fills: a manual live test that buys, fills and sells back

Decided with the owner on 2026-11-06 in a brainstorming session; every decision is below, and
nothing is left for the plan to settle.

## Goal

On 2026-11-06 every CLOB order test passed against the venue for the first time on 0.48.0 (both
resting round trips, the unmatched FAK, the heartbeat), but each order either rested or was killed:
no test has ever seen a fill. After this work one live test, run by hand, buys about $3 of one
outcome token in two marketable orders, checks every fill path the SDK has (`Trading::place`'s
read-back, the user channel's `Fill` events, positions, balances, the native post response, order
and trade reads), and sells everything back. It is the first run of the partial-FAK branch, the
matched-FOK read-back and the taker `Fill` decode against the venue.

## Non-goals

- The nightly. The test spends real money, so it never runs there (the owner's choice), whether or
  not the `POLYMARKET_*` secrets are ever set.
- Protocol V2 fills. No V2 market exists (`live_v2_market_is_served` was `environmental` on
  2026-11-05); the deferred V2 fill checklist in `deferred-work.md` stays open.
- Maker fills. A maker fill needs a counterparty the test cannot arrange; D17's maker-only frame
  capture stays deferred.
- Markets with a taker fee or a taker delay, and markets off a 0.01 tick. The selection skips them.
- Changes to the existing live tests. Nothing in `live_clob.rs`, `live_clob_trading.rs` or
  `live_clob_ws.rs` moves or changes, so the test-bodies job is untouched.

## Part 1: `manual` live targets in the registry

Every `tests/live_*.rs` must have a `[package.metadata.polyoxide.live.<target>]` entry, and every
entry becomes a nightly job. A new optional key keeps an entry registered but out of the nightly.

- **The key.** `manual = "<why>"`, a non-empty string saying why the target is run only by hand.
  `scripts/gen_registry.py` adds `"manual"` to `LIVE_KEYS`, a `manual: str | None` field to `Live`,
  and parses it in `_live` with `_string(..., optional=True)`; an empty or whitespace-only string is
  a `RegistryError` ("manual must say why the target is run only by hand"). A manual entry may not
  name a `runner` (a runner exists only to run a target in the nightly): that is a `RegistryError`
  too. The key is documented in the module docstring's live block beside `runner`.
- **The one seam.** `Registry.jobs()` skips every `Live` whose `manual` is set. Every nightly output
  is built from `jobs()` (`nightly_live_jobs`, `nightly_aggregate_needs`, `claude_nightly`'s job
  lines, `selfheal_behavioral`), so a manual target appears in neither workflow nor in SELF-HEALING's
  table. A suite whose every target is manual makes no job.
- **What still holds for a manual entry.** It counts as registered for
  `test_every_live_target_is_registered`; its `features` must still match the target's
  `required-features`; its `secrets` must still equal the loader names the target reads; its tests
  must still all be `#[ignore]`d; `scripts/live_unwraps.py` still reads it.
- **Where it is listed.** `claude_nightly` appends, after the job lines, one line per manual target:
  ``- Manual, never in the nightly: `<target>` in `<crate>` with `--features <f>`: <why>``. Run it
  by hand with `-- --ignored`. So CLAUDE.md's generated `claude-nightly` region names it.
- **Rule text (AD-21).** `.claude/rules/live-tests.md` gains a paragraph under its first section:
  a `manual` entry is registered and checked like any other but makes no nightly job, and today's
  one is `live_clob_fills`, which spends real money.
- **Tests** in `.github/scripts/tests/test_live_registry.py`, each shown to fail before the
  `gen_registry.py` change: a manual entry yields no `Job`; an empty `manual` is refused; `manual`
  with `runner` is refused; the real `live_clob_fills` entry appears in neither
  `nightly-behavioral.yml` nor the aggregate's `needs:`, and does appear in CLAUDE.md's
  `claude-nightly` region. `test_the_generated_jobs_equal_the_registered_entries` compares the jobs
  with the non-manual entries.

## Part 2: `polyoxide-polymarket/tests/live_clob_fills.rs`

One `#[tokio::test] #[ignore]` test, `live_clob_fills_buy_partially_and_in_full_then_sell_back`.

**Registration.** A `[[test]] name = "live_clob_fills"`, `required-features = ["clob-ws", "data",
"gamma"]`, and the live entry `suite = "clob"`, `timeout = 15`, the same `features`, `secrets` the
four `POLYMARKET_*` names `live_clob_trading` reads, and `manual = "spends real money: buys about $3
of one outcome token in two marketable orders and sells it back, at a cost of about the spread"`.
Run locally, through loom:

```
cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data,gamma,keychain \
  --test live_clob_fills -- --ignored --nocapture
```

**Clients.** The account is loaded as in `live_clob_trading.rs` (`load_env`, then `keychain` under
`polyoxide-clob`, then `or_auth_gated`); one `Clob` built with `SignatureType::PolyProxy` serves
both the native calls and the `Trading` calls (through `DynTrading::new_arc` on a clone), so the
client-id ledger and the events stream are the same client's.

### Before any spend

Each failure here is `environmental` and moves no money.

1. **Funds.** `Trading::balances()`; the pUSD `total` must be at least $5.00 (the CLOB reports no `available`), else
   "fund the proxy wallet with at least $5 of pUSD".
2. **Selection.** From gamma's open markets (`closed(false)`, `limit(100)`), take each market's first
   token whose gamma `best_ask` is in [0.10, 0.40], and confirm on the CLOB (at most 15 `/book`
   probes, as `find_token_the_bid_can_rest_on` does, failing with the last error when every probe
   errored). A token qualifies when its live book has:
   - a best ask `a` in [0.10, 0.40] and `tick_size` exactly 0.01;
   - `min_order_size` at most 5 (absent counts as 5);
   - a best bid `b` with `a - b <= 0.02`;
   - a top ask level of depth `d` with `d * a < 1.50`;
   - bid depth at prices `>= b - 0.03` of at least the planned total buy (below);
   - and the CLOB's market details for its condition (`ClobMarketDetails`) show `tbf` absent or 0
     and `itode` absent or false.
3. **Approval.** The token's `balance_allowance` must show a non-zero allowance for every exchange
   it reports, else "the wallet has not approved the exchange for outcome tokens".

**Sizes**, all whole shares so a BUY's pUSD amount has at most 2 decimals:
- partial FAK: `s1 = max(floor(d) + 1, ceil(1.00 / a))`, which is more than `d`, and whose order
  notional `s1 * a` is at least the $1 marketable minimum; it spends `d * a < $1.50`;
- FOK: computed after the partial, on the re-read book with best ask `a2`:
  `s2 = max(min_order_size, ceil(1.00 / a2))`; its limit `p2` is the lowest ask price at which the
  cumulative ask depth reaches `s2`, and must be `<= a2 + 0.02`, else `environmental` (nothing spent
  on it). It spends at most `s2 * p2`, about $2.10 at most.
- The planned total buy for the bid-depth check is `s1 + max(5, ceil(1.00 / a))`.

### The three orders

`Trading::events()` is opened first and its first item must be `Resync` (30 s, else `transient`).
pUSD `total` is read again just before the first order: the baseline.

1. **Partial FAK buy**, through `Trading::place`: `Side::Buy`, price `a`, size `s1`,
   `TimeInForce::ImmediateOrCancel`, a unique `ClientOrderId`. Expected: `Killed { Unmatched }` with
   `0 < filled < s1`. If it comes back `Filled` (the level refilled at `a` in between), the run
   goes on: the fills are checked the same way, and the test ends `environmental` after the sale
   ("the top level refilled; the partial branch was not reached"), since everything else was
   checked.
2. **FOK buy**, through `Trading::place`: `Side::Buy`, price `p2`, size `s2`,
   `TimeInForce::FillOrKill`, a unique `ClientOrderId`. Expected: `Filled` with `filled == s2`. A
   `Killed { Unfillable }` (the book moved; nothing filled) is `environmental` after the guard sells
   what the partial bought.
3. **Sale**, through the native `place_order`: `OrderSide::Sell`, `OrderKind::Fak`, size the
   credited token balance floored to 0.01 share, price `max(0.01, b' - 0.03)` where `b'` is the
   re-read best bid. Expected native response: `success`, `status` `matched`, `making_amount` equal
   to the size sold, and `taking_amount` between `size * price` and `size * b'` (both inclusive).
   A sale short of its size is followed by the guard's sale of the rest, then fails `environmental`
   when the re-read book's bids at `>= b' - 0.03` hold less than the remainder, else `real`.

### What each order is checked against

- **Read-back.** `GET /data/order/{id}` (`orders().get(id)`): `size_matched` equals the `filled`
  `Trading` reported (orders 1 and 2) or the native sale's size (order 3).
- **Fill events.** For each order, the `Fill` events with its `order_id` (30 s from placement,
  else `transient`; other events are skipped): sizes summing to its filled size; `side` the order's;
  each `price` at or better than its limit (`<=` for a buy, `>=` for the sale); `liquidity`
  `Taker`; `client_order_id` the request's for orders 1 and 2 and `None` for the native sale;
  `fee` `None` or zero; `FillExt::taker_order_id` the order's id.
- **Trades.** The account's `/data/trades` (queried with the proxy wallet's address, the read-back's
  `maker_address`; polled every 3 s for up to 30 s) lists a trade whose taker order id is each order's id,
  with `owner` the API key (a UUID, not an address), `side` the order's, in status `MATCHED`,
  `MINED` or `CONFIRMED`.
- **Credit.** Before the sale, the token's `balance_allowance` balance is polled (every 3 s, up to
  90 s, refreshing it with `update_balance_allowance` first, else `transient`) until it reaches the
  sum of the two buys' fills.
- **Positions.** `Trading::positions()` (Data v2, `max-age=5`) is polled every 5 s for up to 60 s:
  after the buys it lists the token with size equal to the credited balance (within 0.01); after
  the sale it no longer lists it (what is left is under 0.01 share, which v2 serves only as
  `CLOSED`). Not seen in 60 s fails `real`.
- **Cost.** The cost is read from the three orders' trades, not from the `Fill` events: a taker
  `Fill` carries the trade's one `price`, but a marketable order can match several makers, and a
  maker on the complementary outcome (a NO buyer at 0.70 filling a YES buy at 0.30). For each
  maker entry of each of the three trades, the price per share of the test's token is the maker's
  `price` when its `asset_id` is the token and `1 - price` otherwise; the cost is the buys'
  `sum(matched_amount * per-share price)` less the sale's. After the sale, pUSD `total` is polled
  (every 3 s, up to 90 s, refreshing it with `update_balance_allowance` first) until it has fallen
  from the baseline by that cost, within 0.01; else `real`. The cost is printed.

### When something fails after money moves

- From the first `place` on, the test body runs inside `AssertUnwindSafe(..).catch_unwind()`, as
  `live_trading_places_acks_and_cancels_a_resting_post_only_bid` does around its cancel.
- On any failure the guard reads the token's balance and, if at least 0.01 share is held, sends a
  native FAK SELL of all of it floored to 0.01 share at `max(0.01, best bid - 0.03)`, then resumes
  the original panic. If the guard's sale fails or falls short, it prints one line naming the token
  id, the market slug, the size still held and "sell it by hand on polymarket.com", and still
  resumes the original panic.
- A `place` that fails with `Network`, `Unavailable` or `Decode` may have reached the venue (D18),
  so it runs the guard too.
- None of the orders rest, so the test and `live_heartbeat` cannot cancel each other's orders.
- At most about $3.60 of outcome tokens is ever held, and it stays sellable.

### Tags

As every live test: `.or_fail` and `fail` take their tag from the error's class (a network error,
a 5xx or a 429 is `transient`); the credential loaders fail `auth-gated`; the pre-spend checks and
the market-moved branches above call `environmental`; every assertion fails untagged, so `real`.
`scripts/live_unwraps.py` holds the new file at zero bare sites, with any opt-out on its line.

## Testing

- The registry tests of Part 1.
- The fill test is proven by one run against the venue, with the owner's go-ahead, through
  `loom job`. A run without credentials (no keychain feature) must fail `auth-gated` and send no
  order.
- Before the commit: the format job, the lint & test job's clippy (which builds the new target),
  the CI scripts job (the registry tests, the generated regions and `live_unwraps.py`) and the test
  bodies job, all through `direnv exec .`.

## After it runs

Once the account has traded, `live_trade_owner_is_a_uuid_not_an_address` and `live_list_trades`
in `live_clob.rs` stop proving nothing; their next credentialed run reads real trades. The memory
note on live order safety records the result.
