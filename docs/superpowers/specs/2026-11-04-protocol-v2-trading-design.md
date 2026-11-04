# Protocol V2 trading: ExchangeV3 signing, CONDITIONAL-V2 balances, Router position ops (#51)

Issue: https://github.com/dilettante-trading/polyoxide/issues/51. Decided with the owner on
2026-11-04 in a brainstorming session; every decision is below, and nothing is left for the plan to
settle.

## Goal

polyoxide can read a Polymarket Protocol V2 market (gamma's `Market::version`, `position_ids`,
`resolution_status`, on `main` since e9a4982c) but cannot trade one. After this work it signs V2
orders for ExchangeV3, reads V2 balances, chooses the right outcome ids from a gamma `Market`, lists
V2 outcomes in event groups, and builds and submits the Router's split, merge and redeem. V1 (CTF)
markets behave exactly as before.

## Non-goals

- `PositionManager.getPayout` and any on-chain balance read (`balanceOf`, `balanceOfBatch`) over
  RPC. py-sdk 0.12.0 calls neither for its own redeem path's payout; the caller supplies amounts.
- A typed `asset_type` for `update_balance_allowance`, which keeps its `impl Into<String>`.
- Live trading of a V2 market. No V2 market exists (none among 40,000 open markets on 2026-10-07);
  upstream's BUY/SELL fill checklist goes to `deferred-work.md` (see *Live*).
- Python bindings. `polyoxide-py` exposes no CLOB order, balance or relay call (only Data), so
  nothing changes there. The CLI places no orders and reads no balances, so it does not change
  either.
- Combo (module 3) splits through `CombinatorialModule.prepareCondition`. The Router builders
  accept a module-3 condition, as py-sdk's do, but no combo-leg helper is added.

## The reference

`polymarket-client` 0.12.0 (py-sdk, PyPI, uploaded 2026-10-01) is the reference; where its source
and the prose guides disagree, the source wins. Facts read from it, which this design follows:

1. **The protocol is a function of the asset id.** `_internal/protocol.py`'s
   `is_v2_position_id(asset_id)` parses the id as a uint256 (decimal, or `0x`/`0b`/`0o`, after
   stripping whitespace; anything else, or a value outside uint256, is *not* V2) and returns
   `(value & (((1 << 64) - 1) << 40)) == 0`: bits 40 to 103 all zero. V1 CTF token ids are keccak
   hashes and never pass. No lookup is made.
2. That one predicate chooses, per order: the domain version (`orders.py`: `"3"` if V2, else
   `"2"`), the exchange (`context.py`'s `resolve_order_exchange_address`: ExchangeV3 if V2, else
   the standard or neg-risk exchange by `neg_risk`), the balance asset type (`allowance.py`'s
   `resolve_order_balance_allowance_target`: a BUY is `COLLATERAL` with no asset id; a SELL is
   `CONDITIONAL-V2` if V2, else `CONDITIONAL`, with the asset id), and the ERC-1155 to approve
   (PositionManager if V2, else Conditional Tokens).
3. **Signing.** `typed_data.py` keeps one `Order` struct and one domain shape
   (`Polymarket CTF Exchange`, `version`, `chainId`, `verifyingContract`); only `version` and
   `verifyingContract` change. For signature type 3 the `TypedDataSign` envelope's outer (app)
   domain is that same exchange domain, so a V2 Deposit Wallet order is hashed under ExchangeV3 v3,
   and the ERC-7739 trailer is `appDomainSeparator ‖ contentsHash ‖ ORDER_TYPE ‖ uint16(len)`, as
   for V1.
4. **Order amounts** (`limit.py`, `market.py`) have no V2 branch. The API guide's note that
   ExchangeV3 reduces a BUY's collateral budget by the amount spent changes no client-side
   calculation.
5. **Order metadata** is read for both protocols through `/markets-by-token/{token}` then
   `/clob-markets/{condition}`, and `/tick-size`; py-sdk ignores `neg_risk` when choosing a V2
   exchange.
6. **Router** (`relayer/calls.py`): `split(bytes31,uint256)`, `merge(bytes31,uint256)`,
   `redeem(bytes31,uint256,uint256)` to `protocol_v2_router`
   `0x12121212006e4CD160D18e3f00711DA5c3372600`. `_protocol_v2_condition_id_bytes` accepts
   `0x` plus 62 hex digits, or `0x` plus 64 hex digits ending in `00` **or `01`** (the issue said
   zero only; the SDK wins), and narrows to bytes31; anything else is refused. `router_redeem_call`
   refuses an `outcome_index` other than 0 or 1 (CTF's index sets 1 and 2 do not apply). Amounts are
   uint256 base units (6 decimals); zero is accepted.
7. **Position ids** (`protocol.py`'s `decode_v2_outcome_position_id`): the 32-byte big-endian value's
   first byte is the module id, which must be 1 (binary), 2 (neg-risk) or 3 (combinatorial); its
   last byte is the outcome index, which must be 0 or 1; the first 31 bytes are the condition id.
8. **Addresses** (`environments.py`, production only): ExchangeV3
   `0xe3333700cA9d93003F00f0F71f8515005F6c00Aa`, PositionManager
   `0x006F54F7f9A22e0000CC2AB60031000000ae9fEF`, Router as above. py-sdk knows no Amoy V2
   deployment.
9. **Outcome ids from gamma** (`relayer/positions.py`'s `normalize_market_position_context`): a
   missing `version` is refused; `v2` takes `positionIds` and refuses an incomplete or missing
   pair; anything else takes `clobTokenIds` and refuses an incomplete or missing pair.

## Design

### 1. `shared::protocol` (crate-private, built with `clob` or `relay`)

A new module `polyoxide-polymarket/src/shared/protocol.rs`, declared in `shared/mod.rs` under
`#[cfg(any(feature = "clob", feature = "relay"))]` as `pub(crate) mod protocol;`:

- `pub(crate) fn parse_position_id(id: &str) -> Option<U256>`: fact 1's parse (trim, then decimal
  or `0x`/`0b`/`0o`, then uint256 range), an exact port.
- `pub(crate) fn decode_v2_outcome_position_id(id: &str) -> Option<(FixedBytes<31>, u8)>`: fact 7,
  an exact port; `None` for any refusal.
- `pub(crate) fn is_v2_position_id(id: &str) -> bool`, under `#[cfg(feature = "clob")]` (only
  `clob` asks it): **true when py-sdk's `is_v2_position_id` (fact 1) and
  `decode_v2_outcome_position_id` (fact 7) both accept the id**. It is one condition stricter
  than py-sdk's predicate (the owner's decision, 2026-11-04). py-sdk's predicate alone calls any
  id with bits 40 to 103 clear V2, toy ids such as `1`, `100` and `123456` included. Those can
  name no V2 position: their module byte is 0. They are also the ids existing tests sign with
  pinned values: `v2_order_signature_is_deterministic` and
  `order_digest_uses_correct_exchange_for_neg_risk` (token `100`) and the session-key vectors
  (py-sdk's golden order, token `1`). Requiring the id to decode too keeps every such test as it
  is. It agrees with py-sdk on every id the venue can issue: a V1 id is a keccak hash, so its
  bits 40 to 103 are not all clear; a V2 id has module byte 1 to 3 and outcome byte 0 or 1. The
  only ids classified differently are ones no market can carry, and the rules files say so.

Nothing here is public. Each module wraps it in its own types and errors.

### 2. CLOB

**Exchange table** (`clob/core/chain.rs`). Add `pub const fn exchange_v3(&self) -> Option<Address>`
on `Chain`: `Some(0xe3333700cA9d93003F00f0F71f8515005F6c00Aa)` for `PolygonMainnet`, `None` for
`PolygonAmoy`. No field is added to `Contracts`, whose fields are all public, so nothing breaks.

**Signing** (`clob/core/eip712.rs`).

The unit tests in this file call `exchange_address(chain_id, neg_risk)`,
`exchange_domain(chain_id, exchange)` and `deposit_wallet::envelope_digest(order, chain_id,
exchange, wallet)`, so all three keep their signatures and meanings, and no existing test body
changes:

- `exchange_address(chain_id, neg_risk)` stays the CTF table (standard or neg-risk exchange).
- `exchange_domain(chain_id, exchange)` stays the version `"2"` domain. It becomes a call to a new
  `fn order_domain(chain_id: u64, exchange: Address, version: &'static str) -> protocol::EIP712Domain`,
  which builds the domain at any version.
- New `pub(crate) fn order_exchange(chain_id: u64, order: &ClobOrder) -> Result<(Address, &'static str), ClobError>`:
  for an order whose `token_id` passes `is_v2_position_id`, `(chain.exchange_v3(), "3")`, and a
  chain with no ExchangeV3 is `ClobError::validation("Protocol V2 orders need ExchangeV3, which is
  not deployed on chain {id}")`; for any other order, `(exchange_address(chain_id,
  order.neg_risk)?, "2")`. An unsupported chain stays `ClobError::Crypto`, as today.
- `order_hash`, `sign_order` (through `compute_order_digest`) and `sign_order_as` (the type-3 app
  domain separator and trailer) take their exchange and version from `order_exchange` and their
  domain from `order_domain`. The `Order` struct, its type string and the trailer do not change.
- `deposit_wallet::envelope_digest(order, chain_id, exchange, wallet)`, the vector-pinned entry
  point, takes its version from `order.token_id` as `order_exchange` does (`"3"` for a V2 id,
  else `"2"`), so its V1 callers see no change and the V2 vectors call it with ExchangeV3.
- The doc comments that say "v2 domain" say which version applies when.

**Order creation** (`clob/client.rs`). In `get_market_metadata`, for a token that passes
`is_v2_position_id`, skip the `/neg-risk` lookup and use `options.neg_risk.unwrap_or(false)`; keep
the `/tick-size` lookup. Document on `Order::neg_risk` (`clob/types.rs`) and on
`ClobOrderOptions::neg_risk` (`clob/venue`) that it selects between the CTF exchanges and is
ignored for a Protocol V2 position id, which always settles on ExchangeV3.

**Balances** (`clob/api/account.rs`). `balance_allowance(token_id)` sends
`asset_type=CONDITIONAL-V2` when the id passes `is_v2_position_id`, else `CONDITIONAL`. The doc
comment of `update_balance_allowance` lists `"CONDITIONAL-V2"` among the asset types, for a V2
position id. `usdc_balance` does not change.

**`Trading`.** `place` needs no code of its own: its order id is `order_hash`, and it builds the
order with `create_order`. `balances`, `positions`, `open_orders`, `cancel*` and `events()` do not
change.

### 3. Gamma: the outcome-id selector

In `gamma/types.rs`:

```rust
/// One outcome of a market and the id it trades under on the CLOB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeId {
    /// The outcome's label, from `outcomes`.
    pub outcome: String,
    /// The id the CLOB trades it under: a clob token id for `v1`, a position id for `v2`.
    pub id: String,
}

impl Market {
    /// The market's outcomes with the ids they trade under, chosen by `version`.
    pub fn outcome_ids(&self) -> Result<Vec<OutcomeId>, OutcomeIdsError>;
}
```

The rule, in order:

1. `version` is `None`: `OutcomeIdsError::MissingVersion`. `Some(ProtocolVersion::Other(v))`:
   `OutcomeIdsError::UnsupportedVersion(v)`.
2. The ids: for `V1`, `clob_token_ids` decoded as a JSON array of strings (absent:
   `MissingIds`; not such an array: `MalformedIds`); for `V2`, `position_ids` (absent:
   `MissingIds`). The other field is never read, whatever it holds.
3. The labels: `outcomes` decoded as a JSON array of strings (absent or not such an array:
   `MalformedOutcomes`).
4. A different number of ids and labels: `OutcomeIdsError::CountMismatch { outcomes, ids }`.
5. Any id that is empty or not all ASCII digits: `OutcomeIdsError::InvalidId(id)`.
6. Otherwise the pairs, in order.

`OutcomeIdsError` is a new `thiserror` enum in `gamma/error.rs`, `#[non_exhaustive]`, re-exported
beside `GammaError`, with exactly the variants above. Its `Classify` is `Class::Decode` for every
variant (a response that arrived but carries no ids the client can trade), `is_fault` is `true`,
and `retry_after` is `None`. It is added to the `const _` assertion in
`polyoxide-polymarket/src/lib.rs`. It is not a `GammaError` variant, because `GammaError` decodes
only through its one `From<ApiError>`.

### 4. `clob::venue::event_group`

- `event_id_of(token)` queries gamma's `/markets` with `position_ids=<token>` when the token passes
  `is_v2_position_id`, else `clob_token_ids=<token>` as today; `closed=any` either way.
- `event_group` reads each market's outcomes through `Market::outcome_ids`. A market whose
  `version` is missing or `Other` is skipped, as today's non-`v1` markets are. Any other selector
  error fails the group as `ClassifiedError::new(Class::Decode, err)`, as today's malformed ids
  do. Each `OutcomeId` becomes an `EventOutcome` keyed `keys::clob(&TokenId::new(id))`, so `v1`
  and `v2` outcomes appear together.
- The private `strings` helper goes if nothing else uses it.
- `tests/venue_clob.rs`'s `event_group_leaves_out_the_outcomes_of_a_v2_market` is deleted and
  replaced by `event_group_keys_a_v2_market_by_its_position_ids`, whose event fixture gives
  market 1 `version: "v2"` and a `positionIds` pair and asserts that its outcomes appear with
  those ids. `tests/venue_clob.rs` did not exist at the `v0.40.0` baseline, so the test-bodies
  gate tracks none of its items: the replacement needs no `[[retired]]` or `[[drift]]` entry
  (an entry that matches nothing fails the job) and no DRIFT row. The change is visible to users
  (event groups now list V2 outcomes) and is recorded in the commit message and the CLOB rules.
- A second new test, `event_group_skips_a_market_with_no_known_version`, gives market 1
  `version: "v3"` and asserts that its outcomes are absent and the group still returns.

### 5. Relay: Router position ops

**Types** (`relay/types.rs`, re-exported from `relay`):

```rust
/// A Protocol V2 YES/NO position: its 31-byte condition and its outcome index (0 or 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct V2PositionId { condition_id: FixedBytes<31>, outcome_index: u8 }

impl V2PositionId {
    /// Decode a position id (fact 7).
    pub fn parse(id: &str) -> Result<Self, RelayError>;
    pub fn condition_id(&self) -> FixedBytes<31>;
    pub fn outcome_index(&self) -> u8;
}

/// Narrow a Protocol V2 condition id to bytes31 (fact 6).
pub fn parse_v2_condition_id(id: &str) -> Result<FixedBytes<31>, RelayError>;
```

Refusals are `RelayError::validation` (`InvalidRequest`), with py-sdk's messages.

**Calldata** (`relay/deposit_wallet.rs`, beside `redeem_positions_calldata`), pure and infallible:

- `pub fn router_split_calldata(condition_id: FixedBytes<31>, amount: U256) -> Vec<u8>`
- `pub fn router_merge_calldata(condition_id: FixedBytes<31>, amount: U256) -> Vec<u8>`
- `pub fn router_redeem_calldata(position: &V2PositionId, amount: U256) -> Vec<u8>`

declared through `alloy::sol!` as `split(bytes31,uint256)`, `merge(bytes31,uint256)` and
`redeem(bytes31,uint256,uint256)`.

**Client** (`relay/client.rs`). A private `PROTOCOL_V2_ROUTER` constant
(`0x12121212006e4CD160D18e3f00711DA5c3372600`), and:

- `pub fn v2_split_call(&self, condition_id: FixedBytes<31>, amount: U256) -> Result<DepositWalletCall, RelayError>`
- `pub fn v2_merge_call(&self, condition_id: FixedBytes<31>, amount: U256) -> Result<DepositWalletCall, RelayError>`
- `pub fn v2_redeem_call(&self, position: &V2PositionId, amount: U256) -> Result<DepositWalletCall, RelayError>`

each targeting the Router with `value: 0`, and refusing a chain other than 137 as
`deposit_wallet_trading_approvals` does;

- `pub async fn submit_v2_split(&self, condition_id, amount) -> Result<SubmitResponse, RelayError>`
- `pub async fn submit_v2_merge(&self, condition_id, amount) -> Result<SubmitResponse, RelayError>`
- `pub async fn submit_v2_redeem(&self, position: &V2PositionId, amount) -> Result<SubmitResponse, RelayError>`

each sending its one call through `execute(vec![tx], None)` with `operation: CALL_OPERATION`, so
it works for a Safe, a Proxy and a Deposit Wallet. A Deposit Wallet with an external signer uses
the existing `deposit_wallet_batch_typed_data` and `submit_deposit_wallet_batch_from` with the call.
The docs of `redeem_typed_data`, `submit_deposit_wallet_redemption` and the Safe/Proxy redemption
functions replace "Protocol-V2 markets (router `redeem`) are not supported" with a pointer to the
V2 helpers.

### 6. Resolution

No code change. `docs/specs/data-v2/openapi.json` says `/v2/resolutions` returns condition-grain
rows "preferring native V2 lifecycle, then UMA lifecycle, then terminal CTF state", and
`Resolution` models `payouts` and `market_type`. Record in `docs/specs/data-v2/OBSERVED.md` that
V2 conditions are covered by the mirror's text and not yet seen live; that payouts are read from
rows whose `status` is `resolved`; and, in `polymarket-gamma.md`, that a V2 market's
`resolution_status` replaces `uma_resolution_status`.

## Limits

- **Outside services.** No new host or route. Every call goes through an existing client: the
  CLOB (`/tick-size`, `/balance-allowance`, `/order`), gamma (`/markets`, with `position_ids=`
  in place of `clob_token_ids=`, and `/events/{id}`) and the relayer (`execute`'s existing
  routes). Each keeps its client's window-quota table, timeout, retry policy and error
  classification unchanged, and a failure surfaces as that client's error, as today. Story 2
  removes one request (`/neg-risk`) for V2 ids. `Trading::place` makes the same requests as
  today.
- **The live probe** makes at most 7 requests per run: one gamma list of 100 markets, then
  `/book`, `/tick-size` and `/neg-risk` for each of a binary market's 2 ids. It runs on the
  clients' default quotas and timeouts. A non-2xx is a real failure; a network failure is
  `transient` through test-support's tag.
- **Supplied numbers.** Router amounts are any uint256 (`U256`), zero included, as in py-sdk; the
  chain enforces balances. An outcome index is 0 or 1, enforced by `V2PositionId::parse`,
  its one constructor. A position id is a uint256 (fact 1's parse), and a condition id is 62 hex digits,
  or 64 ending `00` or `01` (fact 6). `outcome_ids` takes no number. Nothing else is supplied.

## Testing

### Golden vectors

A new script, `scripts/capture_polymarket_protocol_v2.py`, in the style of
`scripts/capture_polymarket_session_keys.py`: a uv inline script pinned to
`polymarket-client==0.12.0` with `exclude-newer = "2026-10-02T00:00:00Z"`, importing py-sdk's
private modules. The existing 0.11.0 script and its fixtures do not change. Every value comes from
the reference's own choice-making entry point, so the reference, not the script, picks the
version, exchange, target and asset type:

- **Ids.** A V1 id (the token id of py-sdk's golden order fixture, which the existing script uses);
  a binary-module (1) and a neg-risk-module (2) V2 id built from a fixed layout (module byte, 18
  bytes of `keccak("polyoxide-v2-vector")`, 8 zero bytes, 4 fixed bytes, outcome byte), kept only
  if py-sdk's `is_v2_position_id` and `decode_v2_outcome_position_id` accept them; and a combo (3)
  id from `derive_combo_position_context`.
- **`order_vectors.json`** (`tests/fixtures/clob/protocol_v2/`). For each of: V2 EOA (type 0),
  V2 Deposit Wallet owner (type 3), V2 Deposit Wallet session key (type 3), and a V1 EOA control,
  an `OrderDraft` whose `exchange_address` is `resolve_order_exchange_address(asset_id=…)`'s
  choice, passed through `create_unsigned_order` (which sets `protocol_version`) with salt and
  timestamp fixed afterwards. The file records the chosen exchange and version, the app domain
  separator, the struct hash, the signing digest, the Anvil key #0 signature, and
  `build_order_signature`'s output, with the session-key envelope built by the same py-sdk calls
  the existing script uses.
- **`id_vectors.json`** (same directory). One table over these inputs: the ids above, a V1 id,
  `1`, `100`, `123456`, hex, binary and octal spellings of a V2 id, surrounding whitespace, a
  negative value, 2^256, the empty string, non-numeric text, and V2-shaped ids with module byte 0,
  module byte 4 and outcome byte 2. Each row records both of py-sdk's answers:
  `is_v2_position_id`, and `decode_v2_outcome_position_id`'s condition id and outcome index, or
  `null` when it refuses. Rust's `is_v2_position_id` is tested as the conjunction of the two
  recorded answers (§1), and its `decode_v2_outcome_position_id` against the second. A second
  table records `resolve_order_balance_allowance_target` for BUY and SELL against V1 and V2.
- **`router_vectors.json`** (`tests/fixtures/relay/protocol_v2/`). Calldata from
  `router_split_call`, `router_merge_call` and `router_redeem_call` with `router=` py-sdk's
  `protocol_v2_router`; `_protocol_v2_condition_id_bytes` over accepted (62 hex, 64 ending `00`,
  64 ending `01`, upper-case `0X`) and refused (64 ending `02`, 60 hex, no prefix, non-hex) inputs;
  `router_redeem_call` refusing outcome index 2; and one Deposit Wallet batch digest
  (`build_deposit_wallet_typed_data`) carrying the redeem call.
- **`selector_vectors.json`** (`tests/fixtures/gamma/protocol_v2/`). Market JSON cases run
  through py-sdk's `Market.model_validate` and `normalize_market_position_context`: `v1` that also
  carries `positionIds`, `v2` with both fields, a missing `version`, `version: "v3"`, `v2` with one
  position id, and `v1` with no `clobTokenIds`. Each records the chosen pair or the refusal.
- Each directory gets a `PROVENANCE.md` naming the script, the py-sdk version and the date.

Rust tests pin to these files, never to a self-computed value:

- `shared::protocol` against `id_vectors.json`.
- `eip712.rs`: each order vector's digest, signature and wrapped signature; and the V1 control
  against today's value, unchanged. The order type string stays asserted directly.
- `account.rs` or `mock_clob.rs`: the asset-type table, through the request each case sends.
- `deposit_wallet.rs` and `client.rs`: the Router calldata, the condition-id table, the
  outcome-index refusal, and the batch digest.
- `gamma`: `Market::outcome_ids` over `selector_vectors.json` (the binary cases agree with
  py-sdk's pair or refusal), plus Rust-only cases py-sdk cannot express: three outcomes, a count
  mismatch, a non-decimal id, malformed `outcomes`.

### Mock tests

- `balance_allowance` with a V2 id sends `asset_type=CONDITIONAL-V2`; with a V1 id, `CONDITIONAL`.
- `create_order` with a V2 id calls `/tick-size` and never `/neg-risk` (`expect(0)` with
  `match_query(Matcher::Any)`), and the order it signs verifies under ExchangeV3 v3.
- `Trading::place` with a V2 key reports the V3 `order_hash` as the order's id.
- `create_order` with a V2 id on a client built for Amoy is refused as `InvalidRequest` when it
  signs, and posts nothing.
- `event_group` with a V2 market (§4), and a V2 key's `event_id_of` querying `position_ids=`.
- `submit_v2_redeem` posts the Router call through `execute` for a Safe, a Proxy and a Deposit
  Wallet, and a chain other than 137 is refused before any request.

### Mutation battery

Rows added to `docs/MUTANTS.md`, each KILLED by a named test, plus one control mutation that must
survive:

- `"3"` → `"2"` for V2 orders.
- A V2 order sent to the CTF (or neg-risk) exchange.
- The type-3 envelope's app domain built at version `"2"` for a V2 order.
- The reserved-bit mask shifted by one bit, or inverted.
- `is_v2_position_id` dropping its decode condition (then `1` is V2, and
  `v2_order_signature_is_deterministic` fails).
- `CONDITIONAL-V2` → `CONDITIONAL`.
- `outcome_ids` choosing by which field is present.
- `parse_v2_condition_id` accepting any last byte.
- Redeem sending CTF index sets (1/2) where outcome index 0/1 goes.
- `create_order` calling `/neg-risk` for a V2 id.

### Live

`live_v2_market_is_served` in `polyoxide-polymarket/tests/live_clob.rs` (features `clob`,
`gamma`; read-only; no credentials):

1. List the newest 100 open gamma markets (`order=id`, `ascending=false`, `limit=100`), as
   `live_markets_carry_a_known_protocol_version` does, and take the first with
   `version == Some(V2)`.
2. If there is none, fail through test-support's `environmental` tag with "no Protocol V2 market
   among the newest 100 open markets". This is the signal the issue asks for: the nightly reports it
   as environmental until a V2 market exists.
3. Otherwise read its ids with `outcome_ids()` (a refusal is a real failure), then for each id read
   `/book`, `/tick-size` and `/neg-risk`, and fail as real on any non-2xx, which answers the issue's
   open question about the per-token helpers.

No bare `unwrap`, `expect` or `panic!` (`live_unwraps.py`). The test posts no order. Upstream's
checklist (BUY and SELL fills and balances on a V2 and a CTF market) goes to
`_bmad-output/implementation-artifacts/deferred-work.md`, blocked on the first V2 market and on
the owner's go-ahead for a credentialed order run.

## Docs

- `.claude/rules/polymarket-clob.md`: the order-signing row and the "signs exchange domain `"2"`
  only" paragraph describe the per-id choice; `ClobOrderOptions::neg_risk` is ignored for V2.
- `.claude/rules/session-keys.md`: a V2 Deposit Wallet order's envelope is hashed under
  ExchangeV3 v3, pinned by `tests/fixtures/clob/protocol_v2/order_vectors.json`.
- `.claude/rules/polymarket-gamma.md`: the Protocol V2 section names `Market::outcome_ids`, and
  that trading is supported (live verification pending); the resolution note from §6.
- `.claude/rules/polymarket-relay.md`: the Router helpers and their input rules. Its `paths:`
  gain `tests/fixtures/relay/protocol_v2/**`, and `polymarket-clob.md`'s and
  `polymarket-gamma.md`'s gain their fixture directories and the capture script, so
  `test_claude_rules.py` stays green.
- `docs/specs/clob/auth.md`: the order-signing domain's version and verifying contract per
  protocol.
- `docs/specs/session-keys/OBSERVED.md`: signature type 3 on ExchangeV3.
- `docs/specs/gamma/OBSERVED.md`: "What polyoxide does" says trading is implemented.
- `docs/specs/data-v2/OBSERVED.md`: §6.
- `docs/MUTANTS.md`: the battery's rows.

## Release and gates

- Additive: no public item is removed, so `docs/s3-removals.md` gains nothing. Every function
  an existing test calls keeps its signature (§2), and the one replaced test postdates the
  baseline (§4), so `ci/test-body-diff.toml` gains nothing. A test body that would change
  anywhere else is a stop: it needs a `[[drift]]` entry and a DRIFT row, which the owner
  decides.
- No version bump on this branch. The release notes record the behaviour users can see: V2
  orders, balances and event groups, and the selector.
- Run the CI jobs this touches before handing over: format; lint and test; package; msrv;
  features (each of `clob`, `gamma` and `relay` alone builds, since `shared::protocol` is under
  `any(clob, relay)` and gamma's selector uses none of it); removals; test bodies; CI scripts
  (rules files, dep allowlists: no new dependency); wasm (`clob` and `relay` code is in the wasm
  set where it already is).

## Stories

Each leaves the tree building and green.

1. **Vectors.** `scripts/capture_polymarket_protocol_v2.py` and the four fixture files with their
   `PROVENANCE.md`. No Rust, so no story carries code that nothing calls yet.
2. **CLOB.** `shared::protocol` with its `id_vectors.json` tests, `Chain::exchange_v3`,
   `order_exchange` and the versioned domain, `create_order`'s
   `/neg-risk` skip, `balance_allowance`'s asset type, their vector and mock tests, the doc
   comments, and the CLOB, session-key and `auth.md` docs.
3. **Gamma and event groups.** `OutcomeId`, `Market::outcome_ids`, `OutcomeIdsError` and its
   `Classify`; `event_group` and `event_id_of`; the replaced and new `event_group` tests; the
   gamma docs.
4. **Relay.** `V2PositionId`, `parse_v2_condition_id`, the calldata builders, the client calls and
   submits, their tests, and the relay docs and redemption doc comments.
5. **Live, resolution and mutations.** `live_v2_market_is_served`, the `deferred-work.md` entry,
   the Data v2 OBSERVED note, and the mutation battery with its `docs/MUTANTS.md` rows.
