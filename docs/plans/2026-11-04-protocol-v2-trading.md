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
- `.claude/rules/testing.md` (modify): the capture-script section names the new PEP 723 script.

**Story 2: CLOB.**

- `pp/src/shared/protocol.rs` (create), `pp/src/shared/mod.rs` (modify): the predicate and its parse and decode, under `clob`.
- `pp/src/clob/core/chain.rs` (modify): `Chain::exchange_v3`.
- `pp/src/clob/core/eip712.rs` (modify): `order_exchange`, `order_domain`, the signing paths, new vector tests after the last test.
- `pp/src/clob/client.rs` (modify): `get_market_metadata` skips `/neg-risk` for a V2 id.
- `pp/src/clob/api/account.rs` (modify): `balance_allowance`'s asset type; `update_balance_allowance`'s docs.
- `pp/src/clob/types.rs`, `pp/src/clob/venue/trading.rs` (modify): the `neg_risk` doc comments.
- `pp/tests/mock_clob.rs`, `pp/tests/trading_clob.rs` (modify): appended tests.
- `.claude/rules/polymarket-clob.md`, `.claude/rules/session-keys.md`, `docs/specs/clob/auth.md`, `docs/specs/session-keys/OBSERVED.md` (modify).
- `docs/specs/clob/orders.md` (its polyoxide note on the domain version), `docs/specs/session-keys/README.md` (its type-3 domain, tree :57-58) (modify).

**Story 3: gamma selector, event groups and the CLI's prices discovery.**

- `pp/src/gamma/types.rs` (modify): `OutcomeId`, `Market::outcome_ids`.
- `pp/src/gamma/error.rs`, `pp/src/gamma/mod.rs`, `pp/src/lib.rs` (modify): `OutcomeIdsError`, its re-export and its `Classify` assertion.
- `pp/src/clob/venue.rs` (modify): `event_group`, `event_id_of`.
- `pp/tests/outcome_ids_gamma.rs` (create), `pp/Cargo.toml` (modify: its `[[test]]` stanza), `pp/tests/error_classes_gamma.rs`, `pp/tests/venue_clob.rs` (modify).
- `.claude/rules/polymarket-gamma.md`, `.claude/rules/polymarket-clob.md`, `docs/specs/gamma/OBSERVED.md` (modify).
- `polyoxide-cli/src/commands/polymarket/clob/prices/select.rs` (modify): `discover_targets` reads a V2 market through `Market::outcome_ids` (spec §4a); `polyoxide-cli/tests/prices_discovery.rs` (modify): one appended test; `.claude/rules/cli.md` (modify).

**Story 4: relay Router ops.**

- `pp/src/shared/mod.rs`, `pp/src/shared/protocol.rs` (modify): the module widens to `any(clob, relay)`.
- `pp/src/relay/types.rs`, `pp/src/relay/mod.rs` (modify): `V2PositionId`, `parse_v2_condition_id`, their re-exports.
- `pp/src/relay/deposit_wallet.rs` (modify): the three calldata builders and their tests.
- `pp/src/relay/client.rs` (modify): `PROTOCOL_V2_ROUTER`, the three calls and three submits, the redemption doc comments.
- `pp/tests/mock_relay.rs` (modify): appended tests.
- `.claude/rules/polymarket-relay.md`, `pp/src/relay/README.md`, `docs/specs/session-keys/README.md` (its V2-router sentence, tree :151-152) (modify).

**Story 5: live probe, resolution note, mutation battery.**

- `pp/tests/live_clob.rs` (modify): `live_v2_market_is_served`.
- `docs/specs/data-v2/OBSERVED.md`, `_bmad-output/implementation-artifacts/deferred-work.md` (modify).
- `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py` (modify): the battery's rows and their cited lines.

## Names contract

Every name one story defines for another, with its type, signature and visibility, and every text a later story's test matches. A drafter who needs a cross-story name this section lacks stops and asks; it never invents one. Paths under `src/` and `tests/` are `polyoxide-polymarket/`'s.

**Story 1 → Stories 2, 3 and 4: the capture.**

- `uv run scripts/capture_polymarket_protocol_v2.py <clob_dir> <relay_dir> <gamma_dir>` creates the three directories if missing and writes the files below into them, and nothing else. Its PEP 723 header pins `polymarket-client==0.12.0` and `exclude-newer = "2026-10-02T00:00:00Z"`. Its output is byte-identical across runs on one day (fixed salts, timestamps and keys; `PROVENANCE.md` dates to the day, so a check that re-runs it diffs with `-x PROVENANCE.md`). It writes its JSON through `scripts/capture_common.py`'s `write_json` and each `PROVENANCE.md` through its `write_provenance`.
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

- `position_ids` holds, in this order: the four `ids`; `"1"`; `"100"`; `"123456"`; `v2_binary` as `0x` hex, as `0b` binary and as `0o` octal; `v2_binary` with a leading and a trailing space; `"-1"`; `2^256` as decimal; `""`; `"abc"`; `v2_binary` with its module byte set to `0x00`, to `0x04`, and with its outcome byte set to `0x02`; then `v2_binary` with bit 40 set, and `v2_binary` with bit 104 set (20 rows; indices 0 to 17 as before, so the two new rows are 18 and 19; they sit on the mask's two edges, so a mask shifted up one bit changes their answers; a mask shifted down one bit is caught instead by bit 39, which every V2 id's `0xdeadbeef` tail sets). `is_v2_position_id` and `decoded` are py-sdk's own answers (`decoded` is `null` when `decode_v2_outcome_position_id` raises).
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

- Each `market` is `tests/fixtures/clob/venue/gamma_markets.json`'s first market with only `version`, `clobTokenIds` and `positionIds` edited (`outcomes` stays the base `["Yes","No"]` text). As gamma serves them, `clobTokenIds` is JSON text (a string holding an array) and `positionIds` a real JSON array of strings, never `json.dumps`'d; `ids` is the `(yes, no)` pair py-sdk's `normalize_market_position_context` returns, or `null` when it raises. The cases, in order: `v1_with_position_ids` (`v1`, both fields), `v2_with_both` (`v2`, both fields, `positionIds` = `[v2_binary, its sibling]`), `missing_version`, `unknown_version` (`"v3"`), `v2_one_position_id`, `v1_no_clob_token_ids`.

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
- `pub enum OutcomeIdsError` in `gamma::error`, re-exported as `gamma::OutcomeIdsError`, `#[non_exhaustive]`, deriving `Debug, Clone, PartialEq, Eq` and `thiserror::Error`, with exactly: `MissingVersion`, `UnsupportedVersion(String)`, `MissingIds`, `MalformedIds`, `MalformedOutcomes`, `CountMismatch { outcomes: usize, ids: usize }`, `InvalidId(String)`. Every variant's `Classify::class()` is `Class::Decode`, `is_fault()` `true`, `retry_after()` `None`.

**Story 4 → Story 5.** In `polyoxide-polymarket::relay`:

- `pub struct V2PositionId` (`Debug, Clone, Copy, PartialEq, Eq, Hash`, fields private) with `pub fn parse(id: &str) -> Result<Self, RelayError>`, `pub fn condition_id(&self) -> FixedBytes<31>`, `pub fn outcome_index(&self) -> u8`; and `pub fn parse_v2_condition_id(id: &str) -> Result<FixedBytes<31>, RelayError>`; both in `relay::types`, re-exported from `relay`.
- Refusal messages (`RelayError::validation`), py-sdk's verbatim: `Position ID must be a uint256 value`, `Position ID must use a supported protocol v2 module`, `Protocol v2 position ID must be a YES/NO position ID`, `Protocol v2 condition ID must be bytes31, or bytes32 with a binary outcome byte`.
- In `relay::deposit_wallet` (a `pub mod`), beside `redeem_positions_calldata` and, like it, reached as `relay::deposit_wallet::…` with no re-export from `relay`'s root: `pub fn router_split_calldata(condition_id: FixedBytes<31>, amount: U256) -> Vec<u8>`, `pub fn router_merge_calldata(condition_id: FixedBytes<31>, amount: U256) -> Vec<u8>`, `pub fn router_redeem_calldata(position: &V2PositionId, amount: U256) -> Vec<u8>`.
- On `RelayClient`: `pub fn v2_split_call(&self, condition_id: FixedBytes<31>, amount: U256) -> Result<DepositWalletCall, RelayError>`, `v2_merge_call` (same signature), `pub fn v2_redeem_call(&self, position: &V2PositionId, amount: U256) -> Result<DepositWalletCall, RelayError>`, and `pub async fn submit_v2_split(&self, condition_id: FixedBytes<31>, amount: U256) -> Result<SubmitResponse, RelayError>`, `submit_v2_merge` (same), `pub async fn submit_v2_redeem(&self, position: &V2PositionId, amount: U256) -> Result<SubmitResponse, RelayError>`. A chain other than 137 is refused with `Protocol V2 Router calls are only known for Polygon mainnet (137), not chain <id>`.

**Texts a later story quotes from an earlier one.** A fixer leaves each unchanged:

- Story 2's `src/shared/mod.rs` declaration (`#[cfg(feature = "clob")] pub(crate) mod protocol;`), which Story 4 widens; and Story 2's two mask lines in `is_v2_position_id`, `let reserved = U256::from(u64::MAX) << 40;` and `(value & reserved) == U256::ZERO && …`, which Story 5's rows (q) quote and mutate.
- Story 2's `src/shared/protocol.rs` layout: `is_v2_position_id` after `parse_position_id` and `decode_v2_outcome_position_id`, with its tests and the `Ids` struct and `Vectors::ids` field that only they read, which Story 4 puts under `#[cfg(feature = "clob")]`.
- `docs/specs/session-keys/README.md`: Story 2 edits only its type-3 domain passage (tree :57-58); the V2-router sentence (tree :151-152) is Story 4's.
- `.claude/rules/polymarket-clob.md`: Story 2 leaves the `event_group` bullet (tree :82-84), which Story 3 quotes, unchanged.

**Story 5 reads** every line Stories 2 to 4 cite in `docs/MUTANTS.md` at its own HEAD; no story before it edits `docs/MUTANTS.md` or `.github/scripts/tests/test_mutants_ledger.py`.

**Design stage:** /flow-draft session 163e5fd7-f150-4956-a5aa-cbfaaba7117a, 2026-11-04T12:06:49Z to 2026-11-04T13:02:22Z UTC: $20.12 logged, $20.25 with estimated thinking; 6067 plan lines, $3.32 per 1,000 lines.

## Stories

- **Story 1** — Golden vectors from py-sdk 0.12.0 (spec: The reference; Testing, Golden vectors).

  Writes `scripts/capture_polymarket_protocol_v2.py` (named, as a PEP 723 `uv run` script, in `.claude/rules/testing.md`'s capture-script section) on the model of `scripts/capture_polymarket_session_keys.py` (PEP 723, py-sdk's private modules, `capture_common`), runs it, and commits its four fixtures and three `PROVENANCE.md` files exactly as the names contract lays them out. Every recorded choice (version, exchange, asset type, classification, narrowing, selected pair) is the return value of py-sdk's own function given the case's inputs; the script never decides one itself, which is the failure the 2026-09-25 redemption-target vector had. No Rust.

  - **Developer:** intermediate-dev
  - **Files:** `scripts/capture_polymarket_protocol_v2.py`, `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/order_vectors.json`, `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/id_vectors.json`, `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/PROVENANCE.md`, `polyoxide-polymarket/tests/fixtures/relay/protocol_v2/router_vectors.json`, `polyoxide-polymarket/tests/fixtures/relay/protocol_v2/PROVENANCE.md`, `polyoxide-polymarket/tests/fixtures/gamma/protocol_v2/selector_vectors.json`, `polyoxide-polymarket/tests/fixtures/gamma/protocol_v2/PROVENANCE.md`, `.claude/rules/testing.md`
  - **Interfaces:** exposes the capture command and the four fixture schemas under "Story 1 → Stories 2, 3 and 4"; uses nothing from earlier stories
  - **Verify:** `bash -c 'd=$(mktemp -d) && uv run scripts/capture_polymarket_protocol_v2.py $d/clob $d/relay $d/gamma && diff -r -x PROVENANCE.md $d/clob polyoxide-polymarket/tests/fixtures/clob/protocol_v2 && diff -r -x PROVENANCE.md $d/relay polyoxide-polymarket/tests/fixtures/relay/protocol_v2 && diff -r -x PROVENANCE.md $d/gamma polyoxide-polymarket/tests/fixtures/gamma/protocol_v2'` (each `PROVENANCE.md` carries its capture day, so the diff leaves it out)
  - **Review:** deep (the fixtures are every later story's yardstick; a value the script chose would pin a wrong behaviour)
  - **Reading:** spec "The reference" (facts 1 to 9), Testing "Golden vectors"; `scripts/capture_polymarket_session_keys.py`, `scripts/capture_common.py`, `polyoxide-polymarket/tests/fixtures/clob/session_keys/PROVENANCE.md`, `polyoxide-polymarket/tests/fixtures/clob/venue/gamma_markets.json` (its first market); py-sdk 0.12.0's `_internal/protocol.py`, `_internal/actions/orders/{orders,context,typed_data,allowance}.py`, `_internal/actions/relayer/{calls,positions}.py` and `_internal/actions/relayer/signing/deposit_wallet.py`

- **Story 2** — CLOB: the id rule, ExchangeV3 signing, the `/neg-risk` skip and V2 balances (spec Design §1, §2; Limits).

  Creates `shared::protocol` (under `clob` for now) with tests against `id_vectors.json` (`is_v2_position_id` as the conjunction of the two recorded answers; `decode_v2_outcome_position_id` against `decoded`). Adds `Chain::exchange_v3`, `order_exchange` and `order_domain`, and routes `order_hash`, `sign_order` and `sign_order_as` through them, keeping `exchange_address`, `exchange_domain` and `envelope_digest`'s signatures (the last takes its version from the order's token id). New unit tests after the file's last test pin each `order_vectors.json` case (exchange, version, domain separator, struct hash, digest, signature; for type 3 through `sign_order_as` with `SigningTarget::DepositWallet`, owner and session key). `get_market_metadata` skips `/neg-risk` for a V2 id; `balance_allowance` sends `CONDITIONAL-V2`. Appended mock tests: the asset-type table of `id_vectors.json` through the request each case sends; `create_order` on a V2 id calling `/tick-size` and never `/neg-risk` (`expect(0)` with `match_query(Matcher::Any)`); a V2 id on an Amoy client refused as `InvalidRequest` with the contract's message and nothing posted; `Trading::place` on a V2 key reporting the V3 `order_hash`. Docs: the CLOB and session-key rules (the per-id choice, the id rule's divergence from py-sdk's bare mask on impossible ids, `neg_risk` ignored for V2, the capture script under `polymarket-clob.md`'s `paths:`), `auth.md`'s domain per protocol and `clob/orders.md`'s polyoxide note likewise, `session-keys/OBSERVED.md`'s type 3 on ExchangeV3 and `session-keys/README.md`'s type-3 domain passage (tree :57-58). Every baseline test in `eip712.rs`, `mock_clob.rs` and `trading_clob.rs` passes unedited.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/src/shared/protocol.rs`, `polyoxide-polymarket/src/shared/mod.rs`, `polyoxide-polymarket/src/clob/core/chain.rs`, `polyoxide-polymarket/src/clob/core/eip712.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/api/account.rs`, `polyoxide-polymarket/src/clob/types.rs`, `polyoxide-polymarket/src/clob/venue/trading.rs`, `polyoxide-polymarket/tests/mock_clob.rs`, `polyoxide-polymarket/tests/trading_clob.rs`, `.claude/rules/polymarket-clob.md`, `.claude/rules/session-keys.md`, `docs/specs/clob/auth.md`, `docs/specs/session-keys/OBSERVED.md`, `docs/specs/clob/orders.md`, `docs/specs/session-keys/README.md`
  - **Interfaces:** exposes "Story 2 → Stories 3, 4 and 5"; uses Story 1's `id_vectors.json` and `order_vectors.json`
  - **Verify:** `bash -c 'cargo test -p polyoxide-polymarket --all-features && cargo test -p polyoxide-polymarket --no-default-features --features clob --lib && cargo test -p polyoxide-polymarket --no-default-features --features clob --test mock_clob && python3 scripts/test_body_diff.py --baseline v0.40.0 && cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'`
  - **Review:** deep (order signing: a wrong domain posts orders the exchange rejects, or signs for the wrong contract)
  - **Reading:** spec Design §1, §2, Limits, Testing "Mock tests"; `polyoxide-polymarket/src/shared/mod.rs:1-60`, `polyoxide-polymarket/src/clob/core/chain.rs`, `polyoxide-polymarket/src/clob/core/eip712.rs:1-470` and its tests from `:1100` to the end, `polyoxide-polymarket/src/clob/client.rs:320-510`, `polyoxide-polymarket/src/clob/api/account.rs:1-90`, `polyoxide-polymarket/src/clob/types.rs:270-300`, `polyoxide-polymarket/src/clob/venue/trading.rs:40-80`, `polyoxide-polymarket/tests/mock_clob.rs:1-65`, `polyoxide-polymarket/tests/trading_clob.rs:1-140`; `.claude/rules/polymarket-clob.md`, `.claude/rules/session-keys.md`

- **Story 3** — Gamma's `Market::outcome_ids`, V2 outcomes in event groups, and the CLI's prices discovery (spec Design §3, §4, §4a, §6's gamma note).

  Adds `OutcomeId`, `Market::outcome_ids` with spec §3's six-step rule, and `OutcomeIdsError` with its `Classify`, re-export and `lib.rs` assertion. `tests/outcome_ids_gamma.rs` (a new `[[test]]` with `required-features = ["gamma"]`, after the gamma stanzas) holds the selector to every `selector_vectors.json` case (the pair, or a refusal where py-sdk refused) and adds the Rust-only cases: three outcomes, a count mismatch, a non-decimal id, malformed `outcomes`, malformed `clobTokenIds`. `error_classes_gamma.rs` gains one test asserting every variant's class. `event_group` reads outcomes through the selector, skipping a market whose version is missing or `Other`; `event_id_of` sends `position_ids=` for a V2 key. In `venue_clob.rs`, `event_group_leaves_out_the_outcomes_of_a_v2_market` is replaced by `event_group_keys_a_v2_market_by_its_position_ids`, and `event_group_skips_a_market_with_no_known_version` and `event_group_finds_a_v2_key_by_its_position_ids` (the `/markets` route matched on `position_ids=<id>&closed=any`) are added; the V2 ids come from `id_vectors.json`. Docs: `polymarket-gamma.md` (the selector, trading supported pending live verification, `resolution_status` replacing `uma_resolution_status` for V2), `polymarket-clob.md`'s `event_group` bullet, `gamma/OBSERVED.md`'s "What polyoxide does". In `polyoxide-cli`, `discover_targets` takes a V2 market's ids from `Market::outcome_ids`, skips a market whose version is `Other` with a warning, and keeps today's `clob_token_ids` path for `V1` and for a missing version (spec §4a: the baseline `discover_extracts_token_ids_from_markets` serves unversioned markets and passes unedited); one test is appended to `tests/prices_discovery.rs`, and `cli.md`'s prices section says so.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/src/gamma/types.rs`, `polyoxide-polymarket/src/gamma/error.rs`, `polyoxide-polymarket/src/gamma/mod.rs`, `polyoxide-polymarket/src/lib.rs`, `polyoxide-polymarket/src/clob/venue.rs`, `polyoxide-polymarket/tests/outcome_ids_gamma.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/tests/error_classes_gamma.rs`, `polyoxide-polymarket/tests/venue_clob.rs`, `.claude/rules/polymarket-gamma.md`, `.claude/rules/polymarket-clob.md`, `docs/specs/gamma/OBSERVED.md`, `polyoxide-cli/src/commands/polymarket/clob/prices/select.rs`, `polyoxide-cli/tests/prices_discovery.rs`, `.claude/rules/cli.md`
  - **Interfaces:** exposes "Story 3 → Story 5"; uses Story 1's `selector_vectors.json` and `id_vectors.json`, and Story 2's `shared::protocol::is_v2_position_id`
  - **Verify:** `bash -c 'cargo test -p polyoxide-polymarket --all-features && cargo test -p polyoxide-polymarket --no-default-features --features gamma --test outcome_ids_gamma --test error_classes_gamma && cargo test -p polyoxide-polymarket --features clob,gamma,data --test venue_clob && cargo test -p polyoxide-cli --test prices_discovery && python3 scripts/test_body_diff.py --baseline v0.40.0 && cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_claude_rules.py -q'`
  - **Review:** deep (a public selector whose wrong choice sends orders for ids with no book, and a new public error)
  - **Reading:** spec Design §3, §4, §4a, §6; `polyoxide-cli/src/commands/polymarket/clob/prices/select.rs`, `polyoxide-cli/tests/prices_discovery.rs`, `.claude/rules/cli.md:77-83`; `polyoxide-polymarket/src/gamma/types.rs:1-70` and `:150-245`, `polyoxide-polymarket/src/gamma/error.rs`, `polyoxide-polymarket/src/gamma/mod.rs`, `polyoxide-polymarket/src/lib.rs:85-157`, `polyoxide-polymarket/src/clob/venue.rs:380-510`, `polyoxide-polymarket/tests/venue_clob.rs:1-85` and `:440-545`, `polyoxide-polymarket/tests/error_classes_gamma.rs`, `polyoxide-polymarket/Cargo.toml:236-262`; `.claude/rules/polymarket-gamma.md`, `docs/specs/gamma/OBSERVED.md` (its Protocol V2 section)

- **Story 4** — Relay: `V2PositionId`, condition narrowing, Router calldata, calls and submits (spec Design §5; Limits).

  Widens `shared::protocol` to `#[cfg(any(feature = "clob", feature = "relay"))]` and moves `#[cfg(feature = "clob")]` onto `is_v2_position_id` alone. Adds `V2PositionId` and `parse_v2_condition_id` on `shared::protocol`'s parse and decode with py-sdk's messages, the three `sol!` calldata builders, and on `RelayClient` the private `PROTOCOL_V2_ROUTER`, the three calls (chain 137 only) and the three submits through `execute(vec![tx], None)` with `CALL_OPERATION`. Unit tests in `deposit_wallet.rs` pin every `router_vectors.json` calldata row, the `condition_ids` table through `parse_v2_condition_id`, the `position_ids` rows of `id_vectors.json` through `V2PositionId::parse` (accepted where `decoded` is not `null`, else refused), and the batch digest through `deposit_wallet_batch_typed_data`'s digest for the redeem call. Appended `mock_relay.rs` tests: `submit_v2_redeem` posting the Router call for a Safe, a Proxy and a Deposit Wallet client, and a chain-80002 client refused with the contract's message before any request (`expect(0)` with `match_query(Matcher::Any)`). The redemption doc comments point at the V2 helpers; `polymarket-relay.md`, the relay README and `docs/specs/session-keys/README.md`'s V2-router sentence (tree :151-152) describe them.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-polymarket/src/shared/mod.rs`, `polyoxide-polymarket/src/shared/protocol.rs`, `polyoxide-polymarket/src/relay/types.rs`, `polyoxide-polymarket/src/relay/mod.rs`, `polyoxide-polymarket/src/relay/deposit_wallet.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/tests/mock_relay.rs`, `.claude/rules/polymarket-relay.md`, `polyoxide-polymarket/src/relay/README.md`, `docs/specs/session-keys/README.md`
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

## Story 1: Golden vectors from py-sdk 0.12.0

- **Developer:** intermediate-dev
- **Files:** `scripts/capture_polymarket_protocol_v2.py`, `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/order_vectors.json`, `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/id_vectors.json`, `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/PROVENANCE.md`, `polyoxide-polymarket/tests/fixtures/relay/protocol_v2/router_vectors.json`, `polyoxide-polymarket/tests/fixtures/relay/protocol_v2/PROVENANCE.md`, `polyoxide-polymarket/tests/fixtures/gamma/protocol_v2/selector_vectors.json`, `polyoxide-polymarket/tests/fixtures/gamma/protocol_v2/PROVENANCE.md`, `.claude/rules/testing.md`
- **Interfaces:** exposes the capture command and the four fixture schemas under "Story 1 → Stories 2, 3 and 4"; uses nothing from earlier stories
- **Verify:** `bash -c 'd=$(mktemp -d) && uv run scripts/capture_polymarket_protocol_v2.py $d/clob $d/relay $d/gamma && diff -r -x PROVENANCE.md $d/clob polyoxide-polymarket/tests/fixtures/clob/protocol_v2 && diff -r -x PROVENANCE.md $d/relay polyoxide-polymarket/tests/fixtures/relay/protocol_v2 && diff -r -x PROVENANCE.md $d/gamma polyoxide-polymarket/tests/fixtures/gamma/protocol_v2'` (each `PROVENANCE.md` carries its capture day, so the diff leaves it out)
- **Review:** deep (the fixtures are every later story's yardstick; a value the script chose would pin a wrong behaviour)

This story writes no Rust. It adds `scripts/capture_polymarket_protocol_v2.py`, a PEP 723 `uv run` script on the model of `scripts/capture_polymarket_session_keys.py` (which stays on py-sdk 0.11.0, unchanged), and the four fixtures it writes, with a `PROVENANCE.md` beside each directory's files, and it names the script, as a PEP 723 `uv run` script, in `.claude/rules/testing.md`'s capture-script section. Every recorded choice (the domain version, the exchange, the balance asset type, the V2 classification, the condition-id narrowing, the selected outcome pair) is the return value of py-sdk's own function given the case's inputs: the script never decides one itself, which is the failure the 2026-09-25 redemption-target vector had. Where the script holds an expectation (an id kept only if py-sdk accepts it, a refusal for the reason intended), it only exits non-zero on a mismatch and never records the expectation. The script imports py-sdk's private modules, so it is expected to need import-path fixes after a py-sdk upgrade. No code here holds a landmine marker.

### Task 1.1: Write the capture script

**Files:**
- Create: `scripts/capture_polymarket_protocol_v2.py`

- [ ] **Step 1: Write the failing check**

The check is the capture command itself, run into a scratch directory. There is no Rust test in this story: the Rust stories' tests read these fixtures, and this story's yardstick is that py-sdk, not the script, produced every value.

```bash
bash -c 'd=$(mktemp -d) && uv run scripts/capture_polymarket_protocol_v2.py $d/clob $d/relay $d/gamma'
```

- [ ] **Step 2: Run it to see it fail**

Run the command above. Expected: `uv` exits non-zero because the script does not exist yet (`error: Failed to read ... No such file or directory (os error 2)`).

- [ ] **Step 3: Implement**

Create `scripts/capture_polymarket_protocol_v2.py` with exactly this content:

```python
#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["polymarket-client==0.12.0"]
#
# [tool.uv]
# exclude-newer = "2026-10-02T00:00:00Z"
# ///
"""Generate Protocol V2 golden vectors from Polymarket's official py-sdk 0.12.0.

polyoxide pins its Protocol V2 trading against these bytes: the ExchangeV3 order
signing (domain version "3", Deposit Wallet envelopes included), the asset-id
classification, the balance-allowance asset type, the Router calldata and the
outcome-pair selection. Every choice a vector records is the return value of py-sdk's
own function, given the case's inputs; this script never decides one.

The existing 0.11.0 script, `capture_polymarket_session_keys.py`, and its fixtures do
not change.

This script imports py-sdk's private modules (`polymarket._internal...`), which are not
a stable public API, so it is expected to need import-path fixes after a py-sdk upgrade.

Usage:
    uv run scripts/capture_polymarket_protocol_v2.py \
        polyoxide-polymarket/tests/fixtures/clob/protocol_v2 \
        polyoxide-polymarket/tests/fixtures/relay/protocol_v2 \
        polyoxide-polymarket/tests/fixtures/gamma/protocol_v2

Writes `order_vectors.json`, `id_vectors.json` and `PROVENANCE.md` to the first
directory, `router_vectors.json` and `PROVENANCE.md` to the second, and
`selector_vectors.json` and `PROVENANCE.md` to the third, creating the directories if
they are missing and writing nothing else. The output is byte-identical across runs on
one day: salts, timestamps and keys are fixed, and PROVENANCE.md carries only the day.
Re-run after a py-sdk upgrade; review the diff before committing.
"""
import copy
import dataclasses
import importlib.metadata
import json
import re
import sys
import tomllib
from pathlib import Path

from eth_account import Account
from eth_account.messages import encode_typed_data
from eth_utils.address import to_checksum_address
from eth_utils.crypto import keccak
from polymarket._internal.actions.orders.allowance import resolve_order_balance_allowance_target
from polymarket._internal.actions.orders.context import resolve_order_exchange_address
from polymarket._internal.actions.orders.orders import create_unsigned_order
from polymarket._internal.actions.orders.typed_data import (
    _app_domain_separator,
    _order_contents_hash,
    build_order_signature,
    build_order_typed_data,
)
from polymarket._internal.actions.orders.types import OrderDraft
from polymarket._internal.actions.relayer.calls import (
    _protocol_v2_condition_id_bytes,
    router_merge_call,
    router_redeem_call,
    router_split_call,
)
from polymarket._internal.actions.relayer.positions import (
    canonicalize_combo_legs,
    derive_combo_outcome_position_ids,
    derive_combo_position_context,
    normalize_market_position_context,
)
from polymarket._internal.actions.relayer.signing.deposit_wallet import (
    build_deposit_wallet_typed_data,
)
from polymarket._internal.environment import PRODUCTION_CONFIG
from polymarket._internal.protocol import decode_v2_outcome_position_id, is_v2_position_id
from polymarket._internal.wallet import wrap_deposit_wallet_session_signer_signature
from polymarket.errors import UnexpectedResponseError, UserInputError
from polymarket.models.gamma.market import Market
from polymarket.models.types import TokenId
from polymarket.types import EvmAddress
from pydantic import ValidationError

import capture_common

ANVIL_KEY_0 = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
ANVIL_ADDR_0 = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
ANVIL_ADDR_1 = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
DEPOSIT_WALLET = EvmAddress("0x57ffbc34de23124faeb8387fcd689d314e57accd")
CHAIN_ID = 137
# The addresses the design (and the Rust constants) name. They are only cross-checked
# against py-sdk's PRODUCTION_CONFIG before anything is written; every recorded address
# is read from the config.
EXCHANGE_V3 = "0xe3333700cA9d93003F00f0F71f8515005F6c00Aa"
ROUTER = "0x12121212006e4CD160D18e3f00711DA5c3372600"

# The ids' fixed layout: module byte, the first 18 bytes of keccak(ID_SEED), 8 zero
# bytes, four fixed bytes, outcome byte.
ID_SEED = b"polyoxide-v2-vector"
ID_FIXED_TAIL = bytes.fromhex("deadbeef")

# One order for every case: only the token, the wallet type and `neg_risk` vary.
SALT = 1
MAKER_AMOUNT = 1_000_000
TAKER_AMOUNT = 500_000
TIMESTAMP_MS = 1_700_000_000_000

MAX_UINT256 = (1 << 256) - 1
ROUTER_AMOUNTS = (0, 1_000_000, MAX_UINT256)
REDEEM_AMOUNT = 1_000_000
BATCH_NONCE = "0"
BATCH_DEADLINE = "1700000600"

GAMMA_MARKETS = (
    Path(__file__).resolve().parent.parent
    / "polyoxide-polymarket/tests/fixtures/clob/venue/gamma_markets.json"
)

# (case, id key, neg_risk passed to py-sdk, wallet type, wrapped in the session envelope)
ORDER_CASES = (
    ("v2_eoa", "v2_binary", False, "EOA", False),
    ("v2_neg_risk_eoa", "v2_neg_risk", True, "EOA", False),
    ("v2_deposit_wallet_owner", "v2_binary", False, "DEPOSIT_WALLET", False),
    ("v2_deposit_wallet_session", "v2_binary", False, "DEPOSIT_WALLET", True),
    ("v1_eoa", "v1", False, "EOA", False),
    ("v1_neg_risk_eoa", "v1", True, "EOA", False),
)

COMMAND = (
    "uv run scripts/capture_polymarket_protocol_v2.py "
    "polyoxide-polymarket/tests/fixtures/clob/protocol_v2 "
    "polyoxide-polymarket/tests/fixtures/relay/protocol_v2 "
    "polyoxide-polymarket/tests/fixtures/gamma/protocol_v2"
)


def digest(typed_data: dict) -> str:
    signable = encode_typed_data(full_message=typed_data)
    return "0x" + keccak(b"\x19\x01" + signable.header + signable.body).hex()


def check_environment(signer) -> None:
    """The inputs this script fixes must be the ones py-sdk's production config names."""
    if signer.address != ANVIL_ADDR_0:
        sys.exit(f"Anvil key #0 is {signer.address}, expected {ANVIL_ADDR_0}")
    for name, got, want in (
        ("chain_id", PRODUCTION_CONFIG.chain_id, CHAIN_ID),
        ("exchange_v3", PRODUCTION_CONFIG.exchange_v3, EXCHANGE_V3),
        ("protocol_v2_router", PRODUCTION_CONFIG.protocol_v2_router, ROUTER),
    ):
        if got != want:
            sys.exit(f"py-sdk's PRODUCTION_CONFIG.{name} is {got}, expected {want}")


# --- ids ---------------------------------------------------------------------------


def v2_id(module: int, outcome: int) -> str:
    """A decimal V2-shaped id: module byte, 18 hashed bytes, 8 zero bytes, `deadbeef`,
    outcome byte (32 bytes, big-endian)."""
    raw = bytes([module]) + keccak(ID_SEED)[:18] + bytes(8) + ID_FIXED_TAIL + bytes([outcome])
    if len(raw) != 32:
        sys.exit(f"v2_id layout is {len(raw)} bytes, expected 32")
    return str(int.from_bytes(raw, "big"))


def require_v2(name: str, position_id: str, outcome: int):
    """Keep an id only if py-sdk's `is_v2_position_id` and `decode_v2_outcome_position_id`
    both accept it, and the decode reads the outcome the layout put there."""
    if not is_v2_position_id(position_id):
        sys.exit(f"{name} is not a V2 position id to py-sdk: {position_id}")
    try:
        decoded = decode_v2_outcome_position_id(position_id)
    except UserInputError as error:
        sys.exit(f"{name} is refused by decode_v2_outcome_position_id: {error}")
    if decoded.outcome_index != outcome:
        sys.exit(f"{name} decodes to outcome {decoded.outcome_index}, expected {outcome}")
    return decoded


def first_gamma_market() -> dict:
    return json.loads(GAMMA_MARKETS.read_text())[0]


def build_ids() -> tuple[dict, str]:
    """The four ids of the vectors, and the sibling (outcome 1) of `v2_binary`."""
    v1 = json.loads(first_gamma_market()["clobTokenIds"])[0]
    if is_v2_position_id(v1):
        sys.exit(f"the V1 id is a V2 position id to py-sdk: {v1}")

    v2_binary = v2_id(1, 0)
    v2_neg_risk = v2_id(2, 0)
    require_v2("v2_binary", v2_binary, 0)
    require_v2("v2_neg_risk", v2_neg_risk, 0)
    sibling = v2_id(1, 1)
    require_v2("v2_binary sibling", sibling, 1)

    legs = canonicalize_combo_legs([v2_binary, v2_id(2, 1)])
    combo_context = derive_combo_position_context(legs)
    v2_combo = str(derive_combo_outcome_position_ids(combo_context.condition_id)[0])
    require_v2("v2_combo", v2_combo, 0)

    ids = {"v1": v1, "v2_binary": v2_binary, "v2_neg_risk": v2_neg_risk, "v2_combo": v2_combo}
    return ids, sibling


# --- id_vectors.json ---------------------------------------------------------------


def position_id_row(text: str) -> dict:
    answer = is_v2_position_id(text)
    try:
        decoded = decode_v2_outcome_position_id(text)
        recorded = {
            "condition_id": str(decoded.condition_id),
            "outcome_index": decoded.outcome_index,
        }
    except UserInputError:
        recorded = None
    return {"input": text, "is_v2_position_id": answer, "decoded": recorded}


def id_vectors(ids: dict) -> dict:
    binary = ids["v2_binary"]
    value = int(binary)
    inputs = [
        ids["v1"],
        ids["v2_binary"],
        ids["v2_neg_risk"],
        ids["v2_combo"],
        "1",
        "100",
        "123456",
        hex(value),
        bin(value),
        oct(value),
        f" {binary} ",
        "-1",
        str(1 << 256),
        "",
        "abc",
        v2_id(0, 0),
        v2_id(4, 0),
        v2_id(1, 2),
        # The mask's edges: bit 40 is the lowest reserved bit, bit 104 the lowest bit
        # above the reserved range. A mask shifted one bit either way changes an answer.
        str(value | (1 << 40)),
        str(value | (1 << 104)),
    ]
    targets = []
    for side in ("BUY", "SELL"):
        for key in ("v1", "v2_binary"):
            asset_type, asset_id_sent = resolve_order_balance_allowance_target(
                side=side, asset_id=ids[key]
            )
            targets.append(
                {
                    "side": side,
                    "asset_id": ids[key],
                    "asset_type": asset_type,
                    "asset_id_sent": None if asset_id_sent is None else str(asset_id_sent),
                }
            )
    return {
        "ids": ids,
        "position_ids": [position_id_row(text) for text in inputs],
        "balance_allowance_targets": targets,
    }


# --- order_vectors.json ------------------------------------------------------------


def order_case(signer, token_id: str, neg_risk: bool, wallet_type: str, session: bool) -> dict:
    """One order, through py-sdk's own exchange choice and `create_unsigned_order` (which
    sets `protocol_version`), with salt and timestamp fixed afterwards."""
    funder = DEPOSIT_WALLET if wallet_type == "DEPOSIT_WALLET" else EvmAddress(signer.address)
    exchange = resolve_order_exchange_address(
        PRODUCTION_CONFIG, asset_id=token_id, neg_risk=neg_risk
    )
    draft = OrderDraft(
        chain_id=CHAIN_ID,
        exchange_address=exchange,
        expiration=0,
        funder_address=funder,
        offered_amount=MAKER_AMOUNT,
        order_type="GTC",
        side="BUY",
        signer=EvmAddress(signer.address),
        requested_amount=TAKER_AMOUNT,
        asset_id=TokenId(token_id),
    )
    order = dataclasses.replace(
        create_unsigned_order(draft, wallet=DEPOSIT_WALLET, wallet_type=wallet_type),
        salt=SALT,
        timestamp=TIMESTAMP_MS,
    )
    if wallet_type == "EOA" and (order.maker != signer.address or order.signer != signer.address):
        sys.exit("a type-0 order's maker and signer must both be the signer's address")
    if wallet_type == "DEPOSIT_WALLET" and (
        order.maker != DEPOSIT_WALLET or order.signer != DEPOSIT_WALLET
    ):
        sys.exit("a type-3 order's maker and signer must both be the Deposit Wallet")

    typed_data = build_order_typed_data(order)
    inner = "0x" + signer.sign_typed_data(full_message=typed_data).signature.hex()
    signature = build_order_signature(order, inner)
    if session:
        signature = wrap_deposit_wallet_session_signer_signature(
            EvmAddress(to_checksum_address(ANVIL_ADDR_1)), signature
        )
    return {
        "neg_risk": neg_risk,
        "order": {
            "salt": str(order.salt),
            "maker": str(order.maker),
            "signer": str(order.signer),
            "token_id": str(order.token_id),
            "maker_amount": str(order.maker_amount),
            "taker_amount": str(order.taker_amount),
            "side": order.side,
            "signature_type": order.signature_type,
            "timestamp": str(order.timestamp),
            "expiration": str(order.expiration),
            "metadata": order.metadata,
            "builder": order.builder,
        },
        "exchange": to_checksum_address(order.exchange_address),
        "version": order.protocol_version,
        "domain_separator": _app_domain_separator(order, protocol_version=order.protocol_version),
        "struct_hash": "0x" + _order_contents_hash(order).hex(),
        "digest": digest(typed_data),
        "signature": str(signature),
    }


def order_vectors(signer, ids: dict) -> dict:
    cases = {}
    for name, key, neg_risk, wallet_type, session in ORDER_CASES:
        case = order_case(signer, ids[key], neg_risk, wallet_type, session)
        # Guards, never recorded: an id that py-sdk classified the other way would make
        # the whole case a control for the wrong protocol.
        want_version = "3" if key.startswith("v2") else "2"
        if case["version"] != want_version:
            sys.exit(f"{name}: py-sdk chose domain version {case['version']}, not {want_version}")
        if key.startswith("v2") and case["exchange"] != EXCHANGE_V3:
            sys.exit(f"{name}: py-sdk chose exchange {case['exchange']}, not ExchangeV3")
        cases[name] = case
    return {
        "chain_id": CHAIN_ID,
        "signer_address": signer.address,
        "session_signer": to_checksum_address(ANVIL_ADDR_1),
        "deposit_wallet": str(DEPOSIT_WALLET),
        "cases": cases,
    }


# --- router_vectors.json -----------------------------------------------------------


def bytes31_of(text: str):
    """`_protocol_v2_condition_id_bytes`'s result as `0x` hex, or None when it refuses."""
    try:
        return "0x" + _protocol_v2_condition_id_bytes(text).hex()
    except UserInputError:
        return None


def router_vectors(ids: dict, sibling: str) -> dict:
    router = EvmAddress(PRODUCTION_CONFIG.protocol_v2_router)
    binary = require_v2("v2_binary", ids["v2_binary"], 0)
    condition = str(binary.condition_id)

    def amount_rows(build) -> list[dict]:
        rows = []
        for amount in ROUTER_AMOUNTS:
            call = build(router=router, condition_id=condition, amount=amount)
            if call.to != router:
                sys.exit(f"a Router call targets {call.to}, not the Router {router}")
            rows.append({"condition_id": condition, "amount": str(amount), "calldata": call.data})
        return rows

    redeem = []
    for position_id, outcome in ((ids["v2_binary"], 0), (sibling, 1)):
        decoded = require_v2(f"redeem outcome {outcome}", position_id, outcome)
        call = router_redeem_call(
            router=router,
            condition_id=str(decoded.condition_id),
            outcome_index=decoded.outcome_index,
            amount=REDEEM_AMOUNT,
        )
        redeem.append(
            {
                "position_id": position_id,
                "condition_id": str(decoded.condition_id),
                "outcome_index": decoded.outcome_index,
                "amount": str(REDEEM_AMOUNT),
                "calldata": call.data,
            }
        )

    # py-sdk refuses an outcome index other than 0 or 1. The design records this refusal
    # through `id_vectors.json`'s outcome-byte-2 row, so the script only checks it.
    try:
        router_redeem_call(router=router, condition_id=condition, outcome_index=2, amount=1)
    except UserInputError:
        pass
    else:
        sys.exit("router_redeem_call accepted outcome index 2")

    hex62 = condition[2:]
    condition_inputs = [
        condition,
        condition + "00",
        condition + "01",
        "0X" + hex62.upper(),
        condition + "02",
        condition[:-2],
        hex62,
        "0x" + "zz" * 31,
    ]
    redeem_call = router_redeem_call(
        router=router,
        condition_id=redeem[0]["condition_id"],
        outcome_index=redeem[0]["outcome_index"],
        amount=REDEEM_AMOUNT,
    )
    batch = build_deposit_wallet_typed_data(
        wallet=DEPOSIT_WALLET,
        calls=[redeem_call],
        nonce=BATCH_NONCE,
        deadline=BATCH_DEADLINE,
        chain_id=CHAIN_ID,
    )
    return {
        "chain_id": CHAIN_ID,
        "router": str(router),
        "split": amount_rows(router_split_call),
        "merge": amount_rows(router_merge_call),
        "redeem": redeem,
        "condition_ids": [
            {"input": text, "bytes31": bytes31_of(text)} for text in condition_inputs
        ],
        "deposit_wallet_batch": {
            "wallet": str(DEPOSIT_WALLET),
            "nonce": BATCH_NONCE,
            "deadline": BATCH_DEADLINE,
            "calls": [
                {
                    "target": str(redeem_call.to),
                    "value": str(redeem_call.value),
                    "data": redeem_call.data,
                }
            ],
            "digest": digest(batch),
        },
    }


# --- selector_vectors.json ---------------------------------------------------------


def market_with(base: dict, *, version, clob_token_ids, position_ids) -> dict:
    """The first gamma market with only `version`, `clobTokenIds` and `positionIds`
    replaced; a `None` leaves the key out."""
    market = copy.deepcopy(base)
    for key, value in (
        ("version", version),
        ("clobTokenIds", clob_token_ids),
        ("positionIds", position_ids),
    ):
        if value is None:
            market.pop(key, None)
        else:
            market[key] = value
    return market


def select_pair(market: dict):
    """py-sdk's `(yes, no)` pair for a market, or the exception it refused with."""
    config = PRODUCTION_CONFIG
    try:
        context = normalize_market_position_context(
            Market.model_validate(market),
            context="selector vector",
            collateral_adapter=EvmAddress(config.collateral_adapter),
            neg_risk_collateral_adapter=EvmAddress(config.neg_risk_collateral_adapter),
            conditional_tokens=EvmAddress(config.conditional_tokens),
            neg_risk_adapter=EvmAddress(config.neg_risk_adapter),
            position_manager=EvmAddress(config.position_manager),
        )
    except (UnexpectedResponseError, ValidationError) as error:
        return None, error
    return [str(token_id) for token_id in context.token_ids], None


def selector_vectors(ids: dict, sibling: str) -> dict:
    base = first_gamma_market()
    clob = base["clobTokenIds"]
    v1_pair = json.loads(clob)
    # `positionIds` is a real JSON array, as gamma serves it; `clobTokenIds` stays text.
    positions = [ids["v2_binary"], sibling]
    one_position = [ids["v2_binary"]]
    # (case, version, clobTokenIds, positionIds, expected pair or the refusal's type)
    cases = (
        ("v1_with_position_ids", "v1", clob, positions, v1_pair),
        ("v2_with_both", "v2", clob, positions, [ids["v2_binary"], sibling]),
        ("missing_version", None, clob, positions, UnexpectedResponseError),
        ("unknown_version", "v3", clob, positions, ValidationError),
        ("v2_one_position_id", "v2", clob, one_position, UnexpectedResponseError),
        ("v1_no_clob_token_ids", "v1", None, positions, UnexpectedResponseError),
    )
    rows = []
    for name, version, clob_token_ids, position_ids, expected in cases:
        market = market_with(
            base, version=version, clob_token_ids=clob_token_ids, position_ids=position_ids
        )
        pair, error = select_pair(market)
        # Guards, never recorded: a case must be refused for the reason it exists, or an
        # unrelated validation failure would turn every case into a vacuous `null`.
        if isinstance(expected, list):
            if pair != expected:
                sys.exit(f"{name}: py-sdk chose {pair} ({error!r}), expected {expected}")
        else:
            if not isinstance(error, expected):
                sys.exit(f"{name}: py-sdk gave {pair} / {error!r}, expected a {expected.__name__}")
            if isinstance(error, ValidationError) and not any(
                item["loc"] == ("version",) for item in error.errors()
            ):
                sys.exit(f"{name}: refused, but not on the `version` field: {error}")
        rows.append({"name": name, "market": market, "ids": pair})
    return {"cases": rows}


# --- PROVENANCE.md -----------------------------------------------------------------

# The canonical PEP 723 block-extraction regex, straight from the spec.
_PEP_723_BLOCK_RE = re.compile(
    r"(?m)^# /// (?P<type>[a-zA-Z0-9-]+)$\s(?P<content>(^#(| .*)$\s)+)^# ///$"
)


def read_dependency_cutoff() -> str:
    """Read `[tool.uv].exclude-newer` from this file's own PEP 723 metadata block, so
    PROVENANCE.md states the pin that governs resolution, not a second literal."""
    match = _PEP_723_BLOCK_RE.search(Path(__file__).read_text())
    content = "".join(
        line[2:] if line.startswith("# ") else line[1:]
        for line in match.group("content").splitlines(keepends=True)
    )
    return tomllib.loads(content)["tool"]["uv"]["exclude-newer"]


def commands_table(*fixtures: str) -> str:
    """The Fixture/Command table, built by `capture_common.provenance_table`."""
    rows = [(f"`{name}`", f"`{COMMAND}`") for name in fixtures]
    return "\n".join(
        capture_common.provenance_table(("Fixture", "Command"), rows, short_rule=True)
    )


def reproducibility(sdk_version: str, cutoff: str, day: str) -> str:
    return (
        "Generated by `scripts/capture_polymarket_protocol_v2.py` (run:\n"
        f"`{COMMAND}`)\n"
        f"on {day} against Polymarket's official Python SDK, "
        f"`polymarket-client=={sdk_version}`, not a live\n"
        f"host. Dependency resolution is pinned to `exclude-newer = {cutoff}` (the\n"
        "script's own PEP 723 metadata), so a rerun reproduces byte-identical output\n"
        "until that cutoff or the pinned `polymarket-client` version changes; only the\n"
        "date above moves. Every recorded choice is the return value of py-sdk's own\n"
        "function for the case's inputs; the script decides none. Anvil key #0 signs; the\n"
        "session signer is Anvil address #1; the Deposit Wallet is the one the\n"
        "`session_keys` fixtures use."
    )


def clob_provenance(sdk_version: str, cutoff: str, day: str) -> str:
    return f"""# Protocol V2 CLOB vectors

{reproducibility(sdk_version, cutoff, day)}

`id_vectors.json`: the four ids (`v1` is the first `clobTokenIds` entry of
`tests/fixtures/clob/venue/gamma_markets.json`'s first market; `v2_binary`, `v2_neg_risk`
and `v2_combo` are kept only if py-sdk's `is_v2_position_id` and
`decode_v2_outcome_position_id` accept them), then one row per probed input with both of
py-sdk's answers (`is_v2_position_id`, and the decode, or `null` when it refuses), then
`resolve_order_balance_allowance_target` for BUY and SELL against `v1` and `v2_binary`.
py-sdk's bare reserved-bit test is true for `"1"`, `"100"` and `"123456"`, which its
decode refuses: those rows are why polyoxide's rule is the conjunction of the two. The
last two rows are `v2_binary` with bit 40 and with bit 104 set, the edges of the reserved
bit range.

`order_vectors.json`: six orders. Each is an `OrderDraft` whose exchange is
`resolve_order_exchange_address`'s choice (`neg_risk` is what the script passed to it),
turned into an order by `create_unsigned_order` (which sets `protocol_version`), with salt
and timestamp fixed afterwards. `digest` is what the key signs; `signature` is what is
posted (`build_order_signature`'s output; for the session key, wrapped by
`wrap_deposit_wallet_session_signer_signature`). As in the `session_keys` fixtures, the
session-signer envelope names Anvil address #1 but Anvil key #0 produced the inner
signature: a byte-pinning vector, not a signature a real session key would produce.

{commands_table("id_vectors.json", "order_vectors.json")}
"""


def relay_provenance(sdk_version: str, cutoff: str, day: str) -> str:
    return f"""# Protocol V2 Router vectors

{reproducibility(sdk_version, cutoff, day)}

`router_vectors.json`: `split`, `merge` and `redeem` hold the `.data` of py-sdk's
`router_split_call`, `router_merge_call` and `router_redeem_call` called with
`router=` py-sdk's `protocol_v2_router`. `condition_ids` holds
`_protocol_v2_condition_id_bytes` over accepted and refused inputs (`bytes31` is `null`
when it raises). `deposit_wallet_batch` is `build_deposit_wallet_typed_data`'s EIP-712
digest for a batch of the one redeem call at outcome 0. Generation also checks that
`router_redeem_call` refuses outcome index 2; the refusal is not recorded here, because
the id table in the `clob` fixtures records the outcome-byte-2 position id.

{commands_table("router_vectors.json")}
"""


def gamma_provenance(sdk_version: str, cutoff: str, day: str) -> str:
    return f"""# Protocol V2 outcome-pair selector vectors

{reproducibility(sdk_version, cutoff, day)}

`selector_vectors.json`: each `market` is the first market of
`tests/fixtures/clob/venue/gamma_markets.json` with only `version`, `clobTokenIds` and
`positionIds` replaced (a missing key is a key left out). `ids` is the `(yes, no)` pair
that py-sdk's `normalize_market_position_context` returns for `Market.model_validate` of
that market, or `null` when either raises. Generation checks that each refusal is the one
the case exists for (an `UnexpectedResponseError`, or a validation error on `version`).

{commands_table("selector_vectors.json")}
"""


# --- main --------------------------------------------------------------------------


def emit_json(path: Path, value) -> None:
    capture_common.write_json(path, value, ensure_ascii=True)
    print(f"wrote {path}")


def emit_provenance(out_dir: Path, text: str) -> None:
    print(f"wrote {capture_common.write_provenance(out_dir, text)}")


def main(clob_dir: Path, relay_dir: Path, gamma_dir: Path) -> None:
    signer = Account.from_key(ANVIL_KEY_0)
    check_environment(signer)
    sdk_version = importlib.metadata.version("polymarket-client")
    if sdk_version != "0.12.0":
        sys.exit(f"polymarket-client is {sdk_version}, expected 0.12.0")
    cutoff = read_dependency_cutoff()
    day = capture_common.stamp("%Y-%m-%d")

    # --- compute everything first; nothing is written until every guard passed. ---
    ids, sibling = build_ids()
    ids_out = id_vectors(ids)
    orders_out = order_vectors(signer, ids)
    router_out = router_vectors(ids, sibling)
    selector_out = selector_vectors(ids, sibling)

    emit_json(clob_dir / "order_vectors.json", orders_out)
    emit_json(clob_dir / "id_vectors.json", ids_out)
    emit_provenance(clob_dir, clob_provenance(sdk_version, cutoff, day))
    emit_json(relay_dir / "router_vectors.json", router_out)
    emit_provenance(relay_dir, relay_provenance(sdk_version, cutoff, day))
    emit_json(gamma_dir / "selector_vectors.json", selector_out)
    emit_provenance(gamma_dir, gamma_provenance(sdk_version, cutoff, day))


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]))
```

If an import fails (py-sdk moved a private module between the versions the story was read against and the pin), fix the import path only; a name that no longer exists is a stop: report it, never replace the call with a value the script computes.

- [ ] **Step 4: Run it to see it pass**

```bash
bash -c 'd=$(mktemp -d) && uv run scripts/capture_polymarket_protocol_v2.py $d/clob $d/relay $d/gamma'
```

Expected: exit 0 and seven lines, `wrote <d>/clob/order_vectors.json`, `wrote <d>/clob/id_vectors.json`, `wrote <d>/clob/PROVENANCE.md`, `wrote <d>/relay/router_vectors.json`, `wrote <d>/relay/PROVENANCE.md`, `wrote <d>/gamma/selector_vectors.json`, `wrote <d>/gamma/PROVENANCE.md`. A guard that trips prints its message to stderr, exits 1 and writes nothing; read the message before changing anything.

- [ ] **Step 5: Commit**

```bash
git add -N scripts/capture_polymarket_protocol_v2.py
git commit -m "test(polymarket): capture script for Protocol V2 golden vectors from py-sdk 0.12.0 (#51) (protocol v2, Story 1)" -- scripts/capture_polymarket_protocol_v2.py
```

### Task 1.2: Capture the fixtures and check them

**Files:**
- Create: `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/order_vectors.json`
- Create: `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/id_vectors.json`
- Create: `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/PROVENANCE.md`
- Create: `polyoxide-polymarket/tests/fixtures/relay/protocol_v2/router_vectors.json`
- Create: `polyoxide-polymarket/tests/fixtures/relay/protocol_v2/PROVENANCE.md`
- Create: `polyoxide-polymarket/tests/fixtures/gamma/protocol_v2/selector_vectors.json`
- Create: `polyoxide-polymarket/tests/fixtures/gamma/protocol_v2/PROVENANCE.md`

- [ ] **Step 1: Write the failing check**

The check reads the committed fixtures back against what py-sdk's source says each row must be. These expectations are read off py-sdk 0.12.0's code (`_internal/protocol.py`, `orders/context.py`, `orders/allowance.py`, `relayer/calls.py`, `relayer/positions.py`), not off the script, so a script that decided a value would fail them. Save it as `/tmp/check_protocol_v2_fixtures.py` outside the repo (a scratch file; do not add it to the tree):

```python
import json
import sys
from pathlib import Path

root = Path(sys.argv[1]) / "polyoxide-polymarket/tests/fixtures"
clob = root / "clob/protocol_v2"
relay = root / "relay/protocol_v2"
gamma = root / "gamma/protocol_v2"
EXCHANGE_V3 = "0xe3333700cA9d93003F00f0F71f8515005F6c00Aa"

ids_file = json.loads((clob / "id_vectors.json").read_text())
ids = ids_file["ids"]
assert list(ids) == ["v1", "v2_binary", "v2_neg_risk", "v2_combo"], list(ids)

rows = ids_file["position_ids"]
assert len(rows) == 20, len(rows)
binary_decoded = rows[1]["decoded"]
assert binary_decoded["outcome_index"] == 0 and len(binary_decoded["condition_id"]) == 64
assert binary_decoded["condition_id"] == binary_decoded["condition_id"].lower()
want_v2 = [False, True, True, True, True, True, True, True, True, True, True,
           False, False, False, False, True, True, True, False, True]
assert [r["is_v2_position_id"] for r in rows] == want_v2, [r["is_v2_position_id"] for r in rows]
# decoded is null for the toy ids, the refused spellings and the three malformed V2 shapes
null_rows = {0, 4, 5, 6, 11, 12, 13, 14, 15, 16, 17}
assert {i for i, r in enumerate(rows) if r["decoded"] is None} == null_rows
# rows 18 and 19 are the mask's edges (v2_binary with bit 40 and with bit 104 set): the
# first is refused by the reserved-bit test yet decodes; the second is accepted by both
assert int(rows[18]["input"]) == int(ids["v2_binary"]) | (1 << 40)
assert int(rows[19]["input"]) == int(ids["v2_binary"]) | (1 << 104)
assert rows[18]["is_v2_position_id"] is False and rows[18]["decoded"] is not None
assert rows[19]["is_v2_position_id"] is True and rows[19]["decoded"] is not None
# the spellings and the padded id decode like v2_binary itself
for i in (7, 8, 9, 10):
    assert rows[i]["decoded"] == binary_decoded, i
assert rows[2]["decoded"]["outcome_index"] == 0 and rows[3]["decoded"]["outcome_index"] == 0

targets = ids_file["balance_allowance_targets"]
got = [(t["side"], t["asset_id"] == ids["v2_binary"], t["asset_type"], t["asset_id_sent"]) for t in targets]
assert got == [
    ("BUY", False, "COLLATERAL", None),
    ("BUY", True, "COLLATERAL", None),
    ("SELL", False, "CONDITIONAL", ids["v1"]),
    ("SELL", True, "CONDITIONAL-V2", ids["v2_binary"]),
], got

orders = json.loads((clob / "order_vectors.json").read_text())
assert list(orders["cases"]) == [
    "v2_eoa", "v2_neg_risk_eoa", "v2_deposit_wallet_owner",
    "v2_deposit_wallet_session", "v1_eoa", "v1_neg_risk_eoa",
]
for name, case in orders["cases"].items():
    v2 = name.startswith("v2")
    assert case["version"] == ("3" if v2 else "2"), name
    assert (case["exchange"] == EXCHANGE_V3) == v2, name
    assert case["neg_risk"] == (name in ("v2_neg_risk_eoa", "v1_neg_risk_eoa")), name
    assert case["order"]["signature_type"] == (3 if "deposit_wallet" in name else 0), name
    for key in ("domain_separator", "struct_hash", "digest"):
        assert len(case[key]) == 66, (name, key)
cases = orders["cases"]
assert cases["v1_eoa"]["exchange"] != cases["v1_neg_risk_eoa"]["exchange"]
assert cases["v1_eoa"]["digest"] != cases["v1_neg_risk_eoa"]["digest"]
assert cases["v2_eoa"]["domain_separator"] == cases["v2_deposit_wallet_owner"]["domain_separator"]
# type 0 is a 65-byte signature; the owner's is longer (ERC-7739 trailer); the session's longer still
lengths = [len(cases[n]["signature"]) for n in ("v2_eoa", "v2_deposit_wallet_owner", "v2_deposit_wallet_session")]
assert lengths[0] == 132 and lengths[0] < lengths[1] < lengths[2], lengths

router = json.loads((relay / "router_vectors.json").read_text())
assert router["router"] == "0x12121212006e4CD160D18e3f00711DA5c3372600"
for key in ("split", "merge"):
    assert [r["amount"] for r in router[key]] == ["0", "1000000", str(2**256 - 1)], key
assert [(r["outcome_index"], r["amount"]) for r in router["redeem"]] == [(0, "1000000"), (1, "1000000")]
assert router["redeem"][0]["position_id"] == ids["v2_binary"]
assert router["redeem"][0]["condition_id"] == binary_decoded["condition_id"]
selectors = {key: router[key][0]["calldata"][:10] for key in ("split", "merge")}
assert selectors["split"] != selectors["merge"]
accepted = [c["bytes31"] is not None for c in router["condition_ids"]]
assert accepted == [True, True, True, True, False, False, False, False], accepted
first = router["condition_ids"][0]["bytes31"]
assert all(c["bytes31"] == first for c in router["condition_ids"][:4])
assert len(first) == 64 and "0x" + first[2:] == binary_decoded["condition_id"]
batch = router["deposit_wallet_batch"]
assert batch["calls"][0]["target"] == router["router"] and batch["calls"][0]["data"] == router["redeem"][0]["calldata"]
assert len(batch["digest"]) == 66

selector = json.loads((gamma / "selector_vectors.json").read_text())
by_name = {c["name"]: c for c in selector["cases"]}
assert list(by_name) == ["v1_with_position_ids", "v2_with_both", "missing_version",
                         "unknown_version", "v2_one_position_id", "v1_no_clob_token_ids"]
v1_pair = json.loads(by_name["v1_with_position_ids"]["market"]["clobTokenIds"])
assert by_name["v1_with_position_ids"]["ids"] == v1_pair
assert by_name["v2_with_both"]["ids"] == [ids["v2_binary"], by_name["v2_with_both"]["market"]["positionIds"][1]]
for name in ("missing_version", "unknown_version", "v2_one_position_id", "v1_no_clob_token_ids"):
    assert by_name[name]["ids"] is None, name
assert isinstance(by_name["v2_with_both"]["market"]["positionIds"], list)
assert isinstance(by_name["v2_with_both"]["market"]["clobTokenIds"], str)
assert "version" not in by_name["missing_version"]["market"]
assert by_name["unknown_version"]["market"]["version"] == "v3"
assert "clobTokenIds" not in by_name["v1_no_clob_token_ids"]["market"]

for d in (clob, relay, gamma):
    text = (d / "PROVENANCE.md").read_text()
    assert "polymarket-client==0.12.0" in text and "capture_polymarket_protocol_v2.py" in text, d
print("fixtures agree with py-sdk 0.12.0's source")
```

Run it before the fixtures exist:

```bash
python3 -I /tmp/check_protocol_v2_fixtures.py "$PWD"
```

- [ ] **Step 2: Run it to see it fail**

Expected: it fails with `FileNotFoundError: ... polyoxide-polymarket/tests/fixtures/clob/protocol_v2/id_vectors.json`, because the fixtures have not been captured yet.

- [ ] **Step 3: Implement**

Run the capture into the three fixture directories (the same command the `PROVENANCE.md` files record):

```bash
uv run scripts/capture_polymarket_protocol_v2.py polyoxide-polymarket/tests/fixtures/clob/protocol_v2 polyoxide-polymarket/tests/fixtures/relay/protocol_v2 polyoxide-polymarket/tests/fixtures/gamma/protocol_v2
```

Expected: the seven `wrote ...` lines of Task 1.1 Step 4, now under the three fixture directories. Do not edit any generated file by hand. If the check in Step 4 fails on a row, the row is py-sdk's answer: re-read the py-sdk source cited in Step 1 before concluding the expectation or the script is wrong, and report a disagreement you cannot resolve instead of adjusting either side to match.

- [ ] **Step 4: Run it to see it pass**

```bash
python3 -I /tmp/check_protocol_v2_fixtures.py "$PWD"
```

Expected: `fixtures agree with py-sdk 0.12.0's source`. Then the story's done-check, which reruns the capture into a scratch directory and diffs it against the committed files, leaving out each `PROVENANCE.md`, which carries the capture day:

```bash
bash -c 'd=$(mktemp -d) && uv run scripts/capture_polymarket_protocol_v2.py $d/clob $d/relay $d/gamma && diff -r -x PROVENANCE.md $d/clob polyoxide-polymarket/tests/fixtures/clob/protocol_v2 && diff -r -x PROVENANCE.md $d/relay polyoxide-polymarket/tests/fixtures/relay/protocol_v2 && diff -r -x PROVENANCE.md $d/gamma polyoxide-polymarket/tests/fixtures/gamma/protocol_v2'
```

Expected: the seven `wrote` lines and no diff output (exit 0). Then remove the scratch check, `rm /tmp/check_protocol_v2_fixtures.py`. Finally run the Python CI checks that scan `scripts/` and the fixtures tree: `bash -c 'cd .github/scripts && uv run pytest tests/test_capture_common.py tests/test_claude_rules.py -q'`, which must still pass.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-polymarket/tests/fixtures/clob/protocol_v2 polyoxide-polymarket/tests/fixtures/relay/protocol_v2 polyoxide-polymarket/tests/fixtures/gamma/protocol_v2
git commit -m "test(polymarket): Protocol V2 golden vectors captured from py-sdk 0.12.0 (#51) (protocol v2, Story 1)" -- polyoxide-polymarket/tests/fixtures/clob/protocol_v2 polyoxide-polymarket/tests/fixtures/relay/protocol_v2 polyoxide-polymarket/tests/fixtures/gamma/protocol_v2
```

### Task 1.3: Name the script in the capture-script rules

**Files:**
- Modify: `.claude/rules/testing.md:62-70`

- [ ] **Step 1: Write the failing check**

The `## Capture scripts` section lists the PEP 723 `uv run` scripts; the new script must be among them.

```bash
grep -c 'scripts/capture_polymarket_protocol_v2.py' .claude/rules/testing.md
```

- [ ] **Step 2: Run it to see it fail**

Expected: it prints `0` and exits 1: the rules file does not name the script yet.

- [ ] **Step 3: Implement**

In `.claude/rules/testing.md`, replace the last sentence of the `## Capture scripts` section, currently

```
`python3 -I`, which keeps `scripts/` off `sys.path`. `scripts/capture_binance_usdm.py` and
`scripts/capture_binance_spot.py` are PEP 723 `uv run` scripts.
```

with

```
`python3 -I`, which keeps `scripts/` off `sys.path`. `scripts/capture_binance_usdm.py`,
`scripts/capture_binance_spot.py` and `scripts/capture_polymarket_protocol_v2.py` are PEP 723
`uv run` scripts.
```

- [ ] **Step 4: Run it to see it pass**

```bash
grep -c 'scripts/capture_polymarket_protocol_v2.py' .claude/rules/testing.md
bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'
```

Expected: `1`, then the rules-file tests pass.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(rules): name the Protocol V2 capture script among the PEP 723 scripts (#51) (protocol v2, Story 1)" -- .claude/rules/testing.md
```

## Story 2: CLOB: the id rule, ExchangeV3 signing, the `/neg-risk` skip and V2 balances

- **Developer:** intermediate-dev
- **Files:** `polyoxide-polymarket/src/shared/protocol.rs`, `polyoxide-polymarket/src/shared/mod.rs`, `polyoxide-polymarket/src/clob/core/chain.rs`, `polyoxide-polymarket/src/clob/core/eip712.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/api/account.rs`, `polyoxide-polymarket/src/clob/types.rs`, `polyoxide-polymarket/src/clob/venue/trading.rs`, `polyoxide-polymarket/tests/mock_clob.rs`, `polyoxide-polymarket/tests/trading_clob.rs`, `.claude/rules/polymarket-clob.md`, `.claude/rules/session-keys.md`, `docs/specs/clob/auth.md`, `docs/specs/session-keys/OBSERVED.md`, `docs/specs/clob/orders.md`, `docs/specs/session-keys/README.md`
- **Interfaces:** exposes "Story 2 → Stories 3, 4 and 5"; uses Story 1's `id_vectors.json` and `order_vectors.json`
- **Verify:** `bash -c 'cargo test -p polyoxide-polymarket --all-features && cargo test -p polyoxide-polymarket --no-default-features --features clob --lib && cargo test -p polyoxide-polymarket --no-default-features --features clob --test mock_clob && python3 scripts/test_body_diff.py --baseline v0.40.0 && cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'`
- **Review:** deep (order signing: a wrong domain posts orders the exchange rejects, or signs for the wrong contract)

The CLOB learns which protocol an order is from its token id alone, as py-sdk 0.12.0 does. `shared::protocol` (crate-private, under `clob` for now) ports py-sdk's id parse and decode, and `is_v2_position_id` is true only when py-sdk's reserved-bit test and its decode both accept the id, so toy ids (`1`, `100`, `123456`) stay V1 and every pinned baseline test is untouched. `Chain::exchange_v3`, `order_exchange` and `order_domain` route `order_hash`, `sign_order` and `sign_order_as` (the type-3 envelope included) to ExchangeV3 at domain version `"3"` for a V2 id, and to the CTF exchanges at `"2"` as before for any other; `exchange_address`, `exchange_domain` and `envelope_digest` keep their signatures and meanings. `get_market_metadata` skips `/neg-risk` for a V2 id, and `balance_allowance` sends `CONDITIONAL-V2`. Every digest, signature, id classification and asset type the new tests assert is read from Story 1's fixtures. The story touches no landmine marker. Four tasks change library code and are followed by one task that appends the integration tests of `trading_clob.rs` and one that edits the rules and spec docs (no test fails first in a docs task; the rules test is its check).

### Task 2.1: `shared::protocol`, the id rule

**Files:**
- Create: `polyoxide-polymarket/src/shared/protocol.rs`
- Modify: `polyoxide-polymarket/src/shared/mod.rs:31-37`
- Test: `polyoxide-polymarket/src/shared/protocol.rs`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-polymarket/src/shared/protocol.rs` holding only this test module (Step 3 adds the code above it):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::hex;

    const VECTORS: &str = include_str!("../../tests/fixtures/clob/protocol_v2/id_vectors.json");

    #[derive(serde::Deserialize)]
    struct Vectors {
        ids: Ids,
        position_ids: Vec<Row>,
    }

    #[derive(serde::Deserialize)]
    struct Ids {
        v1: String,
        v2_binary: String,
        v2_neg_risk: String,
        v2_combo: String,
    }

    /// One input and both of py-sdk's answers about it.
    #[derive(serde::Deserialize)]
    struct Row {
        input: String,
        is_v2_position_id: bool,
        decoded: Option<Decoded>,
    }

    #[derive(serde::Deserialize)]
    struct Decoded {
        condition_id: String,
        outcome_index: u8,
    }

    fn vectors() -> Vectors {
        serde_json::from_str(VECTORS).unwrap()
    }

    #[test]
    fn the_table_holds_the_rows_the_rule_depends_on() {
        // Not a test of production code: it proves the fixture can tell the
        // rule from py-sdk's bare mask, so matching it means something.
        let rows = vectors().position_ids;
        assert!(rows.len() >= 20, "{} rows", rows.len());
        let mask_only = rows
            .iter()
            .filter(|r| r.is_v2_position_id && r.decoded.is_none())
            .count();
        let both = rows
            .iter()
            .filter(|r| r.is_v2_position_id && r.decoded.is_some())
            .count();
        let neither = rows.iter().filter(|r| !r.is_v2_position_id).count();
        // py-sdk's mask refuses it, its decode accepts it: the conjunction
        // makes it not V2 (the row with bit 40 set).
        let decode_only = rows
            .iter()
            .filter(|r| !r.is_v2_position_id && r.decoded.is_some())
            .count();
        assert!(mask_only >= 3, "toy ids 1, 100 and 123456: {mask_only}");
        assert!(both >= 6, "the four ids and their spellings: {both}");
        assert!(neither >= 1, "ids py-sdk calls V1: {neither}");
        assert!(decode_only >= 1, "mask refuses, decode accepts: {decode_only}");
    }

    #[test]
    fn the_rule_is_the_conjunction_of_py_sdks_two_answers() {
        for row in vectors().position_ids {
            let expected = row.is_v2_position_id && row.decoded.is_some();
            assert_eq!(is_v2_position_id(&row.input), expected, "{:?}", row.input);
        }
    }

    #[test]
    fn decode_agrees_with_py_sdk_on_every_row() {
        for row in vectors().position_ids {
            let got = decode_v2_outcome_position_id(&row.input);
            match row.decoded {
                Some(decoded) => {
                    let (condition_id, outcome_index) =
                        got.unwrap_or_else(|| panic!("refused {:?}", row.input));
                    assert_eq!(
                        format!("0x{}", hex::encode(condition_id)),
                        decoded.condition_id,
                        "{:?}",
                        row.input
                    );
                    assert_eq!(outcome_index, decoded.outcome_index, "{:?}", row.input);
                }
                None => assert!(got.is_none(), "accepted {:?}", row.input),
            }
        }
    }

    #[test]
    fn the_four_fixture_ids_classify_as_the_venue_issues_them() {
        let ids = vectors().ids;
        assert!(!is_v2_position_id(&ids.v1), "a CTF id is a keccak hash");
        assert!(is_v2_position_id(&ids.v2_binary));
        assert!(is_v2_position_id(&ids.v2_neg_risk));
        assert!(is_v2_position_id(&ids.v2_combo));
        assert!(parse_position_id(&ids.v1).is_some());
    }

    #[test]
    fn toy_ids_stay_v1_though_py_sdks_mask_alone_accepts_them() {
        // The divergence the rules files record: py-sdk's mask calls these V2,
        // but their module byte is 0, so they name no V2 position.
        let rows = vectors().position_ids;
        for toy in ["1", "100", "123456"] {
            let row = rows.iter().find(|r| r.input == toy).unwrap();
            assert!(row.is_v2_position_id, "py-sdk's mask on {toy}");
            assert!(row.decoded.is_none(), "py-sdk's decode on {toy}");
            assert!(!is_v2_position_id(toy), "{toy} must stay V1");
        }
    }
}
```

Then declare the module in `polyoxide-polymarket/src/shared/mod.rs`. Quoted current lines:

```rust
#[cfg(any(
    feature = "clob",
    feature = "gamma",
    feature = "data",
    feature = "perps"
))]
pub mod keys;
#[cfg(any(feature = "clob", feature = "relay"))]
pub mod session_signer;
```

becomes:

```rust
#[cfg(any(
    feature = "clob",
    feature = "gamma",
    feature = "data",
    feature = "perps"
))]
pub mod keys;
#[cfg(feature = "clob")]
pub(crate) mod protocol;
#[cfg(any(feature = "clob", feature = "relay"))]
pub mod session_signer;
```

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob --lib shared::protocol`

Expected: a compile error, `cannot find function `is_v2_position_id` in this scope` (and the same for `decode_v2_outcome_position_id` and `parse_position_id`).

- [ ] **Step 3: Implement**

Put this at the top of `polyoxide-polymarket/src/shared/protocol.rs`, above the `#[cfg(test)]` line:

```rust
//! Which Polymarket protocol an asset id belongs to.
//!
//! The protocol is a function of the id alone, as in py-sdk 0.12.0
//! (`_internal/protocol.py`): no lookup is made. Nothing here is public; each
//! module wraps these in its own types and errors.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Task 2.3 calls is_v2_position_id from the signing path"
    )
)]

use alloy::primitives::{FixedBytes, U256};

/// Parses an asset id as py-sdk does: whitespace trimmed, then decimal or a
/// `0x`, `0b` or `0o` prefix (any case), and a value that fits a uint256.
/// Anything else is `None`.
pub(crate) fn parse_position_id(id: &str) -> Option<U256> {
    let text = id.trim();
    let (digits, radix) = match text.get(..2) {
        Some(prefix) if prefix.eq_ignore_ascii_case("0x") => (&text[2..], 16_u32),
        Some(prefix) if prefix.eq_ignore_ascii_case("0b") => (&text[2..], 2),
        Some(prefix) if prefix.eq_ignore_ascii_case("0o") => (&text[2..], 8),
        _ => (text, 10),
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    U256::from_str_radix(digits, u64::from(radix)).ok()
}

/// Splits a V2 position id into its condition id and outcome index.
///
/// The 32-byte big-endian value's first byte is the module (1 binary, 2
/// neg-risk, 3 combinatorial), its last byte the outcome index (0 or 1), and
/// its first 31 bytes the condition id. `None` for any refusal.
pub(crate) fn decode_v2_outcome_position_id(id: &str) -> Option<(FixedBytes<31>, u8)> {
    let bytes = parse_position_id(id)?.to_be_bytes::<32>();
    if !matches!(bytes[0], 1..=3) || bytes[31] > 1 {
        return None;
    }
    Some((FixedBytes::<31>::from_slice(&bytes[..31]), bytes[31]))
}

/// Whether an id names a Protocol V2 position: py-sdk's `is_v2_position_id`
/// (bits 40 to 103 all clear) **and** `decode_v2_outcome_position_id` both
/// accept it.
///
/// One condition stricter than py-sdk's mask, which also calls toy ids such as
/// `1`, `100` and `123456` V2. Those name no V2 position (their module byte is
/// 0) and are the ids older tests sign with. It agrees with py-sdk on every id
/// a market can carry: a V1 id is a keccak hash, so its bits 40 to 103 are not
/// all clear, and a V2 id has module byte 1 to 3 and outcome byte 0 or 1.
pub(crate) fn is_v2_position_id(id: &str) -> bool {
    let Some(value) = parse_position_id(id) else {
        return false;
    };
    let reserved = U256::from(u64::MAX) << 40;
    (value & reserved) == U256::ZERO && decode_v2_outcome_position_id(id).is_some()
}

```

The temporary `#![cfg_attr(not(test), expect(dead_code, …))]` is there because nothing outside the tests calls these yet; Task 2.3 deletes it.

Run `direnv exec . cargo fmt --all` and keep its result.

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob --lib shared::protocol`

Expected: `test result: ok. 5 passed`. Then `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings` prints no warning.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-polymarket/src/shared/protocol.rs
git commit -m "feat(polymarket): the Protocol V2 id rule in shared::protocol (protocol v2, Story 2)" -- polyoxide-polymarket/src/shared/protocol.rs polyoxide-polymarket/src/shared/mod.rs
```

### Task 2.2: `Chain::exchange_v3`

**Files:**
- Modify: `polyoxide-polymarket/src/clob/core/chain.rs:32-38` and the end of `mod tests`
- Test: `polyoxide-polymarket/src/clob/core/chain.rs`

- [ ] **Step 1: Write the failing test**

Append after the last test of `mod tests` in `polyoxide-polymarket/src/clob/core/chain.rs`. Quoted current end of the file:

```rust
    #[test]
    fn test_chain_equality() {
        assert_eq!(Chain::PolygonMainnet, Chain::PolygonMainnet);
        assert_eq!(Chain::PolygonAmoy, Chain::PolygonAmoy);
        assert_ne!(Chain::PolygonMainnet, Chain::PolygonAmoy);
    }
}
```

becomes:

```rust
    #[test]
    fn test_chain_equality() {
        assert_eq!(Chain::PolygonMainnet, Chain::PolygonMainnet);
        assert_eq!(Chain::PolygonAmoy, Chain::PolygonAmoy);
        assert_ne!(Chain::PolygonMainnet, Chain::PolygonAmoy);
    }

    #[test]
    fn exchange_v3_is_py_sdks_mainnet_address_and_absent_on_amoy() {
        // py-sdk 0.12.0's `environments.py` (production only); it knows no
        // Amoy Protocol V2 deployment.
        assert_eq!(
            Chain::PolygonMainnet.exchange_v3(),
            Some(address!("e3333700cA9d93003F00f0F71f8515005F6c00Aa"))
        );
        assert_eq!(Chain::PolygonAmoy.exchange_v3(), None);
        // It is not one of the CTF exchanges.
        let ctf = Contracts::POLYGON_MAINNET;
        assert_ne!(Chain::PolygonMainnet.exchange_v3(), Some(ctf.exchange));
        assert_ne!(
            Chain::PolygonMainnet.exchange_v3(),
            Some(ctf.neg_risk_exchange)
        );
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob --lib clob::core::chain`

Expected: compile error `no method named `exchange_v3` found for enum `Chain``.

- [ ] **Step 3: Implement**

In the `impl Chain` block, after `from_chain_id`. Quoted current lines:

```rust
    /// Create Chain from chain ID
    pub const fn from_chain_id(chain_id: u64) -> Option<Self> {
        match chain_id {
            137 => Some(Self::PolygonMainnet),
            80002 => Some(Self::PolygonAmoy),
            _ => None,
        }
    }
}
```

becomes:

```rust
    /// Create Chain from chain ID
    pub const fn from_chain_id(chain_id: u64) -> Option<Self> {
        match chain_id {
            137 => Some(Self::PolygonMainnet),
            80002 => Some(Self::PolygonAmoy),
            _ => None,
        }
    }

    /// The Protocol V2 exchange (ExchangeV3) on this chain, where one is
    /// deployed.
    ///
    /// Polygon mainnet has it. `None` on Amoy, where no Protocol V2
    /// deployment is known. It is not a field of [`Contracts`], whose fields
    /// are all public.
    pub const fn exchange_v3(&self) -> Option<Address> {
        match self {
            Chain::PolygonMainnet => Some(address!("e3333700cA9d93003F00f0F71f8515005F6c00Aa")),
            Chain::PolygonAmoy => None,
        }
    }
}
```

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob --lib clob::core::chain`

Expected: `test result: ok. 9 passed` (the eight chain tests and the new one).

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(clob): Chain::exchange_v3 (protocol v2, Story 2)" -- polyoxide-polymarket/src/clob/core/chain.rs
```

### Task 2.3: `order_exchange`, `order_domain` and the signing paths

**Files:**
- Modify: `polyoxide-polymarket/src/clob/core/eip712.rs:8-13`, `:204-212`, `:243-248`, `:275-322`, `:408-411`, and the end of the file
- Modify: `polyoxide-polymarket/src/shared/protocol.rs` (drop the temporary attribute)
- Test: `polyoxide-polymarket/src/clob/core/eip712.rs`

- [ ] **Step 1: Write the failing test**

Append a new module at the very end of `polyoxide-polymarket/src/clob/core/eip712.rs`, after `mod order_hash_vectors` (its last test is `the_hash_follows_the_orders_exchange_and_refuses_an_unsupported_chain`). No existing test is edited.

```rust
/// Protocol V2 order signing, pinned to
/// `tests/fixtures/clob/protocol_v2/order_vectors.json`, which
/// `scripts/capture_polymarket_protocol_v2.py` takes from py-sdk 0.12.0: the
/// exchange, domain version, domain separator, struct hash, digest and
/// signature of each case are py-sdk's own choices and results. The V1 cases
/// are the control: they must still sign at version `"2"`.
#[cfg(test)]
mod protocol_v2_vectors {
    use std::collections::BTreeMap;

    use super::*;
    use crate::clob::core::chain::Contracts;
    use crate::clob::types::OrderSide;
    use alloy::signers::local::PrivateKeySigner;
    use polyoxide_venue::{Class, Classify};

    const VECTORS: &str =
        include_str!("../../../tests/fixtures/clob/protocol_v2/order_vectors.json");
    const ANVIL_KEY_0: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    #[derive(serde::Deserialize)]
    struct File {
        chain_id: u64,
        signer_address: Address,
        session_signer: Address,
        deposit_wallet: Address,
        cases: BTreeMap<String, Case>,
    }

    #[derive(serde::Deserialize)]
    struct Case {
        neg_risk: bool,
        order: Wire,
        exchange: Address,
        version: String,
        domain_separator: B256,
        struct_hash: B256,
        digest: B256,
        signature: String,
    }

    #[derive(serde::Deserialize)]
    struct Wire {
        salt: String,
        maker: Address,
        signer: Address,
        token_id: String,
        maker_amount: String,
        taker_amount: String,
        side: String,
        signature_type: u8,
        timestamp: String,
        expiration: String,
        metadata: B256,
        builder: B256,
    }

    fn file() -> File {
        serde_json::from_str(VECTORS).unwrap()
    }

    /// The vector's order as the native type, `neg_risk` as py-sdk was asked.
    fn native(case: &Case) -> ClobOrder {
        let wire = &case.order;
        ClobOrder {
            salt: wire.salt.clone(),
            maker: wire.maker,
            signer: wire.signer,
            token_id: wire.token_id.clone(),
            maker_amount: wire.maker_amount.clone(),
            taker_amount: wire.taker_amount.clone(),
            side: match wire.side.as_str() {
                "BUY" => OrderSide::Buy,
                "SELL" => OrderSide::Sell,
                other => panic!("unknown side {other}"),
            },
            expiration: wire.expiration.clone(),
            signature_type: match wire.signature_type {
                0 => SignatureType::Eoa,
                1 => SignatureType::PolyProxy,
                2 => SignatureType::PolyGnosisSafe,
                3 => SignatureType::Poly1271,
                other => panic!("unknown signature type {other}"),
            },
            timestamp: wire.timestamp.clone(),
            metadata: wire.metadata,
            builder: wire.builder,
            neg_risk: case.neg_risk,
        }
    }

    fn named(name: &str) -> ClobOrder {
        native(&file().cases[name])
    }

    #[test]
    fn the_vectors_hold_the_six_cases_the_contract_names() {
        let file = file();
        let mut names: Vec<&str> = file.cases.keys().map(String::as_str).collect();
        names.sort_unstable();
        let mut expected = vec![
            "v1_eoa",
            "v1_neg_risk_eoa",
            "v2_deposit_wallet_owner",
            "v2_deposit_wallet_session",
            "v2_eoa",
            "v2_neg_risk_eoa",
        ];
        expected.sort_unstable();
        assert_eq!(names, expected);
        assert_eq!(file.chain_id, 137);
        // The case that proves `neg_risk` is ignored for a V2 id is only that
        // if the script really passed `true`.
        assert!(file.cases["v2_neg_risk_eoa"].neg_risk);
        assert!(!file.cases["v2_eoa"].neg_risk);
    }

    #[test]
    fn each_case_signs_at_the_version_and_exchange_py_sdk_chose() {
        let file = file();
        for (name, case) in &file.cases {
            let (exchange, version) = order_exchange(file.chain_id, &native(case)).unwrap();
            assert_eq!(exchange, case.exchange, "{name} exchange");
            assert_eq!(version, case.version, "{name} version");
        }
        let cases = &file.cases;
        // A V2 id settles on ExchangeV3 whatever `neg_risk` says; a V1 id
        // follows `neg_risk` between the two CTF exchanges.
        assert_eq!(cases["v2_eoa"].exchange, cases["v2_neg_risk_eoa"].exchange);
        assert_eq!(cases["v2_eoa"].version, "3");
        assert_ne!(cases["v1_eoa"].exchange, cases["v1_neg_risk_eoa"].exchange);
        assert_eq!(cases["v1_eoa"].version, "2");
        assert_eq!(cases["v1_neg_risk_eoa"].version, "2");
    }

    #[test]
    fn the_domain_separator_and_struct_hash_match_py_sdk() {
        let file = file();
        for (name, case) in &file.cases {
            let order = native(case);
            let (exchange, version) = order_exchange(file.chain_id, &order).unwrap();
            assert_eq!(
                order_domain(file.chain_id, exchange, version).eip712_hash_struct(),
                case.domain_separator,
                "{name} domain separator"
            );
            assert_eq!(
                order_to_protocol(&order).unwrap().eip712_hash_struct(),
                case.struct_hash,
                "{name} struct hash"
            );
        }
    }

    #[test]
    fn the_digest_is_what_py_sdk_signs_and_the_order_hash_is_its_id() {
        let file = file();
        for (name, case) in &file.cases {
            let order = native(case);
            if order.signature_type == SignatureType::Poly1271 {
                // The envelope digest, with ExchangeV3 as the app domain.
                let digest = deposit_wallet::envelope_digest(
                    &order,
                    file.chain_id,
                    case.exchange,
                    file.deposit_wallet,
                )
                .unwrap();
                assert_eq!(digest, case.digest, "{name} envelope digest");
            } else {
                assert_eq!(
                    compute_order_digest(&order, file.chain_id).unwrap(),
                    case.digest,
                    "{name} order digest"
                );
            }
            // The exchange names every order, type 3 included, by the hash of
            // its struct under the exchange domain.
            assert_eq!(
                order_hash(&order, file.chain_id).unwrap(),
                eip712_digest(case.domain_separator, case.struct_hash),
                "{name} order hash"
            );
        }
    }

    #[tokio::test]
    async fn plain_cases_reproduce_py_sdks_signatures() {
        let file = file();
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        assert_eq!(signer.address(), file.signer_address);
        for name in ["v2_eoa", "v2_neg_risk_eoa", "v1_eoa", "v1_neg_risk_eoa"] {
            let case = &file.cases[name];
            let signature = sign_order(&native(case), &signer, file.chain_id)
                .await
                .unwrap();
            assert_eq!(signature, case.signature.to_lowercase(), "{name}");
        }
    }

    #[tokio::test]
    async fn a_v2_deposit_wallet_owner_reproduces_py_sdks_signature() {
        let file = file();
        let case = &file.cases["v2_deposit_wallet_owner"];
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        let target = SigningTarget::DepositWallet {
            wallet: file.deposit_wallet,
            role: DepositWalletRole::Owner,
        };
        let signature = sign_order_as(&native(case), &signer, file.chain_id, &target)
            .await
            .unwrap();
        assert_eq!(signature, case.signature.to_lowercase());
    }

    #[tokio::test]
    async fn a_v2_deposit_wallet_session_key_reproduces_py_sdks_signature() {
        // Anvil key #0 signs while the vector's envelope names the session
        // signer (Anvil #1), so the envelope differs from the vector only in
        // the 32-byte signer id, as in `deposit_wallet_vectors`.
        let file = file();
        let case = &file.cases["v2_deposit_wallet_session"];
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        let target = SigningTarget::DepositWallet {
            wallet: file.deposit_wallet,
            role: DepositWalletRole::SessionKey,
        };
        let signature = sign_order_as(&native(case), &signer, file.chain_id, &target)
            .await
            .unwrap();
        let bytes = hex::decode(signature.trim_start_matches("0x")).unwrap();
        let expected = hex::decode(case.signature.trim_start_matches("0x")).unwrap();
        assert_eq!(bytes.len(), expected.len());
        assert_eq!(&bytes[..12], &expected[..12]);
        assert_eq!(&expected[12..32], file.session_signer.as_slice());
        assert_eq!(&bytes[12..32], signer.address().as_slice());
        assert_eq!(&bytes[32..], &expected[32..]);
    }

    #[test]
    fn a_v2_id_is_refused_where_exchange_v3_is_not_deployed() {
        let order = named("v2_eoa");
        let results = [
            order_exchange(80002, &order).map(|_| ()),
            order_hash(&order, 80002).map(|_| ()),
        ];
        for result in results {
            let err = result.unwrap_err();
            assert!(
                err.to_string()
                    .contains("Protocol V2 orders need ExchangeV3, which is not deployed on chain 80002"),
                "{err}"
            );
            assert_eq!(err.class(), Class::InvalidRequest);
        }
    }

    #[tokio::test]
    async fn signing_a_v2_id_on_amoy_is_refused_before_any_signature() {
        let signer: PrivateKeySigner = ANVIL_KEY_0.parse().unwrap();
        let err = sign_order(&named("v2_eoa"), &signer, 80002)
            .await
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("Protocol V2 orders need ExchangeV3, which is not deployed on chain 80002"),
            "{err}"
        );
    }

    #[test]
    fn an_unsupported_chain_stays_a_crypto_error_for_both_protocols() {
        for name in ["v2_eoa", "v1_eoa"] {
            let err = order_exchange(1, &named(name)).unwrap_err();
            assert!(matches!(err, ClobError::Crypto(_)), "{name}: {err:?}");
        }
    }

    #[test]
    fn a_v1_id_on_amoy_still_picks_a_ctf_exchange_at_version_2() {
        assert_eq!(
            order_exchange(80002, &named("v1_eoa")).unwrap(),
            (Contracts::POLYGON_AMOY.exchange, "2")
        );
        assert_eq!(
            order_exchange(80002, &named("v1_neg_risk_eoa")).unwrap(),
            (Contracts::POLYGON_AMOY.neg_risk_exchange, "2")
        );
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob --lib clob::core::eip712::protocol_v2_vectors`

Expected: compile errors `cannot find function `order_exchange` in this scope` and `cannot find function `order_domain` in this scope`.

- [ ] **Step 3: Implement**

Edit `polyoxide-polymarket/src/clob/core/eip712.rs` in six places.

(a) The imports. Quoted current lines:

```rust
use crate::clob::{
    account::{DepositWalletRole, SigningTarget},
    core::chain::Chain,
    error::ClobError,
    types::{Order as ClobOrder, SignatureType},
};
```

becomes:

```rust
use crate::clob::{
    account::{DepositWalletRole, SigningTarget},
    core::chain::Chain,
    error::ClobError,
    types::{Order as ClobOrder, SignatureType},
};
use crate::shared::protocol::is_v2_position_id;
```

(b) The domain. Quoted current lines:

```rust
/// The `Polymarket CTF Exchange` v2 domain for one exchange contract.
fn exchange_domain(chain_id: u64, exchange: Address) -> protocol::EIP712Domain {
    protocol::EIP712Domain {
        name: "Polymarket CTF Exchange".to_string(),
        version: "2".to_string(),
        chainId: U256::from(chain_id),
        verifyingContract: exchange,
    }
}
```

becomes:

```rust
/// The exchange an order is settled on and the domain version it signs under.
///
/// An order whose token id is a Protocol V2 position id
/// ([`is_v2_position_id`]) settles on ExchangeV3 at version `"3"`, whatever its
/// `neg_risk` says; a chain with no ExchangeV3 is refused as a validation
/// error. Any other order settles on the standard or neg-risk CTF exchange
/// that `neg_risk` picks, at version `"2"`. An unsupported chain is a
/// [`ClobError::Crypto`] in both cases.
pub(crate) fn order_exchange(
    chain_id: u64,
    order: &ClobOrder,
) -> Result<(Address, &'static str), ClobError> {
    if is_v2_position_id(&order.token_id) {
        let chain = Chain::from_chain_id(chain_id)
            .ok_or_else(|| ClobError::Crypto(format!("Unsupported chain ID: {}", chain_id)))?;
        let exchange = chain.exchange_v3().ok_or_else(|| {
            ClobError::validation(format!(
                "Protocol V2 orders need ExchangeV3, which is not deployed on chain {chain_id}"
            ))
        })?;
        return Ok((exchange, "3"));
    }
    Ok((exchange_address(chain_id, order.neg_risk)?, "2"))
}

/// The `Polymarket CTF Exchange` domain at `version` for one exchange
/// contract: `"2"` for the CTF exchanges, `"3"` for ExchangeV3.
fn order_domain(
    chain_id: u64,
    exchange: Address,
    version: &'static str,
) -> protocol::EIP712Domain {
    protocol::EIP712Domain {
        name: "Polymarket CTF Exchange".to_string(),
        version: version.to_string(),
        chainId: U256::from(chain_id),
        verifyingContract: exchange,
    }
}

/// The `Polymarket CTF Exchange` version `"2"` domain for one exchange
/// contract.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the signing paths take order_domain; tests build the version 2 domain by this name"
    )
)]
fn exchange_domain(chain_id: u64, exchange: Address) -> protocol::EIP712Domain {
    order_domain(chain_id, exchange, "2")
}
```

(c) `order_hash`. Quoted current lines:

```rust
pub(crate) fn order_hash(order: &ClobOrder, chain_id: u64) -> Result<B256, ClobError> {
    let exchange = exchange_address(chain_id, order.neg_risk)?;
    let domain = exchange_domain(chain_id, exchange);
    let struct_hash = order_to_protocol(order)?.eip712_hash_struct();
    Ok(eip712_digest(domain.eip712_hash_struct(), struct_hash))
}
```

becomes:

```rust
pub(crate) fn order_hash(order: &ClobOrder, chain_id: u64) -> Result<B256, ClobError> {
    let (exchange, version) = order_exchange(chain_id, order)?;
    let domain = order_domain(chain_id, exchange, version);
    let struct_hash = order_to_protocol(order)?.eip712_hash_struct();
    Ok(eip712_digest(domain.eip712_hash_struct(), struct_hash))
}
```

(d) The Deposit Wallet entry point. Quoted current lines:

```rust
    use super::{eip712_digest, exchange_domain, order_to_protocol, protocol};
    use crate::clob::{error::ClobError, types::Order as ClobOrder};
```

becomes:

```rust
    use super::{eip712_digest, is_v2_position_id, order_domain, order_to_protocol, protocol};
    use crate::clob::{error::ClobError, types::Order as ClobOrder};
```

and, quoted:

```rust
    /// `exchange` is the verifying contract of the outer (app) domain and
    /// `wallet` the Deposit Wallet named inside the envelope. This computes
```

becomes:

```rust
    /// `exchange` is the verifying contract of the outer (app) domain and
    /// `wallet` the Deposit Wallet named inside the envelope. The app domain's
    /// version comes from `order.token_id`, as `order_exchange` picks it:
    /// `"3"` for a Protocol V2 position id (pass ExchangeV3), else `"2"`. This computes
```

and, quoted:

```rust
        let contents = order_to_protocol(order)?;
        let app_domain_separator = exchange_domain(chain_id, exchange).eip712_hash_struct();
        Ok(envelope_digest_from(
```

becomes:

```rust
        let contents = order_to_protocol(order)?;
        let version = if is_v2_position_id(&order.token_id) {
            "3"
        } else {
            "2"
        };
        let app_domain_separator = order_domain(chain_id, exchange, version).eip712_hash_struct();
        Ok(envelope_digest_from(
```

(e) `sign_order_as`. Quoted current lines:

```rust
    let exchange = exchange_address(chain_id, order.neg_risk)?;
    let contents = order_to_protocol(order)?;
    let contents_hash = contents.eip712_hash_struct();
    let app_domain_separator = exchange_domain(chain_id, exchange).eip712_hash_struct();
```

becomes:

```rust
    let (exchange, version) = order_exchange(chain_id, order)?;
    let contents = order_to_protocol(order)?;
    let contents_hash = contents.eip712_hash_struct();
    let app_domain_separator = order_domain(chain_id, exchange, version).eip712_hash_struct();
```

(f) Two doc comments. Quoted:

```rust
/// Sign an order with EIP-712
pub async fn sign_order<S: AlloySigner + ?Sized>(
```

becomes:

```rust
/// Sign an order with EIP-712.
///
/// The order is signed under the exchange its token id selects: ExchangeV3 at
/// domain version `"3"` for a Protocol V2 position id (refused on a chain with
/// no ExchangeV3), else the CTF or neg-risk exchange at version `"2"`.
pub async fn sign_order<S: AlloySigner + ?Sized>(
```

and, quoted:

```rust
/// The EIP-712 hash of an order under its exchange's domain, for every
/// signature type: the id the exchange gives the order, known before it is
/// signed.
```

becomes:

```rust
/// The EIP-712 hash of an order under its exchange's domain (version `"3"` for
/// a Protocol V2 position id, else `"2"`), for every signature type: the id the
/// exchange gives the order, known before it is signed.
```

Then delete the temporary attribute from `polyoxide-polymarket/src/shared/protocol.rs`. Quoted:

```rust
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Task 2.3 calls is_v2_position_id from the signing path"
    )
)]

```

is removed (with its blank line). `is_v2_position_id` is now called from non-test code, and so are `parse_position_id` and `decode_v2_outcome_position_id` through it.

Run `direnv exec . cargo fmt --all` and keep its result.

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob --lib clob::core::eip712`

Expected: every `eip712` test passes, including the baseline `v2_order_signature_is_deterministic`, `order_digest_uses_correct_exchange_for_neg_risk`, `deposit_wallet_vectors::*` and `order_hash_vectors::*` (unedited), and the 11 new `protocol_v2_vectors` tests.

Then `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob --lib` passes whole, and `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings` prints no warning (a leftover `expect(dead_code)` on `shared::protocol` would warn `unfulfilled_lint_expectations`).

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(clob): sign Protocol V2 orders for ExchangeV3 at domain version 3 (protocol v2, Story 2)" -- polyoxide-polymarket/src/clob/core/eip712.rs polyoxide-polymarket/src/shared/protocol.rs
```

### Task 2.4: the `/neg-risk` skip, `CONDITIONAL-V2` balances, the `neg_risk` docs

**Files:**
- Modify: `polyoxide-polymarket/src/clob/client.rs:1-3`, `:476-490`
- Modify: `polyoxide-polymarket/src/clob/api/account.rs:7`, `:21-33`, `:44-50`
- Modify: `polyoxide-polymarket/src/clob/types.rs:276-298`
- Modify: `polyoxide-polymarket/src/clob/venue/trading.rs:44-48`
- Test: `polyoxide-polymarket/tests/mock_clob.rs` (append)

- [ ] **Step 1: Write the failing tests**

Append to the end of `polyoxide-polymarket/tests/mock_clob.rs` (after `heartbeat_v1_invalid_payload_stays_a_generic_400`). Every id, asset type and domain separator is read from Story 1's fixtures.

```rust
// ── Protocol V2 (ExchangeV3): ids, balances and order metadata ──

const PROTOCOL_V2_IDS: &str = include_str!("fixtures/clob/protocol_v2/id_vectors.json");
const PROTOCOL_V2_ORDERS: &str = include_str!("fixtures/clob/protocol_v2/order_vectors.json");

/// Hardhat account #0, the key the test clients sign with.
const HARDHAT_0: alloy::primitives::Address =
    alloy::primitives::address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

fn protocol_v2_ids() -> serde_json::Value {
    serde_json::from_str(PROTOCOL_V2_IDS).unwrap()
}

/// One of the fixture's four ids (`v1`, `v2_binary`, `v2_neg_risk`, `v2_combo`).
fn protocol_v2_id(name: &str) -> String {
    protocol_v2_ids()["ids"][name].as_str().unwrap().to_string()
}

/// The exchange domain separator py-sdk recorded for one order case.
fn protocol_v2_separator(case: &str) -> alloy::primitives::B256 {
    let all: serde_json::Value = serde_json::from_str(PROTOCOL_V2_ORDERS).unwrap();
    all["cases"][case]["domain_separator"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

/// An order's digest under a given exchange domain separator, built here from
/// the order's fields so the test does not lean on the crate's own hashing.
mod protocol_v2_digest {
    use alloy::primitives::{keccak256, Address, Signature, B256, U256};
    use alloy::sol_types::SolStruct;
    use polyoxide_polymarket::clob::{Order as ClobOrder, OrderSide};
    use std::str::FromStr;

    alloy::sol! {
        struct Order {
            uint256 salt;
            address maker;
            address signer;
            uint256 tokenId;
            uint256 makerAmount;
            uint256 takerAmount;
            uint8 side;
            uint8 signatureType;
            uint256 timestamp;
            bytes32 metadata;
            bytes32 builder;
        }
    }

    pub fn digest(domain_separator: B256, order: &ClobOrder) -> B256 {
        let number = |text: &str| U256::from_str_radix(text, 10).unwrap();
        let typed = Order {
            salt: number(&order.salt),
            maker: order.maker,
            signer: order.signer,
            tokenId: number(&order.token_id),
            makerAmount: number(&order.maker_amount),
            takerAmount: number(&order.taker_amount),
            side: match order.side {
                OrderSide::Buy => 0,
                OrderSide::Sell => 1,
            },
            signatureType: order.signature_type as u8,
            timestamp: number(&order.timestamp),
            metadata: order.metadata,
            builder: order.builder,
        };
        let mut message = Vec::with_capacity(66);
        message.extend_from_slice(b"\x19\x01");
        message.extend_from_slice(domain_separator.as_slice());
        message.extend_from_slice(typed.eip712_hash_struct().as_slice());
        keccak256(&message)
    }

    /// The address that signed `digest`, recovered from a `0x` signature.
    pub fn signer_of(signature: &str, digest: B256) -> Address {
        Signature::from_str(signature)
            .unwrap()
            .recover_address_from_prehash(&digest)
            .unwrap()
    }
}

fn protocol_v2_params(token_id: &str) -> polyoxide_polymarket::clob::CreateOrderParams {
    polyoxide_polymarket::clob::CreateOrderParams {
        token_id: token_id.to_string(),
        price: 0.55,
        size: 100.0,
        side: polyoxide_polymarket::clob::OrderSide::Buy,
        order_type: polyoxide_polymarket::clob::OrderKind::Gtc,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: None,
    }
}

async fn protocol_v2_tick_size(server: &mut mockito::ServerGuard, token_id: &str) -> mockito::Mock {
    server
        .mock("GET", "/tick-size")
        .match_query(Matcher::UrlEncoded("token_id".into(), token_id.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .expect(1)
        .create_async()
        .await
}

#[tokio::test]
async fn balance_asset_types_follow_py_sdks_table() {
    let rows = protocol_v2_ids()["balance_allowance_targets"]
        .as_array()
        .unwrap()
        .clone();
    // BUY rows are COLLATERAL with no asset id; SELL rows are CONDITIONAL for
    // the V1 id and CONDITIONAL-V2 for the V2 id.
    let types: Vec<&str> = rows
        .iter()
        .map(|row| row["asset_type"].as_str().unwrap())
        .collect();
    assert_eq!(rows.len(), 4);
    assert_eq!(types.iter().filter(|t| **t == "COLLATERAL").count(), 2);
    assert_eq!(types.iter().filter(|t| **t == "CONDITIONAL").count(), 1);
    assert_eq!(types.iter().filter(|t| **t == "CONDITIONAL-V2").count(), 1);

    for row in &rows {
        let side = row["side"].as_str().unwrap();
        let asset_id = row["asset_id"].as_str().unwrap();
        let asset_type = row["asset_type"].as_str().unwrap();
        let sent = row["asset_id_sent"].as_str();
        // The whole query, in the order the request sends it, so a token id
        // the table does not send would fail the match.
        let query = match sent {
            Some(sent) => {
                assert_eq!(side, "SELL");
                assert_eq!(sent, asset_id);
                format!("^asset_type={asset_type}&token_id={sent}&signature_type=0$")
            }
            None => {
                assert_eq!(side, "BUY");
                format!("^asset_type={asset_type}&signature_type=0$")
            }
        };

        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/balance-allowance")
            .match_query(Matcher::Regex(query))
            .match_header("POLY_API_KEY", "test-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"balance": "0", "allowances": {}}"#)
            .expect(1)
            .create_async()
            .await;

        let clob = test_authed_clob(&server);
        let account = clob.account_api().unwrap();
        let request = match sent {
            Some(_) => account.balance_allowance(asset_id),
            None => account.usdc_balance(),
        };
        request.send().await.unwrap();
        mock.assert_async().await;
    }
}

#[tokio::test]
async fn create_order_for_a_v2_id_reads_the_tick_size_never_neg_risk_and_signs_for_exchange_v3() {
    let id = protocol_v2_id("v2_binary");
    let mut server = Server::new_async().await;
    // `match_query(Matcher::Any)` makes `expect(0)` real: without it mockito
    // compares the path and the query, and a bare "/neg-risk" never matches
    // "/neg-risk?token_id=…".
    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let tick_size_mock = protocol_v2_tick_size(&mut server, &id).await;

    let clob = test_authed_clob(&server);
    let order = clob
        .create_order(&protocol_v2_params(&id), None)
        .await
        .unwrap();

    assert_eq!(order.token_id, id);
    assert!(!order.neg_risk, "with no lookup, neg_risk is false");
    assert_eq!(order.maker_amount, "55000000");
    assert_eq!(order.taker_amount, "100000000");

    // The signature verifies under py-sdk's ExchangeV3 domain, and not under
    // the CTF exchange's.
    let signed = clob.sign_order(&order).await.unwrap();
    let v3 = protocol_v2_separator("v2_eoa");
    let v2 = protocol_v2_separator("v1_eoa");
    assert_ne!(v3, v2, "the fixture's two domains differ");
    assert_eq!(
        protocol_v2_digest::signer_of(&signed.signature, protocol_v2_digest::digest(v3, &order)),
        HARDHAT_0
    );
    assert_ne!(
        protocol_v2_digest::signer_of(&signed.signature, protocol_v2_digest::digest(v2, &order)),
        HARDHAT_0
    );

    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
}

#[tokio::test]
async fn create_order_for_a_v1_id_still_reads_neg_risk() {
    let id = protocol_v2_id("v1");
    let mut server = Server::new_async().await;
    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::UrlEncoded("token_id".into(), id.clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"neg_risk": true}"#)
        .expect(1)
        .create_async()
        .await;
    let tick_size_mock = protocol_v2_tick_size(&mut server, &id).await;

    let clob = test_authed_clob(&server);
    let order = clob
        .create_order(&protocol_v2_params(&id), None)
        .await
        .unwrap();

    assert!(order.neg_risk, "the V1 market's answer is kept");
    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
}

#[tokio::test]
async fn a_v2_id_on_an_amoy_client_is_refused_at_signing_and_nothing_is_posted() {
    let id = protocol_v2_id("v2_binary");
    let mut server = Server::new_async().await;
    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let tick_size_mock = protocol_v2_tick_size(&mut server, &id).await;
    let post_mock = server
        .mock("POST", "/order")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;

    let account = Account::new(
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        Credentials {
            key: "test-key".into(),
            secret: "c2VjcmV0".into(), // base64("secret")
            passphrase: "test-pass".into(),
        },
    )
    .unwrap();
    let clob = ClobBuilder::new()
        .base_url(server.url())
        .with_account(account)
        .chain(polyoxide_polymarket::clob::Chain::PolygonAmoy)
        .build()
        .unwrap();

    // `place_order` creates, signs and would post; it stops at the signature.
    let err = clob
        .place_order(&protocol_v2_params(&id), None)
        .await
        .unwrap_err();

    assert_eq!(err.class(), polyoxide_venue::Class::InvalidRequest);
    assert!(
        err.to_string()
            .contains("Protocol V2 orders need ExchangeV3, which is not deployed on chain 80002"),
        "{err}"
    );
    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
    post_mock.assert_async().await;
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob --test mock_clob -- balance_asset_types_follow create_order_for_a_v2_id create_order_for_a_v1_id a_v2_id_on_an_amoy_client`

Expected: `balance_asset_types_follow_py_sdks_table` fails on the V2 SELL row (the request sends `asset_type=CONDITIONAL`, so no mock matches and `send().await.unwrap()` panics on the 501); the two V2 `create_order` tests fail (the request reaches the `expect(0)` `/neg-risk` mock: `Expected 0 request(s) … received 1`, or an unwrap on its empty answer); `create_order_for_a_v1_id_still_reads_neg_risk` passes already (it is the control).

- [ ] **Step 3: Implement**

`polyoxide-polymarket/src/clob/client.rs`, three edits.

The import. Quoted current lines:

```rust
use crate::shared::{
    self, PolymarketRetryPolicy, RateLimitStatus, SignerLimiter, Tier, TradingRequest,
};
```

becomes:

```rust
use crate::shared::{
    self, protocol::is_v2_position_id, PolymarketRetryPolicy, RateLimitStatus, SignerLimiter, Tier,
    TradingRequest,
};
```

The metadata lookup. Quoted current lines:

```rust
    /// Fetch market metadata (neg_risk and tick_size) for a token
    async fn get_market_metadata(
        &self,
        token_id: &str,
        options: Option<PartialCreateOrderOptions>,
    ) -> Result<(bool, TickSize), ClobError> {
        // Fetch or use provided neg_risk status
        let neg_risk = if let Some(neg_risk) = options.and_then(|o| o.neg_risk) {
            neg_risk
        } else {
            let neg_risk_resp = self.markets().neg_risk(token_id.to_string()).send().await?;
            neg_risk_resp.neg_risk
        };
```

becomes:

```rust
    /// Fetch market metadata (neg_risk and tick_size) for a token
    ///
    /// A Protocol V2 position id settles on ExchangeV3 whatever `neg_risk` says,
    /// so for one the `/neg-risk` lookup is skipped and `neg_risk` is the
    /// options' value, else `false`. The tick size is looked up for every token.
    async fn get_market_metadata(
        &self,
        token_id: &str,
        options: Option<PartialCreateOrderOptions>,
    ) -> Result<(bool, TickSize), ClobError> {
        // Fetch or use provided neg_risk status
        let neg_risk = if let Some(neg_risk) = options.and_then(|o| o.neg_risk) {
            neg_risk
        } else if is_v2_position_id(token_id) {
            false
        } else {
            let neg_risk_resp = self.markets().neg_risk(token_id.to_string()).send().await?;
            neg_risk_resp.neg_risk
        };
```

`polyoxide-polymarket/src/clob/api/account.rs`, three edits.

The import. Quoted:

```rust
use crate::shared::{SessionSignerScope, Tier};
```

becomes:

```rust
use crate::shared::{protocol::is_v2_position_id, SessionSignerScope, Tier};
```

`balance_allowance`. Quoted:

```rust
    /// Get balance and allowance for a token
    pub fn balance_allowance(
        &self,
        token_id: impl Into<String>,
    ) -> Request<BalanceAllowanceResponse, ClobError> {
        Request::new(self.http_client.clone(), "/balance-allowance")
            .authenticator(self.l2.clone())
            .query("asset_type", "CONDITIONAL")
            .query("token_id", token_id.into())
            .query("signature_type", self.signature_type as u8)
    }
```

becomes:

```rust
    /// Get balance and allowance for a token
    ///
    /// Sends `asset_type=CONDITIONAL-V2` for a Protocol V2 position id and
    /// `CONDITIONAL` for any other.
    pub fn balance_allowance(
        &self,
        token_id: impl Into<String>,
    ) -> Request<BalanceAllowanceResponse, ClobError> {
        let token_id = token_id.into();
        let asset_type = if is_v2_position_id(&token_id) {
            "CONDITIONAL-V2"
        } else {
            "CONDITIONAL"
        };
        Request::new(self.http_client.clone(), "/balance-allowance")
            .authenticator(self.l2.clone())
            .query("asset_type", asset_type)
            .query("token_id", token_id)
            .query("signature_type", self.signature_type as u8)
    }
```

`update_balance_allowance`'s docs. Quoted:

```rust
    /// - `asset_type` (required): `"COLLATERAL"` or `"CONDITIONAL"`.
```

becomes:

```rust
    /// - `asset_type` (required): `"COLLATERAL"`, `"CONDITIONAL"`, or
    ///   `"CONDITIONAL-V2"` for a Protocol V2 position id.
```

`polyoxide-polymarket/src/clob/types.rs`: quoted:

```rust
/// NOT part of the signed struct. `neg_risk` is local-only (`#[serde(skip)]`) and
/// selects the exchange vs. neg-risk-exchange verifying contract during signing.
```

becomes:

```rust
/// NOT part of the signed struct. `neg_risk` is local-only (`#[serde(skip)]`) and
/// selects the exchange vs. neg-risk-exchange verifying contract during signing
/// of a CTF token id. A Protocol V2 position id always settles on ExchangeV3
/// (domain version `"3"`), so `neg_risk` is ignored for one.
```

and quoted:

```rust
    #[serde(skip)]
    pub neg_risk: bool,
```

becomes:

```rust
    /// Picks between the CTF and neg-risk CTF exchanges; ignored for a Protocol V2
    /// position id, which always settles on ExchangeV3.
    #[serde(skip)]
    pub neg_risk: bool,
```

`polyoxide-polymarket/src/clob/venue/trading.rs`: quoted:

```rust
    /// Whether the market is in a negative-risk event, which selects the
    /// exchange contract the order is signed for.
    pub neg_risk: Option<bool>,
```

becomes:

```rust
    /// Whether the market is in a negative-risk event, which selects between
    /// the CTF exchanges the order is signed for. Ignored for a Protocol V2
    /// position id, which always settles on ExchangeV3.
    pub neg_risk: Option<bool>,
```

Run `direnv exec . cargo fmt --all` and keep its result.

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob --test mock_clob`

Expected: `test result: ok.` with all baseline tests (unedited) and the four new ones passing. Then `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings` prints no warning, and `bash -c 'RUSTDOCFLAGS="-D warnings" direnv exec . cargo doc --no-deps -p polyoxide-polymarket --all-features'` builds (the new docs link no `pub(crate)` item).

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(clob): skip /neg-risk and send CONDITIONAL-V2 for a Protocol V2 id (protocol v2, Story 2)" -- polyoxide-polymarket/src/clob/client.rs polyoxide-polymarket/src/clob/api/account.rs polyoxide-polymarket/src/clob/types.rs polyoxide-polymarket/src/clob/venue/trading.rs polyoxide-polymarket/tests/mock_clob.rs
```

### Task 2.5: `Trading::place` on a V2 key

**Files:**
- Test: `polyoxide-polymarket/tests/trading_clob.rs` (append)

- [ ] **Step 1: Write the failing test**

`Trading::place` needs no code of its own (its order id is `order_hash`, and it builds the order with `create_order`); this task proves that end to end. Append to the end of `polyoxide-polymarket/tests/trading_clob.rs` (after `an_order_id_the_clob_names_differently_is_rekeyed_and_logged_as_a_warning`):

```rust
// --- Protocol V2 ---

const PROTOCOL_V2_IDS: &str = include_str!("fixtures/clob/protocol_v2/id_vectors.json");
const PROTOCOL_V2_ORDERS: &str = include_str!("fixtures/clob/protocol_v2/order_vectors.json");

/// An order's digest under a given exchange domain separator, built here from
/// the order's fields so the test does not lean on the crate's own hashing.
mod protocol_v2_digest {
    use alloy::primitives::{keccak256, Address, Signature, B256, U256};
    use alloy::sol_types::SolStruct;
    use polyoxide_polymarket::clob::{Order as ClobOrder, OrderSide};
    use std::str::FromStr;

    alloy::sol! {
        struct Order {
            uint256 salt;
            address maker;
            address signer;
            uint256 tokenId;
            uint256 makerAmount;
            uint256 takerAmount;
            uint8 side;
            uint8 signatureType;
            uint256 timestamp;
            bytes32 metadata;
            bytes32 builder;
        }
    }

    pub fn digest(domain_separator: B256, order: &ClobOrder) -> B256 {
        let number = |text: &str| U256::from_str_radix(text, 10).unwrap();
        let typed = Order {
            salt: number(&order.salt),
            maker: order.maker,
            signer: order.signer,
            tokenId: number(&order.token_id),
            makerAmount: number(&order.maker_amount),
            takerAmount: number(&order.taker_amount),
            side: match order.side {
                OrderSide::Buy => 0,
                OrderSide::Sell => 1,
            },
            signatureType: order.signature_type as u8,
            timestamp: number(&order.timestamp),
            metadata: order.metadata,
            builder: order.builder,
        };
        let mut message = Vec::with_capacity(66);
        message.extend_from_slice(b"\x19\x01");
        message.extend_from_slice(domain_separator.as_slice());
        message.extend_from_slice(typed.eip712_hash_struct().as_slice());
        keccak256(&message)
    }

    /// The address that signed `digest`, recovered from a `0x` signature.
    pub fn signer_of(signature: &str, digest: B256) -> Address {
        Signature::from_str(signature)
            .unwrap()
            .recover_address_from_prehash(&digest)
            .unwrap()
    }
}

/// `place` on a Protocol V2 key reports the ExchangeV3 order hash as the
/// order's id: the venue's answer names no order, so the id is the one `place`
/// computed before signing. The hash is rebuilt here from py-sdk's recorded
/// ExchangeV3 domain separator and the posted order, and the posted signature
/// must recover to the account under that same digest.
#[allow(clippy::field_reassign_with_default)]
#[tokio::test]
async fn place_on_a_protocol_v2_key_names_the_order_by_its_exchange_v3_hash() {
    let ids: Value = serde_json::from_str(PROTOCOL_V2_IDS).unwrap();
    let orders: Value = serde_json::from_str(PROTOCOL_V2_ORDERS).unwrap();
    let v2_id = ids["ids"]["v2_binary"].as_str().unwrap().to_string();
    let separator = |case: &str| -> alloy::primitives::B256 {
        orders["cases"][case]["domain_separator"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    };
    let v3 = separator("v2_eoa");
    let v2 = separator("v1_eoa");
    assert_ne!(v3, v2, "the fixture's two domains differ");

    let mut server = Server::new_async().await;
    // `match_query(Matcher::Any)` makes `expect(0)` real.
    let neg_risk = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let posted = Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
    let sink = posted.clone();
    let post = server
        .mock("POST", "/order")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body_from_request(move |request| {
            sink.lock().unwrap().push(request.body().unwrap().clone());
            br#"{"success":true,"status":"live","transactionsHashes":[]}"#.to_vec()
        })
        .expect(1)
        .create_async()
        .await;
    let clob = client(&server);
    let mut options = ClobOrderOptions::default();
    options.tick_size = Some(TickSize::Hundredth);
    let request = with_options(
        OrderRequest::new(
            keys::clob(&TokenId::new(v2_id.clone()).unwrap()),
            Side::Buy,
            d("0.52"),
            d("10"),
        ),
        options,
    );

    let placed = clob.place(request).await.unwrap();

    neg_risk.assert_async().await;
    post.assert_async().await;
    let body: Value = serde_json::from_slice(&posted.lock().unwrap()[0]).unwrap();
    // The wire writes the salt as a number and the order holds it as text.
    let mut wire = body["order"].clone();
    wire["salt"] = Value::String(wire["salt"].to_string());
    let signed: polyoxide_polymarket::clob::SignedOrder = serde_json::from_value(wire).unwrap();
    assert_eq!(signed.order.token_id, v2_id);

    let digest = protocol_v2_digest::digest(v3, &signed.order);
    assert_eq!(
        placed.id,
        OrderId::new(format!("0x{}", alloy::primitives::hex::encode(digest)))
    );
    assert_eq!(placed.status, OrderStatus::Open);
    let hardhat_0 = account().address();
    assert_eq!(
        protocol_v2_digest::signer_of(&signed.signature, digest),
        hardhat_0
    );
    // Not the id, nor the signed digest, of the same order under the CTF domain.
    let ctf_digest = protocol_v2_digest::digest(v2, &signed.order);
    assert_ne!(ctf_digest, digest);
    assert_ne!(
        protocol_v2_digest::signer_of(&signed.signature, ctf_digest),
        hardhat_0
    );
}
```

- [ ] **Step 2: Run it to see it fail**

This task adds only a test over the code Tasks 2.3 and 2.4 landed, so it passes at once. To see it catch the bug, temporarily make the first line of `order_exchange` in `polyoxide-polymarket/src/clob/core/eip712.rs` read `if false && is_v2_position_id(&order.token_id) {` and run the command below. Expected: the `/neg-risk` mock's `Expected 0 request(s)` failure, or `assertion `left == right` failed` on `placed.id`. Then undo the temporary edit with `git checkout polyoxide-polymarket/src/clob/core/eip712.rs` (the file is committed as of Task 2.3).

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob -- place_on_a_protocol_v2_key`

- [ ] **Step 3: Implement**

No library change: `place` already builds the order with `create_order` (Task 2.4's skip) and names it by `order_hash` (Task 2.3's routing). Confirm `git diff --stat` shows only `tests/trading_clob.rs` before the commit.

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob-ws,data --test trading_clob`

Expected: `test result: ok.` with every baseline test and the new one passing. Then `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings` prints no warning.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(clob): Trading::place names a Protocol V2 order by its ExchangeV3 hash (protocol v2, Story 2)" -- polyoxide-polymarket/tests/trading_clob.rs
```

### Task 2.6: the rules and spec docs

**Files:**
- Modify: `.claude/rules/polymarket-clob.md:1-10`, `:30-48`, `:112-116`
- Modify: `.claude/rules/session-keys.md:14-20`
- Modify: `docs/specs/clob/auth.md:5-6`, end of file
- Modify: `docs/specs/clob/orders.md:5-6`
- Modify: `docs/specs/session-keys/README.md:57-58`
- Modify: `docs/specs/session-keys/OBSERVED.md` (row 11, then a row 12)
- Test: `.github/scripts/tests/test_claude_rules.py`

A rule's text changes in the commit that changes the rule (AD-21), so these edits are one commit. No test fails first here; the rules test is the check, and it fails if the new `paths:` glob matches no file.

- [ ] **Step 1: Edit the rules and the specs**

`.claude/rules/polymarket-clob.md`, four edits.

The `paths:` front matter. Quoted:

```
  - "scripts/capture_polymarket_clob.py"
  - "docs/specs/clob/**"
```

becomes:

```
  - "scripts/capture_polymarket_clob.py"
  - "scripts/capture_polymarket_protocol_v2.py"
  - "docs/specs/clob/**"
```

The order-signing row. Quoted:

```
| **Order signing** | the signed order payload itself, posted under L2 | EIP-712 `Order`, domain `Polymarket CTF Exchange` v2, **with** `verifyingContract` |
```

becomes:

```
| **Order signing** | the signed order payload itself, posted under L2 | EIP-712 `Order`, domain `Polymarket CTF Exchange` at version `"2"` (the CTF exchanges) or `"3"` (ExchangeV3, for a Protocol V2 position id), **with** `verifyingContract` |
```

The paragraph. Quoted:

```
The module signs exchange domain `"2"` only, which is correct for Gamma `v1` markets; Protocol V2
markets sign for ExchangeV3 (domain `"3"`) and are not supported (#51, `polymarket-gamma.md`).
```

becomes:

```
The domain is chosen per order, from its token id alone (`order_exchange` in `core/eip712.rs`, on
`shared::protocol::is_v2_position_id`), as py-sdk 0.12.0 does. An id is a Protocol V2 position id
when py-sdk's `is_v2_position_id` (bits 40 to 103 clear) **and** its `decode_v2_outcome_position_id`
(module byte 1 to 3, outcome byte 0 or 1) both accept it. A V2 id signs domain `"3"` on ExchangeV3
(`Chain::exchange_v3`, Polygon mainnet only; on Amoy the order is refused as `InvalidRequest` when
it signs), **whatever `neg_risk` says**; any other id signs `"2"` on the CTF or neg-risk exchange
that `neg_risk` picks, exactly as before. The rule is one condition stricter than py-sdk's bare
mask, which also calls toy ids such as `1`, `100` and `123456` V2; those name no V2 position (module
byte 0) and are the ids the older pinned tests sign with, so they stay V1. It agrees with py-sdk on
every id a market can carry. The same rule makes `get_market_metadata` skip `/neg-risk` for a V2 id
(it keeps `/tick-size`) and `balance_allowance` send `CONDITIONAL-V2`. `tests/fixtures/clob/protocol_v2/`
pins all of it, captured from py-sdk 0.12.0 by `scripts/capture_polymarket_protocol_v2.py`. Gamma's
side, the outcome ids of a V2 market, is `polymarket-gamma.md`.
```

The `place` bullet. Quoted:

```
  signing for every signature type, and a Deposit Wallet's order is signed as its ERC-7739
  envelope.
```

becomes:

```
  signing for every signature type, and a Deposit Wallet's order is signed as its ERC-7739
  envelope. For a Protocol V2 id that domain is ExchangeV3's (version `"3"`), and `neg_risk` in
  `ClobOrderOptions` is ignored.
```

`.claude/rules/session-keys.md`. Quoted:

```
  digest, and `polyoxide-polymarket/src/clob/core/eip712.rs` checks it. A session key adds a 6492
  envelope on top.
```

becomes:

```
  digest, and `polyoxide-polymarket/src/clob/core/eip712.rs` checks it. A session key adds a 6492
  envelope on top. For a Protocol V2 position id the same envelope is hashed under ExchangeV3
  (domain version `"3"`), not a CTF exchange; `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/order_vectors.json`
  pins the owner's and the session key's signatures (py-sdk 0.12.0).
```

`docs/specs/clob/auth.md`. Quoted:

```
> **Note (CLOB V2):** polyoxide now targets the CLOB **V2** order scheme — orders
> are signed with EIP-712 domain version `"2"` against the V2 exchange contracts.
```

becomes:

```
> **Note (CLOB V2):** polyoxide now targets the CLOB **V2** order scheme — orders
> are signed with EIP-712 domain version `"2"` against the V2 exchange contracts, and
> orders for a Protocol V2 position id with version `"3"` against ExchangeV3
> (see "Order-signing domain, per protocol" below).
```

and append at the end of the file, after the last line `- \`DELETE /auth/builder-api-key\` — Revoke builder key (L2 auth)`:

```

## Order-signing domain, per protocol

The order domain is `EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)`
with `name = "Polymarket CTF Exchange"`. Its `version` and `verifyingContract` follow the order's
token id (py-sdk 0.12.0, `_internal/protocol.py`, `orders.py`, `context.py`); nothing else changes,
and a signature type 3 envelope uses the same domain as its outer (app) domain.

| Token id | `version` | `verifyingContract` |
|---|---|---|
| Protocol V2 position id (py-sdk's `is_v2_position_id` and `decode_v2_outcome_position_id` both accept it) | `"3"` | ExchangeV3 `0xe3333700cA9d93003F00f0F71f8515005F6c00Aa` (Polygon mainnet; none known on Amoy), whatever `neg_risk` is |
| any other (a CTF token id) | `"2"` | the standard exchange, or the neg-risk exchange when `neg_risk` is true |
```

`docs/specs/clob/orders.md`. Quoted:

```
> **Note (CLOB V2):** polyoxide signs and submits orders using the CLOB **V2**
> scheme (EIP-712 domain version `"2"`, V2 exchange contracts). The V2 signed
```

becomes:

```
> **Note (CLOB V2):** polyoxide signs and submits orders using the CLOB **V2**
> scheme (EIP-712 domain version `"2"`, V2 exchange contracts; version `"3"` on
> ExchangeV3 for a Protocol V2 position id, see `auth.md`'s "Order-signing domain,
> per protocol"). The V2 signed
```

`docs/specs/session-keys/README.md`, the type-3 domain passage only (the V2-router sentence further down is Story 4's, leave it). Quoted:

```
1. EIP-712 domain `Polymarket CTF Exchange`, version `2`, chain 137, verifying
   contract = the exchange (standard or neg-risk, as for any V2 order).
```

becomes:

```
1. EIP-712 domain `Polymarket CTF Exchange`, chain 137: version `3` and verifying
   contract = ExchangeV3 for a Protocol V2 position id; version `2` and the standard
   or neg-risk exchange (as for any V2 order) otherwise.
   `polyoxide-polymarket/tests/fixtures/clob/protocol_v2/order_vectors.json` pins both.
```

`docs/specs/session-keys/OBSERVED.md`: add row 12 directly under row 11. Quoted (the end of row 11):

```
the batch signature does not cover `metadata` |
```

becomes:

```
the batch signature does not cover `metadata` |
| 12 | Nothing about which exchange a Deposit Wallet order is hashed under for a Protocol V2 market | py-sdk 0.12.0 keeps one `Order` struct and one domain shape; for signature type 3 the `TypedDataSign` envelope's outer (app) domain is the same exchange domain as any order's, so a V2 order is hashed under ExchangeV3 at domain version `"3"`, with the ERC-7739 trailer unchanged (`_internal/actions/orders/typed_data.py`, `context.py`'s `resolve_order_exchange_address`). Captured offline from `polymarket-client==0.12.0`; not observed against the live host | `order_exchange` picks ExchangeV3 and version `"3"` for a Protocol V2 position id and `sign_order_as` hashes the envelope under it; pinned to `tests/fixtures/clob/protocol_v2/order_vectors.json` → `v2_deposit_wallet_owner` and `v2_deposit_wallet_session` |
```

- [ ] **Step 2: Run the rules check**

Run: `bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'`

Expected: all pass (the new `paths:` glob matches Story 1's `scripts/capture_polymarket_protocol_v2.py`; if it fails with a glob matching no file, Story 1 has not landed and the plan order is broken: stop and report).

- [ ] **Step 3: Run the story gate**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --all-features`, then `python3 scripts/test_body_diff.py --baseline v0.40.0`.

Expected: all tests pass and the test-bodies script reports no changed or lost baseline test.

- [ ] **Step 4: Commit**

```bash
git commit -m "docs(clob): the per-id order domain, ExchangeV3 and CONDITIONAL-V2 (protocol v2, Story 2)" -- .claude/rules/polymarket-clob.md .claude/rules/session-keys.md docs/specs/clob/auth.md docs/specs/clob/orders.md docs/specs/session-keys/README.md docs/specs/session-keys/OBSERVED.md
```

## Story 3: Gamma's `Market::outcome_ids`, V2 outcomes in event groups, and the CLI's prices discovery

- **Developer:** intermediate-dev
- **Files:** `polyoxide-polymarket/src/gamma/types.rs`, `polyoxide-polymarket/src/gamma/error.rs`, `polyoxide-polymarket/src/gamma/mod.rs`, `polyoxide-polymarket/src/lib.rs`, `polyoxide-polymarket/src/clob/venue.rs`, `polyoxide-polymarket/tests/outcome_ids_gamma.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/tests/error_classes_gamma.rs`, `polyoxide-polymarket/tests/venue_clob.rs`, `.claude/rules/polymarket-gamma.md`, `.claude/rules/polymarket-clob.md`, `docs/specs/gamma/OBSERVED.md`, `polyoxide-cli/src/commands/polymarket/clob/prices/select.rs`, `polyoxide-cli/tests/prices_discovery.rs`, `.claude/rules/cli.md`
- **Interfaces:** exposes "Story 3 → Story 5"; uses Story 1's `selector_vectors.json` and `id_vectors.json`, and Story 2's `shared::protocol::is_v2_position_id`
- **Verify:** `bash -c 'cargo test -p polyoxide-polymarket --all-features && cargo test -p polyoxide-polymarket --no-default-features --features gamma --test outcome_ids_gamma --test error_classes_gamma && cargo test -p polyoxide-polymarket --features clob,gamma,data --test venue_clob && cargo test -p polyoxide-cli --test prices_discovery && python3 scripts/test_body_diff.py --baseline v0.40.0 && cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_claude_rules.py -q'`
- **Review:** deep (a public selector whose wrong choice sends orders for ids with no book, and a new public error)

Gamma serves a market's outcome ids in one of two fields, and only the market's `version` says which one the CLOB trades: `clobTokenIds` (JSON text) for `v1`, `positionIds` (a real array) for `v2`. This story adds `gamma::types::OutcomeId`, `Market::outcome_ids` with spec §3's six-step rule, and `gamma::OutcomeIdsError` (a new public error: `#[non_exhaustive]`, `Classify` as `Class::Decode` for every variant, listed in `lib.rs`'s `const _` assertion). It then makes `clob::venue`'s `event_group` read every market's outcomes through the selector, so a `v2` market's outcomes appear keyed by their position ids, and makes `event_id_of` ask gamma for `position_ids=` when the key's id passes Story 2's `is_v2_position_id`. The selector's tests hold it to Story 1's `selector_vectors.json` (py-sdk's pair, or a refusal where py-sdk refused) and add the Rust-only cases py-sdk cannot express. Last, `polyoxide-cli`'s `discover_targets` (the `clob prices` market discovery) takes a `v2` market's ids from `Market::outcome_ids`, skips a market whose version is `Other` with a warning, and keeps today's `clob_token_ids` path for `v1` and for a missing version (spec §4a: the baseline `discover_extracts_token_ids_from_markets` serves unversioned markets and passes unedited); one test is appended to `tests/prices_discovery.rs`, and `cli.md`'s prices section says so. No landmine marker appears in this story's code.

### Task 3.1: `OutcomeIdsError`

**Files:**
- Modify: `polyoxide-polymarket/src/gamma/error.rs` (after the `impl Classify for GammaError` block, before `mod tests`)
- Modify: `polyoxide-polymarket/src/gamma/mod.rs:53`
- Modify: `polyoxide-polymarket/src/lib.rs` (the `gamma` block of `const _` assertions)
- Test: `polyoxide-polymarket/tests/error_classes_gamma.rs` (append after the last test)

- [ ] **Step 1: Write the failing test**

Append to `polyoxide-polymarket/tests/error_classes_gamma.rs`, after `a_2xx_body_that_does_not_decode_is_decode`:

```rust

/// Every way the outcome-id selector can refuse is a response that arrived and
/// carries no ids the client can trade: a `Decode` fault with no wait.
#[test]
fn every_outcome_ids_error_is_a_decode_fault_with_no_wait() {
    use polyoxide_polymarket::gamma::OutcomeIdsError;

    let rows = [
        OutcomeIdsError::MissingVersion,
        OutcomeIdsError::UnsupportedVersion("v3".to_owned()),
        OutcomeIdsError::MissingIds,
        OutcomeIdsError::MalformedIds,
        OutcomeIdsError::MalformedOutcomes,
        OutcomeIdsError::CountMismatch {
            outcomes: 2,
            ids: 3,
        },
        OutcomeIdsError::InvalidId("abc".to_owned()),
    ];
    for err in rows {
        assert_eq!(err.class(), Class::Decode, "{err:?}");
        assert!(err.is_fault(), "{err:?}");
        assert_eq!(err.retry_after(), None, "{err:?}");
        assert!(!err.is_retriable(), "{err:?}");
        assert!(!err.to_string().is_empty(), "{err:?}");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features gamma --test error_classes_gamma`

Expected: it fails to compile, `error[E0432]: unresolved import polyoxide_polymarket::gamma::OutcomeIdsError`.

- [ ] **Step 3: Implement**

In `polyoxide-polymarket/src/gamma/error.rs`, insert after the closing brace of `impl Classify for GammaError` (the block that ends with `Self::Api(err) => Classify::retry_after(err),` and two closing braces) and before `#[cfg(test)]`:

```rust

/// Why [`Market::outcome_ids`](super::types::Market::outcome_ids) found no
/// ids a caller can trade.
///
/// Every variant is a response that arrived but carries no usable ids, so each
/// classifies as [`Class::Decode`]. It is not a [`GammaError`] variant:
/// `GammaError` decodes only through its one `From<ApiError>`.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutcomeIdsError {
    /// The market has no `version`, so the field to read cannot be chosen.
    #[error("the market has no `version`, so its outcome ids cannot be chosen")]
    MissingVersion,
    /// The market's `version` is neither `v1` nor `v2`. Carries the value.
    #[error("the market's protocol version `{0}` is not supported")]
    UnsupportedVersion(String),
    /// The field the version names is absent: `clobTokenIds` for `v1`,
    /// `positionIds` for `v2`.
    #[error("the market carries no outcome ids for its protocol version")]
    MissingIds,
    /// A `v1` market's `clobTokenIds` is not a JSON array of strings.
    #[error("the market's `clobTokenIds` is not a JSON array of strings")]
    MalformedIds,
    /// The market's `outcomes` is absent or not a JSON array of strings.
    #[error("the market's `outcomes` is absent or not a JSON array of strings")]
    MalformedOutcomes,
    /// The market has a different number of outcomes and ids.
    #[error("the market has {outcomes} outcomes and {ids} outcome ids")]
    CountMismatch {
        /// How many labels `outcomes` holds.
        outcomes: usize,
        /// How many ids the version's field holds.
        ids: usize,
    },
    /// An id is empty or not all ASCII digits. Carries the id.
    #[error("the outcome id `{0}` is not a decimal integer")]
    InvalidId(String),
}

/// Every variant is a response that arrived but carries no ids the client can
/// trade.
impl Classify for OutcomeIdsError {
    fn class(&self) -> Class {
        Class::Decode
    }
}
```

`Classify`'s defaults give `is_fault() == true` and `retry_after() == None` (the class's own, which `Decode` has none of).

In `polyoxide-polymarket/src/gamma/mod.rs`, replace

```rust
pub use error::GammaError;
```

with

```rust
pub use error::{GammaError, OutcomeIdsError};
```

In `polyoxide-polymarket/src/lib.rs`, replace

```rust
    is::<gamma::GammaError>();
};
```

with

```rust
    is::<gamma::GammaError>();
    is::<gamma::OutcomeIdsError>();
};
```

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features gamma --test error_classes_gamma`

Expected: `test result: ok. 3 passed; 0 failed`.

Then: `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings` prints no warning, and `cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q` passes (the new `pub enum` is listed in `lib.rs`).

Finally `direnv exec . cargo fmt --all -- --check`; if it reports a diff in a file of this task, run `direnv exec . cargo fmt --all` and keep the result in this task's commit.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(polymarket): OutcomeIdsError, the outcome-id selector's refusals (protocol v2, Story 3)" -- polyoxide-polymarket/src/gamma/error.rs polyoxide-polymarket/src/gamma/mod.rs polyoxide-polymarket/src/lib.rs polyoxide-polymarket/tests/error_classes_gamma.rs
```

### Task 3.2: `OutcomeId` and `Market::outcome_ids`

**Files:**
- Modify: `polyoxide-polymarket/src/gamma/types.rs:1-5` (imports), `:217-242` (the `version` doc comment), and after the `Market` struct (ends `pub event_start_time: Option<String>,` then `}`)
- Modify: `polyoxide-polymarket/Cargo.toml` (a `[[test]]` stanza after `wire_agreement_gamma`)
- Create: `polyoxide-polymarket/tests/outcome_ids_gamma.rs`

- [ ] **Step 1: Write the failing test**

In `polyoxide-polymarket/Cargo.toml`, replace

```toml
[[test]]
name = "wire_agreement_gamma"
required-features = ["gamma"]
```

with

```toml
[[test]]
name = "wire_agreement_gamma"
required-features = ["gamma"]

# `Market::outcome_ids` against py-sdk's selector vectors (Protocol V2).
[[test]]
name = "outcome_ids_gamma"
required-features = ["gamma"]
```

Create `polyoxide-polymarket/tests/outcome_ids_gamma.rs`:

```rust
//! `Market::outcome_ids` chooses a market's trading ids by its `version`.
//!
//! The binary cases are held to py-sdk 0.12.0's
//! `normalize_market_position_context` through
//! `tests/fixtures/gamma/protocol_v2/selector_vectors.json`: the pair py-sdk
//! chose, or a refusal where py-sdk refused. py-sdk cannot express a
//! three-outcome market or a malformed `outcomes`, so those cases assert the
//! refusal or the pairing read straight off the input they were built from.

use polyoxide_polymarket::gamma::{
    types::{Market, OutcomeId},
    OutcomeIdsError,
};
use polyoxide_test_support::fixtures;
use serde_json::Value;

/// The first market of the captured CLOB venue fixture, which the vectors edit.
fn base_market() -> Value {
    fixtures!("clob/venue").json("gamma_markets")[0].clone()
}

fn market_of(value: &Value) -> Market {
    serde_json::from_value(value.clone()).unwrap()
}

/// The base market with only `version`, `clobTokenIds`, `positionIds` and
/// `outcomes` set to what is given, and removed where `None`.
fn market(
    version: Option<&str>,
    clob_token_ids: Option<&str>,
    position_ids: Option<&[&str]>,
    outcomes: Option<&str>,
) -> Market {
    let mut value = base_market();
    let object = value.as_object_mut().unwrap();
    let edits = [
        ("version", version.map(Value::from)),
        ("clobTokenIds", clob_token_ids.map(Value::from)),
        (
            "positionIds",
            position_ids.map(|ids| Value::from(ids.to_vec())),
        ),
        ("outcomes", outcomes.map(Value::from)),
    ];
    for (key, edit) in edits {
        match edit {
            Some(edit) => {
                object.insert(key.to_owned(), edit);
            }
            None => {
                object.remove(key);
            }
        }
    }
    market_of(&value)
}

/// A JSON-text array of strings, as Gamma sends `outcomes` and `clobTokenIds`.
fn json_strings(text: &Value) -> Vec<String> {
    serde_json::from_str(text.as_str().unwrap()).unwrap()
}

fn strings(array: &Value) -> Vec<String> {
    serde_json::from_value(array.clone()).unwrap()
}

fn pairs(outcomes: &[&str], ids: &[&str]) -> Vec<OutcomeId> {
    outcomes
        .iter()
        .zip(ids)
        .map(|(outcome, id)| OutcomeId {
            outcome: (*outcome).to_owned(),
            id: (*id).to_owned(),
        })
        .collect()
}

fn vectors() -> Value {
    fixtures!("gamma/protocol_v2").json("selector_vectors")
}

fn case<'a>(vectors: &'a Value, name: &str) -> &'a Value {
    vectors["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("no selector vector named {name}"))
}

#[test]
fn the_selector_agrees_with_py_sdk_on_every_vector() {
    let vectors = vectors();
    let cases = vectors["cases"].as_array().unwrap();
    let names: Vec<&str> = cases
        .iter()
        .map(|case| case["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "v1_with_position_ids",
            "v2_with_both",
            "missing_version",
            "unknown_version",
            "v2_one_position_id",
            "v1_no_clob_token_ids",
        ]
    );

    let (mut chosen, mut refused) = (0, 0);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let result = market_of(&case["market"]).outcome_ids();
        match &case["ids"] {
            Value::Null => {
                refused += 1;
                assert!(result.is_err(), "{name}: py-sdk refused, got {result:?}");
            }
            pair => {
                chosen += 1;
                let labels = json_strings(&case["market"]["outcomes"]);
                let expected: Vec<OutcomeId> = strings(pair)
                    .into_iter()
                    .zip(labels)
                    .map(|(id, outcome)| OutcomeId { outcome, id })
                    .collect();
                assert_eq!(result.as_ref(), Ok(&expected), "{name}");
            }
        }
    }
    assert_eq!((chosen, refused), (2, 4), "the vectors changed shape");
}

/// py-sdk records only that it refused; which refusal the selector gives is
/// the rule of the design, read off each case's input.
#[test]
fn each_refused_vector_is_refused_for_its_reason() {
    let vectors = vectors();
    let outcomes = |case: &Value| json_strings(&case["market"]["outcomes"]).len();

    let case_of = |name| case(&vectors, name);
    assert_eq!(
        market_of(&case_of("missing_version")["market"]).outcome_ids(),
        Err(OutcomeIdsError::MissingVersion)
    );
    assert_eq!(
        market_of(&case_of("unknown_version")["market"]).outcome_ids(),
        Err(OutcomeIdsError::UnsupportedVersion("v3".to_owned()))
    );
    let one = case_of("v2_one_position_id");
    assert_eq!(
        market_of(&one["market"]).outcome_ids(),
        Err(OutcomeIdsError::CountMismatch {
            outcomes: outcomes(one),
            ids: strings(&one["market"]["positionIds"]).len(),
        })
    );
    assert_eq!(
        market_of(&case_of("v1_no_clob_token_ids")["market"]).outcome_ids(),
        Err(OutcomeIdsError::MissingIds)
    );
}

/// The vectors that carry both fields differ in them, and the version alone
/// picks: `v1` takes `clobTokenIds`, `v2` takes `positionIds`.
#[test]
fn the_version_picks_the_field_when_both_are_present() {
    let vectors = vectors();
    for (name, takes_position_ids) in [("v1_with_position_ids", false), ("v2_with_both", true)] {
        let case = case(&vectors, name);
        let clob = json_strings(&case["market"]["clobTokenIds"]);
        let position = strings(&case["market"]["positionIds"]);
        assert_ne!(clob, position, "{name}: the fields must differ to tell them apart");

        let got: Vec<String> = market_of(&case["market"])
            .outcome_ids()
            .unwrap()
            .into_iter()
            .map(|outcome| outcome.id)
            .collect();
        assert_eq!(got, if takes_position_ids { position } else { clob }, "{name}");
    }
}

#[test]
fn a_market_with_three_outcomes_pairs_each_in_order() {
    let labels = r#"["A","B","C"]"#;
    let v1 = market(Some("v1"), Some(r#"["11","22","33"]"#), None, Some(labels));
    assert_eq!(
        v1.outcome_ids(),
        Ok(pairs(&["A", "B", "C"], &["11", "22", "33"]))
    );
    let v2 = market(Some("v2"), None, Some(&["44", "55", "66"]), Some(labels));
    assert_eq!(
        v2.outcome_ids(),
        Ok(pairs(&["A", "B", "C"], &["44", "55", "66"]))
    );
}

#[test]
fn a_different_number_of_outcomes_and_ids_is_a_count_mismatch() {
    let more_ids = market(
        Some("v1"),
        Some(r#"["1","2","3"]"#),
        None,
        Some(r#"["Yes","No"]"#),
    );
    assert_eq!(
        more_ids.outcome_ids(),
        Err(OutcomeIdsError::CountMismatch {
            outcomes: 2,
            ids: 3
        })
    );
    let fewer_ids = market(Some("v2"), None, Some(&["1"]), Some(r#"["Yes","No"]"#));
    assert_eq!(
        fewer_ids.outcome_ids(),
        Err(OutcomeIdsError::CountMismatch {
            outcomes: 2,
            ids: 1
        })
    );
    // The count is checked before the digits, so a short list of bad ids is a
    // count mismatch.
    let both = market(Some("v1"), Some(r#"["x"]"#), None, Some(r#"["Yes","No"]"#));
    assert_eq!(
        both.outcome_ids(),
        Err(OutcomeIdsError::CountMismatch {
            outcomes: 2,
            ids: 1
        })
    );
}

#[test]
fn an_id_that_is_not_all_ascii_digits_is_refused_with_the_id() {
    // Empty, letters, a sign, a prefix, a fraction, a space and full-width
    // digits are all refused; none is a decimal integer.
    for bad in ["", "abc", "-1", "0x10", "1.5", " 7", "７"] {
        let ids = serde_json::to_string(&["12", bad]).unwrap();
        let v1 = market(Some("v1"), Some(ids.as_str()), None, Some(r#"["Yes","No"]"#));
        assert_eq!(
            v1.outcome_ids(),
            Err(OutcomeIdsError::InvalidId(bad.to_owned())),
            "v1 {bad:?}"
        );
        let v2 = market(Some("v2"), None, Some(&["12", bad]), Some(r#"["Yes","No"]"#));
        assert_eq!(
            v2.outcome_ids(),
            Err(OutcomeIdsError::InvalidId(bad.to_owned())),
            "v2 {bad:?}"
        );
    }
}

#[test]
fn outcomes_that_are_not_a_json_array_of_strings_are_malformed() {
    for outcomes in [
        None,
        Some("not json"),
        Some(r#"{"a":"b"}"#),
        Some("[1,2]"),
        Some(r#"["Yes",null]"#),
    ] {
        let v1 = market(Some("v1"), Some(r#"["1","2"]"#), None, outcomes);
        assert_eq!(
            v1.outcome_ids(),
            Err(OutcomeIdsError::MalformedOutcomes),
            "v1 {outcomes:?}"
        );
        let v2 = market(Some("v2"), None, Some(&["1", "2"]), outcomes);
        assert_eq!(
            v2.outcome_ids(),
            Err(OutcomeIdsError::MalformedOutcomes),
            "v2 {outcomes:?}"
        );
    }
}

#[test]
fn clob_token_ids_that_are_not_a_json_array_of_strings_are_malformed() {
    for ids in ["not json", r#"{"a":"b"}"#, "[1,2]", r#""12""#] {
        // The `positionIds` beside them are fine and never read for `v1`.
        let market = market(
            Some("v1"),
            Some(ids),
            Some(&["1", "2"]),
            Some(r#"["Yes","No"]"#),
        );
        assert_eq!(
            market.outcome_ids(),
            Err(OutcomeIdsError::MalformedIds),
            "{ids:?}"
        );
    }
}

#[test]
fn the_field_the_version_does_not_name_is_never_read() {
    let labels = Some(r#"["Yes","No"]"#);
    // A `v2` market takes `positionIds` whatever `clobTokenIds` holds.
    let v2 = market(Some("v2"), Some("garbage"), Some(&["7", "8"]), labels);
    assert_eq!(v2.outcome_ids(), Ok(pairs(&["Yes", "No"], &["7", "8"])));
    // A `v1` market takes `clobTokenIds` whatever `positionIds` holds.
    let v1 = market(
        Some("v1"),
        Some(r#"["5","6"]"#),
        Some(&["abc", "def", "ghi"]),
        labels,
    );
    assert_eq!(v1.outcome_ids(), Ok(pairs(&["Yes", "No"], &["5", "6"])));
}

#[test]
fn the_field_the_version_names_must_be_present() {
    let labels = Some(r#"["Yes","No"]"#);
    // Present in the other field, absent in the named one.
    let v1 = market(Some("v1"), None, Some(&["1", "2"]), labels);
    assert_eq!(v1.outcome_ids(), Err(OutcomeIdsError::MissingIds));
    let v2 = market(Some("v2"), Some(r#"["1","2"]"#), None, labels);
    assert_eq!(v2.outcome_ids(), Err(OutcomeIdsError::MissingIds));
}

#[test]
fn the_version_is_checked_before_anything_else() {
    // No version, and nothing else either.
    assert_eq!(
        market(None, None, None, None).outcome_ids(),
        Err(OutcomeIdsError::MissingVersion)
    );
    // A version this client does not know, beside ids that would be fine.
    for version in ["v3", "V1", ""] {
        let unknown = market(
            Some(version),
            Some(r#"["1","2"]"#),
            Some(&["1", "2"]),
            Some(r#"["Yes","No"]"#),
        );
        assert_eq!(
            unknown.outcome_ids(),
            Err(OutcomeIdsError::UnsupportedVersion(version.to_owned())),
            "{version:?}"
        );
    }
}

#[test]
fn the_ids_are_read_before_the_outcomes() {
    let both_bad = market(Some("v1"), Some("not json"), None, Some("not json"));
    assert_eq!(both_bad.outcome_ids(), Err(OutcomeIdsError::MalformedIds));
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features gamma --test outcome_ids_gamma`

Expected: it fails to compile, `error[E0432]: unresolved import polyoxide_polymarket::gamma::types::OutcomeId` and `error[E0599]: no method named outcome_ids found for struct Market`.

- [ ] **Step 3: Implement**

In `polyoxide-polymarket/src/gamma/types.rs`, replace

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
```

with

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::error::OutcomeIdsError;
```

Replace the last paragraph of `Market::version`'s doc comment, which now reads

```rust
    /// The `clob` module signs orders for `v1` markets only. A `v2` market
    /// needs a different exchange and signing domain, which it does not
    /// implement yet. See `docs/specs/gamma/OBSERVED.md`.
    pub version: Option<ProtocolVersion>,
```

with

```rust
    /// [`outcome_ids`](Self::outcome_ids) makes this choice for a caller. See
    /// `docs/specs/gamma/OBSERVED.md`.
    pub version: Option<ProtocolVersion>,
```

Insert after the `Market` struct (after the line `    pub event_start_time: Option<String>,` and its closing `}`, before the `/// Market token (outcome)` doc comment):

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
    /// The market's outcomes with the ids they trade under, chosen by
    /// [`version`](Self::version), never by which id field is present.
    ///
    /// A `v1` market's ids are its [`clob_token_ids`](Self::clob_token_ids), a
    /// JSON array of strings, and a `v2` market's its
    /// [`position_ids`](Self::position_ids). The other field is never read,
    /// whatever it holds. The labels are [`outcomes`](Self::outcomes), a JSON
    /// array of strings, paired with the ids in order.
    ///
    /// # Errors
    ///
    /// In this order: [`OutcomeIdsError::MissingVersion`] when `version` is
    /// absent and [`OutcomeIdsError::UnsupportedVersion`] when it is neither
    /// `v1` nor `v2`; [`OutcomeIdsError::MissingIds`] when the version's field
    /// is absent and [`OutcomeIdsError::MalformedIds`] when a `v1` market's
    /// `clobTokenIds` is not a JSON array of strings;
    /// [`OutcomeIdsError::MalformedOutcomes`] when `outcomes` is absent or not
    /// one; [`OutcomeIdsError::CountMismatch`] when the counts differ; and
    /// [`OutcomeIdsError::InvalidId`] for an id that is empty or not all ASCII
    /// digits.
    pub fn outcome_ids(&self) -> Result<Vec<OutcomeId>, OutcomeIdsError> {
        let ids: Vec<String> = match &self.version {
            None => return Err(OutcomeIdsError::MissingVersion),
            Some(ProtocolVersion::Other(version)) => {
                return Err(OutcomeIdsError::UnsupportedVersion(version.clone()));
            }
            Some(ProtocolVersion::V1) => {
                let json = self
                    .clob_token_ids
                    .as_deref()
                    .ok_or(OutcomeIdsError::MissingIds)?;
                serde_json::from_str(json).map_err(|_| OutcomeIdsError::MalformedIds)?
            }
            Some(ProtocolVersion::V2) => self
                .position_ids
                .clone()
                .ok_or(OutcomeIdsError::MissingIds)?,
        };
        let outcomes: Vec<String> = self
            .outcomes
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .ok_or(OutcomeIdsError::MalformedOutcomes)?;
        if outcomes.len() != ids.len() {
            return Err(OutcomeIdsError::CountMismatch {
                outcomes: outcomes.len(),
                ids: ids.len(),
            });
        }
        if let Some(bad) = ids
            .iter()
            .find(|id| id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return Err(OutcomeIdsError::InvalidId(bad.clone()));
        }
        Ok(outcomes
            .into_iter()
            .zip(ids)
            .map(|(outcome, id)| OutcomeId { outcome, id })
            .collect())
    }
}
```

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features gamma --test outcome_ids_gamma`

Expected: `test result: ok. 12 passed; 0 failed`. If `the_selector_agrees_with_py_sdk_on_every_vector` fails on `(chosen, refused)`, the fixture is not Story 1's: stop and report, never edit the assertion.

Then `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings` prints no warning.

Finally `direnv exec . cargo fmt --all -- --check`; if it reports a diff in a file of this task, run `direnv exec . cargo fmt --all` and keep the result in this task's commit.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-polymarket/tests/outcome_ids_gamma.rs
git commit -m "feat(polymarket): Market::outcome_ids chooses a market's trading ids by its version (protocol v2, Story 3)" -- polyoxide-polymarket/src/gamma/types.rs polyoxide-polymarket/Cargo.toml polyoxide-polymarket/tests/outcome_ids_gamma.rs
```

### Task 3.3: `event_group` and `event_id_of`

**Files:**
- Modify: `polyoxide-polymarket/src/clob/venue.rs:380-510` (the `events` module)
- Test: `polyoxide-polymarket/tests/venue_clob.rs` (replace `event_group_leaves_out_the_outcomes_of_a_v2_market`)

The existing test `event_group_leaves_out_the_outcomes_of_a_v2_market` flips market 1 to `v2` with no `positionIds`; under the selector that market is now a `MissingIds` failure, so the test is replaced as the spec says (`tests/venue_clob.rs` postdates the `v0.40.0` baseline, so the test-bodies gate tracks none of it and no `ci/test-body-diff.toml` entry is added). Every other `event_group` test in the file keeps its body and still passes: its markets are `v1`, and its failures (`["Yes"]` against two ids; `"not-a-token-id"`) are now `CountMismatch` and `InvalidId`, both `Class::Decode`, which is all they assert.

- [ ] **Step 1: Write the failing tests**

In `polyoxide-polymarket/tests/venue_clob.rs`, replace the whole of

```rust
#[tokio::test]
async fn event_group_leaves_out_the_outcomes_of_a_v2_market() {
    let mut server = Server::new_async().await;
    let markets = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={}&closed=any", token()),
        &fixture("gamma_markets"),
        1,
    )
    .await;
    let body = event_body(|markets| markets[1]["version"] = "v2".into());
    let event = route(
        &mut server,
        &format!("/events/{}", event_id()),
        "",
        &body,
        1,
    )
    .await;
    let v2_condition = json("gamma_event")["markets"][1]["conditionId"]
        .as_str()
        .unwrap()
        .to_owned();

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue.event_group(&key()).await.unwrap();
    markets.assert_async().await;
    event.assert_async().await;

    let conditions: Vec<&str> = group.outcomes.iter().map(|o| o.market.as_str()).collect();
    assert!(
        !conditions.contains(&v2_condition.as_str()),
        "{conditions:?}"
    );
    assert_eq!(group.outcomes.len(), 4, "the two v1 markets' Yes and No");
}
```

with

```rust
/// A Protocol V2 outcome id of Story 1's `id_vectors.json`, by its name.
fn v2_id(name: &str) -> String {
    fixtures!("clob/protocol_v2").json("id_vectors")["ids"][name]
        .as_str()
        .unwrap()
        .to_owned()
}

fn clob_key(id: &str) -> MarketKey {
    keys::clob(&TokenId::new(id).unwrap())
}

#[tokio::test]
async fn event_group_keys_a_v2_market_by_its_position_ids() {
    let mut server = Server::new_async().await;
    let markets = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={}&closed=any", token()),
        &fixture("gamma_markets"),
        1,
    )
    .await;
    let (yes, no) = (v2_id("v2_binary"), v2_id("v2_neg_risk"));
    let body = event_body(|markets| {
        markets[1]["version"] = "v2".into();
        markets[1]["positionIds"] = Value::from(vec![yes.clone(), no.clone()]);
    });
    let event = route(
        &mut server,
        &format!("/events/{}", event_id()),
        "",
        &body,
        1,
    )
    .await;
    let v2_condition = json("gamma_event")["markets"][1]["conditionId"]
        .as_str()
        .unwrap()
        .to_owned();
    // The market still carries its v1 token ids; the version, not their
    // presence, decides which ids the group lists.
    let v1_tokens: Vec<String> = serde_json::from_str(
        json("gamma_event")["markets"][1]["clobTokenIds"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue.event_group(&key()).await.unwrap();
    markets.assert_async().await;
    event.assert_async().await;

    let theirs: Vec<(MarketKey, &str)> = group
        .outcomes
        .iter()
        .filter(|o| o.market == v2_condition)
        .map(|o| (o.key.clone(), o.label.as_str()))
        .collect();
    assert_eq!(
        theirs,
        [(clob_key(&yes), "Yes"), (clob_key(&no), "No")],
        "the v2 market's outcomes, keyed by its position ids"
    );
    for token in &v1_tokens {
        assert!(
            group.outcomes.iter().all(|o| o.key != clob_key(token)),
            "the v2 market's clobTokenIds must not be listed"
        );
    }
    assert_eq!(group.outcomes.len(), 6, "three markets' Yes and No");
}

#[tokio::test]
async fn event_group_skips_a_market_with_no_known_version() {
    // `v3` is a protocol this client does not know; a missing version is none
    // at all. Either way the market's outcomes are absent and the group still
    // returns.
    for version in [Some("v3"), None] {
        let mut server = Server::new_async().await;
        let markets = route(
            &mut server,
            "/markets",
            &format!("clob_token_ids={}&closed=any", token()),
            &fixture("gamma_markets"),
            1,
        )
        .await;
        let body = event_body(|markets| match version {
            Some(version) => markets[1]["version"] = version.into(),
            None => {
                markets[1].as_object_mut().unwrap().remove("version");
            }
        });
        let event = route(
            &mut server,
            &format!("/events/{}", event_id()),
            "",
            &body,
            1,
        )
        .await;
        let skipped = json("gamma_event")["markets"][1]["conditionId"]
            .as_str()
            .unwrap()
            .to_owned();

        let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
        let group = venue.event_group(&key()).await.unwrap();
        markets.assert_async().await;
        event.assert_async().await;

        let conditions: Vec<&str> = group.outcomes.iter().map(|o| o.market.as_str()).collect();
        assert!(
            !conditions.contains(&skipped.as_str()),
            "{version:?}: {conditions:?}"
        );
        assert_eq!(
            group.outcomes.len(),
            4,
            "{version:?}: the two v1 markets' Yes and No"
        );
    }
}

#[tokio::test]
async fn event_group_finds_a_v2_key_by_its_position_ids() {
    let mut server = Server::new_async().await;
    let id = v2_id("v2_binary");
    // A V2 key asks for `position_ids=`; asking for `clob_token_ids=` would
    // find no market, since a V2 outcome is not a clob token. The exact query
    // matches that wrong request, so `expect(0)` is not vacuous.
    let by_position = route(
        &mut server,
        "/markets",
        &format!("position_ids={id}&closed=any"),
        &fixture("gamma_markets"),
        1,
    )
    .await;
    let by_clob_token = route(
        &mut server,
        "/markets",
        &format!("clob_token_ids={id}&closed=any"),
        "[]",
        0,
    )
    .await;
    let body = event_body(|markets| {
        markets[0]["version"] = "v2".into();
        markets[0]["positionIds"] = Value::from(vec![id.clone(), v2_id("v2_neg_risk")]);
    });
    let event = route(
        &mut server,
        &format!("/events/{}", event_id()),
        "",
        &body,
        1,
    )
    .await;

    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue.event_group(&clob_key(&id)).await.unwrap();
    by_position.assert_async().await;
    by_clob_token.assert_async().await;
    event.assert_async().await;

    assert_eq!(group.id, event_id());
    let outcome = group
        .outcomes
        .iter()
        .find(|o| o.key == clob_key(&id))
        .expect("the V2 key's own outcome is in its event group");
    assert_eq!(outcome.label, "Yes");
}
```

(`expect` on `Option` in a mock test is fine; `live_unwraps.py` reads live tests only.)

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --features clob,gamma,data --test venue_clob event_group`

Expected: `event_group_keys_a_v2_market_by_its_position_ids` fails (`the v2 market's outcomes, keyed by its position ids`: left is empty, as today's code skips a non-`v1` market), `event_group_skips_a_market_with_no_known_version` passes (today's code already skips it) and `event_group_finds_a_v2_key_by_its_position_ids` fails: the code sent `clob_token_ids=`, so the `expect(0)` route is hit and the group is `NotListed`, which the `unwrap` panics on.

- [ ] **Step 3: Implement**

In `polyoxide-polymarket/src/clob/venue.rs`, inside `mod events`, replace the imports

```rust
    use super::decode;
    use crate::clob::Clob;
    use crate::gamma::{
        types::{ClosedFilter, ProtocolVersion},
        GammaError,
    };
    use crate::shared::keys::{self, TokenId};
```

with

```rust
    use crate::clob::Clob;
    use crate::gamma::{types::ClosedFilter, GammaError, OutcomeIdsError};
    use crate::shared::keys::{self, TokenId};
    use crate::shared::protocol;
```

Replace the `strings` helper and `event_id_of`:

```rust
    /// A JSON-encoded array of strings, as Gamma sends a market's outcomes
    /// and token ids.
    fn strings(json: Option<&str>, what: &str) -> Result<Vec<String>, ClassifiedError> {
        json.map_or(Ok(Vec::new()), |json| {
            serde_json::from_str(json).map_err(|_| decode(what))
        })
    }

    impl Clob {
        /// The id of the first event Gamma lists for a token's market, open
        /// or closed: Gamma leaves a closed market out unless asked for it,
        /// and `closed=any` asks for both states, which it takes with the
        /// token as the identifier filter it requires.
        async fn event_id_of(&self, token: &TokenId) -> Result<Option<String>, ClassifiedError> {
            let markets = Request::<Vec<MarketEvents>, GammaError>::new(
                self.gamma.http_client.clone(),
                "/markets",
            )
            .query("clob_token_ids", token.as_str())
            .query("closed", ClosedFilter::Any)
```

with

```rust
    impl Clob {
        /// The id of the first event Gamma lists for a token's market, open
        /// or closed: Gamma leaves a closed market out unless asked for it,
        /// and `closed=any` asks for both states, which it takes with the
        /// token as the identifier filter it requires. A Protocol V2 id is a
        /// position id, which Gamma filters by `position_ids`; any other is a
        /// clob token id.
        async fn event_id_of(&self, token: &TokenId) -> Result<Option<String>, ClassifiedError> {
            let filter = if protocol::is_v2_position_id(token.as_str()) {
                "position_ids"
            } else {
                "clob_token_ids"
            };
            let markets = Request::<Vec<MarketEvents>, GammaError>::new(
                self.gamma.http_client.clone(),
                "/markets",
            )
            .query(filter, token.as_str())
            .query("closed", ClosedFilter::Any)
```

Replace the market loop of `event_group`:

```rust
            for market in &event.markets {
                // Only a v1 market's outcomes are the CLOB's tokens.
                if market.version != Some(ProtocolVersion::V1) {
                    continue;
                }
                let tokens = strings(
                    market.clob_token_ids.as_deref(),
                    "token ids that are not a JSON array of strings",
                )?;
                let labels = strings(
                    market.outcomes.as_deref(),
                    "outcomes that are not a JSON array of strings",
                )?;
                if tokens.len() != labels.len() {
                    return Err(decode(
                        "a market whose token ids and outcomes differ in number",
                    ));
                }
                for (token, label) in tokens.into_iter().zip(labels) {
                    // A token id no key can carry is a response this client
                    // cannot read, not a value the caller sent.
                    let token = TokenId::new(token)
                        .map_err(|err| ClassifiedError::new(Class::Decode, err))?;
                    let mut outcome =
                        EventOutcome::new(keys::clob(&token), market.condition_id.clone(), label);
```

with

```rust
            for market in &event.markets {
                let outcomes = match market.outcome_ids() {
                    Ok(outcomes) => outcomes,
                    // A market on a protocol this client does not know has no
                    // outcomes the CLOB trades here.
                    Err(
                        OutcomeIdsError::MissingVersion | OutcomeIdsError::UnsupportedVersion(_),
                    ) => continue,
                    Err(err) => return Err(ClassifiedError::new(Class::Decode, err)),
                };
                for outcome_id in outcomes {
                    // An id no key can carry is a response this client cannot
                    // read, not a value the caller sent.
                    let token = TokenId::new(outcome_id.id)
                        .map_err(|err| ClassifiedError::new(Class::Decode, err))?;
                    let mut outcome = EventOutcome::new(
                        keys::clob(&token),
                        market.condition_id.clone(),
                        outcome_id.outcome,
                    );
```

The rest of the loop body (the `outcome.market_title = ...` statement, `group.outcomes.push(outcome);` and the closing braces) is unchanged.

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --features clob,gamma,data --test venue_clob`

Expected: `test result: ok. 19 passed; 0 failed` (the 17 tests of the file, the replaced one now three).

Then `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings` prints no warning (no unused import: `decode` is not imported any more, since nothing left in `mod events` calls it, and `ProtocolVersion` is gone from this module), and `direnv exec . env CARGO_INCREMENTAL=0 cargo build -p polyoxide-polymarket --no-default-features --features clob` builds (`shared::protocol` is Story 2's, under `clob`; the `events` module is gated on `gamma`, which this build leaves out).

Finally `direnv exec . cargo fmt --all -- --check`; if it reports a diff in a file of this task, run `direnv exec . cargo fmt --all` and keep the result in this task's commit.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(polymarket): event groups list V2 outcomes by position id (protocol v2, Story 3)" -- polyoxide-polymarket/src/clob/venue.rs polyoxide-polymarket/tests/venue_clob.rs
```

### Task 3.4: The CLI's `clob prices` discovery reads V2 position ids

**Files:**
- Modify: `polyoxide-cli/src/commands/polymarket/clob/prices/select.rs:1-6` (the import) and `:51-93` (`discover_targets`)
- Modify: `.claude/rules/cli.md` (the `## polymarket clob prices download` section, tree :77-83)
- Test: `polyoxide-cli/tests/prices_discovery.rs` (append after the last test, `discover_extracts_token_ids_from_markets`, which stays unedited)

- [ ] **Step 1: Write the failing test**

Append to `polyoxide-cli/tests/prices_discovery.rs`, after `discover_extracts_token_ids_from_markets`:

```rust

/// A Protocol V2 market's `clobTokenIds` has no book: discovery takes each
/// market's ids from the field its `version` names. Every expected id is read
/// off the JSON the server serves, so the test cannot agree with the code by
/// restating a constant.
#[tokio::test]
async fn discover_picks_the_id_field_by_the_market_version() {
    fn market(
        id: &str,
        version: Option<&str>,
        clob: &[&str],
        position: &[&str],
    ) -> serde_json::Value {
        let mut market = serde_json::json!({
            "id": id,
            "conditionId": format!("c{id}"),
            "question": format!("q{id}"),
            "marketMakerAddress": "0x1",
            "outcomes": "[\"Yes\",\"No\"]",
            "clobTokenIds": serde_json::to_string(clob).unwrap(),
            "positionIds": position,
        });
        if let Some(version) = version {
            market["version"] = version.into();
        }
        market
    }
    let served = serde_json::json!([
        market("1", Some("v1"), &["111", "112"], &["911", "912"]),
        market("2", Some("v2"), &["221", "222"], &["921", "922"]),
        market("3", None, &["331", "332"], &["931", "932"]),
        market("4", Some("v3"), &["441", "442"], &["941", "942"]),
    ]);

    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/markets")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(served.to_string())
        .create_async()
        .await;

    let gamma = polyoxide_polymarket::gamma::Gamma::builder()
        .base_url(server.url())
        .build()
        .unwrap();
    let ids = discover_targets(&gamma, &DiscoverFilters::default())
        .await
        .unwrap();

    let clob = |index: usize| -> Vec<String> {
        serde_json::from_str(served[index]["clobTokenIds"].as_str().unwrap()).unwrap()
    };
    let position = |index: usize| -> Vec<String> {
        serde_json::from_value(served[index]["positionIds"].clone()).unwrap()
    };
    let mut expected = clob(0);
    expected.extend(position(1));
    expected.extend(clob(2));
    assert_eq!(ids, expected);
    assert_eq!(ids, ["111", "112", "921", "922", "331", "332"]);

    // Nothing of the `v2` market's `clobTokenIds`, and nothing of the `v3` market.
    for left_out in clob(1).into_iter().chain(clob(3)).chain(position(3)) {
        assert!(!ids.contains(&left_out), "{left_out} must not be discovered");
    }
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-cli --test prices_discovery`

Expected: `discover_extracts_token_ids_from_markets` passes and `discover_picks_the_id_field_by_the_market_version` fails at `assert_eq!(ids, expected)`: the code reads `clobTokenIds` for every market, so `left` holds `["111", "112", "221", "222", "331", "332", "441", "442"]` against `right` `["111", "112", "921", "922", "331", "332"]`.

- [ ] **Step 3: Implement**

In `polyoxide-cli/src/commands/polymarket/clob/prices/select.rs`, replace the import

```rust
use polyoxide_polymarket::gamma::Gamma;
```

with

```rust
use polyoxide_polymarket::gamma::{types::ProtocolVersion, Gamma};
```

Replace the doc comment of `discover_targets` and its signature line

```rust
/// Discover token ids via `gamma.markets().list()`, applying the given filters.
///
/// Each returned market's `clob_token_ids` (a JSON-encoded string array) is
/// parsed and flattened. Markets with a missing or unparseable field are
/// skipped with a warning to stderr.
pub async fn discover_targets(gamma: &Gamma, filters: &DiscoverFilters) -> Result<Vec<String>> {
```

with

```rust
/// Discover token ids via `gamma.markets().list()`, applying the given filters.
///
/// The field each market's ids come from depends on its `version`. A `v2`
/// market's ids are the ids of its `Market::outcome_ids`, which reads
/// `position_ids` (the CLOB has no book for a `v2` market's `clob_token_ids`).
/// A `v1` market, and a market with no `version`, take `clob_token_ids` (a
/// JSON-encoded string array), parsed and flattened. A market of any other
/// version is skipped with a warning naming it and its version, and so is a
/// market whose field is missing or unparseable (or, for `v2`, which the
/// selector refuses), with a warning to stderr.
pub async fn discover_targets(gamma: &Gamma, filters: &DiscoverFilters) -> Result<Vec<String>> {
```

and replace the loop

```rust
    let mut ids = Vec::new();
    for market in markets {
        match market.clob_token_ids.as_deref() {
            Some(raw) => match parse_clob_token_ids(raw) {
                Ok(parsed) => ids.extend(parsed),
                Err(e) => eprintln!("warning: skipping market {}: {e}", market.id),
            },
            None => eprintln!("warning: skipping market {} (no clob_token_ids)", market.id),
        }
    }
    Ok(ids)
```

with

```rust
    let mut ids = Vec::new();
    for market in markets {
        match &market.version {
            Some(ProtocolVersion::V2) => match market.outcome_ids() {
                Ok(outcomes) => ids.extend(outcomes.into_iter().map(|outcome| outcome.id)),
                Err(e) => eprintln!("warning: skipping market {}: {e}", market.id),
            },
            Some(ProtocolVersion::Other(version)) => eprintln!(
                "warning: skipping market {} (unsupported protocol version {version})",
                market.id
            ),
            // `v1` and a market with no `version` (`ProtocolVersion` is
            // `#[non_exhaustive]`, so the arm is a wildcard).
            _ => match market.clob_token_ids.as_deref() {
                Some(raw) => match parse_clob_token_ids(raw) {
                    Ok(parsed) => ids.extend(parsed),
                    Err(e) => eprintln!("warning: skipping market {}: {e}", market.id),
                },
                None => eprintln!("warning: skipping market {} (no clob_token_ids)", market.id),
            },
        }
    }
    Ok(ids)
```

In `.claude/rules/cli.md`, in the `## polymarket clob prices download` section, replace

```markdown
`manifest.jsonl`. Parquet output requires building the CLI with the `parquet` feature. Its
`--format csv|jsonl|parquet` is a different `OutputFormat`, in `polymarket::clob::prices::types`.
```

with

```markdown
`manifest.jsonl`. Parquet output requires building the CLI with the `parquet` feature. Its
`--format csv|jsonl|parquet` is a different `OutputFormat`, in `polymarket::clob::prices::types`.

Market discovery (`select::discover_targets`) picks each Gamma market's ids by its `version`: a
`v2` market's are its position ids, read through `Market::outcome_ids` (a selector refusal skips
the market with a stderr warning), since the CLOB has no book for its `clob_token_ids`; a `v1`
market's, and an unversioned market's, are its `clob_token_ids`; a market of another version is
skipped with a warning.
```

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-cli --test prices_discovery`

Expected: `test result: ok. 2 passed; 0 failed`.

Then `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-cli --all-targets --all-features -- -D warnings` prints no warning, `direnv exec . cargo fmt --all -- --check` reports no diff in these files (if it does, run `direnv exec . cargo fmt --all` and keep the result in this task's commit), and `cd .github/scripts && uv run pytest tests/test_claude_rules.py -q` passes (`cli.md` already matches `polyoxide-cli/**`).

- [ ] **Step 5: Commit**

```bash
git commit -m "fix(cli): clob prices discovery reads a V2 market's position ids (protocol v2, Story 3)" -- polyoxide-cli/src/commands/polymarket/clob/prices/select.rs polyoxide-cli/tests/prices_discovery.rs .claude/rules/cli.md
```

### Task 3.5: Rules and the gamma observation

**Files:**
- Modify: `.claude/rules/polymarket-gamma.md` (the last section)
- Modify: `.claude/rules/polymarket-clob.md:82-84`
- Modify: `docs/specs/gamma/OBSERVED.md` ("What polyoxide does" of the Protocol V2 section)

- [ ] **Step 1: Write the docs**

In `.claude/rules/polymarket-gamma.md`, replace the section body

```markdown
`Market::version` says which protocol a market trades on. A `v2` market takes its outcome ids from
`position_ids`, signs orders for ExchangeV3 (domain version `"3"`) and reads balances as
`CONDITIONAL-V2`. The `clob` module signs domain `"2"` only, which is correct for `v1` markets.
Choose ids by `version`, never by which field is present: 35% of open `v1` markets also carry
`position_ids`, and the CLOB has no book for them. No `v2` market existed on 2026-10-07; trading
support is tracked in #51. Upstream's guide is `docs.polymarket.com/migrate/polymarket-v2/`; the
wire facts are in `docs/specs/gamma/OBSERVED.md`.
```

with

```markdown
`Market::version` says which protocol a market trades on. A `v2` market takes its outcome ids from
`position_ids`, signs orders for ExchangeV3 (domain version `"3"`) and reads balances as
`CONDITIONAL-V2`; trading it is supported (#51), pending live verification, since no `v2` market
existed on 2026-10-07 and `live_v2_market_is_served` reports `environmental` until one does.

`Market::outcome_ids` is the one place that chooses the ids: `clob_token_ids` (JSON text) for `v1`,
`position_ids` for `v2`, the other field never read. Never choose by which field is present: 35% of
open `v1` markets also carry `position_ids`, and the CLOB has no book for them. It pairs the ids
with `outcomes` in order, and refuses with `OutcomeIdsError` (`gamma::OutcomeIdsError`, not a
`GammaError` variant, which decodes only through its `From<ApiError>`): `MissingVersion`,
`UnsupportedVersion` (any other `version`, which arrives as `ProtocolVersion::Other`), `MissingIds`,
`MalformedIds`, `MalformedOutcomes`, `CountMismatch` and `InvalidId` (an id that is empty or not all
ASCII digits). Each is `Class::Decode`, a fault, with no wait. The binary cases are pinned to
py-sdk's `normalize_market_position_context` by `tests/fixtures/gamma/protocol_v2/`
(`scripts/capture_polymarket_protocol_v2.py`); a three-outcome market and a malformed `outcomes`,
which py-sdk cannot express, are Rust-only cases in `tests/outcome_ids_gamma.rs`.

A `v2` market's `resolution_status` replaces `uma_resolution_status`: read the first for `v2` and
the second for `v1`. Upstream's guide is `docs.polymarket.com/migrate/polymarket-v2/`; the wire
facts are in `docs/specs/gamma/OBSERVED.md`.
```

In `.claude/rules/polymarket-clob.md`, replace

```markdown
- `event_group` reads the event id from Gamma's `/markets?clob_token_ids=` (a private decode,
  since Gamma's `Market` does not model its `events`) with `closed=any`, since Gamma leaves closed
  markets out unless asked (DRIFT R13), then `/events/{id}`, keeping only `v1` markets' outcomes.
```

with

```markdown
- `event_group` reads the event id from Gamma's `/markets?clob_token_ids=` (a private decode,
  since Gamma's `Market` does not model its `events`), or `?position_ids=` when the key's id
  passes the id rule (`shared::protocol::is_v2_position_id`), with `closed=any`, since Gamma leaves
  closed markets out unless asked (DRIFT R13), then `/events/{id}`. It reads every market's
  outcomes through `Market::outcome_ids`, so a `v2` market's outcomes are listed keyed by their
  position ids beside the `v1` markets' token ids (event groups did not list V2 outcomes before
  #51). A market whose `version` is missing or unknown is skipped; any other selector refusal
  fails the group as `Decode`.
```

In `docs/specs/gamma/OBSERVED.md`, in the Protocol V2 section's "What polyoxide does" paragraph, replace

```markdown
because neither schema lists it in `required`. Trading a V2 market
(ExchangeV3 signing, `CONDITIONAL-V2`, the new approvals) is not implemented.
```

with

```markdown
because neither schema lists it in `required`. `Market::outcome_ids` chooses
the trading ids by `version` (`clobTokenIds` for `v1`, `positionIds` for `v2`,
the other field never read) and refuses a missing or unknown version, so a
caller cannot take the `v1` market's useless `positionIds` for a book. Trading
a V2 market is implemented (ExchangeV3 signing, `CONDITIONAL-V2` balance
reads, `event_group`'s V2 outcomes and the Router's split, merge and redeem),
pending live verification: no `v2` market had been served when this was
written, and `live_v2_market_is_served` in
`polyoxide-polymarket/tests/live_clob.rs` is the probe that reads one's book,
tick size and neg-risk flag when it appears.
```

- [ ] **Step 2: Run the rules check**

Run: `cd .github/scripts && uv run pytest tests/test_claude_rules.py -q`

Expected: all pass (no `paths:` globs changed: `polymarket-gamma.md` already matches `src/gamma/**`, `tests/*gamma*.rs` and `tests/fixtures/gamma/**`).

- [ ] **Step 3: Run the story gates**

Run, in order: `direnv exec . cargo fmt --all -- --check`; `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket -p polyoxide -p polyoxide-cli --all-targets --all-features -- -D warnings`; `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket -p polyoxide -p polyoxide-cli --all-features`; `direnv exec . bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-polymarket -p polyoxide-cli --all-features'`; `python3 scripts/gen_registry.py --check`; `python3 scripts/venue_gate.py`; `python3 scripts/live_unwraps.py`; `python3 scripts/test_body_diff.py --baseline v0.40.0`.

Expected: every one passes. The doc build resolves the new intra-doc links in `outcome_ids`'s docs, all to `pub` items.

- [ ] **Step 4: Commit**

```bash
git commit -m "docs(polymarket): the outcome-id selector and V2 event groups (protocol v2, Story 3)" -- .claude/rules/polymarket-gamma.md .claude/rules/polymarket-clob.md docs/specs/gamma/OBSERVED.md
```

## Story 4: Relay: `V2PositionId`, condition narrowing, Router calldata, calls and submits

- **Developer:** intermediate-dev
- **Files:** `polyoxide-polymarket/src/shared/mod.rs`, `polyoxide-polymarket/src/shared/protocol.rs`, `polyoxide-polymarket/src/relay/types.rs`, `polyoxide-polymarket/src/relay/mod.rs`, `polyoxide-polymarket/src/relay/deposit_wallet.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/tests/mock_relay.rs`, `.claude/rules/polymarket-relay.md`, `polyoxide-polymarket/src/relay/README.md`, `docs/specs/session-keys/README.md`
- **Interfaces:** exposes "Story 4 → Story 5"; uses Story 1's `router_vectors.json` and `id_vectors.json`, and Story 2's `shared::protocol`
- **Verify:** `bash -c 'cargo test -p polyoxide-polymarket --all-features && cargo test -p polyoxide-polymarket --no-default-features --features relay --lib && cargo test -p polyoxide-polymarket --no-default-features --features relay --test mock_relay && cargo test -p polyoxide-polymarket --no-default-features --features clob --lib && cargo test --doc -p polyoxide-polymarket --features relay && python3 scripts/test_body_diff.py --baseline v0.40.0 && cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_module_readmes.py -q'`
- **Review:** deep (new public relay API whose calldata moves funds on chain)

The relay learns Protocol V2's Router. `shared::protocol` (Story 2's, crate-private) widens from `clob` to `any(clob, relay)`, with `is_v2_position_id` alone staying under `clob` since only the CLOB asks it. On its `parse_position_id` and `decode_v2_outcome_position_id`, `relay::types` gains `V2PositionId` (the one constructor, `parse`, refuses with py-sdk's three messages) and `parse_v2_condition_id` (py-sdk's `_protocol_v2_condition_id_bytes`). `relay::deposit_wallet` gains the three pure calldata builders, declared through `sol!` as `split(bytes31,uint256)`, `merge(bytes31,uint256)` and `redeem(bytes31,uint256,uint256)` (py-sdk's argument order: condition, outcome index, amount). `RelayClient` gains the private `PROTOCOL_V2_ROUTER`, three `v2_*_call` builders that refuse any chain but 137, and three `submit_v2_*` calls that send that one call through `execute(vec![tx], None)` with `CALL_OPERATION`, so one path serves a Safe, a Proxy and a Deposit Wallet. Every calldata row, condition narrowing, position-id decode and the batch digest is pinned to the py-sdk 0.12.0 fixtures of Story 1; no expected value is computed by the Rust code. No landmine marker appears in this story's code. Amounts are any `U256` (zero included), an outcome index is 0 or 1 by `V2PositionId::parse` alone. The docs task also rewords `docs/specs/session-keys/README.md:151-152` (the V2-router sentence) to point at the V2 helpers.

### Task 4.1: Widen `shared::protocol`; add `V2PositionId` and `parse_v2_condition_id`

**Files:**
- Modify: `polyoxide-polymarket/src/shared/mod.rs`
- Modify: `polyoxide-polymarket/src/shared/protocol.rs`
- Modify: `polyoxide-polymarket/src/relay/types.rs:1-2`
- Modify: `polyoxide-polymarket/src/relay/mod.rs:115-155`
- Test: `polyoxide-polymarket/src/relay/deposit_wallet.rs` (tests module, after its last test)

- [ ] **Step 1: Write the failing tests**

In `polyoxide-polymarket/src/relay/deposit_wallet.rs`, add two imports to the tests module. Quote of the current lines:

```rust
    use alloy::signers::local::PrivateKeySigner;
    use alloy::signers::Signer as _;

    const VECTORS: &str =
```

Replace with:

```rust
    use alloy::signers::local::PrivateKeySigner;
    use alloy::signers::Signer as _;

    use crate::relay::error::RelayError;
    use crate::relay::types::{parse_v2_condition_id, V2PositionId};

    const VECTORS: &str =
```

Then append, after the last test of the module (`session_lifetime_is_the_venues_fixed_value`, whose closing `}` is followed by the module's closing `}`), these items inside the module:

```rust

    // ── Protocol V2: position ids and condition ids ─────────────

    const ROUTER_VECTORS: &str =
        include_str!("../../tests/fixtures/relay/protocol_v2/router_vectors.json");
    const ID_VECTORS: &str = include_str!("../../tests/fixtures/clob/protocol_v2/id_vectors.json");

    /// py-sdk's refusals of a position id, verbatim.
    const POSITION_UINT256: &str = "Position ID must be a uint256 value";
    const POSITION_MODULE: &str = "Position ID must use a supported protocol v2 module";
    const POSITION_YES_NO: &str = "Protocol v2 position ID must be a YES/NO position ID";
    /// py-sdk's refusal of a condition id, verbatim.
    const CONDITION_REFUSED: &str =
        "Protocol v2 condition ID must be bytes31, or bytes32 with a binary outcome byte";

    fn router_vectors() -> serde_json::Value {
        serde_json::from_str(ROUTER_VECTORS).unwrap()
    }

    fn id_vectors() -> serde_json::Value {
        serde_json::from_str(ID_VECTORS).unwrap()
    }

    /// The message of a refusal made before anything was sent.
    fn refusal(err: RelayError) -> String {
        match err {
            RelayError::Api(polyoxide_core::ApiError::Validation(message)) => message,
            other => panic!("expected a validation refusal, got {other:?}"),
        }
    }

    #[test]
    fn v2_position_id_parse_agrees_with_py_sdk_on_every_row() {
        let v = id_vectors();
        let rows = v["position_ids"].as_array().unwrap();
        assert!(rows.len() >= 20, "{} rows", rows.len());
        let mut accepted = 0;
        let mut refused = 0;
        for row in rows {
            let input = row["input"].as_str().unwrap();
            match (&row["decoded"], V2PositionId::parse(input)) {
                (serde_json::Value::Null, Err(err)) => {
                    refused += 1;
                    let message = refusal(err);
                    assert!(
                        [POSITION_UINT256, POSITION_MODULE, POSITION_YES_NO]
                            .contains(&message.as_str()),
                        "{input:?}: {message}"
                    );
                }
                (decoded, Ok(position)) if !decoded.is_null() => {
                    accepted += 1;
                    assert_eq!(
                        format!("0x{}", hex::encode(position.condition_id())),
                        decoded["condition_id"].as_str().unwrap(),
                        "{input:?}"
                    );
                    assert_eq!(
                        u64::from(position.outcome_index()),
                        decoded["outcome_index"].as_u64().unwrap(),
                        "{input:?}"
                    );
                }
                (decoded, result) => {
                    panic!("{input:?}: py-sdk decoded {decoded}, parse gave {result:?}")
                }
            }
        }
        assert!(accepted >= 6, "{accepted} accepted rows");
        assert!(refused >= 8, "{refused} refused rows");
    }

    #[test]
    fn v2_position_id_refusals_carry_py_sdks_messages() {
        let v2: U256 = id_vectors()["ids"]["v2_binary"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        // The id with one byte of its 32 replaced: index 0 is the module byte,
        // index 31 the outcome byte.
        let with = |index: usize, byte: u8| {
            let mut bytes = v2.to_be_bytes::<32>();
            bytes[index] = byte;
            U256::from_be_bytes(bytes).to_string()
        };
        let two_pow_256 =
            "115792089237316195423570985008687907853269984665640564039457584007913129639936";
        for (input, message) in [
            ("abc".to_string(), POSITION_UINT256),
            (String::new(), POSITION_UINT256),
            ("-1".to_string(), POSITION_UINT256),
            (two_pow_256.to_string(), POSITION_UINT256),
            ("1".to_string(), POSITION_MODULE),
            ("100".to_string(), POSITION_MODULE),
            (with(0, 0), POSITION_MODULE),
            (with(0, 4), POSITION_MODULE),
            (with(31, 2), POSITION_YES_NO),
            // py-sdk checks the module before the outcome.
            ({
                let mut bytes = v2.to_be_bytes::<32>();
                bytes[0] = 0;
                bytes[31] = 2;
                U256::from_be_bytes(bytes).to_string()
            }, POSITION_MODULE),
        ] {
            let err = V2PositionId::parse(&input).expect_err(&input);
            assert_eq!(refusal(err), message, "{input}");
        }
    }

    #[test]
    fn v2_position_id_accepts_the_neg_risk_module_and_outcome_one() {
        let ids = &id_vectors()["ids"];
        let v2: U256 = ids["v2_binary"].as_str().unwrap().parse().unwrap();
        let with = |index: usize, byte: u8| {
            let mut bytes = v2.to_be_bytes::<32>();
            bytes[index] = byte;
            U256::from_be_bytes(bytes).to_string()
        };
        let base = V2PositionId::parse(ids["v2_binary"].as_str().unwrap()).unwrap();
        let other_module = V2PositionId::parse(&with(0, 2)).unwrap();
        let sibling = V2PositionId::parse(&with(31, 1)).unwrap();
        assert_eq!(base.outcome_index(), 0);
        assert_eq!(sibling.outcome_index(), 1);
        // Only the outcome byte moved, so the condition is the same; the module byte is
        // part of the condition.
        assert_eq!(sibling.condition_id(), base.condition_id());
        assert_ne!(other_module.condition_id(), base.condition_id());
    }

    #[test]
    fn v2_condition_ids_narrow_as_py_sdk_does() {
        let v = router_vectors();
        let rows = v["condition_ids"].as_array().unwrap();
        let mut narrowed = 0;
        let mut refused = 0;
        for row in rows {
            let input = row["input"].as_str().unwrap();
            match (row["bytes31"].as_str(), parse_v2_condition_id(input)) {
                (Some(expected), Ok(got)) => {
                    narrowed += 1;
                    assert_eq!(format!("0x{}", hex::encode(got)), expected, "{input:?}");
                }
                (None, Err(err)) => {
                    refused += 1;
                    assert_eq!(refusal(err), CONDITION_REFUSED, "{input:?}");
                }
                (expected, result) => {
                    panic!("{input:?}: py-sdk gave {expected:?}, parse gave {result:?}")
                }
            }
        }
        assert!(rows.len() >= 8, "{} rows", rows.len());
        assert!(narrowed >= 4, "{narrowed} narrowed rows");
        assert!(refused >= 4, "{refused} refused rows");
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features relay --lib deposit_wallet::tests::v2_`

Expected: the build fails with ``error[E0432]: unresolved imports `crate::relay::types::parse_v2_condition_id`, `crate::relay::types::V2PositionId` ``. Story 1's fixtures must exist: if `include_str!` fails first with `couldn't read ... router_vectors.json`, stop (Story 1 is a dependency) rather than work around it.

- [ ] **Step 3: Implement**

(a) `polyoxide-polymarket/src/shared/mod.rs`: widen the module's gate. Quote of the lines Story 2 added:

```rust
#[cfg(feature = "clob")]
pub(crate) mod protocol;
```

Replace with:

```rust
#[cfg(any(feature = "clob", feature = "relay"))]
pub(crate) mod protocol;
```

(b) `polyoxide-polymarket/src/shared/protocol.rs` (Story 2's file): only `clob` asks `is_v2_position_id`, so gate that function alone. Put `#[cfg(feature = "clob")]` directly above the line `pub(crate) fn is_v2_position_id(id: &str) -> bool {` (after its doc comment). Then, in the file's `#[cfg(test)] mod tests`, put `#[cfg(feature = "clob")]` on every test fn (and on any helper or const only those tests use) that names `is_v2_position_id`, and on Story 2's `Ids` struct and the `Vectors::ids` field (only the clob-gated tests read them, so under `--features relay --lib` they warn as dead code; if a test of `parse_position_id` or `decode_v2_outcome_position_id` reads either, that test gets no gate and the warning names what still needs one); the tests of `parse_position_id` and `decode_v2_outcome_position_id` stay ungated. Add no other change to the file.

Check both single-feature builds for warnings, and gate whatever they name (a private const or helper only `is_v2_position_id` uses becomes `dead_code` under `relay`; give it the same `#[cfg(feature = "clob")]`):

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo check -p polyoxide-polymarket --no-default-features --features relay --lib`
Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo check -p polyoxide-polymarket --no-default-features --features clob --lib`
Expected: both finish with no warning.

(c) `polyoxide-polymarket/src/relay/types.rs`: quote of the top of the file:

```rust
use alloy::sol;
use serde::{Deserialize, Serialize};
```

Replace with:

```rust
use crate::relay::error::RelayError;
use crate::shared::protocol::{decode_v2_outcome_position_id, parse_position_id};
use alloy::primitives::FixedBytes;
use alloy::sol;
use serde::{Deserialize, Serialize};
```

and append at the end of the file:

```rust

/// A Protocol V2 YES/NO position: its 31-byte condition and its outcome index (0 or 1).
///
/// [`V2PositionId::parse`] is the only constructor, so an outcome index outside
/// 0 and 1 cannot be held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct V2PositionId {
    condition_id: FixedBytes<31>,
    outcome_index: u8,
}

impl V2PositionId {
    /// Decode a position id, as py-sdk's `decode_v2_outcome_position_id` does.
    ///
    /// `id` is a uint256 in decimal, or in `0x`, `0b` or `0o` notation, with
    /// surrounding whitespace ignored. Its first byte is the module (1 binary, 2 neg-risk,
    /// 3 combinatorial), its last the outcome index (0 or 1) and the 31 bytes before
    /// the last are the condition. Every refusal is an `InvalidRequest` with py-sdk's message.
    pub fn parse(id: &str) -> Result<Self, RelayError> {
        let value = parse_position_id(id)
            .ok_or_else(|| RelayError::validation("Position ID must be a uint256 value"))?;
        match decode_v2_outcome_position_id(id) {
            Some((condition_id, outcome_index)) => Ok(Self {
                condition_id,
                outcome_index,
            }),
            // py-sdk checks the module byte before the outcome byte.
            None if !(1..=3).contains(&value.to_be_bytes::<32>()[0]) => Err(
                RelayError::validation("Position ID must use a supported protocol v2 module"),
            ),
            None => Err(RelayError::validation(
                "Protocol v2 position ID must be a YES/NO position ID",
            )),
        }
    }

    /// The position's condition: the 31 bytes before its outcome byte.
    pub fn condition_id(&self) -> FixedBytes<31> {
        self.condition_id
    }

    /// The outcome index, 0 or 1.
    pub fn outcome_index(&self) -> u8 {
        self.outcome_index
    }
}

/// Narrow a Protocol V2 condition id to 31 bytes, as py-sdk does.
///
/// Accepts `0x` followed by 62 hex digits, or by 64 hex digits ending in `00` or `01`
/// (a bytes32 with a binary outcome byte, which is dropped), in either case of the
/// prefix and the digits. Anything else is an `InvalidRequest` with py-sdk's message.
pub fn parse_v2_condition_id(id: &str) -> Result<FixedBytes<31>, RelayError> {
    const REFUSED: &str =
        "Protocol v2 condition ID must be bytes31, or bytes32 with a binary outcome byte";
    let lower = id.to_ascii_lowercase();
    let mut digits = lower
        .strip_prefix("0x")
        .ok_or_else(|| RelayError::validation(REFUSED))?;
    if digits.len() == 64 && (digits.ends_with("00") || digits.ends_with("01")) {
        digits = &digits[..62];
    }
    if digits.len() != 62 {
        return Err(RelayError::validation(REFUSED));
    }
    let bytes = hex::decode(digits).map_err(|_| RelayError::validation(REFUSED))?;
    Ok(FixedBytes::<31>::from_slice(&bytes))
}
```

(d) `polyoxide-polymarket/src/relay/mod.rs`: quote:

```rust
pub use types::{
    GaslessTransaction, RelayerApiKey, RelayerTransaction, SafeTransaction, SafeTx, SubmitResponse,
    TransactionRequest, TransactionState, WalletType,
};
```

Replace with:

```rust
pub use types::{
    parse_v2_condition_id, GaslessTransaction, RelayerApiKey, RelayerTransaction, SafeTransaction,
    SafeTx, SubmitResponse, TransactionRequest, TransactionState, V2PositionId, WalletType,
};
```

Run `direnv exec . env CARGO_INCREMENTAL=0 cargo fmt --all` (it re-wraps whatever above differs from rustfmt's layout).

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features relay --lib deposit_wallet::tests`
Expected: `test result: ok.` with the four new `v2_` tests and the seven baseline tests of the module passing.

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob --lib shared::protocol`
Expected: `test result: ok.` (Story 2's protocol tests, unchanged).

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings`
Expected: no warning.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(polymarket): V2PositionId and parse_v2_condition_id for the Router (protocol v2, Story 4)" -- polyoxide-polymarket/src/shared/mod.rs polyoxide-polymarket/src/shared/protocol.rs polyoxide-polymarket/src/relay/types.rs polyoxide-polymarket/src/relay/mod.rs polyoxide-polymarket/src/relay/deposit_wallet.rs
```

### Task 4.2: The three Router calldata builders

**Files:**
- Modify: `polyoxide-polymarket/src/relay/deposit_wallet.rs:13-15`, `:58-63`, `:223-236`
- Test: `polyoxide-polymarket/src/relay/deposit_wallet.rs` (tests module, after the tests of Task 4.1)

- [ ] **Step 1: Write the failing tests**

In the tests module of `deposit_wallet.rs`, the import Task 4.1 added is:

```rust
    use crate::relay::types::{parse_v2_condition_id, V2PositionId};
```

Replace it with the following (the module has `use super::*;`, which will bring `V2PositionId` in once Step 3 imports it at the top of the file):

```rust
    use crate::relay::types::parse_v2_condition_id;
```

Append after the last test of Task 4.1 (`v2_condition_ids_narrow_as_py_sdk_does`), inside the module:

```rust

    // ── Protocol V2: Router calldata ────────────────────────────

    fn hex_of(bytes: &[u8]) -> String {
        format!("0x{}", hex::encode(bytes))
    }

    #[test]
    fn router_split_and_merge_calldata_match_py_sdk() {
        let v = router_vectors();
        for (name, build) in [
            ("split", router_split_calldata as fn(FixedBytes<31>, U256) -> Vec<u8>),
            ("merge", router_merge_calldata),
        ] {
            let rows = v[name].as_array().unwrap();
            // Amounts 0, 1000000 and 2^256 - 1, for one condition.
            assert_eq!(rows.len(), 3, "{name}");
            for row in rows {
                let condition: FixedBytes<31> = row["condition_id"].as_str().unwrap().parse().unwrap();
                let amount: U256 = row["amount"].as_str().unwrap().parse().unwrap();
                assert_eq!(
                    hex_of(&build(condition, amount)),
                    row["calldata"].as_str().unwrap(),
                    "{name} {amount}"
                );
            }
        }
    }

    #[test]
    fn router_split_and_merge_calldata_differ_only_in_the_selector() {
        let v = router_vectors();
        let condition: FixedBytes<31> = v["split"][0]["condition_id"].as_str().unwrap().parse().unwrap();
        let split = router_split_calldata(condition, U256::from(7));
        let merge = router_merge_calldata(condition, U256::from(7));
        assert_ne!(split[..4], merge[..4]);
        assert_eq!(split[4..], merge[4..]);
        assert_eq!(split.len(), 4 + 64);
    }

    #[test]
    fn router_redeem_calldata_matches_py_sdk_for_both_outcomes() {
        let v = router_vectors();
        let rows = v["redeem"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        for row in rows {
            let position = V2PositionId::parse(row["position_id"].as_str().unwrap()).unwrap();
            // The position's own decode agrees with py-sdk's recorded one.
            assert_eq!(
                hex_of(position.condition_id().as_slice()),
                row["condition_id"].as_str().unwrap()
            );
            assert_eq!(
                u64::from(position.outcome_index()),
                row["outcome_index"].as_u64().unwrap()
            );
            let amount: U256 = row["amount"].as_str().unwrap().parse().unwrap();
            assert_eq!(
                hex_of(&router_redeem_calldata(&position, amount)),
                row["calldata"].as_str().unwrap(),
                "outcome {}",
                position.outcome_index()
            );
        }
        // The two rows are the two outcomes of one condition.
        assert_ne!(rows[0]["calldata"], rows[1]["calldata"]);
    }

    #[test]
    fn the_router_redeem_batch_digest_matches_py_sdk() {
        let v = router_vectors();
        let batch = &v["deposit_wallet_batch"];
        let redeem = &v["redeem"][0];
        let position = V2PositionId::parse(redeem["position_id"].as_str().unwrap()).unwrap();
        let amount: U256 = redeem["amount"].as_str().unwrap().parse().unwrap();
        let fixture_call = &batch["calls"][0];
        let call = DepositWalletCall {
            target: fixture_call["target"].as_str().unwrap().parse().unwrap(),
            value: fixture_call["value"].as_str().unwrap().parse().unwrap(),
            data: router_redeem_calldata(&position, amount).into(),
        };
        // The call is the Router's, with the calldata py-sdk built.
        assert_eq!(call.target, v["router"].as_str().unwrap().parse::<Address>().unwrap());
        assert_eq!(hex_of(&call.data), fixture_call["data"].as_str().unwrap());
        let digest = batch_digest(
            v["chain_id"].as_u64().unwrap(),
            batch["wallet"].as_str().unwrap().parse().unwrap(),
            &[call],
            batch["nonce"].as_str().unwrap().parse().unwrap(),
            batch["deadline"].as_str().unwrap().parse().unwrap(),
        );
        assert_eq!(
            digest,
            batch["digest"].as_str().unwrap().parse::<B256>().unwrap()
        );
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features relay --lib deposit_wallet::tests::router`
Expected: the build fails with `error[E0425]: cannot find function `router_split_calldata` in this scope` (and `router_merge_calldata`, `router_redeem_calldata`), and `cannot find type `V2PositionId``.

- [ ] **Step 3: Implement**

In `deposit_wallet.rs`, the top imports are:

```rust
use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::sol;
use alloy::sol_types::{Eip712Domain, SolCall, SolStruct, SolValue};
```

Replace with:

```rust
use crate::relay::types::V2PositionId;
use alloy::primitives::{Address, Bytes, FixedBytes, B256, U256};
use alloy::sol;
use alloy::sol_types::{Eip712Domain, SolCall, SolStruct, SolValue};
```

In the `sol!` block of `mod protocol`, quote the last function:

```rust
        function redeemPositions(address collateral, bytes32 parentCollectionId, bytes32 conditionId, uint256[] indexSets);
    }
```

Replace with:

```rust
        function redeemPositions(address collateral, bytes32 parentCollectionId, bytes32 conditionId, uint256[] indexSets);

        function split(bytes31 conditionId, uint256 amount);
        function merge(bytes31 conditionId, uint256 amount);
        function redeem(bytes31 conditionId, uint256 outcomeIndex, uint256 amount);
    }
```

After `redeem_positions_calldata` (quote of its end, followed by the tests module):

```rust
    protocol::redeemPositionsCall {
        collateral,
        parentCollectionId: B256::ZERO,
        conditionId: condition_id,
        indexSets: index_sets.to_vec(),
    }
    .abi_encode()
}

#[cfg(test)]
mod tests {
```

Replace with:

```rust
    protocol::redeemPositionsCall {
        collateral,
        parentCollectionId: B256::ZERO,
        conditionId: condition_id,
        indexSets: index_sets.to_vec(),
    }
    .abi_encode()
}

/// Calldata for the Protocol V2 Router's `split(bytes31,uint256)`.
///
/// `amount` is in base units (6 decimals); any `U256` encodes, zero included.
pub fn router_split_calldata(condition_id: FixedBytes<31>, amount: U256) -> Vec<u8> {
    protocol::splitCall {
        conditionId: condition_id,
        amount,
    }
    .abi_encode()
}

/// Calldata for the Protocol V2 Router's `merge(bytes31,uint256)`.
///
/// `amount` is in base units (6 decimals); any `U256` encodes, zero included.
pub fn router_merge_calldata(condition_id: FixedBytes<31>, amount: U256) -> Vec<u8> {
    protocol::mergeCall {
        conditionId: condition_id,
        amount,
    }
    .abi_encode()
}

/// Calldata for the Protocol V2 Router's `redeem(bytes31,uint256,uint256)`: the
/// position's condition, its outcome index, then `amount` in base units.
pub fn router_redeem_calldata(position: &V2PositionId, amount: U256) -> Vec<u8> {
    protocol::redeemCall {
        conditionId: position.condition_id(),
        outcomeIndex: U256::from(position.outcome_index()),
        amount,
    }
    .abi_encode()
}

#[cfg(test)]
mod tests {
```

Run `direnv exec . env CARGO_INCREMENTAL=0 cargo fmt --all`.

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features relay --lib deposit_wallet::tests`
Expected: `test result: ok.` with the four `router_`/digest tests, the Task 4.1 tests and the seven baseline tests passing.

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings`
Expected: no warning.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(polymarket): Router split, merge and redeem calldata builders (protocol v2, Story 4)" -- polyoxide-polymarket/src/relay/deposit_wallet.rs
```

### Task 4.3: `RelayClient`'s Router calls and submits; the redemption docs

**Files:**
- Modify: `polyoxide-polymarket/src/relay/client.rs:3`, `:9-12`, `:17`, `:29-36` (constants), `:1471`, `:1527`, `:1555-1557`, the two Safe/Proxy submit docs near `:1694` and `:1712`
- Test: `polyoxide-polymarket/tests/mock_relay.rs` (appended)

- [ ] **Step 1: Write the failing tests**

Append to the end of `polyoxide-polymarket/tests/mock_relay.rs` (after `a_refused_relay_call_is_an_invalid_request`):

```rust

// ── Protocol V2 Router ─────────────────────────────────────────

const ROUTER_VECTORS: &str = include_str!("fixtures/relay/protocol_v2/router_vectors.json");

fn router_vectors() -> serde_json::Value {
    serde_json::from_str(ROUTER_VECTORS).unwrap()
}

fn fixture_u256(value: &serde_json::Value) -> alloy::primitives::U256 {
    value.as_str().unwrap().parse().unwrap()
}

fn fixture_condition(value: &serde_json::Value) -> alloy::primitives::FixedBytes<31> {
    value.as_str().unwrap().parse().unwrap()
}

/// The fixture's first redeem row (outcome 0): the position, the amount and the calldata.
fn fixture_redeem() -> (
    polyoxide_polymarket::relay::V2PositionId,
    alloy::primitives::U256,
    String,
) {
    let v = router_vectors();
    let row = &v["redeem"][0];
    (
        polyoxide_polymarket::relay::V2PositionId::parse(row["position_id"].as_str().unwrap())
            .unwrap(),
        fixture_u256(&row["amount"]),
        row["calldata"].as_str().unwrap().to_string(),
    )
}

fn fixture_router() -> alloy::primitives::Address {
    router_vectors()["router"].as_str().unwrap().parse().unwrap()
}

fn proxy_client(server: &mockito::ServerGuard) -> RelayClient {
    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    RelayClient::builder()
        .expect("builder")
        .url(&server.url())
        .expect("valid mock URL")
        .with_account(account)
        .wallet_type(polyoxide_polymarket::relay::WalletType::Proxy)
        .build()
        .expect("build client")
}

#[test]
fn v2_calls_target_the_router_with_py_sdks_calldata() {
    let v = router_vectors();
    let client = RelayClient::builder()
        .expect("builder")
        .build()
        .expect("build client");
    for row in v["split"].as_array().unwrap() {
        let call = client
            .v2_split_call(fixture_condition(&row["condition_id"]), fixture_u256(&row["amount"]))
            .unwrap();
        assert_eq!(call.target, fixture_router());
        assert_eq!(call.value, alloy::primitives::U256::ZERO);
        assert_eq!(
            format!("0x{}", alloy::primitives::hex::encode(&call.data)),
            row["calldata"].as_str().unwrap()
        );
    }
    for row in v["merge"].as_array().unwrap() {
        let call = client
            .v2_merge_call(fixture_condition(&row["condition_id"]), fixture_u256(&row["amount"]))
            .unwrap();
        assert_eq!(call.target, fixture_router());
        assert_eq!(call.value, alloy::primitives::U256::ZERO);
        assert_eq!(
            format!("0x{}", alloy::primitives::hex::encode(&call.data)),
            row["calldata"].as_str().unwrap()
        );
    }
    for row in v["redeem"].as_array().unwrap() {
        let position =
            polyoxide_polymarket::relay::V2PositionId::parse(row["position_id"].as_str().unwrap())
                .unwrap();
        let call = client
            .v2_redeem_call(&position, fixture_u256(&row["amount"]))
            .unwrap();
        assert_eq!(call.target, fixture_router());
        assert_eq!(call.value, alloy::primitives::U256::ZERO);
        assert_eq!(
            format!("0x{}", alloy::primitives::hex::encode(&call.data)),
            row["calldata"].as_str().unwrap()
        );
    }
    // The Router call is the one the fixture's Deposit Wallet batch carries.
    let (position, amount, _) = fixture_redeem();
    let call = client.v2_redeem_call(&position, amount).unwrap();
    let batch_call = &v["deposit_wallet_batch"]["calls"][0];
    assert_eq!(
        call.target,
        batch_call["target"].as_str().unwrap().parse().unwrap()
    );
    assert_eq!(
        format!("0x{}", alloy::primitives::hex::encode(&call.data)),
        batch_call["data"].as_str().unwrap()
    );
}

#[tokio::test]
async fn submit_v2_redeem_posts_the_router_call_for_a_safe() {
    let (position, amount, calldata) = fixture_redeem();
    let router = fixture_router();
    let mut server = Server::new_async().await;
    let deployed = server
        .mock("GET", "/deployed")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":true}"#)
        .expect(1)
        .create_async()
        .await;
    let nonce = server
        .mock("GET", "/nonce")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"nonce":"5"}"#)
        .expect(1)
        .create_async()
        .await;
    // A lone transaction is not wrapped in MultiSend: the Router is the Safe's `to`, the
    // call is a plain CALL (operation 0) and the data is py-sdk's calldata.
    let submit = server
        .mock("POST", "/submit")
        .match_body(Matcher::PartialJsonString(format!(
            r#"{{"type":"SAFE","to":"{router}","data":"{calldata}","value":"0","nonce":"5","signatureParams":{{"operation":"0"}}}}"#
        )))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-v2-safe","state":"STATE_NEW"}"#)
        .expect(1)
        .create_async()
        .await;

    let resp = client_with_builder_auth(&server)
        .submit_v2_redeem(&position, amount)
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-v2-safe");
    deployed.assert_async().await;
    nonce.assert_async().await;
    submit.assert_async().await;
}

#[tokio::test]
async fn submit_v2_redeem_posts_the_router_call_for_a_proxy() {
    let (position, amount, calldata) = fixture_redeem();
    let router = fixture_router();
    let mut server = Server::new_async().await;
    let payload = server
        .mock("GET", "/relay-payload")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"1"}"#)
        .expect(1)
        .create_async()
        .await;
    // The proxy factory receives `proxy([ProxyTransaction])`; the Router's address (padded
    // to a word) and py-sdk's calldata sit inside that encoding.
    let submit = server
        .mock("POST", "/submit")
        .match_body(Matcher::AllOf(vec![
            Matcher::PartialJsonString(r#"{"type":"PROXY","nonce":"1"}"#.into()),
            Matcher::Regex(alloy::primitives::hex::encode(router.as_slice())),
            Matcher::Regex(calldata.trim_start_matches("0x").to_string()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-v2-proxy","state":"STATE_NEW"}"#)
        .expect(1)
        .create_async()
        .await;

    let resp = proxy_client(&server)
        .submit_v2_redeem(&position, amount)
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-v2-proxy");
    payload.assert_async().await;
    submit.assert_async().await;
}

#[tokio::test]
async fn submit_v2_redeem_posts_the_router_call_for_a_deposit_wallet() {
    let (position, amount, calldata) = fixture_redeem();
    let router = fixture_router();
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded(
                "address".into(),
                "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".into(),
            ),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"4"}"#)
        .expect(1)
        .create_async()
        .await;
    let submit = server
        .mock("POST", "/submit")
        .match_body(Matcher::AllOf(vec![
            Matcher::PartialJsonString(format!(
                r#"{{"type":"WALLET","nonce":"4","depositWalletParams":{{"depositWallet":"{}","calls":[{{"target":"{router}","value":"0","data":"{calldata}"}}]}}}}"#,
                relay_vectors()["wallet"].as_str().unwrap()
            )),
            Matcher::Regex(r#""signature":"0x[0-9a-f]{130}""#.into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-v2-wallet","state":"STATE_NEW"}"#)
        .expect(1)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let resp = client.submit_v2_redeem(&position, amount).await.unwrap();
    assert_eq!(resp.transaction_id, "tx-v2-wallet");
    params.assert_async().await;
    submit.assert_async().await;
}

#[tokio::test]
async fn submit_v2_split_and_merge_send_their_router_calldata() {
    let v = router_vectors();
    let split = &v["split"][1];
    let merge = &v["merge"][1];
    let router = fixture_router();
    for (route, row) in [("split", split), ("merge", merge)] {
        let mut server = Server::new_async().await;
        let _deployed = server
            .mock("GET", "/deployed")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"deployed":true}"#)
            .create_async()
            .await;
        let _nonce = server
            .mock("GET", "/nonce")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"nonce":"5"}"#)
            .create_async()
            .await;
        let submit = server
            .mock("POST", "/submit")
            .match_body(Matcher::PartialJsonString(format!(
                r#"{{"type":"SAFE","to":"{router}","data":"{}","value":"0"}}"#,
                row["calldata"].as_str().unwrap()
            )))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"transactionID":"tx-v2","state":"STATE_NEW"}"#)
            .expect(1)
            .create_async()
            .await;
        let client = client_with_builder_auth(&server);
        let condition = fixture_condition(&row["condition_id"]);
        let amount = fixture_u256(&row["amount"]);
        let resp = match route {
            "split" => client.submit_v2_split(condition, amount).await,
            _ => client.submit_v2_merge(condition, amount).await,
        }
        .unwrap();
        assert_eq!(resp.transaction_id, "tx-v2", "{route}");
        submit.assert_async().await;
    }
}

#[tokio::test]
async fn v2_router_calls_are_refused_off_polygon_before_any_request() {
    use polyoxide_venue::{Class, Classify};

    let v = router_vectors();
    let condition = fixture_condition(&v["split"][0]["condition_id"]);
    let amount = alloy::primitives::U256::from(1_000_000u64);
    let (position, _, _) = fixture_redeem();

    let mut server = Server::new_async().await;
    // `expect(0)` with `match_query(Any)`: a bare path mock would not match a request that
    // carries a query string, so it could not catch one.
    let mut mocks = Vec::new();
    for (method, path) in [
        ("GET", "/deployed"),
        ("GET", "/nonce"),
        ("GET", "/relay-payload"),
        ("GET", "/v1/account/transactions/params"),
        ("POST", "/submit"),
    ] {
        mocks.push(
            server
                .mock(method, path)
                .match_query(Matcher::Any)
                .expect(0)
                .create_async()
                .await,
        );
    }

    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    let client = RelayClient::builder()
        .unwrap()
        .url(&server.url())
        .unwrap()
        .chain_id(80002)
        .with_account(account)
        .build()
        .unwrap();

    let wanted =
        "Protocol V2 Router calls are only known for Polygon mainnet (137), not chain 80002";
    let errs = [
        client.v2_split_call(condition, amount).unwrap_err(),
        client.v2_merge_call(condition, amount).unwrap_err(),
        client.v2_redeem_call(&position, amount).unwrap_err(),
        client.submit_v2_split(condition, amount).await.unwrap_err(),
        client.submit_v2_merge(condition, amount).await.unwrap_err(),
        client.submit_v2_redeem(&position, amount).await.unwrap_err(),
    ];
    for err in errs {
        assert!(err.to_string().contains(wanted), "{err}");
        assert_eq!(err.class(), Class::InvalidRequest);
    }
    for mock in mocks {
        mock.assert_async().await;
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features relay --test mock_relay v2_`
Expected: the build fails with `error[E0599]: no method named `v2_split_call` found for struct `RelayClient``, and the same for `v2_merge_call`, `v2_redeem_call`, `submit_v2_split`, `submit_v2_merge` and `submit_v2_redeem`.

- [ ] **Step 3: Implement**

All in `polyoxide-polymarket/src/relay/client.rs`.

(a) Imports. Replace

```rust
use crate::relay::deposit_wallet::DepositWalletCall;
```

with

```rust
use crate::relay::deposit_wallet::{
    router_merge_calldata, router_redeem_calldata, router_split_calldata, DepositWalletCall,
};
```

Replace

```rust
use crate::relay::types::{
    ExecuteParams, GaslessTransaction, NonceResponse, RelayerApiKey, RelayerTransaction,
    SafeTransaction, SafeTx, SubmitResponse, WalletType,
};
```

with

```rust
use crate::relay::types::{
    ExecuteParams, GaslessTransaction, NonceResponse, RelayerApiKey, RelayerTransaction,
    SafeTransaction, SafeTx, SubmitResponse, V2PositionId, WalletType,
};
```

and replace

```rust
use alloy::primitives::{address, keccak256, Address, Bytes, B256, U256};
```

with

```rust
use alloy::primitives::{address, keccak256, Address, Bytes, FixedBytes, B256, U256};
```

(b) The constant. Insert directly after the `NEG_RISK_COLLATERAL_ADAPTER` constant (line 36 of the file, before `deposit_wallet_redemption_refused`):

```rust
/// py-sdk's `protocol_v2_router` on Polygon mainnet: the contract a Protocol V2 position's
/// `split`, `merge` and `redeem` go to.
const PROTOCOL_V2_ROUTER: Address = address!("12121212006e4CD160D18e3f00711DA5c3372600");
```

(c) The calls and submits. Quote (the end of `submit_deposit_wallet_redemption` and the start of the next item):

```rust
        self.execute_deposit_wallet(vec![tx], None).await
    }

    /// Simulate a Deposit Wallet redemption call from the configured wallet, returning
```

Replace with:

```rust
        self.execute_deposit_wallet(vec![tx], None).await
    }

    /// A call to the Protocol V2 Router with `value: 0`, refused on any chain but
    /// Polygon mainnet, whose Router address is the only one known.
    fn router_call(&self, data: Vec<u8>) -> Result<DepositWalletCall, RelayError> {
        if self.chain_id != 137 {
            return Err(RelayError::validation(format!(
                "Protocol V2 Router calls are only known for Polygon mainnet (137), not chain {}",
                self.chain_id
            )));
        }
        Ok(DepositWalletCall {
            target: PROTOCOL_V2_ROUTER,
            value: U256::ZERO,
            data: data.into(),
        })
    }

    /// Send one Router call through [`RelayClient::execute`], as a plain CALL, so it works
    /// for a Safe, a Proxy and a Deposit Wallet.
    async fn submit_router_call(
        &self,
        call: DepositWalletCall,
    ) -> Result<SubmitResponse, RelayError> {
        let tx = SafeTransaction {
            to: call.target,
            value: call.value,
            data: call.data,
            operation: CALL_OPERATION,
        };
        self.execute(vec![tx], None).await
    }

    /// The Protocol V2 Router's `split(bytes31,uint256)` call for `condition_id`, as py-sdk's
    /// `router_split_call` builds it. `amount` is in base units (6 decimals); any `U256` is
    /// accepted, zero included, and the chain enforces balances.
    ///
    /// Get `condition_id` from [`parse_v2_condition_id`](crate::relay::parse_v2_condition_id)
    /// or [`V2PositionId::condition_id`]. For a Deposit Wallet with an external signer, pass the
    /// call to [`RelayClient::deposit_wallet_batch_typed_data`] and
    /// [`RelayClient::submit_deposit_wallet_batch_from`].
    ///
    /// The Router address is Polygon mainnet's; any other chain is an error.
    pub fn v2_split_call(
        &self,
        condition_id: FixedBytes<31>,
        amount: U256,
    ) -> Result<DepositWalletCall, RelayError> {
        self.router_call(router_split_calldata(condition_id, amount))
    }

    /// The Protocol V2 Router's `merge(bytes31,uint256)` call for `condition_id`, as py-sdk's
    /// `router_merge_call` builds it. Amounts and chains are as for
    /// [`RelayClient::v2_split_call`].
    pub fn v2_merge_call(
        &self,
        condition_id: FixedBytes<31>,
        amount: U256,
    ) -> Result<DepositWalletCall, RelayError> {
        self.router_call(router_merge_calldata(condition_id, amount))
    }

    /// The Protocol V2 Router's `redeem(bytes31,uint256,uint256)` call for one position, as
    /// py-sdk's `router_redeem_call` builds it. Amounts and chains are as for
    /// [`RelayClient::v2_split_call`].
    ///
    /// Use this, not [`RelayClient::submit_deposit_wallet_redemption`], for a Protocol V2
    /// market.
    pub fn v2_redeem_call(
        &self,
        position: &V2PositionId,
        amount: U256,
    ) -> Result<DepositWalletCall, RelayError> {
        self.router_call(router_redeem_calldata(position, amount))
    }

    /// Split `amount` of collateral into both outcomes of a Protocol V2 condition, through
    /// the Router, signed by this client's account and submitted gaslessly.
    ///
    /// Works for a Safe, a Proxy and a Deposit Wallet client (one CALL through
    /// [`RelayClient::execute`]). A chain other than Polygon mainnet is refused before any
    /// request. Sends `metadata: ""` for a Deposit Wallet.
    pub async fn submit_v2_split(
        &self,
        condition_id: FixedBytes<31>,
        amount: U256,
    ) -> Result<SubmitResponse, RelayError> {
        let call = self.v2_split_call(condition_id, amount)?;
        self.submit_router_call(call).await
    }

    /// Merge `amount` of both outcomes of a Protocol V2 condition back into collateral,
    /// through the Router. Wallets, chains and metadata are as for
    /// [`RelayClient::submit_v2_split`].
    pub async fn submit_v2_merge(
        &self,
        condition_id: FixedBytes<31>,
        amount: U256,
    ) -> Result<SubmitResponse, RelayError> {
        let call = self.v2_merge_call(condition_id, amount)?;
        self.submit_router_call(call).await
    }

    /// Redeem `amount` of one Protocol V2 position through the Router. Wallets, chains and
    /// metadata are as for [`RelayClient::submit_v2_split`].
    pub async fn submit_v2_redeem(
        &self,
        position: &V2PositionId,
        amount: U256,
    ) -> Result<SubmitResponse, RelayError> {
        let call = self.v2_redeem_call(position, amount)?;
        self.submit_router_call(call).await
    }

    /// Simulate a Deposit Wallet redemption call from the configured wallet, returning
```

(d) The redemption doc comments. In `redeem_typed_data`, replace the line

```rust
    /// `negRisk`). Protocol-V2 markets (router `redeem`) are not supported.
```

with

```rust
    /// `negRisk`). A Protocol V2 market redeems through the Router instead: see
    /// [`RelayClient::v2_redeem_call`], which pairs with
    /// [`RelayClient::deposit_wallet_batch_typed_data`] for an external signer.
```

In `submit_deposit_wallet_redemption`, replace the line

```rust
    /// or gamma's `negRisk`). Protocol-V2 markets (router `redeem`) are not supported.
```

with

```rust
    /// or gamma's `negRisk`). A Protocol V2 market redeems through the Router instead: see
    /// [`RelayClient::submit_v2_redeem`].
```

In `submit_gasless_redemption`, replace

```rust
    /// neg-risk flag to pick the collateral adapter py-sdk sends to.
    pub async fn submit_gasless_redemption(
```

with

```rust
    /// neg-risk flag to pick the collateral adapter py-sdk sends to.
    ///
    /// This is the CTF path; a Protocol V2 position redeems through the Router with
    /// [`RelayClient::submit_v2_redeem`], for any wallet type.
    pub async fn submit_gasless_redemption(
```

In `submit_gasless_redemption_with_gas_estimation`, replace

```rust
    /// neg-risk flag to pick the collateral adapter py-sdk sends to.
    pub async fn submit_gasless_redemption_with_gas_estimation(
```

with

```rust
    /// neg-risk flag to pick the collateral adapter py-sdk sends to.
    ///
    /// This is the CTF path; a Protocol V2 position redeems through the Router with
    /// [`RelayClient::submit_v2_redeem`], for any wallet type.
    pub async fn submit_gasless_redemption_with_gas_estimation(
```

(`submit_deposit_wallet_redemption`'s "Protocol-V2" line and `redeem_typed_data`'s are the only two lines with that phrase; `grep -n 'Protocol-V2' polyoxide-polymarket/src/relay/client.rs` prints nothing afterwards.)

Run `direnv exec . env CARGO_INCREMENTAL=0 cargo fmt --all`.

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features relay --test mock_relay`
Expected: `test result: ok.` with the six new tests (`v2_calls_target_the_router_with_py_sdks_calldata`, the Safe, Proxy and Deposit Wallet submits, `submit_v2_split_and_merge_send_their_router_calldata`, `v2_router_calls_are_refused_off_polygon_before_any_request`) and every baseline test passing.

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings`
Expected: no warning.

Run: `direnv exec . bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-polymarket --all-features'`
Expected: no warning (the new docs name `pub` items only).

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(polymarket): RelayClient Router split, merge and redeem calls and submits (protocol v2, Story 4)" -- polyoxide-polymarket/src/relay/client.rs polyoxide-polymarket/tests/mock_relay.rs
```

### Task 4.4: The relay rules file and README

**Files:**
- Modify: `.claude/rules/polymarket-relay.md`
- Modify: `polyoxide-polymarket/src/relay/README.md`
- Modify: `docs/specs/session-keys/README.md:151-152`

- [ ] **Step 1: Write the failing test**

This task adds prose and one README doctest (the README is compiled by `cargo test --doc`; `test_module_readmes.py` counts its include) over an API Task 4.3 already built, so it has no red state. Take the baseline: run `direnv exec . env CARGO_INCREMENTAL=0 cargo test --doc -p polyoxide-polymarket --features relay` and note the number of doctests that pass.

- [ ] **Step 2: Run it to see it fail**

Not applicable, for the reason in Step 1. Step 4 must show the doctest count grown by exactly one.

- [ ] **Step 3: Implement**

(a) `.claude/rules/polymarket-relay.md`: quote the last bullet,

```markdown
- Deposit Wallets and session signers (`WalletType::DepositWallet`, `deposit_wallet`,
  `session_signers`, `resolve_wallet`, `with_auth`): `session-keys.md`.
```

and append after it:

```markdown
- **Protocol V2 positions (the Router).** `V2PositionId::parse` and `parse_v2_condition_id`
  (`relay::types`, re-exported from `relay`) sit on the crate-private `shared::protocol`, which
  `clob` or `relay` builds, and refuse with `RelayError::validation` and py-sdk's messages. A
  position id is a uint256 (decimal, `0x`, `0b` or `0o`, whitespace trimmed) whose first byte is a
  module (1, 2 or 3; checked first) and whose last byte is the outcome, 0 or 1; the 31 bytes
  between are the condition. A condition id is `0x` plus 62 hex digits, or 64 ending `00` or `01`
  (py-sdk accepts `01` too; the byte is dropped), in any case. `deposit_wallet::router_split_calldata`,
  `router_merge_calldata` and `router_redeem_calldata` are pure and take any `U256` amount, zero
  included. `RelayClient::v2_split_call`, `v2_merge_call` and `v2_redeem_call` target the Router
  `0x12121212006e4CD160D18e3f00711DA5c3372600` with value 0 and refuse any chain but 137;
  `submit_v2_split`, `submit_v2_merge` and `submit_v2_redeem` send that one call through
  `execute(vec![tx], None)` as a plain CALL, so one path serves a Safe, a Proxy and a Deposit
  Wallet (a Deposit Wallet with an external signer uses `deposit_wallet_batch_typed_data` and
  `submit_deposit_wallet_batch_from` with the call). Pinned to
  `tests/fixtures/relay/protocol_v2/router_vectors.json` and
  `tests/fixtures/clob/protocol_v2/id_vectors.json`, which `scripts/capture_polymarket_protocol_v2.py`
  takes from py-sdk 0.12.0. The CTF redemption functions are the V1 path and their docs point here.
```

(b) `polyoxide-polymarket/src/relay/README.md`: quote the start of the next section,

```markdown
### Execute Arbitrary Transactions
```

and insert before it:

````markdown
### Protocol V2 Positions (the Router)

A Protocol V2 market splits, merges and redeems through the Router, not the Conditional
Tokens contract. `V2PositionId::parse` decodes an outcome's token id (its condition and its
outcome index, 0 or 1), and `parse_v2_condition_id` narrows a condition id (`0x` plus 62 hex
digits, or 64 ending `00` or `01`) to the 31 bytes the Router takes. `submit_v2_split`,
`submit_v2_merge` and `submit_v2_redeem` send one call through `execute`, so they work for a
Safe, a Proxy and a Deposit Wallet; `v2_split_call`, `v2_merge_call` and `v2_redeem_call`
return the `DepositWalletCall` for an external signer. Amounts are base units (6 decimals), and
only Polygon mainnet (137) is known.

```rust
# use polyoxide_polymarket::relay::{
#     parse_v2_condition_id, BuilderAccount, BuilderConfig, RelayClient, V2PositionId,
# };
# use alloy::primitives::U256;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let config = BuilderConfig::new("key".into(), "secret".into(), None);
# let account = BuilderAccount::new("0xprivatekey...", Some(config))?;
# let client = RelayClient::from_account(account)?;
# let condition_hex = "0x...";
# let token_id = "1234...";
let condition = parse_v2_condition_id(condition_hex)?;
let split = client.submit_v2_split(condition, U256::from(1_000_000)).await?;

let position = V2PositionId::parse(token_id)?;
let redeem = client.submit_v2_redeem(&position, U256::from(1_000_000)).await?;
# let _ = (split, redeem);
# Ok(())
# }
```

````

and in the "API Coverage" list, after the line starting `- **Gasless Redemptions**`, add:

```markdown
- **Protocol V2 Router**: Split, merge and redeem a Protocol V2 position (`submit_v2_split`, `submit_v2_merge`, `submit_v2_redeem`, or the `v2_*_call` builders for an external signer), for a Safe, a Proxy or a Deposit Wallet on Polygon mainnet
```

(c) `docs/specs/session-keys/README.md:151-152` (edit only these two lines; Story 2 edits only the type-3 domain passage at :57-58). Quote the current lines,

```markdown
contract directly. A protocol-V2 market goes through the V2 router's `redeem` and is not
covered here. pUSD (`0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB`) is the Deposit Wallet
```

and replace them with:

```markdown
contract directly. A protocol-V2 market goes through the V2 router's `redeem`, which
polyoxide builds with `RelayClient::v2_redeem_call` and sends with `submit_v2_redeem` (for an
external signer, pass the call to `deposit_wallet_batch_typed_data`). pUSD (`0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB`) is the Deposit Wallet
```

- [ ] **Step 4: Run it to see it pass**

Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test --doc -p polyoxide-polymarket --features relay`
Expected: `test result: ok.` with one more doctest than the Step 1 count (the README fence is compiled, not run).

Run: `bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_module_readmes.py -q'`
Expected: all pass (`polymarket-relay.md`'s `paths:` already hold `polyoxide-polymarket/tests/fixtures/relay/**`, which covers `protocol_v2/`).

Run the story's gates in order, each from the repo root with the prefix, with `{packages}` = `-p polyoxide-polymarket -p polyoxide -p polyoxide-cli -p polyoxide-py`: `cargo fmt --all -- --check`, `cargo clippy {packages} --all-targets --all-features -- -D warnings`, `cargo test {packages} --all-features`, `bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps {packages} --all-features'`, `python3 scripts/gen_registry.py --check`, `python3 scripts/venue_gate.py`, `python3 scripts/live_unwraps.py`, `python3 scripts/test_body_diff.py --baseline v0.40.0`.
Expected: each exits 0 (a baseline test that would need a body change is a stop: report it, never edit it).

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(polymarket): the relay rules and README describe the Protocol V2 Router helpers (protocol v2, Story 4)" -- .claude/rules/polymarket-relay.md polyoxide-polymarket/src/relay/README.md docs/specs/session-keys/README.md
```

## Story 5: The live V2 probe, the resolution note, the deferred checklist and the mutation battery

- **Developer:** intermediate-dev
- **Files:** `polyoxide-polymarket/tests/live_clob.rs`, `docs/specs/data-v2/OBSERVED.md`, `_bmad-output/implementation-artifacts/deferred-work.md`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`
- **Interfaces:** uses Story 3's `Market::outcome_ids` and every line Stories 2 to 4 wrote; exposes none
- **Verify:** `bash -c 'out=$(cargo test -p polyoxide-polymarket --no-default-features --features clob,gamma --test live_clob -- --ignored --exact live_v2_market_is_served 2>&1); echo "$out" | tail -20; echo "$out" | grep -qE "polyoxide-class=environmental|test result: ok. 1 passed" && python3 scripts/live_unwraps.py && cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'`
- **Review:** light (a read-only live test, records and a re-provable ledger; no library code)

This story closes the protocol-V2 work without adding library code. It appends `live_v2_market_is_served` to `live_clob.rs`: a read-only probe that takes the first Protocol V2 market among the newest 100 open ones (failing as `environmental` until one exists, which is the signal issue #51 asks the nightly to give), reads its ids through `Market::outcome_ids`, and reads `/book`, `/tick-size` and `/neg-risk` for each, so the per-token helpers' answer on V2 ids is recorded. It writes the Data v2 resolution note and the deferred BUY and SELL fill checklist as text. Last, it runs the spec's mutation battery against the HEAD that Stories 2 to 4 left, one mutation at a time, and records each result as a `docs/MUTANTS.md` row whose cited lines `test_mutants_ledger.py` pins. No landmine marker appears in this story's code. Run the battery with nothing else touching the tree or `target/`: it is serial, edits tracked files and restores each one.

### Task 5.1: `live_v2_market_is_served`

**Files:**
- Modify: `polyoxide-polymarket/tests/live_clob.rs` (append after the last test)

- [ ] **Step 1: Write the test**

The file's last item is a test that ends with

```rust
    assert!(
        !err.is_retriable(),
        "an unmatched FAK must not be retriable"
    );
}
```

Append after that closing brace (end of file):

```rust

// ── Protocol V2 (read-only, no credentials) ─────────────────────

/// A Protocol V2 market is served end to end: its ids come out of
/// `Market::outcome_ids`, and the CLOB answers `/book`, `/tick-size` and
/// `/neg-risk` for each. This answers the open question of whether the
/// per-token helpers serve position ids.
///
/// Fails as `environmental` while the newest open markets include no V2 one:
/// the nightly logs that and moves on, so the first run after a V2 market
/// exists is the first real signal. It reads three routes per id and posts no
/// order.
#[tokio::test]
#[ignore]
async fn live_v2_market_is_served() {
    use polyoxide_polymarket::gamma::types::ProtocolVersion;

    let gamma = Gamma::builder().build().or_fail("gamma client");
    let markets = gamma
        .markets()
        .list()
        .closed(false)
        .order("id")
        .ascending(false)
        .limit(100)
        .send()
        .await
        .or_fail("list the newest open markets");

    let Some(market) = markets
        .iter()
        .find(|market| market.version == Some(ProtocolVersion::V2))
    else {
        environmental("no Protocol V2 market among the newest 100 open markets");
    };

    let outcomes = market
        .outcome_ids()
        .or_fail("the V2 market's outcome ids");
    assert!(
        !outcomes.is_empty(),
        "market {} lists no outcome ids",
        market.id
    );

    let clob = public_client();
    for outcome in &outcomes {
        let id = &outcome.id;
        let book = clob
            .markets()
            .order_book(id)
            .send()
            .await
            .or_fail("the V2 outcome's order book");
        assert_eq!(
            &book.asset_id, id,
            "the book answers for the id that was asked"
        );
        clob.markets()
            .tick_size(id)
            .send()
            .await
            .or_fail("the V2 outcome's tick size");
        clob.markets()
            .neg_risk(id)
            .send()
            .await
            .or_fail("the V2 outcome's neg-risk flag");
    }
}
```

The test has no `unwrap`, `expect(` or `panic!`, so `scripts/live_unwraps.py` counts nothing new. It makes at most 7 requests per run, on the clients' default quotas and timeouts: one gamma list of 100 markets, then the three routes for each of a binary market's two ids.

- [ ] **Step 2: Run it to see its result**

A live test cannot be made to fail on demand, so this step shows the tag it fails with while no V2 market exists. Run: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --no-default-features --features clob,gamma --test live_clob -- --ignored --exact live_v2_market_is_served`

Expected while no V2 market is among the newest 100 open markets (none existed on 2026-10-07): the test fails, its stderr holds the line `polyoxide-class=environmental`, and the panic message reads `no Protocol V2 market among the newest 100 open markets`. If a V2 market exists, the run prints `test result: ok. 1 passed`, and a `/book`, `/tick-size` or `/neg-risk` refusal fails with a `real` tag that names the route: report that refusal, it is the answer to the open question. A network failure fails with `polyoxide-class=transient`: re-run it.

- [ ] **Step 3: Implement**

Nothing more: the test is the story's code, and it relies on Story 3's `Market::outcome_ids`.

- [ ] **Step 4: Run it to see it pass**

Run the same command as Step 2, then `python3 scripts/live_unwraps.py` and `direnv exec . env CARGO_INCREMENTAL=0 cargo clippy -p polyoxide-polymarket --no-default-features --features clob,gamma --all-targets -- -D warnings` and `direnv exec . cargo fmt --all -- --check`.

Expected: the run is either `ok. 1 passed` or the `environmental` failure above (both leave the story's Verify satisfied); `live_unwraps.py` exits 0 with no file over its count; clippy and fmt print nothing. If fmt reports a diff in `live_clob.rs`, run `direnv exec . cargo fmt --all` and keep the result.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(polymarket): live_v2_market_is_served, a read-only Protocol V2 probe (protocol v2, Story 5)" -- polyoxide-polymarket/tests/live_clob.rs
```

### Task 5.2: The resolution note and the deferred fill checklist

**Files:**
- Modify: `docs/specs/data-v2/OBSERVED.md` (a new section before the `/v2/tokens` section)
- Modify: `_bmad-output/implementation-artifacts/deferred-work.md` (append one entry at the end)

- [ ] **Step 1: Write the resolution note**

In `docs/specs/data-v2/OBSERVED.md`, insert before the heading ``## `/v2/tokens` (added after 2026-09-14)`` (keep a blank line on each side):

```markdown
## Protocol V2 conditions on `/v2/resolutions` (2026-11-04)

No code changed for Polymarket Protocol V2 (#51). The mirror's text already covers V2
conditions: `/v2/resolutions` "return[s] condition-grain rows in deterministic order, preferring
native V2 lifecycle, then UMA lifecycle, then terminal CTF state", and `Resolution` models
`payouts` (per-outcome micro-USDC per share, present on resolved condition-keyed rows) and
`market_type` (`BINARY`, `INCREMENTAL_NEGRISK` or `ATOMIC_NEGRISK`).

Not yet seen live: no V2 market was being served when this was written (none among the 40,000 open
markets sampled on 2026-10-07; `live_v2_market_is_served` reports `environmental` until one
exists), so no V2 condition's row has been read and the fields above are the mirror's claim, not an
observation. A caller reads payouts only from rows whose `status` is `resolved`. On the Gamma side a
V2 market's `resolution_status` replaces `uma_resolution_status` (`docs/specs/gamma/OBSERVED.md`).
```

- [ ] **Step 2: Write the deferred entry**

Append to the end of `_bmad-output/implementation-artifacts/deferred-work.md` (after the last entry's final line; the file's entries are `- source_spec:` items with `summary:`, `evidence:` and, once done, `status:`):

```markdown
- source_spec: `docs/superpowers/specs/2026-11-04-protocol-v2-trading-design.md`
  summary: Run upstream's Protocol V2 fill checklist on the first V2 market: a BUY fill and a SELL fill, each with the balance read before and after, on a V2 market and on a CTF (V1) market, through `Trading::place` and the CLOB client; check that the V2 BUY's `CONDITIONAL-V2` balance and the SELL's allowance read agree with the fill, that ExchangeV3 accepts the signed order, and that `Trading::place` reports the V3 `order_hash`.
  evidence: Spec "Non-goals" and "Live". No V2 market existed (none among 40,000 open markets on 2026-10-07), so ExchangeV3 signing, the `/neg-risk` skip and `CONDITIONAL-V2` reads are pinned to py-sdk 0.12.0's golden vectors and mock tests only; `live_v2_market_is_served` reads one V2 market's book, tick size and neg-risk flag once it exists and posts nothing. A fill run posts real orders, so it is blocked on the first V2 market and on the owner's go-ahead for a credentialed order run (live order safety).
```

- [ ] **Step 3: Check the docs**

Run: `cd .github/scripts && uv run pytest tests/test_claude_rules.py -q` and `python3 scripts/gen_registry.py --check` (from the repo root).

Expected: both pass (neither file has a generated region touched by this edit).

- [ ] **Step 4: Commit**

```bash
git commit -m "docs: the Protocol V2 resolution note and the deferred fill checklist (protocol v2, Story 5)" -- docs/specs/data-v2/OBSERVED.md _bmad-output/implementation-artifacts/deferred-work.md
```

### Task 5.3: The mutation battery and its ledger rows

**Files:**
- Modify: `docs/MUTANTS.md` (a paragraph before `## The rules`; rows after row (m))
- Modify: `.github/scripts/tests/test_mutants_ledger.py` (`SNIPPETS`)

This task edits no library code. It proves eleven mutations (ten rows of the spec's battery, one with three mutants, and a control) against the tree Stories 2 to 4 left, and records them. The mutations name code by item, because the lines they sit on were written by earlier stories; the task finds each line with `grep -n` at its own HEAD.

**Procedure for every mutation** (the "run" below):

1. `git status --short` prints nothing (no edit left from the last mutation). If it prints anything, `git checkout -- <file>` for each tracked file it names, and stop and report if a file you did not mutate is dirty.
2. Apply the mutation with one `Edit` to the file named, changing only what the mutation says.
3. Run the mutation's test command (below) with `--no-fail-fast`, keeping the output: `direnv exec . env CARGO_INCREMENTAL=0 cargo test -p polyoxide-polymarket --all-features --no-fail-fast <targets> 2>&1 | tee $SCRATCH/<row>.log | grep -E '^test .* FAILED|^test result'`, where `$SCRATCH` is the session's scratchpad directory and `<targets>` is the mutation's. The failing test names are the ones that kill the mutation. A unit test's file is the one whose module path its name gives (`clob::core::eip712::tests::x` is `polyoxide-polymarket/src/clob/core/eip712.rs`); an integration test's is the `--test` binary's file under `polyoxide-polymarket/tests/`.
4. Run `git diff` and keep it, then restore: `git checkout -- <file>`; `git diff --exit-code` prints nothing.
5. Run the same command again without the mutation: every test passes (`test result: ok`) and the failing ones from step 3 are among them.
6. **A mutation that no test fails is a stop**: restore the file, keep the `git diff` from step 4, and report it with its patch. Never weaken the mutation, never add a row for it, and never edit a test to kill it.

To cite a test, find its line with `grep -n 'fn <name>(' <file>`. To cite a rule line, `grep -n` for the mutated code. Record `git rev-parse --short HEAD` and `direnv exec . rustc --version` once, before the first mutation.

**Before the first mutation**, with a clean tree: `cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q`. It must pass: no story before this one edits `docs/MUTANTS.md`, so it fails only if Stories 2 to 4 moved a line an existing row cites. If it fails, each failing `(file, line)` names a row to re-cite: find where the cited code now is, update that row's `file:line` and its `SNIPPETS` key, re-prove that row's own mutant as its text says (the mutation column), and say so in the paragraph of Step 3. If more than three rows moved, stop and report.

The mutations, in order. `<clob targets>` is `--lib --test mock_clob --test trading_clob`; `<relay targets>` is `--lib --test mock_relay`; `<gamma targets>` is `--lib --test outcome_ids_gamma --test venue_clob`.

| Row | File and item | Mutation | Targets |
| --- | --- | --- | --- |
| (n) | `polyoxide-polymarket/src/clob/core/eip712.rs`, `order_exchange`'s V2 arm | the domain version it returns, `"3"`, becomes `"2"` | `<clob targets>` |
| (o) | the same arm | the exchange address it returns for a V2 order is what the V1 path would pick (`exchange_address(chain_id, <the order's neg_risk>)`) instead of `Chain::exchange_v3`'s, the version left `"3"` | `<clob targets>` |
| (p) | `polyoxide-polymarket/src/clob/core/eip712.rs`, `deposit_wallet::envelope_digest`, where the app domain's version is taken from the order's token id | the version is always `"2"` | `<clob targets>` |
| (q) | `polyoxide-polymarket/src/shared/protocol.rs`, `is_v2_position_id`'s two mask lines, `let reserved = U256::from(u64::MAX) << 40;` and `(value & reserved) == U256::ZERO && …` | three mutants, one at a time: at the first line the shift becomes `<< 41`, then `<< 39`; at the second line the test is inverted (`!= U256::ZERO` for `== U256::ZERO`) | `<clob targets>` |
| (r) | the same function | the decode condition dropped, so the function returns the mask test alone (then `1` is V2) | `<clob targets>` |
| (s) | `polyoxide-polymarket/src/clob/api/account.rs`, `balance_allowance`'s asset type for a V2 id | `CONDITIONAL-V2` becomes `CONDITIONAL` | `<clob targets>` |
| (t) | `polyoxide-polymarket/src/clob/client.rs`, `get_market_metadata`'s skip of `/neg-risk` for a V2 id | the V2 test removed, so `/neg-risk` is requested for every id | `<clob targets>` |
| (u) | `polyoxide-polymarket/src/gamma/types.rs`, `Market::outcome_ids`'s choice of the id field by `version` | the field is chosen by presence instead: `position_ids` when it is `Some`, else `clob_token_ids` (the version checks and every error stay) | `<gamma targets>` |
| (v) | `polyoxide-polymarket/src/relay/types.rs` (or wherever Story 4 put it), `parse_v2_condition_id`'s acceptance of 64 hex digits ending `00` or `01` | any last byte is accepted | `<relay targets>` |
| (w) | `polyoxide-polymarket/src/relay/deposit_wallet.rs`, `router_redeem_calldata`'s outcome-index argument | CTF's index set is sent in place of the outcome index (`1 << outcome_index`, so 1 and 2 for outcomes 0 and 1) | `<relay targets>` |
| (x), control | `polyoxide-polymarket/src/shared/protocol.rs`, `is_v2_position_id`'s conjunction of the mask test and the decode | the two operands of the `&&` swapped. Both are pure, so the function answers the same on every input: this one must **survive** | `<clob targets>` |

For (q), run the three mutants and record all three (each must be killed on its own). The `<< 41` and `<< 39` mutants are killed by the `id_vectors.json` rows 18 and 19 (`v2_binary` with bit 40 set, and with bit 104 set), which Story 1 recorded as the mask's edges, read by `shared::protocol`'s `the_rule_is_the_conjunction_of_py_sdks_two_answers`; if either shift mutant survives, stop and report it. For (x), step 3 prints no `FAILED` line and step 5 is the same result: that is the row's outcome, SURVIVES. If the function's shape differs from "mask test `&&` decode" (for example a single `matches!`), pick the nearest behaviour-neutral rewrite of the same function, say what it was in the row, and report the choice. If (x) fails any test, the battery's tests depend on an evaluation order or the control is not neutral: stop and report it.

- [ ] **Step 1: Run the battery**

Run the eleven rows as above (thirteen runs, (q) being three). After each, write down: the file and line of the mutated code (`grep -n` on the original line, after restoring), the failing test names with their files and lines, and for (x) the result "survives". Keep a list; Step 2 turns it into rows.

Expected for each row, as a sanity check on the tests rather than on the names (record whatever actually fails):

- (n), (o): the V2 digest and signature tests of `eip712.rs` for `v2_eoa`, `v2_neg_risk_eoa` and the two Deposit Wallet cases.
- (p): `the_digest_is_what_py_sdk_signs_and_the_order_hash_is_its_id` alone (it hashes the Deposit Wallet envelope through `envelope_digest`); the owner and session `sign_order_as` tests go through `envelope_digest_from` and `order_exchange`, so they do not fail.
- (q): the `id_vectors.json` tests of `shared::protocol` (rows 18 and 19 for the two shifts) and the V2 order tests; the inverted mask also fails the V1 control.
- (r): `v2_order_signature_is_deterministic` in `eip712.rs` (token `100`), `order_digest_uses_correct_exchange_for_neg_risk`, the session-key vectors (token `1`) and `shared::protocol`'s `id_vectors.json` conjunction test.
- (s): the `mock_clob.rs` asset-type table test.
- (t): the `mock_clob.rs` test that expects no `/neg-risk` request for a V2 id.
- (u): `outcome_ids_gamma.rs`'s `the_selector_agrees_with_py_sdk_on_every_vector` and `the_version_picks_the_field_when_both_are_present`.
- (v): the `condition_ids` table test in `deposit_wallet.rs`.
- (w): the redeem calldata tests in `deposit_wallet.rs` and `submit_v2_redeem`'s `mock_relay.rs` tests.

A row whose expected group did not fail is not by itself a stop (the tests may be named differently); a row where nothing failed is.

- [ ] **Step 2: Record the rows**

In `docs/MUTANTS.md`, insert the rows directly after row (m) (the table's last row), with no blank line between them and (m). Replace the exact text

```markdown

`polyoxide-polymarket/tests/polymarket_limits.rs:1054` `a_cooldown_extended_mid_wait_is_honoured_in_full`
held row (c), a hold extended mid-wait, until Story 3.3.
```

with the eleven rows below, then a blank line, then that same text unchanged. Each cell in angle brackets is filled from Step 1's record, with no angle bracket left; a `|` inside a cell is written `\|`; every `file:line` is written `polyoxide-polymarket/...rs:<line>` so the ledger test reads it.

```markdown
| (n) A Protocol V2 order is hashed under domain version `"3"` | `<eip712.rs>:<line>`, `order_exchange`'s V2 arm returns `"3"` | `"2"` in place of `"3"` | `<file>:<line>` `<test>`; one entry per failing test |
| (o) A Protocol V2 order is signed for ExchangeV3, never the CTF or neg-risk exchange, whatever `neg_risk` says | `<eip712.rs>:<line>`, `order_exchange`'s V2 arm takes `Chain::exchange_v3`'s address | the address the V1 path would pick, `version` left `"3"` | `<file>:<line>` `<test>`; one entry per failing test |
| (p) A Deposit Wallet order's envelope is hashed under the order's own protocol: a V2 order's app domain is ExchangeV3 at version `"3"` | `<eip712.rs>:<line>`, `deposit_wallet::envelope_digest` takes the version from the order's token id | the version always `"2"` | `<file>:<line>` `<test>`; one entry per failing test |
| (q) The id rule's reserved-bit mask is bits 40 to 103, tested for zero | `<protocol.rs>:<line>`, `is_v2_position_id`'s mask test | `<< 41` then `<< 39` at the first line, `!= U256::ZERO` for `== U256::ZERO` at the second | `<file>:<line>` `<test>`; one entry per failing test, naming which mutant each fails under |
| (r) The id rule needs the decode as well as the mask, so toy ids such as `1`, `100` and `123456` stay V1 | `<protocol.rs>:<line>`, `is_v2_position_id`'s conjunction | the decode condition dropped, so `1` is V2 | `<file>:<line>` `<test>`; one entry per failing test |
| (s) A V2 id's balance is read as `CONDITIONAL-V2` | `<account.rs>:<line>`, `balance_allowance`'s asset type | `CONDITIONAL` for `CONDITIONAL-V2` | `<file>:<line>` `<test>`; one entry per failing test |
| (t) `create_order` never asks `/neg-risk` about a V2 id | `<client.rs>:<line>`, `get_market_metadata`'s skip | the V2 test removed | `<file>:<line>` `<test>`; one entry per failing test |
| (u) A market's trading ids are chosen by `version`, never by which field is present | `<types.rs>:<line>`, `Market::outcome_ids`'s choice | the field chosen by presence | `<file>:<line>` `<test>`; one entry per failing test |
| (v) A Router condition id is 62 hex digits, or 64 ending `00` or `01` | `<file>:<line>`, `parse_v2_condition_id`'s last-byte check | any last byte accepted | `<file>:<line>` `<test>`; one entry per failing test |
| (w) A redeem sends the outcome index (0 or 1), never CTF's index set (1 or 2) | `<deposit_wallet.rs>:<line>`, `router_redeem_calldata`'s outcome-index argument | `1 << outcome_index` | `<file>:<line>` `<test>`; one entry per failing test |
| (x) Control: the order of the id rule's two conditions changes no answer | `<protocol.rs>:<line>`, `is_v2_position_id`'s conjunction | the operands of `&&` swapped (or the neutral rewrite Step 1 recorded) | none: the mutant SURVIVES, every test passes |
```

Then insert before the line `## The rules` (keep one blank line on each side) a paragraph that states what was proved. Fill the commit, date and compiler from Step 1:

```markdown
The Protocol V2 trading work (#51) added rows (n) to (x). Each was proved on <date> on top of `<short sha>`, on <rustc version> with `CARGO_INCREMENTAL=0`, by running `cargo test -p polyoxide-polymarket --all-features --no-fail-fast` over the test targets of its mutation (`--lib` with `--test mock_clob --test trading_clob` for the CLOB rows, with `--test outcome_ids_gamma --test venue_clob` for (u), with `--test mock_relay` for (v) and (w)): its tests passed, then failed with the mutant in place, then passed again once the file was restored, with `git diff --exit-code` clean. Row (x) is the control: its mutant swaps two pure conditions and every test passed with it in place, which shows the battery fails a mutant by its behaviour and not by the edit. <When Step 1's precheck re-cited an older row, say which rows and why here; otherwise delete this sentence.>
```

(Delete the angle-bracket sentence's text if nothing was re-cited; the paragraph must not keep an angle bracket.)

In `.github/scripts/tests/test_mutants_ledger.py`, add one `SNIPPETS` entry for every `file:line` the new rows and paragraph cite, once each, after the last entry. The last entry is

```python
    ("polyoxide-binance/tests/mock_spot.rs", 977): "async fn a_deep_depth_is_charged_its_band(",
}
```

Replace it with the same entry followed by the new ones and the closing brace. A rule line's value is a piece of that line that identifies the mutated code (for example `"3"` is too short to be unique; use the surrounding tokens as they stand, such as the `return`/tuple text of the arm); a test line's value is `fn <name>(`, which holds for `async fn` and `fn` alike:

```python
    ("polyoxide-binance/tests/mock_spot.rs", 977): "async fn a_deep_depth_is_charged_its_band(",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", <line>): "<piece of the order_exchange V2 arm's line>",
    ("polyoxide-polymarket/src/clob/core/eip712.rs", <line>): "fn <killing test name>(",
}
```

and so on for every cited pair. Python tuple keys repeat nowhere: a test cited by two rows has one entry.

- [ ] **Step 3: Run the ledger test**

Run: `cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q`

Expected: all pass. `test_every_citation_has_a_recorded_snippet_and_every_snippet_is_cited` fails if a `file:line` in a new row has no `SNIPPETS` entry or an entry is cited nowhere; `test_each_cited_line_still_holds_its_code` fails if a line number or snippet is off. Fix the row or the entry, never the mutation.

Then, from the repo root: `python3 scripts/gen_registry.py --check` and `cd .github/scripts && uv run pytest tests/test_claude_rules.py -q`; both pass (`docs/MUTANTS.md` has no generated region).

- [ ] **Step 4: Check that nothing else changed**

Run: `git status --short`. Expected: only `docs/MUTANTS.md` and `.github/scripts/tests/test_mutants_ledger.py` are modified (the mutated library files are restored). Any other modified tracked file is a mutation left in place: `git checkout -- <file>` it, and re-run Step 3.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs: the Protocol V2 mutation battery, rows (n) to (x) (protocol v2, Story 5)" -- docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py
```
