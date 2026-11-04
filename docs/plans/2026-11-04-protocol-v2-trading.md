# Protocol V2 Trading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Under the supervised option, `/flow-supervise` runs it story by story.

**Goal:** `polyoxide-polymarket` trades Polymarket Protocol V2 markets: it signs V2 orders for ExchangeV3 (domain version `"3"`, Deposit Wallet envelopes included), reads V2 balances as `CONDITIONAL-V2`, chooses a gamma market's outcome ids by its `version`, lists V2 outcomes in event groups, and builds and submits the Router's split, merge and redeem, while every V1 (CTF) market behaves exactly as before (issue #51).

**Architecture:** One crate-private predicate in `shared::protocol` decides, from the asset id alone, whether an order is V2 (py-sdk 0.12.0's reserved-bit test, plus its decode, so toy ids stay V1). The CLOB's signing, order creation and balance reads key on it; gamma gains a public `Market::outcome_ids` selector that `clob::venue::event_group` adopts; the relay gains `V2PositionId`, a condition-id narrowing, pure Router calldata builders, and `RelayClient` calls and submits on the existing `execute`. Every value is pinned to golden vectors a new capture script takes from py-sdk 0.12.0.

**Tech Stack:** Rust (MSRV 1.91, dev shell 1.99.0), alloy (`sol!`, `U256`, `FixedBytes`, EIP-712), serde, thiserror, mockito, `polyoxide-test-support`'s failure tags, and a PEP 723 `uv run` capture script on `polymarket-client==0.12.0`.

**Spec:** `docs/superpowers/specs/2026-11-04-protocol-v2-trading-design.md`. Its "The reference" facts 1 to 9 are what py-sdk 0.12.0 does; its Design §1 to §6, Limits, Testing, Docs, and Release and gates sections are the decisions. Read with `.claude/rules/polymarket-clob.md`, `session-keys.md`, `polymarket-gamma.md`, `polymarket-relay.md`, `testing.md` and `live-tests.md`.

---

## Rules for every story

- **Run commands with the dev shell's prefix**, `direnv exec .` (never `nix develop --command`). A story's gates are the profile's; its **Verify** line is the developer's done-check.
- **No test that exists at the `v0.40.0` baseline changes its body** (AD-12). New tests go after a file's last test, or in a new file. Every function an existing test calls keeps its signature and meaning: `exchange_address(chain_id, neg_risk)`, `exchange_domain(chain_id, exchange)` and `deposit_wallet::envelope_digest(order, chain_id, exchange, wallet)` in `src/clob/core/eip712.rs` above all. A baseline test that would fail or need a body change is a stop: report it, never edit it. `tests/venue_clob.rs` postdates the baseline (spec §4), so Story 3 may replace its one test there; it adds no `ci/test-body-diff.toml` entry.
- **Golden vectors only.** Every digest, signature, calldata, asset type, id classification and selector result a test asserts comes from Story 1's fixtures, never from a value the Rust code computed. A case py-sdk cannot express (a three-outcome market, a malformed `outcomes`) asserts a refusal or a pairing read straight off the input JSON.
- **The id rule** is spec §1's: an id is V2 when py-sdk's `is_v2_position_id` *and* `decode_v2_outcome_position_id` both accept it. Never the mask alone, never "which gamma field is present".
- **Nothing changes for V1.** An order whose token id is not V2 signs at domain `"2"` on the exchange `neg_risk` picks, sends `CONDITIONAL`, and calls `/neg-risk`, exactly as today.
- **Public errors** implement `polyoxide_venue::Classify`, are `#[non_exhaustive]`, have no inherent `is_retriable` or `retry_after`, and are listed in `src/lib.rs`'s `const _` assertion.
- **Live tests** are `#[ignore]`d and fail only through `polyoxide-test-support`'s tags (`or_fail`, `fail`, `environmental`, `transient`); no `.unwrap()`, `.expect(` or `panic!` (`python3 scripts/live_unwraps.py`). No test posts an order (`polyoxide-live-order-safety`).
- **Doc comments on `pub` items** name no `pub(crate)` item with ``[`link`]`` syntax (the doc build fails on it); placeholders go in backticks.
- **A rule's text changes in the commit that changes the rule** (AD-21): each story edits its rules file in its own commit.
- **Nothing publishes.** No `cargo publish` without `--dry-run`, no `scripts/finish_release.sh`, no `gh workflow run Release`, no version bump.
- **Commit by pathspec** (`git commit -- <paths>`), conventional, with `Cargo.lock` only if cargo changes it, never `--no-verify`, never `git add -A`/`.`/`-a`; every message ends with `(protocol v2, Story <n>)` and the attribution lines the session gives.

## File structure

Paths are from the repo root; `pp/` is `polyoxide-polymarket/`.

**Story 1: golden vectors (no Rust).**

- `scripts/capture_polymarket_protocol_v2.py` (create): the capture.
- `pp/tests/fixtures/clob/protocol_v2/order_vectors.json`, `id_vectors.json`, `PROVENANCE.md` (create).
- `pp/tests/fixtures/relay/protocol_v2/router_vectors.json`, `PROVENANCE.md` (create).
- `pp/tests/fixtures/gamma/protocol_v2/selector_vectors.json`, `PROVENANCE.md` (create).

**Story 2: CLOB.**

- `pp/src/shared/protocol.rs` (create), `pp/src/shared/mod.rs` (modify): the predicate and its parse and decode, under `clob`.
- `pp/src/clob/core/chain.rs` (modify): `Chain::exchange_v3`.
- `pp/src/clob/core/eip712.rs` (modify): `order_exchange`, `order_domain`, the signing paths, new vector tests after the last test.
- `pp/src/clob/client.rs` (modify): `get_market_metadata` skips `/neg-risk` for a V2 id.
- `pp/src/clob/api/account.rs` (modify): `balance_allowance`'s asset type; `update_balance_allowance`'s docs.
- `pp/src/clob/types.rs`, `pp/src/clob/venue/trading.rs` (modify): the `neg_risk` doc comments.
- `pp/tests/mock_clob.rs`, `pp/tests/trading_clob.rs` (modify): appended tests.
- `.claude/rules/polymarket-clob.md`, `.claude/rules/session-keys.md`, `docs/specs/clob/auth.md`, `docs/specs/session-keys/OBSERVED.md` (modify).

**Story 3: gamma selector and event groups.**

- `pp/src/gamma/types.rs` (modify): `OutcomeId`, `Market::outcome_ids`.
- `pp/src/gamma/error.rs`, `pp/src/gamma/mod.rs`, `pp/src/lib.rs` (modify): `OutcomeIdsError`, its re-export and its `Classify` assertion.
- `pp/src/clob/venue.rs` (modify): `event_group`, `event_id_of`.
- `pp/tests/outcome_ids_gamma.rs` (create), `pp/Cargo.toml` (modify: its `[[test]]` stanza), `pp/tests/error_classes_gamma.rs`, `pp/tests/venue_clob.rs` (modify).
- `.claude/rules/polymarket-gamma.md`, `.claude/rules/polymarket-clob.md`, `docs/specs/gamma/OBSERVED.md` (modify).

**Story 4: relay Router ops.**

- `pp/src/shared/mod.rs`, `pp/src/shared/protocol.rs` (modify): the module widens to `any(clob, relay)`.
- `pp/src/relay/types.rs`, `pp/src/relay/mod.rs` (modify): `V2PositionId`, `parse_v2_condition_id`, their re-exports.
- `pp/src/relay/deposit_wallet.rs` (modify): the three calldata builders and their tests.
- `pp/src/relay/client.rs` (modify): `PROTOCOL_V2_ROUTER`, the three calls and three submits, the redemption doc comments.
- `pp/tests/mock_relay.rs` (modify): appended tests.
- `.claude/rules/polymarket-relay.md`, `pp/src/relay/README.md` (modify).

**Story 5: live probe, resolution note, mutation battery.**

- `pp/tests/live_clob.rs` (modify): `live_v2_market_is_served`.
- `docs/specs/data-v2/OBSERVED.md`, `_bmad-output/implementation-artifacts/deferred-work.md` (modify).
- `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py` (modify): the battery's rows and their cited lines.

## Names contract

Every name one story defines for another, with its type, signature and visibility, and every text a later story's test matches. A drafter who needs a cross-story name this section lacks stops and asks; it never invents one. Paths under `src/` and `tests/` are `polyoxide-polymarket/`'s.

**Story 1 → Stories 2, 3 and 4: the capture.**

- `uv run scripts/capture_polymarket_protocol_v2.py <clob_dir> <relay_dir> <gamma_dir>` creates the three directories if missing and writes the files below into them, and nothing else. Its PEP 723 header pins `polymarket-client==0.12.0` and `exclude-newer = "2026-10-02T00:00:00Z"`. Its output is byte-identical across runs on one day (fixed salts, timestamps and keys; `PROVENANCE.md` dates to the day). It shares `scripts/capture_common.py`'s `write_json` and `write_provenance`.
- Keys: Anvil key #0 (`0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80`, address `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`) signs; Anvil address #1 (`0x70997970C51812dc3A010C7d01b50e0d17dc79C8`, key `0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d`) is the session signer; the Deposit Wallet is the existing fixture's, `0x57ffbc34de23124faeb8387fcd689d314e57accd`.
- The ids. `v1` is the first `clobTokenIds` entry of `tests/fixtures/clob/venue/gamma_markets.json`'s first market. `v2_binary` and `v2_neg_risk` are built as the 32 bytes module byte (`0x01`, `0x02`) ‖ the first 18 bytes of `keccak("polyoxide-v2-vector")` ‖ 8 zero bytes ‖ `0xdeadbeef` ‖ outcome byte `0x00`, as decimal, and kept only if py-sdk's `is_v2_position_id` and `decode_v2_outcome_position_id` accept them (the script exits non-zero otherwise). `v2_combo` is the first of `derive_combo_outcome_position_ids(derive_combo_position_context(canonicalize_combo_legs([v2_binary, <v2_neg_risk's sibling with outcome byte 0x01>])).condition_id)`.

**`tests/fixtures/clob/protocol_v2/id_vectors.json`:**

```json
{
  "ids": { "v1": "<dec>", "v2_binary": "<dec>", "v2_neg_risk": "<dec>", "v2_combo": "<dec>" },
  "position_ids": [
    { "input": "<string>", "is_v2_position_id": true, "decoded": { "condition_id": "0x<62 hex, lower>", "outcome_index": 0 } },
    { "input": "<string>", "is_v2_position_id": true, "decoded": null }
  ],
  "balance_allowance_targets": [
    { "side": "BUY", "asset_id": "<dec>", "asset_type": "COLLATERAL", "asset_id_sent": null }
  ]
}
```

- `position_ids` holds, in this order: the four `ids`; `"1"`; `"100"`; `"123456"`; `v2_binary` as `0x` hex, as `0b` binary and as `0o` octal; `v2_binary` with a leading and a trailing space; `"-1"`; `2^256` as decimal; `""`; `"abc"`; and `v2_binary` with its module byte set to `0x00`, to `0x04`, and with its outcome byte set to `0x02`. `is_v2_position_id` and `decoded` are py-sdk's own answers (`decoded` is `null` when `decode_v2_outcome_position_id` raises).
- `balance_allowance_targets` holds `resolve_order_balance_allowance_target` for BUY and SELL against `ids.v1` and `ids.v2_binary` (4 rows); `asset_type` is its first return value, `asset_id_sent` its second.

**`tests/fixtures/clob/protocol_v2/order_vectors.json`:**

```json
{
  "chain_id": 137,
  "signer_address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
  "session_signer": "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
  "deposit_wallet": "0x57ffbc34de23124faeb8387fcd689d314e57accd",
  "cases": {
    "<case>": {
      "neg_risk": false,
      "order": {
        "salt": "<dec>", "maker": "0x…", "signer": "0x…", "token_id": "<dec>",
        "maker_amount": "<dec>", "taker_amount": "<dec>", "side": "BUY",
        "signature_type": 0, "timestamp": "<dec>", "expiration": "0",
        "metadata": "0x<64 hex>", "builder": "0x<64 hex>"
      },
      "exchange": "0x<checksummed>",
      "version": "3",
      "domain_separator": "0x<64 hex>",
      "struct_hash": "0x<64 hex>",
      "digest": "0x<64 hex>",
      "signature": "0x…"
    }
  }
}
```

- `<case>` is each of `v2_eoa` (`ids.v2_binary`, type 0, `neg_risk` false), `v2_neg_risk_eoa` (`ids.v2_neg_risk`, type 0, `neg_risk` **true**), `v2_deposit_wallet_owner` and `v2_deposit_wallet_session` (`ids.v2_binary`, type 3, maker and signer the Deposit Wallet), `v1_eoa` (`ids.v1`, type 0, `neg_risk` false) and `v1_neg_risk_eoa` (`ids.v1`, type 0, `neg_risk` true). Every case uses salt `"1"`, maker amount `"1000000"`, taker amount `"500000"`, side `BUY`, timestamp `"1700000000000"`, zero metadata and builder; type-0 maker and signer are `signer_address`.
- `neg_risk` is what the script passed to `resolve_order_exchange_address`; `exchange` is its return value; `version` is the `protocol_version` that `create_unsigned_order` set. `domain_separator` is the exchange domain's separator (for type 3, the app domain separator); `struct_hash` the `Order` struct hash (for type 3, the contents hash); `digest` what the key signs (the order digest for type 0; the `TypedDataSign` envelope digest for type 3); `signature` what is posted (the 65-byte signature for type 0; `build_order_signature`'s ERC-7739-wrapped signature for the owner; that wrapped in the session-signer envelope, by the calls `scripts/capture_polymarket_session_keys.py` uses, for the session key).

**`tests/fixtures/relay/protocol_v2/router_vectors.json`:**

```json
{
  "chain_id": 137,
  "router": "0x12121212006e4CD160D18e3f00711DA5c3372600",
  "split":  [ { "condition_id": "0x<62 hex>", "amount": "<dec>", "calldata": "0x…" } ],
  "merge":  [ { "condition_id": "0x<62 hex>", "amount": "<dec>", "calldata": "0x…" } ],
  "redeem": [ { "position_id": "<dec>", "condition_id": "0x<62 hex>", "outcome_index": 0, "amount": "<dec>", "calldata": "0x…" } ],
  "condition_ids": [ { "input": "<string>", "bytes31": "0x<62 hex>" } ],
  "deposit_wallet_batch": {
    "wallet": "0x57ffbc34de23124faeb8387fcd689d314e57accd", "nonce": "0", "deadline": "1700000600",
    "calls": [ { "target": "0x…", "value": "0", "data": "0x…" } ],
    "digest": "0x<64 hex>"
  }
}
```

- `split` and `merge` each hold amounts `0`, `1000000` and `2^256 - 1` for `ids.v2_binary`'s condition; `redeem` holds outcome 0 (`ids.v2_binary`) and outcome 1 (its sibling) at `1000000`. `calldata` is py-sdk's `router_split_call`, `router_merge_call` or `router_redeem_call` `.data`, with `router=` py-sdk's `protocol_v2_router`.
- `condition_ids` holds the condition as `0x` + 62 hex, as `0x` + 64 hex ending `00`, as 64 ending `01`, upper-case after `0X`, and refused inputs (64 ending `02`, 60 hex, no `0x`, non-hex); `bytes31` is `_protocol_v2_condition_id_bytes`'s result, or `null` when it raises.
- `deposit_wallet_batch.calls` is the one redeem call at outcome 0; `digest` is `build_deposit_wallet_typed_data`'s EIP-712 digest for it.

**`tests/fixtures/gamma/protocol_v2/selector_vectors.json`:**

```json
{ "cases": [ { "name": "<case>", "market": { }, "ids": ["<dec>", "<dec>"] } ] }
```

- Each `market` is `tests/fixtures/clob/venue/gamma_markets.json`'s first market with only `version`, `clobTokenIds`, `positionIds` and `outcomes` edited; `ids` is the `(yes, no)` pair py-sdk's `normalize_market_position_context` returns, or `null` when it raises. The cases, in order: `v1_with_position_ids` (`v1`, both fields), `v2_with_both` (`v2`, both fields, `positionIds` = `[v2_binary, its sibling]`), `missing_version`, `unknown_version` (`"v3"`), `v2_one_position_id`, `v1_no_clob_token_ids`.

**Story 2 → Stories 3, 4 and 5.** In `polyoxide-polymarket`:

- `src/shared/protocol.rs`, declared in `src/shared/mod.rs` as `#[cfg(feature = "clob")] pub(crate) mod protocol;` (Story 4 widens it):
  - `pub(crate) fn parse_position_id(id: &str) -> Option<U256>`
  - `pub(crate) fn decode_v2_outcome_position_id(id: &str) -> Option<(FixedBytes<31>, u8)>`
  - `pub(crate) fn is_v2_position_id(id: &str) -> bool`
- `Chain::exchange_v3(&self) -> Option<Address>`, `pub const fn`, in `src/clob/core/chain.rs`.
- `pub(crate) fn order_exchange(chain_id: u64, order: &Order) -> Result<(Address, &'static str), ClobError>` and `fn order_domain(chain_id: u64, exchange: Address, version: &'static str) -> protocol::EIP712Domain` in `src/clob/core/eip712.rs`. The Amoy refusal's message is `Protocol V2 orders need ExchangeV3, which is not deployed on chain 80002` (`ClobError::validation`).
- `balance_allowance(token_id)` sends `asset_type=CONDITIONAL-V2` for a V2 id.

**Story 3 → Story 5.** In `polyoxide-polymarket::gamma`:

- `pub struct OutcomeId { pub outcome: String, pub id: String }` (`Debug, Clone, PartialEq, Eq`) in `gamma::types`.
- `pub fn outcome_ids(&self) -> Result<Vec<OutcomeId>, OutcomeIdsError>` on `gamma::types::Market`.
- `pub enum OutcomeIdsError` in `gamma::error`, re-exported as `gamma::OutcomeIdsError`, `#[non_exhaustive]`, with exactly: `MissingVersion`, `UnsupportedVersion(String)`, `MissingIds`, `MalformedIds`, `MalformedOutcomes`, `CountMismatch { outcomes: usize, ids: usize }`, `InvalidId(String)`. Every variant's `Classify::class()` is `Class::Decode`, `is_fault()` `true`, `retry_after()` `None`.

**Story 4 → Story 5.** In `polyoxide-polymarket::relay`:

- `pub struct V2PositionId` (`Debug, Clone, Copy, PartialEq, Eq, Hash`, fields private) with `pub fn parse(id: &str) -> Result<Self, RelayError>`, `pub fn condition_id(&self) -> FixedBytes<31>`, `pub fn outcome_index(&self) -> u8`; and `pub fn parse_v2_condition_id(id: &str) -> Result<FixedBytes<31>, RelayError>`; both in `relay::types`, re-exported from `relay`.
- Refusal messages (`RelayError::validation`), py-sdk's verbatim: `Position ID must be a uint256 value`, `Position ID must use a supported protocol v2 module`, `Protocol v2 position ID must be a YES/NO position ID`, `Protocol v2 condition ID must be bytes31, or bytes32 with a binary outcome byte`.
- In `relay::deposit_wallet`, re-exported from `relay` beside `redeem_positions_calldata`: `pub fn router_split_calldata(condition_id: FixedBytes<31>, amount: U256) -> Vec<u8>`, `pub fn router_merge_calldata(condition_id: FixedBytes<31>, amount: U256) -> Vec<u8>`, `pub fn router_redeem_calldata(position: &V2PositionId, amount: U256) -> Vec<u8>`.
- On `RelayClient`: `pub fn v2_split_call(&self, condition_id: FixedBytes<31>, amount: U256) -> Result<DepositWalletCall, RelayError>`, `v2_merge_call` (same signature), `pub fn v2_redeem_call(&self, position: &V2PositionId, amount: U256) -> Result<DepositWalletCall, RelayError>`, and `pub async fn submit_v2_split(&self, condition_id: FixedBytes<31>, amount: U256) -> Result<SubmitResponse, RelayError>`, `submit_v2_merge` (same), `pub async fn submit_v2_redeem(&self, position: &V2PositionId, amount: U256) -> Result<SubmitResponse, RelayError>`. A chain other than 137 is refused with `Protocol V2 Router calls are only known for Polygon mainnet (137), not chain <id>`.

**Story 5 reads** every line Stories 2 to 4 cite in `docs/MUTANTS.md` at its own HEAD; no story before it edits `docs/MUTANTS.md` or `.github/scripts/tests/test_mutants_ledger.py`.

## Stories

- **Story 1** — Golden vectors from py-sdk 0.12.0 (spec: The reference; Testing, Golden vectors).

  Writes `scripts/capture_polymarket_protocol_v2.py` on the model of `scripts/capture_polymarket_session_keys.py` (PEP 723, py-sdk's private modules, `capture_common`), runs it, and commits its four fixtures and three `PROVENANCE.md` files exactly as the names contract lays them out. Every recorded choice (version, exchange, asset type, classification, narrowing, selected pair) is the return value of py-sdk's own function given the case's inputs; the script never decides one itself, which is the failure the 2026-09-25 redemption-target vector had. No Rust.

  - **Developer:** intermediate-dev
  - **Files:** `scripts/capture_polymarket_protocol_v2.py`, `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/order_vectors.json`, `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/id_vectors.json`, `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/PROVENANCE.md`, `polyoxide-polymarket/tests/fixtures/relay/protocol_v2/router_vectors.json`, `polyoxide-polymarket/tests/fixtures/relay/protocol_v2/PROVENANCE.md`, `polyoxide-polymarket/tests/fixtures/gamma/protocol_v2/selector_vectors.json`, `polyoxide-polymarket/tests/fixtures/gamma/protocol_v2/PROVENANCE.md`
  - **Interfaces:** exposes the capture command and the four fixture schemas under "Story 1 → Stories 2, 3 and 4"; uses nothing from earlier stories
  - **Verify:** `bash -c 'd=$(mktemp -d) && uv run scripts/capture_polymarket_protocol_v2.py $d/clob $d/relay $d/gamma && diff -r $d/clob polyoxide-polymarket/tests/fixtures/clob/protocol_v2 && diff -r $d/relay polyoxide-polymarket/tests/fixtures/relay/protocol_v2 && diff -r $d/gamma polyoxide-polymarket/tests/fixtures/gamma/protocol_v2'`
  - **Review:** deep (the fixtures are every later story's yardstick; a value the script chose would pin a wrong behaviour)
  - **Reading:** spec "The reference" (facts 1 to 9), Testing "Golden vectors"; `scripts/capture_polymarket_session_keys.py`, `scripts/capture_common.py`, `polyoxide-polymarket/tests/fixtures/clob/session_keys/PROVENANCE.md`, `polyoxide-polymarket/tests/fixtures/clob/venue/gamma_markets.json` (its first market); py-sdk 0.12.0's `_internal/protocol.py`, `_internal/actions/orders/{orders,context,typed_data,allowance}.py`, `_internal/actions/relayer/{calls,positions}.py` and `_internal/actions/relayer/signing/deposit_wallet.py`

- **Story 2** — CLOB: the id rule, ExchangeV3 signing, the `/neg-risk` skip and V2 balances (spec Design §1, §2; Limits).

  Creates `shared::protocol` (under `clob` for now) with tests against `id_vectors.json` (`is_v2_position_id` as the conjunction of the two recorded answers; `decode_v2_outcome_position_id` against `decoded`). Adds `Chain::exchange_v3`, `order_exchange` and `order_domain`, and routes `order_hash`, `sign_order` and `sign_order_as` through them, keeping `exchange_address`, `exchange_domain` and `envelope_digest`'s signatures (the last takes its version from the order's token id). New unit tests after the file's last test pin each `order_vectors.json` case (exchange, version, domain separator, struct hash, digest, signature; for type 3 through `sign_order_as` with `SigningTarget::DepositWallet`, owner and session key). `get_market_metadata` skips `/neg-risk` for a V2 id; `balance_allowance` sends `CONDITIONAL-V2`. Appended mock tests: the asset-type table of `id_vectors.json` through the request each case sends; `create_order` on a V2 id calling `/tick-size` and never `/neg-risk` (`expect(0)` with `match_query(Matcher::Any)`); a V2 id on an Amoy client refused as `InvalidRequest` with the contract's message and nothing posted; `Trading::place` on a V2 key reporting the V3 `order_hash`. Docs: the CLOB and session-key rules (the per-id choice, the id rule's divergence from py-sdk's bare mask on impossible ids, `neg_risk` ignored for V2, the capture script under `polymarket-clob.md`'s `paths:`), `auth.md`'s domain per protocol, `session-keys/OBSERVED.md`'s type 3 on ExchangeV3. Every baseline test in `eip712.rs`, `mock_clob.rs` and `trading_clob.rs` passes unedited.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/src/shared/protocol.rs`, `polyoxide-polymarket/src/shared/mod.rs`, `polyoxide-polymarket/src/clob/core/chain.rs`, `polyoxide-polymarket/src/clob/core/eip712.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/api/account.rs`, `polyoxide-polymarket/src/clob/types.rs`, `polyoxide-polymarket/src/clob/venue/trading.rs`, `polyoxide-polymarket/tests/mock_clob.rs`, `polyoxide-polymarket/tests/trading_clob.rs`, `.claude/rules/polymarket-clob.md`, `.claude/rules/session-keys.md`, `docs/specs/clob/auth.md`, `docs/specs/session-keys/OBSERVED.md`
  - **Interfaces:** exposes "Story 2 → Stories 3, 4 and 5"; uses Story 1's `id_vectors.json` and `order_vectors.json`
  - **Verify:** `bash -c 'cargo test -p polyoxide-polymarket --all-features && cargo test -p polyoxide-polymarket --no-default-features --features clob --lib && cargo test -p polyoxide-polymarket --no-default-features --features clob --test mock_clob && python3 scripts/test_body_diff.py --baseline v0.40.0 && cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'`
  - **Review:** deep (order signing: a wrong domain posts orders the exchange rejects, or signs for the wrong contract)
  - **Reading:** spec Design §1, §2, Limits, Testing "Mock tests"; `polyoxide-polymarket/src/shared/mod.rs:1-60`, `polyoxide-polymarket/src/clob/core/chain.rs`, `polyoxide-polymarket/src/clob/core/eip712.rs:1-470` and its tests from `:1100` to the end, `polyoxide-polymarket/src/clob/client.rs:320-510`, `polyoxide-polymarket/src/clob/api/account.rs:1-90`, `polyoxide-polymarket/src/clob/types.rs:270-300`, `polyoxide-polymarket/src/clob/venue/trading.rs:40-80`, `polyoxide-polymarket/tests/mock_clob.rs:1-65`, `polyoxide-polymarket/tests/trading_clob.rs:1-140`; `.claude/rules/polymarket-clob.md`, `.claude/rules/session-keys.md`

- **Story 3** — Gamma's `Market::outcome_ids` and V2 outcomes in event groups (spec Design §3, §4, §6's gamma note).

  Adds `OutcomeId`, `Market::outcome_ids` with spec §3's six-step rule, and `OutcomeIdsError` with its `Classify`, re-export and `lib.rs` assertion. `tests/outcome_ids_gamma.rs` (a new `[[test]]` with `required-features = ["gamma"]`, after the gamma stanzas) holds the selector to every `selector_vectors.json` case (the pair, or a refusal where py-sdk refused) and adds the Rust-only cases: three outcomes, a count mismatch, a non-decimal id, malformed `outcomes`, malformed `clobTokenIds`. `error_classes_gamma.rs` gains one test asserting every variant's class. `event_group` reads outcomes through the selector, skipping a market whose version is missing or `Other`; `event_id_of` sends `position_ids=` for a V2 key. In `venue_clob.rs`, `event_group_leaves_out_the_outcomes_of_a_v2_market` is replaced by `event_group_keys_a_v2_market_by_its_position_ids`, and `event_group_skips_a_market_with_no_known_version` and `event_group_finds_a_v2_key_by_its_position_ids` (the `/markets` route matched on `position_ids=<id>&closed=any`) are added; the V2 ids come from `id_vectors.json`. Docs: `polymarket-gamma.md` (the selector, trading supported pending live verification, `resolution_status` replacing `uma_resolution_status` for V2), `polymarket-clob.md`'s `event_group` bullet, `gamma/OBSERVED.md`'s "What polyoxide does".

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/src/gamma/types.rs`, `polyoxide-polymarket/src/gamma/error.rs`, `polyoxide-polymarket/src/gamma/mod.rs`, `polyoxide-polymarket/src/lib.rs`, `polyoxide-polymarket/src/clob/venue.rs`, `polyoxide-polymarket/tests/outcome_ids_gamma.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/tests/error_classes_gamma.rs`, `polyoxide-polymarket/tests/venue_clob.rs`, `.claude/rules/polymarket-gamma.md`, `.claude/rules/polymarket-clob.md`, `docs/specs/gamma/OBSERVED.md`
  - **Interfaces:** exposes "Story 3 → Story 5"; uses Story 1's `selector_vectors.json` and `id_vectors.json`, and Story 2's `shared::protocol::is_v2_position_id`
  - **Verify:** `bash -c 'cargo test -p polyoxide-polymarket --all-features && cargo test -p polyoxide-polymarket --no-default-features --features gamma --test outcome_ids_gamma --test error_classes_gamma && cargo test -p polyoxide-polymarket --features clob,gamma,data --test venue_clob && python3 scripts/test_body_diff.py --baseline v0.40.0 && cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_claude_rules.py -q'`
  - **Review:** deep (a public selector whose wrong choice sends orders for ids with no book, and a new public error)
  - **Reading:** spec Design §3, §4, §6; `polyoxide-polymarket/src/gamma/types.rs:1-70` and `:150-245`, `polyoxide-polymarket/src/gamma/error.rs`, `polyoxide-polymarket/src/gamma/mod.rs`, `polyoxide-polymarket/src/lib.rs:85-157`, `polyoxide-polymarket/src/clob/venue.rs:380-510`, `polyoxide-polymarket/tests/venue_clob.rs:1-85` and `:440-545`, `polyoxide-polymarket/tests/error_classes_gamma.rs`, `polyoxide-polymarket/Cargo.toml:236-262`; `.claude/rules/polymarket-gamma.md`, `docs/specs/gamma/OBSERVED.md` (its Protocol V2 section)

- **Story 4** — Relay: `V2PositionId`, condition narrowing, Router calldata, calls and submits (spec Design §5; Limits).

  Widens `shared::protocol` to `#[cfg(any(feature = "clob", feature = "relay"))]` and moves `#[cfg(feature = "clob")]` onto `is_v2_position_id` alone. Adds `V2PositionId` and `parse_v2_condition_id` on `shared::protocol`'s parse and decode with py-sdk's messages, the three `sol!` calldata builders, and on `RelayClient` the private `PROTOCOL_V2_ROUTER`, the three calls (chain 137 only) and the three submits through `execute(vec![tx], None)` with `CALL_OPERATION`. Unit tests in `deposit_wallet.rs` pin every `router_vectors.json` calldata row, the `condition_ids` table through `parse_v2_condition_id`, the `position_ids` rows of `id_vectors.json` through `V2PositionId::parse` (accepted where `decoded` is not `null`, else refused), and the batch digest through `deposit_wallet_batch_typed_data`'s digest for the redeem call. Appended `mock_relay.rs` tests: `submit_v2_redeem` posting the Router call for a Safe, a Proxy and a Deposit Wallet client, and a chain-80002 client refused with the contract's message before any request (`expect(0)` with `match_query(Matcher::Any)`). The redemption doc comments point at the V2 helpers; `polymarket-relay.md` and the relay README describe them.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/src/shared/mod.rs`, `polyoxide-polymarket/src/shared/protocol.rs`, `polyoxide-polymarket/src/relay/types.rs`, `polyoxide-polymarket/src/relay/mod.rs`, `polyoxide-polymarket/src/relay/deposit_wallet.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/tests/mock_relay.rs`, `.claude/rules/polymarket-relay.md`, `polyoxide-polymarket/src/relay/README.md`
  - **Interfaces:** exposes "Story 4 → Story 5"; uses Story 1's `router_vectors.json` and `id_vectors.json`, and Story 2's `shared::protocol`
  - **Verify:** `bash -c 'cargo test -p polyoxide-polymarket --all-features && cargo test -p polyoxide-polymarket --no-default-features --features relay --lib && cargo test -p polyoxide-polymarket --no-default-features --features relay --test mock_relay && cargo test -p polyoxide-polymarket --no-default-features --features clob --lib && cargo test --doc -p polyoxide-polymarket --features relay && python3 scripts/test_body_diff.py --baseline v0.40.0 && cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_module_readmes.py -q'`
  - **Review:** deep (new public relay API whose calldata moves funds on chain)
  - **Reading:** spec Design §1, §5, Limits; `polyoxide-polymarket/src/shared/protocol.rs` (Story 2's), `polyoxide-polymarket/src/relay/types.rs:1-80`, `polyoxide-polymarket/src/relay/mod.rs:115-155`, `polyoxide-polymarket/src/relay/deposit_wallet.rs`, `polyoxide-polymarket/src/relay/client.rs:25-45`, `:190-215`, `:775-800`, `:975-1045`, `:1400-1560`, `:1600-1730`, `polyoxide-polymarket/tests/mock_relay.rs:1-60`; `.claude/rules/polymarket-relay.md`

- **Story 5** — The live V2 probe, the resolution note, the deferred checklist and the mutation battery (spec Design §6; Testing "Live" and "Mutation battery").

  Appends `live_v2_market_is_served` to `live_clob.rs`: the newest 100 open gamma markets (`order=id`, `ascending=false`, `limit=100`, `closed(false)`), the first whose `version` is `Some(ProtocolVersion::V2)`, else `environmental("no Protocol V2 market among the newest 100 open markets")`; its ids through `outcome_ids()` (`.or_fail`); then `/book`, `/tick-size` and `/neg-risk` for each id through the public client, each `.or_fail`. Writes the Data v2 resolution note in `data-v2/OBSERVED.md` and the deferred V2 fill checklist (BUY and SELL fills and balances on a V2 and a CTF market, blocked on the first V2 market and the owner's go-ahead) in `deferred-work.md`. Runs each mutation of the spec's battery one at a time against the tests that should kill it, records KILLED with the killing test names (and the control's SURVIVES) as `docs/MUTANTS.md` rows in its format, and adds each cited line to `test_mutants_ledger.py`'s `SNIPPETS`. A mutation that survives is a stop: report it with its patch, never weaken the row.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/tests/live_clob.rs`, `docs/specs/data-v2/OBSERVED.md`, `_bmad-output/implementation-artifacts/deferred-work.md`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`
  - **Interfaces:** uses Story 3's `Market::outcome_ids` and every line Stories 2 to 4 wrote; exposes none
  - **Verify:** `bash -c 'out=$(cargo test -p polyoxide-polymarket --no-default-features --features clob,gamma --test live_clob -- --ignored --exact live_v2_market_is_served 2>&1); echo "$out" | tail -20; echo "$out" | grep -qE "polyoxide-class=environmental|test result: ok. 1 passed" && python3 scripts/live_unwraps.py && cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'`
  - **Review:** light (a read-only live test, records and a re-provable ledger; no library code)
  - **Reading:** spec Design §6, Limits, Testing "Live" and "Mutation battery"; `polyoxide-polymarket/tests/live_clob.rs:1-140`, `polyoxide-polymarket/tests/live_gamma.rs:595-630`, `docs/MUTANTS.md:1-40` and one row, `.github/scripts/tests/test_mutants_ledger.py:1-60`, `_bmad-output/implementation-artifacts/deferred-work.md` (its last entry's format), `docs/specs/data-v2/OBSERVED.md` (its headings)

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->
