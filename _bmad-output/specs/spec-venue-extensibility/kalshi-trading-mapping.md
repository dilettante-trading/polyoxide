# Kalshi trading mapping

Epic 7, Story 7.5. This maps every common Kalshi order field and fill field onto `polyoxide_venue::Trading` and its records (`polyoxide-venue/src/trading.rs`, feature `trading`). It shows the trait fits a second event-contract venue before Kalshi ships. Each field lands in one of three places: a trait field, a named field of a future Kalshi option or extension type, or a deferral to Kalshi integration.

This is a planning companion (spec D9 of Epic 7's design, `docs/superpowers/specs/2026-10-17-epic-7-supervised-clob-sockets-and-trading-design.md`: the owner's choice of 2026-10-12). Epic 8 leaves it here (spec D21 of `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`), since moving it would edit `docs/s3-migration.md`'s citation, and `deferred-work.md` holds its move to `docs/specs/kalshi/trading-mapping.md` with Kalshi's `Trading`.

**Sources, read on 2026-10-12:**

- Kalshi's OpenAPI `docs.kalshi.com/openapi.yaml`, `info.version` 3.34.0. The JSON pointers below point into it.
- The AsyncAPI `docs.kalshi.com/asyncapi.yaml`, `info.version` 2.0.0. Pointers into it are marked "AsyncAPI".
- These prose pages: `getting_started/order_direction.md`, `getting_started/fixed_point_migration.md`, `fix/order-entry.md`, and the changelog.

The legacy order-write routes under `/portfolio/orders` have been retired: a call returns "Please switch to the V2 endpoints" (changelog, 2026-06-18). So every write below uses the V2 routes under `/portfolio/events/orders`. Reads stay on `/portfolio/orders`.

## Keys and the YES-side conversion

A Kalshi key names one outcome: `kalshi.predictions:<TICKER>:yes` or `kalshi.predictions:<TICKER>:no` (AD-7, with the product `predictions` of Epic 8's spec D1). The AsyncAPI gives the ticker the pattern `^[A-Z0-9-]+$` (`#/components/schemas/marketTicker`), but Kalshi's tickers and the AsyncAPI's own examples carry dots (`FED-23DEC-T3.00`). Epic 8's Story 1 wrote the product's `KeySpace`, `PredictionsKeys` (`polyoxide-kalshi/src/predictions/key.rs`, spec D8): its `parse_id` splits the id at its last `:`, uppercases the ticker and checks it against `[A-Z0-9.-]` (1 to 200 characters, at least one of them an ASCII letter or digit), and reads `yes` or `no` after it, lowercasing the outcome, and its `id_text` writes `<TICKER>:<outcome>`. A key is made only through it, since `MarketKey` has no other constructor.

A V2 order addresses the YES leg only. `side` is a `BookSide`, `bid` (buy YES) or `ask` (sell YES), and `price` is the YES price. Kalshi's own description of `BookSide` (`#/components/schemas/BookSide`) says: "Selling YES is economically equivalent to buying NO at `1 - price`". An `OrderRequest` therefore converts as follows, with `p` its price:

| Key's outcome | `OrderRequest.side` | Kalshi `side` | Kalshi `price` |
|---|---|---|---|
| `:yes` | `Buy` | `bid` | `p` |
| `:yes` | `Sell` | `ask` | `p` |
| `:no` | `Buy` | `ask` | `1 − p` |
| `:no` | `Sell` | `bid` | `1 − p` |

**Buying NO at p is a YES ask at 1 − p.**

Kalshi's reports keep only the net direction. On `Order` and `Fill`, `outcome_side` and `book_side` follow `bid ≡ yes, ask ≡ no`, "always" (`order_direction.md`). Kalshi itself makes buy-NO and sell-YES the same thing. A report therefore reads back in canonical form:

- `outcome_side: yes` becomes the `:yes` key, `Side::Buy`, at `yes_price_dollars`.
- `outcome_side: no` becomes the `:no` key, `Side::Buy`, at `no_price_dollars`. The WebSocket messages carry no `no_price_dollars`, so the price there is `1 − yes_price_dollars`.

So an order placed as "sell YES at 0.60" reads back as "buy NO at 0.40". It is the same order, converted deterministically as AD-7 requires. Every path reports this canonical form, so one order reads the same on `place`, `open_orders` and `events()`: `place` of `Sell` on `:yes` at 0.60 returns `Buy` on `:no` at 0.40.

**A NO position is a positive position in `kalshi.predictions:<TICKER>:no`.** `MarketPosition.position_fp` is signed: "Negative means NO contracts and positive means YES contracts" (`#/components/schemas/MarketPosition`). It maps to one `Position`:

- A positive value is a `Position` on `:yes` with `size = position_fp`.
- A negative value is a `Position` on `:no` with `size = −position_fp`.
- Zero yields no `Position`.

YES and NO in one market are netted, so the two keys never both hold a position.

## Order fields

### Request: `CreateOrderV2Request` (`#/components/schemas/CreateOrderV2Request`, `POST /portfolio/events/orders`)

| Kalshi field | Required | Maps to | Rule |
|---|---|---|---|
| `ticker` | yes | `OrderRequest.key` | The key's ticker part. A key of another venue or product is refused as `InvalidRequest` before anything is sent. |
| `side` | yes | `OrderRequest.side`, with the key's outcome | The conversion table above. |
| `count` | yes | `OrderRequest.size` | A `FixedPointCount` string of contracts (`SizeUnit::Shares`: each contract pays one dollar on its outcome, as the CLOB's shares do, spec D12 of Epic 8). Requests accept 0–2 decimal places, and the minimum granularity is 0.01 (`#/components/schemas/FixedPointCount`). The size is normalized (trailing zeros dropped) before its scale is checked, so `10.000` passes. One with more than 2 decimals after that is refused as `InvalidRequest`, not rounded. It is sent with at least 2 decimals (`10.00`, `2.50`). |
| `price` | yes | `OrderRequest.price` | A `FixedPointDollars` YES price, with 2–4 decimals on requests (`#/components/schemas/FixedPointDollars`). It is `1 − price` for a `:no` key. The price is normalized before its scale is checked, so `0.5` and `0.5000` both pass. One with more than 4 decimals after that is refused as `InvalidRequest`. It is sent with at least 2 decimals (`0.50`). A price off the market's `price_ranges` step is Kalshi's to refuse, as a `VenueRefusal`. |
| `time_in_force` | yes | `OrderRequest.time_in_force` | `GoodTilCancelled` becomes `good_till_canceled`, `ImmediateOrCancel` becomes `immediate_or_cancel`, and `FillOrKill` becomes `fill_or_kill`. The three are the same set. |
| `client_order_id` | no | `OrderRequest.client_order_id` | Sent as is. A duplicate is a `409` (quick-start page), so a `VenueRefusal`. |
| `post_only` | no | `OrderRequest.post_only` | Sent as is. |
| `reduce_only` | no | `OrderRequest.reduce_only` | Sent as is. Kalshi rejects it unless the order is `immediate_or_cancel`. Integration decides whether to refuse that combination before sending. |
| `expiration_time` | no | `KalshiOrderOptions.expires_at: Option<UnixMillis>` | Sent as epoch seconds. Valid only with `good_till_canceled`, since `immediate_or_cancel` "cannot be combined with `expiration_time`". This is a venue option, as `ClobOrderOptions` carries the CLOB's expiration. |
| `cancel_order_on_pause` | no | `KalshiOrderOptions.cancel_order_on_pause: Option<bool>` | Sent when set. |
| `self_trade_prevention_type` | **yes** | Deferred | Kalshi requires it, so integration sends one fixed value it names until the option is designed. Under `taker_at_cross`, a fill-or-kill order that self-crosses keeps its partial fills (changelog, 2025-09-25). |
| `subaccount` | no | Deferred | Omitted, which means the primary account, 0. |
| `order_group_id` | no | Deferred | Omitted. |
| `exchange_index` | no | Not sent | Omitted, so Kalshi auto-routes by ticker. The shard comes back in `KalshiOrderExt.exchange_index`. |

`KalshiOrderOptions` is the one option type Kalshi's `place` declares, carried in `OrderRequest.extensions`. Any other extension on the request is refused as `InvalidRequest` by `Extensions::ensure_declared(&[TypeId::of::<KalshiOrderOptions>()])`, which returns `UndeclaredExtension`.

V2 has no `action`, no yes/no `side`, no `type` and no `buy_max_cost`. Market orders were withdrawn, and a marketable order is a limit priced through the book. Those fields exist only in the orphaned legacy schema, so nothing maps them.

### Reports: `CreateOrderV2Response`, `Order` and the `user_order` message

`place` gets back a `CreateOrderV2Response` (`#/components/schemas/CreateOrderV2Response`). It holds `order_id`, `client_order_id`, `fill_count`, `remaining_count`, `average_fill_price`, `average_fee_paid` and `ts_ms`, and no status, side or `Order`. The implementation builds the returned `Order` from the request plus those fields.

`open_orders` reads `Order` (`#/components/schemas/Order`), and `events()` reads the AsyncAPI `user_order` message (`#/components/schemas/userOrderPayload`). A `Kalshi*Ext` named below is the one value of that type in the record's `extensions`.

| Kalshi field | Maps to | Rule |
|---|---|---|
| `order_id` | `Order.id` | Kalshi's UUID. |
| `ticker`, `outcome_side` | `Order.key` | The canonical outcome, above. |
| `outcome_side`, `book_side` | `Order.side` | `Buy` on the canonical outcome. |
| `yes_price_dollars`, `no_price_dollars` | `Order.price` | Read by outcome. The WebSocket price for a NO order is `1 − yes_price_dollars`. |
| `initial_count_fp` (or the request's `count`) | `Order.size` | |
| `fill_count_fp`, `fill_count` | `Order.filled` | |
| `status` | `Order.status` | `resting` becomes `Open`, `executed` becomes `Filled`, and `canceled` becomes `Cancelled`, or `Killed` when the order's time in force ended it (below). `status` is `#/components/schemas/OrderStatus`, plus the WebSocket's `unknown`, which integration settles by re-reading the order. |
| `client_order_id` | `Order.client_order_id` | |
| `created_time`, `created_ts_ms`, `ts_ms` | `Order.created` | `created_time` is RFC 3339 on REST. The two `ts_ms` fields are epoch milliseconds. |
| (none) | `Order.time_in_force` | Kalshi does not report it, so the field is `None`, or `Some` for an order the implementation placed. |
| `remaining_count_fp`, `remaining_count` | `KalshiOrderExt.remaining` | |
| `average_fill_price`, `average_fee_paid` | `KalshiOrderExt.average_fill_price`, `.average_fee_paid` | Create response only. |
| `taker_fees_dollars`, `maker_fees_dollars`, `taker_fill_cost_dollars`, `maker_fill_cost_dollars` | `KalshiOrderExt.taker_fees`, `.maker_fees`, `.taker_fill_cost`, `.maker_fill_cost` | |
| `expiration_time`, `expiration_ts_ms` | `KalshiOrderExt.expires_at` | |
| `last_update_time`, `last_updated_ts_ms`, `last_update_reason` | `KalshiOrderExt.last_updated`, `.last_update_reason` | `last_update_reason` names why a `canceled` order ended: `SelfTradeCancel`, `PostOnlyCrossCancel`, `ReduceOnlyCancel`, `ExpiryCancel` and so on (WebSocket only). |
| `exchange_index`, `cancel_order_on_pause` | `KalshiOrderExt.exchange_index`, `.cancel_order_on_pause` | |
| `self_trade_prevention_type`, `order_group_id`, `subaccount_number` | Deferred | |
| `user_id` | Not mapped | The account's own id. |
| `side`, `action`, `is_yes` | Not read | Deprecated in favour of `outcome_side` and `book_side`. |

**Kills (AD-20).** Through the trait, a kill is `Ok` with `Killed { reason }`, never an error.

- **Immediate-or-cancel.** `remaining_count` "reflects the final state after unfilled contracts are canceled". So a create response with `remaining_count` 0 and `fill_count` below `count` is `Killed { reason: Unmatched }`, with `filled = fill_count`.
- **Fill-or-kill that cannot fill.** The REST outcome is not documented: either a `201` with `fill_count` 0, or a 4xx. FIX reports `FOK_INSUFFICIENT_VOLUME` as an execution `Canceled`, not a reject (`fix/order-entry.md`). Whichever it is, the trait returns `Ok` with `Killed { reason: Unfillable }`, and the probe below settles which signal the implementation reads.
- **Fill-or-kill that self-crosses.** It may end partly filled (changelog, "FoK orders that self-cross treated as IoC"). Kalshi cancelled it partway, so it is `Cancelled` with its partial `filled`, not `Killed`, as `TimeInForce::FillOrKill`'s contract says. The reason (`SelfTradeCancel`) goes to `KalshiOrderExt.last_update_reason`.
- **What Kalshi cannot tell the implementation.** Kalshi reports no time in force on `Order`, and `last_update_reason` has no value for an immediate-or-cancel remainder. So a kill is recognised only on an order whose time in force the implementation knows, which means one it placed. Any other order ends `Cancelled`, with `time_in_force: None`. `open_orders` never returns a killed order, so reconciliation is unaffected.

## Fill fields

The fills come from the AsyncAPI `fill` message (`#/components/schemas/fillPayload`, channel `fill`), which feeds `events()`. Trade history comes from the REST `Fill` (`#/components/schemas/Fill`, `GET /portfolio/fills`).

| Kalshi field | Maps to | Rule |
|---|---|---|
| `trade_id` | `Fill.id` (`FillId`) | "Unique identifier for fills". On REST, `trade_id` is the "legacy field name, same as fill_id". It is unique per fill, so it does not go to `Fill.trade_id`, which names one match. `Fill.trade_id` stays `None` until probe 8 shows both sides of a match share an id. |
| `fill_id` (REST) | `Fill.id` | The same value as `trade_id`. |
| `order_id` | `Fill.order_id` | |
| `market_ticker` (WebSocket), `ticker` (REST), `outcome_side` | `Fill.key` | The canonical outcome. |
| `outcome_side`, `book_side` | `Fill.side` | `Buy` on the canonical outcome. |
| `yes_price_dollars`, `no_price_dollars` (REST) | `Fill.price` | Read by outcome. The WebSocket message has no `no_price_dollars`, so its price is `1 − yes_price_dollars`. |
| `count_fp` | `Fill.size` | Contracts, to 2 decimals. |
| `fee_cost` | `Fill.fee` | Dollars, the quote currency, up to 6 decimals. |
| `is_taker` | `Fill.liquidity` | `true` becomes `Liquidity::Taker`, and `false` becomes `Maker`. |
| `ts_ms` (WebSocket), `created_time` (REST) | `Fill.time` | Epoch milliseconds, and RFC 3339 respectively. REST's `ts` states no unit and is not read. |
| `client_order_id` (WebSocket only) | `Fill.client_order_id` | REST fills dropped it on 2026-03-30. There it is `None`, or joined through `order_id`. |
| (none) | `Fill.notional` | `None`: Kalshi reports no quote amount. |
| `post_position_fp` (WebSocket) | `KalshiFillExt.post_position` | The signed net position after the fill. A negative value means NO contracts. |
| `exchange_index` | `KalshiFillExt.exchange_index` | |
| `subaccount`, `subaccount_number` | Deferred | |
| `side`, `action`, `purchased_side`, `ts` | Not read | Deprecated. `ts` is in seconds. |

## The trait's methods

- **`place`** is `POST /portfolio/events/orders`, with the request and report rules above.
- **`cancel(&OrderId)`** is `DELETE /portfolio/events/orders/{order_id}`. Kalshi warns that "an `order_id` alone cannot identify the exchange shard", and auto-routing needs `market_ticker`.
  - The implementation passes the ticker of an order it placed.
  - Otherwise it first reads `GET /portfolio/orders/{order_id}`, which takes no shard parameter and returns the order's `ticker` and `exchange_index`.
  - A `404` is a `VenueRefusal`.
- **`cancel_all(Option<&MarketKey>)`** lists the resting orders with `GET /portfolio/orders?status=resting`, adding `&ticker=` when a key is given. It cancels them with `DELETE /portfolio/events/orders/batched` and returns the ids whose items carry no `error`. The batch size limit scales with the tier, so a long list goes in several batches.
  - An item that errored inside the `200` stays open, and the consumer sees it in `open_orders`.
  - Kalshi's all-shards `DELETE /portfolio/events/orders` is not used. It answers `204` with no ids, and "newly placed orders may also be cancelled during the minute after the request".
- **`open_orders(Option<&MarketKey>)`** is `GET /portfolio/orders?status=resting`, adding `&ticker=` when a key is given, paged by `cursor`.
- **`positions`** reads `market_positions` from `GET /portfolio/positions`, with the sign rule above. Its subaccount defaults to 0.
  - `total_traded_dollars`, `market_exposure_dollars`, `realized_pnl_dollars`, `fees_paid_dollars` and `last_updated_ts` go to `KalshiPositionExt`.
  - `Position.average_price` is `None`, since Kalshi reports cost, not an average.
  - `event_positions` are per-event aggregates with no outcome key, and are not mapped.
- **`balances`** reads `GET /portfolio/balance` into one `Balance { asset: "USD", available: balance_dollars, total: None }`.
  - `portfolio_value` is integer cents with no `_dollars` twin, and goes to `KalshiBalanceExt.portfolio_value`, converted to dollars.
  - `balance_breakdown` (per shard) goes to `KalshiBalanceExt.per_shard`.
  - The integer-cent `balance` truncates sub-cent amounts and is not read.
- **`events()`** combines two WebSocket channels, both behind the signed handshake.
  - The `fill` channel feeds `TradingEvent::Fill`, and the `user_orders` channel feeds `TradingEvent::Ack`.
  - Both channels ride one connection, as subscriptions on Kalshi's one WebSocket endpoint. Neither envelope has a `seq`, and Kalshi replays nothing on resubscribe.
  - **The leading `Resync` (spec D3).** The stream yields `Resync` as its first item, once its connection's first connect attempt has ended, whether the connection went live or began in an outage. A fill between `events()` returning and the first subscription is therefore covered by a reconciliation.
  - **After every reconnect.** `Resync` comes before any later event, acknowledgement or fill (AD-24), so an outage of `user_orders` is flagged as surely as one of `fill`.
  - **The void rule (spec D4).** `Resync` also comes when the venue voids a fill it already reported, because the trait has no event that takes a fill back. Kalshi's `fill` message carries no status, and the pages read for this mapping describe no way for Kalshi to void a reported fill, so an implementation yields none today. If a probe finds one, it yields `Resync`, and `Resync` keeps its meaning: reconcile through `open_orders()` and the trade history, since the stream cannot vouch for what it reported or missed.

## Future Kalshi types named above

Kalshi integration defines these. Every field is an `Option`, so absent data stays absent.

- `KalshiOrderOptions`, the request option `place` declares:
  - `expires_at: Option<UnixMillis>`;
  - `cancel_order_on_pause: Option<bool>`.
- `KalshiOrderExt`:
  - `remaining`, `average_fill_price`, `average_fee_paid`, `taker_fees`, `maker_fees`, `taker_fill_cost` and `maker_fill_cost` (`Decimal`);
  - `expires_at` and `last_updated` (`UnixMillis`);
  - `last_update_reason` (an `open_enum!`);
  - `exchange_index` (`u32`);
  - `cancel_order_on_pause` (`bool`).
- `KalshiFillExt`:
  - `post_position` (`Decimal`);
  - `exchange_index` (`u32`).
- `KalshiPositionExt`:
  - `total_traded`, `market_exposure`, `realized_pnl` and `fees_paid` (`Decimal`);
  - `last_updated` (`UnixMillis`).
- `KalshiBalanceExt`:
  - `portfolio_value` (`Decimal`, dollars);
  - `per_shard` (each shard's index and balance).

## Deferred to Kalshi integration

- **Subaccounts.** These are `subaccount` on requests, and `subaccount_number` on REST `Order` and `Fill` and on the WebSocket `user_order` message. An omitted subaccount means every subaccount on orders, fills and cancel-all, and the primary account everywhere else.
- **Order groups.** These are `order_group_id`, the `/portfolio/order_groups/*` routes and the WebSocket `order_group_updates` channel.
- **RFQ and block trades.** These are the `/communications/*` routes and the WebSocket `communications` channel.
- **Self-trade prevention.** `self_trade_prevention_type` is required on every V2 create, so integration sends a fixed value until the option is designed.

These are also left to the native client and kept out of the trait: amend and decrease (`POST /portfolio/events/orders/{order_id}/amend` and `/decrease`), batch create, and explicit shard routing (`exchange_index`, which is auto-routed here).

## Misfits

None. Every common order field (ticker, side, count, price, time in force, client order id, post-only, reduce-only) maps to a trait field. So does every common fill field (trade and order ids, price, size, fee, taker flag). Each other Kalshi field maps to a named Kalshi option or extension field, or to a deferral. `spine-amendments/epic-7.md` records this as 7.5's "no amendment".

Two behaviours fit the trait without changing it. Every path reports an order and its fills in canonical outcome form (AD-7's deterministic conversion), so a `Sell` on `:yes` reads back as a `Buy` on `:no`. A kill is recognised only on an order whose time in force the implementation knows.

## To probe on the demo host before Kalshi's `Trading` implementation ships

The documentation does not settle these points:

1. The REST outcome of a fill-or-kill order that cannot fill: a `201` with `fill_count` 0, or a 4xx.
2. Whether an immediate-or-cancel order with no fill returns a `201` with `remaining_count` `"0.00"`. The schema's description implies it.
3. Whether a post-only order that would cross is refused (the legacy behaviour) or cancelled. This decides whether it surfaces as a `VenueRefusal` or as `Ok(Cancelled)`.
4. Whether V2 routes return the rejection codes of 2026-01-26 (`invalid_order`, `available_balance_too_low` and so on), which the error decode would key on.
5. The units of REST `Fill.ts`, `GetBalanceResponse.updated_ts` and the `min_ts`/`max_ts` filters. The mapping above reads none of them.
6. Whether `GET /portfolio/orders/{order_id}` finds an order on every shard, which `cancel`'s routing relies on.
7. Whether the bug of 2026-05-21, in which V2 cancel and amend responses described the wrong order, is fixed. `cancel_all` reads the batch-cancel items' `order_id`.
8. Whether both sides of one match carry the same `trade_id`. Kalshi documents it as unique per fill. If both sides share it, it can also fill `Fill.trade_id`; until then that field stays `None`.

## Release note

Epic 7's consumer-impact note is in `docs/s3-migration.md`, under "Epic 7: supervised CLOB sockets and normalized trading". It states that the epic is additive and names the `trading` feature and its items. Nothing public is removed or renamed.
