# polyoxide CLOB V2 Order-Signing Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate `polyoxide-clob`'s order signing/submission from the legacy Polymarket **CLOB V1** scheme to **CLOB V2** (live since 2026-04-28), so polyoxide can place live orders again — and, as a free consequence of V2, attribute fills to a builder code.

**Architecture:** V2 changed the EIP-712 signed `Order` struct (dropped `taker`/`nonce`/`feeRateBps`/`expiration` from signing; added `timestamp`/`metadata`/`builder`), bumped the exchange domain version `"1"→"2"`, and moved to new exchange contracts. Fees are no longer signed (collected on-chain at match). Builder attribution is the signed `builder` (bytes32) field — **not** an auth header. We migrate polyoxide's existing hand-rolled signing (`core/eip712.rs` keeps its `sol!`-struct + manual EIP-712 digest assembly, just swapping the struct + domain) and reshape the wire `Order`/`SignedOrder` types. **V2-only** (V1 is dead on mainnet; see Design Decisions).

**Tech stack:** Rust, `polyoxide-clob`/`polyoxide-core`, `alloy` (EIP-712 via `sol!` + `eip712_hash_struct`), `rust_decimal`, `reqwest`, `tokio`. Tests via `cargo test`/`cargo nextest`. Authoritative V2 reference: Polymarket's official Rust client [`rs-clob-client-v2`](https://github.com/Polymarket/rs-clob-client-v2) and the [V2 migration doc](https://docs.polymarket.com/v2-migration).

---

## ⚠️ Execution context (read first)

- **This plan executes in the polyoxide repo.** Paths are relative to the polyoxide workspace; verify against repo `HEAD` before editing.
- **This is a breaking change** to `polyoxide-clob`'s public `Order`/`SignedOrder`/`SignatureType` and the EIP-712 output. Pre-1.0, ship as a minor bump (**0.18.0**).
- **Re-validation needs live resources** (Task 8): a funded **proxy** Polymarket account (`SignatureType::PolyProxy`) + L2 creds, and willingness to place **one tiny real order**. Until Task 8 passes live, the migration is "compiles + unit-tested," not "proven."
- The Phase-0 spike left throwaway code (`Clob::post_order_with_builder_code` in `client.rs`, `examples/builder_spike.rs`, the `eprintln!`/timestamp/salt patches). **Task 0 reverts it.**

## Authoritative V2 facts (from `rs-clob-client-v2`)

- **EIP-712 domain:** name `"Polymarket CTF Exchange"`, version `"2"`, chainId `137`, verifyingContract = V2 exchange.
- **V2 exchange contracts (Polygon):** CTF `0xE111180000d2663C0091e4f400237545B87B996B`; NegRisk `0xe2222d279d744050d28e00520010520000310F59`.
- **V2 signed type string (the typehash source — field order is load-bearing):**
  `Order(uint256 salt,address maker,address signer,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,uint256 timestamp,bytes32 metadata,bytes32 builder)`
- **Wire `order` body (POST `/order`):** `salt` (u64 *number*), `maker`, `signer`, `tokenId`/`makerAmount`/`takerAmount`/`timestamp`/`expiration` (decimal *strings*), `side` (`"BUY"`/`"SELL"`), `signatureType` (number), `metadata`/`builder` (`0x`+64hex), `signature`. **`expiration` is in the wire body but NOT signed.** Outer wrapper: `{ order, orderType, owner, postOnly?, deferExec? }`.
- **`SignatureType`:** `EOA=0, PolyProxy=1, PolyGnosisSafe=2, Poly1271=3`. `Poly1271` (EIP-1271 smart-wallet, Solady `TypedDataSign` wrapped signature) is **V2-only and deferred** here (see Design Decisions).
- **Build rules:** `maker = funder.unwrap_or(signer)`; `signer = signer` (except Poly1271); `timestamp = now_ms`; `metadata`/`builder` default `B256::ZERO`.

## Design Decisions

1. **V2-only (no runtime version negotiation).** The official client supports both V1 and V2 and negotiates via a `/version` endpoint. V1 is dead on mainnet, so polyoxide hard-targets V2 (YAGNI). *Alternative the reviewer may choose:* port the official client's version-resolution + auto-retry-on-mismatch. Not in this plan.
2. **Keep polyoxide's hand-rolled EIP-712 assembly** (`sol!` struct + `eip712_hash_struct` + manual `\x19\x01` digest in `compute_order_digest`). We only swap the struct fields (V2) and domain version. Smallest correct change.
3. **Defer `Poly1271`.** Add the enum variant (value `3`) for completeness, but `build`/sign only support `EOA`/`PolyProxy`/`PolyGnosisSafe` (standard ECDSA). Constructing an order with `Poly1271` returns a validation error pointing here. The target proxy account uses `PolyProxy`.
4. **Builder code type = `alloy::primitives::B256`** (bytes32), surfaced as `ClobBuilder::builder_code(B256)`; defaults to `B256::ZERO` (no attribution).
5. **Fees leave the signing path.** `feeRateBps` is gone from the signed order; `create_order`/`create_market_order` no longer fetch `/fee-rate` for signing. (`markets().fee_rate()` stays as a public read for callers that want display fees.)

## File structure

| File | Responsibility | Change |
|---|---|---|
| `polyoxide-clob/src/core/chain.rs` | exchange contract addresses | swap `exchange`/`neg_risk_exchange` to V2 |
| `polyoxide-clob/src/core/eip712.rs` | `sol!` `Order` struct + domain + digest + `order_to_protocol` | V2 struct (11 fields), domain version `"2"` |
| `polyoxide-clob/src/types.rs` | `Order`/`SignedOrder` wire types, `SignatureType` | V2 fields (`timestamp`/`metadata`/`builder`, drop `taker`/`nonce`/`feeRateBps` from signing; `expiration` wire-only); add `Poly1271` |
| `polyoxide-clob/src/client.rs` | `build_order`, `create_order`/`create_market_order`, `post_order`/`post_orders`, `ClobBuilder`/`Clob` | V2 order build (+ `timestamp`/`metadata`/`builder`); builder-code plumbing; drop fee-from-signing |
| `polyoxide-clob/src/api/auth.rs` | `create_builder_key` | L2 auth + `BuilderApiKeyResponse{key,...}` |
| `polyoxide-clob/CHANGELOG.md` / workspace `Cargo.toml` | release | `0.18.0` entry + version bump |

---

## Task 0: Revert the Phase-0 spike code

**Files:** Modify `polyoxide-clob/src/client.rs`; Delete `polyoxide-clob/examples/builder_spike.rs`

- [ ] **Step 1: Remove the spike method.** Delete the entire `pub async fn post_order_with_builder_code(...)` method from `client.rs` (the Phase-0 helper that injected `builder`/`timestamp`/`salt`/`eprintln!`).
- [ ] **Step 2: Delete the spike example.**

```bash
git rm polyoxide-clob/examples/builder_spike.rs
```

- [ ] **Step 3: Verify clean build.**

Run: `cargo build -p polyoxide-clob --all-features`
Expected: compiles, no reference to `post_order_with_builder_code`.

- [ ] **Step 4: Commit.**

```bash
git add -A && git commit -m "chore(clob): remove Phase-0 builder-attribution spike code"
```

---

## Task 1: V2 exchange contract addresses

**Files:** Modify `polyoxide-clob/src/core/chain.rs`; Test: same file's `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test.** Pin the V2 mainnet exchange addresses so a regression is caught.

```rust
#[test]
fn polygon_mainnet_uses_v2_exchanges() {
    let c = Contracts::POLYGON_MAINNET;
    assert_eq!(
        c.exchange,
        address!("E111180000d2663C0091e4f400237545B87B996B"),
        "CTF Exchange must be the V2 contract"
    );
    assert_eq!(
        c.neg_risk_exchange,
        address!("e2222d279d744050d28e00520010520000310F59"),
        "NegRisk Exchange must be the V2 contract"
    );
}
```

- [ ] **Step 2: Run it.** Run: `cargo test -p polyoxide-clob polygon_mainnet_uses_v2_exchanges`. Expected: FAIL (still the V1 addresses).
- [ ] **Step 3: Implement.** In `Contracts::POLYGON_MAINNET` (`chain.rs:51`) replace the two exchange addresses:

```rust
    pub const POLYGON_MAINNET: Self = Self {
        exchange: address!("E111180000d2663C0091e4f400237545B87B996B"), // CTF Exchange V2
        neg_risk_exchange: address!("e2222d279d744050d28e00520010520000310F59"), // NegRisk CTF Exchange V2
        neg_risk_adapter: address!("d91E80cF2E7be2e162c6513ceD06f1dD0dA35296"),
        collateral: address!("2791Bca1f2de4661ED88A30C99A7a9449Aa84174"),
        conditional_tokens: address!("4D97DCd97eC945f40cF65F87097ACe5EA0476045"),
    };
```

  > `neg_risk_adapter`/`collateral`/`conditional_tokens` are unchanged by V2. Leave `POLYGON_AMOY` as-is and note in the CHANGELOG that V2 addresses are configured for mainnet only (Amoy testnet V2 addresses unverified).

- [ ] **Step 4: Run.** Expected: PASS.
- [ ] **Step 5: Commit.**

```bash
git add polyoxide-clob/src/core/chain.rs
git commit -m "feat(clob)!: point CLOB exchange addresses at V2 contracts"
```

---

## Task 2: V2 EIP-712 struct, domain, and digest

**Files:** Modify `polyoxide-clob/src/core/eip712.rs`; Test: same file's `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing test.** The V2 typehash preimage must match the on-chain V2 contract's type string exactly. `alloy`'s `sol!` derives `Order::eip712_encode_type()`; assert it.

```rust
#[test]
fn v2_order_type_string_matches_contract() {
    use alloy::sol_types::SolStruct;
    let expected = "Order(uint256 salt,address maker,address signer,uint256 tokenId,\
uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,\
uint256 timestamp,bytes32 metadata,bytes32 builder)";
    assert_eq!(protocol::Order::eip712_encode_type(), expected);
}
```

- [ ] **Step 2: Run it.** Run: `cargo test -p polyoxide-clob v2_order_type_string_matches_contract`. Expected: FAIL (current struct is V1 with `taker`/`expiration`/`nonce`/`feeRateBps`).
- [ ] **Step 3: Implement the V2 struct.** In `eip712.rs` replace the `protocol::Order` `sol!` definition (`eip712.rs:26-39`) with the V2 fields (order is load-bearing):

```rust
        #[derive(Debug, PartialEq, Eq)]
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
```

- [ ] **Step 4: Update `order_to_protocol`.** Rewrite the mapping (`eip712.rs:49-78`) to the V2 fields. `ClobOrder` gains `timestamp`/`metadata`/`builder` and loses `taker`/`nonce`/`fee_rate_bps` in Task 3; write the mapping to the V2 shape now (it will compile after Task 3 lands the type — if doing strict TDD, land Task 3's type first or use a temporary local struct). Final form:

```rust
fn order_to_protocol(order: &ClobOrder) -> Result<protocol::Order, ClobError> {
    Ok(protocol::Order {
        salt: U256::from_str_radix(&order.salt, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid salt: {}", e)))?,
        maker: order.maker,
        signer: order.signer,
        tokenId: U256::from_str_radix(&order.token_id, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid token_id: {}", e)))?,
        makerAmount: U256::from_str_radix(&order.maker_amount, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid maker_amount: {}", e)))?,
        takerAmount: U256::from_str_radix(&order.taker_amount, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid taker_amount: {}", e)))?,
        side: match order.side {
            crate::types::OrderSide::Buy => 0,
            crate::types::OrderSide::Sell => 1,
        },
        signatureType: order.signature_type as u8,
        timestamp: U256::from_str_radix(&order.timestamp, 10)
            .map_err(|e| ClobError::Crypto(format!("Invalid timestamp: {}", e)))?,
        metadata: order.metadata,
        builder: order.builder,
    })
}
```

  > `signatureType as u8` requires `SignatureType` to be `#[repr(u8)]` (Task 3, Step 4). `metadata`/`builder` are `alloy::primitives::B256` on `ClobOrder` (Task 3).

- [ ] **Step 5: Bump the domain version to `"2"`.** In `compute_order_digest` (`eip712.rs:95-100`) change only the version:

```rust
    let domain = protocol::EIP712Domain {
        name: "Polymarket CTF Exchange".to_string(),
        version: "2".to_string(),
        chainId: U256::from(chain_id),
        verifyingContract: verifying_contract,
    };
```

  (The `verifying_contract` selection — `exchange` vs `neg_risk_exchange` — is unchanged and now resolves to V2 addresses via Task 1.)

- [ ] **Step 6: Add a deterministic signature regression test.** Sign a fixed order with the well-known Hardhat key and assert a stable signature (guards the digest end-to-end). Use the existing test scaffolding in `eip712.rs` (`PrivateKeySigner`, `address!`).

```rust
#[tokio::test]
async fn v2_order_signature_is_deterministic() {
    use alloy::signers::local::PrivateKeySigner;
    let signer: PrivateKeySigner =
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            .parse().unwrap();
    let order = ClobOrder {
        salt: "479249096354".to_string(),
        maker: address!("0000000000000000000000000000000000000001"),
        signer: address!("0000000000000000000000000000000000000002"),
        token_id: "100".to_string(),
        maker_amount: "1000000".to_string(),
        taker_amount: "5000000".to_string(),
        side: crate::types::OrderSide::Buy,
        signature_type: crate::types::SignatureType::Eoa,
        timestamp: "1700000000000".to_string(),
        metadata: alloy::primitives::B256::ZERO,
        builder: alloy::primitives::B256::ZERO,
        expiration: "0".to_string(),
        neg_risk: false,
    };
    let sig1 = sign_order(&order, &signer, 137).await.unwrap();
    let sig2 = sign_order(&order, &signer, 137).await.unwrap();
    assert_eq!(sig1, sig2);
    assert!(sig1.starts_with("0x") && sig1.len() == 132);
}
```

- [ ] **Step 7: Run all eip712 tests.** Run: `cargo test -p polyoxide-clob --lib core::eip712`. Expected: PASS (after Task 3's `ClobOrder` shape lands; if running Task 2 standalone, expect the mapping/test to fail to compile until Task 3 — land them together).
- [ ] **Step 8: Commit.**

```bash
git add polyoxide-clob/src/core/eip712.rs
git commit -m "feat(clob)!: sign the CLOB V2 EIP-712 Order struct (domain v2)"
```

---

## Task 3: V2 `Order`/`SignedOrder` types + `SignatureType::Poly1271`

**Files:** Modify `polyoxide-clob/src/types.rs`; Test: same file's `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing wire-shape test.** The serialized `order` body must carry the V2 fields and omit the V1-only signed fields.

```rust
#[test]
fn signed_order_serializes_v2_wire_shape() {
    let order = Order {
        salt: "479249096354".to_string(),
        maker: alloy::primitives::Address::ZERO,
        signer: alloy::primitives::Address::ZERO,
        token_id: "100".to_string(),
        maker_amount: "1000000".to_string(),
        taker_amount: "5000000".to_string(),
        side: OrderSide::Buy,
        expiration: "0".to_string(),
        signature_type: SignatureType::PolyProxy,
        timestamp: "1700000000000".to_string(),
        metadata: alloy::primitives::B256::ZERO,
        builder: alloy::primitives::B256::ZERO,
        neg_risk: false,
    };
    let signed = SignedOrder { order, signature: "0xdead".to_string() };
    let v = serde_json::to_value(&signed).unwrap();
    // present
    for k in ["salt","maker","signer","tokenId","makerAmount","takerAmount",
              "side","expiration","signatureType","timestamp","metadata","builder","signature"] {
        assert!(v.get(k).is_some(), "missing wire field {k}");
    }
    // absent (V1-only signed fields)
    for k in ["taker","nonce","feeRateBps"] {
        assert!(v.get(k).is_none(), "V1 field {k} must be gone");
    }
    // salt is a JSON number; amounts are strings; builder is 0x+64hex
    assert!(v["salt"].is_number());
    assert!(v["makerAmount"].is_string());
    assert_eq!(v["builder"].as_str().unwrap().len(), 66);
}
```

- [ ] **Step 2: Run it.** Run: `cargo test -p polyoxide-clob signed_order_serializes_v2_wire_shape`. Expected: FAIL (current `Order` has `taker`/`nonce`/`feeRateBps`, no `timestamp`/`metadata`/`builder`).
- [ ] **Step 3: Rewrite the `Order` struct.** Replace the `Order` definition (`types.rs:227-243`) with the V2 wire shape. Keep `serialize_salt` (emits a u64 number — matches the official client). Add `metadata`/`builder` as `B256` (alloy serializes to `0x`+64hex). `expiration` stays as a wire string (not signed). Remove `taker`/`nonce`/`fee_rate_bps`.

```rust
use alloy::primitives::B256;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde(serialize_with = "serialize_salt")]
    pub salt: String,
    pub maker: Address,
    pub signer: Address,
    pub token_id: String,
    pub maker_amount: String,
    pub taker_amount: String,
    pub side: OrderSide,
    pub expiration: String, // wire-only (GTD); NOT part of the V2 signed struct
    pub signature_type: SignatureType,
    pub timestamp: String,  // unix ms, used for order uniqueness in V2
    pub metadata: B256,
    pub builder: B256,
    #[serde(skip)]
    pub neg_risk: bool,
}
```

  > `SignedOrder { #[serde(flatten)] order, signature }` (`types.rs:266-270`) is unchanged — flatten still produces the inner `order` body, now V2-shaped.

- [ ] **Step 4: Make `SignatureType` `#[repr(u8)]` + add `Poly1271`.** So `signature_type as u8` works in `eip712.rs` and the V2 variant exists. Keep the existing serde (serializes as the integer).

```rust
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignatureType {
    #[default]
    Eoa = 0,
    PolyProxy = 1,
    PolyGnosisSafe = 2,
    /// EIP-1271 smart-contract wallet (V2). Signing is **not yet implemented**;
    /// see the V2 migration plan's Design Decisions.
    Poly1271 = 3,
}
```

  Update its `Serialize`/`Deserialize` (`types.rs:53-111`) to map `3 <-> Poly1271`, and `is_proxy()` if present (Poly1271 is not a Gamma-proxy resolution case — leave `is_proxy()` true only for `PolyProxy`/`PolyGnosisSafe`).

- [ ] **Step 5: Run.** Run: `cargo test -p polyoxide-clob signed_order_serializes_v2_wire_shape`. Expected: PASS. Then `cargo build -p polyoxide-clob` — expect errors in `client.rs`/`eip712.rs` referencing removed fields; those are fixed in Tasks 2 & 4.
- [ ] **Step 6: Commit.**

```bash
git add polyoxide-clob/src/types.rs
git commit -m "feat(clob)!: V2 Order wire shape (timestamp/metadata/builder; drop taker/nonce/feeRateBps)"
```

---

## Task 4: V2 `build_order` + builder-code plumbing

**Files:** Modify `polyoxide-clob/src/client.rs`; Test: `client.rs` tests

- [ ] **Step 1: Write the failing test.** A `Clob` built with a builder code stamps it onto the order; default is zero.

```rust
#[test]
fn build_order_v2_sets_builder_and_timestamp() {
    let code = alloy::primitives::B256::from([0x11u8; 32]);
    let order = Clob::build_order_v2(
        "100".to_string(),
        alloy::primitives::Address::ZERO,
        alloy::primitives::Address::ZERO,
        "1000000".to_string(),
        "5000000".to_string(),
        OrderSide::Buy,
        SignatureType::PolyProxy,
        false,
        Some(0),
        code,
        alloy::primitives::B256::ZERO,
        1_700_000_000_000,
    );
    assert_eq!(order.builder, code);
    assert_eq!(order.metadata, alloy::primitives::B256::ZERO);
    assert_eq!(order.timestamp, "1700000000000");
    assert_eq!(order.expiration, "0");
}
```

- [ ] **Step 2: Run.** `cargo test -p polyoxide-clob build_order_v2_sets_builder_and_timestamp`. Expected: FAIL (no such fn).
- [ ] **Step 3: Implement `build_order` V2.** Replace `Clob::build_order` (`client.rs:404-432`) with a V2 builder that takes `timestamp_ms`, `metadata`, `builder` and drops `fee_rate_bps`/`taker`/`nonce`:

```rust
#[allow(clippy::too_many_arguments)]
fn build_order_v2(
    token_id: String,
    maker: Address,
    signer: Address,
    maker_amount: String,
    taker_amount: String,
    side: OrderSide,
    signature_type: SignatureType,
    neg_risk: bool,
    expiration: Option<u64>,
    builder: alloy::primitives::B256,
    metadata: alloy::primitives::B256,
    timestamp_ms: u128,
) -> Order {
    Order {
        salt: generate_salt(),
        maker,
        signer,
        token_id,
        maker_amount,
        taker_amount,
        side,
        expiration: expiration.unwrap_or(0).to_string(),
        signature_type,
        timestamp: timestamp_ms.to_string(),
        metadata,
        builder,
        neg_risk,
    }
}
```

- [ ] **Step 4: Add builder-code state.** Add `builder_code: alloy::primitives::B256` to `Clob` (`client.rs:32`) and `ClobBuilder` (`client.rs:586`, default `B256::ZERO`), a `ClobBuilder::builder_code(self, B256) -> Self` setter, and carry it through `build()` (`client.rs:704`). Reject `Poly1271` at build time inside `create_order` (Design Decision 3):

```rust
// in create_order / create_market_order, after resolving signature_type:
if signature_type == SignatureType::Poly1271 {
    return Err(ClobError::validation(
        "Poly1271 (EIP-1271) signing is not yet supported; use EOA/PolyProxy/PolyGnosisSafe",
    ));
}
```

- [ ] **Step 5: Wire `create_order`/`create_market_order` to V2.** In both (`client.rs:174`, `:218`): drop the `get_fee_rate` call from the signing path, compute `timestamp_ms = SystemTime::now()...as_millis()`, and call `build_order_v2(...)` passing `self.builder_code` and `B256::ZERO` metadata. Keep tick-size/neg-risk fetch and `calculate_order_amounts` unchanged.
- [ ] **Step 6: Run.** `cargo test -p polyoxide-clob build_order_v2_sets_builder_and_timestamp` then `cargo build -p polyoxide-clob`. Expected: PASS + compiles.
- [ ] **Step 7: Commit.**

```bash
git add polyoxide-clob/src/client.rs
git commit -m "feat(clob): V2 order build + optional builder_code attribution"
```

---

## Task 5: `post_order`/`post_orders` submit the V2 body

**Files:** Modify `polyoxide-clob/src/client.rs`; Test: `client.rs` tests

- [ ] **Step 1: Write the failing test.** The submit payload nests the V2 `order` and the outer wrapper keys.

```rust
#[test]
fn post_order_payload_is_v2_wrapper() {
    // Build a SignedOrder via the public types, then assert the wrapper the
    // submit path constructs. Extract wrapper construction into a pure helper:
    let signed = SignedOrder {
        order: Order { /* …minimal V2 order, builder = 0x11..; see Task 4 test… */
            salt: "1".into(), maker: Address::ZERO, signer: Address::ZERO,
            token_id: "100".into(), maker_amount: "1".into(), taker_amount: "1".into(),
            side: OrderSide::Buy, expiration: "0".into(),
            signature_type: SignatureType::PolyProxy, timestamp: "1700000000000".into(),
            metadata: alloy::primitives::B256::ZERO,
            builder: alloy::primitives::B256::from([0x11u8; 32]), neg_risk: false },
        signature: "0xabc".into(),
    };
    let payload = Clob::order_submit_payload(&signed, OrderKind::Gtc, false, "owner-key");
    assert_eq!(payload["owner"], "owner-key");
    assert_eq!(payload["orderType"], "GTC");
    assert_eq!(payload["postOnly"], false);
    assert_eq!(payload["order"]["builder"].as_str().unwrap().len(), 66);
    assert!(payload["order"]["timestamp"].is_string());
}
```

- [ ] **Step 2: Run.** `cargo test -p polyoxide-clob post_order_payload_is_v2_wrapper`. Expected: FAIL (no helper).
- [ ] **Step 3: Implement the helper + route both submit fns through it.**

```rust
fn order_submit_payload(
    signed_order: &SignedOrder,
    order_type: OrderKind,
    post_only: bool,
    owner_key: &str,
) -> serde_json::Value {
    serde_json::json!({
        "order": signed_order,
        "owner": owner_key,
        "orderType": order_type,
        "postOnly": post_only,
    })
}
```

  Replace the inline `serde_json::json!` payloads in `post_order` (`client.rs:492`) and `post_orders` (`client.rs:453`) with calls to `order_submit_payload(...)`. (Auth stays plain `AuthMode::L2`.)

- [ ] **Step 4: Run.** Expected: PASS. Then the full crate suite: `cargo test -p polyoxide-clob --all-features`. Expected: PASS.
- [ ] **Step 5: Commit.**

```bash
git add polyoxide-clob/src/client.rs
git commit -m "feat(clob): submit V2 order wire body via shared helper"
```

---

## Task 6: Fix `create_builder_key` (L2 auth + correct response type)

**Files:** Modify `polyoxide-clob/src/api/auth.rs`; Test: `auth.rs` tests

- [ ] **Step 1: Write the failing test.** `BuilderApiKeyResponse` deserializes the `key`/`secret`/`passphrase` shape the endpoint actually returns.

```rust
#[test]
fn builder_api_key_response_deserializes_key_field() {
    let json = r#"{"key":"019894b9-cb40-79c4-b2bd-6aecb6f8c6c5","secret":"c2VjcmV0","passphrase":"pass"}"#;
    let resp: BuilderApiKeyResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.key, "019894b9-cb40-79c4-b2bd-6aecb6f8c6c5");
    assert_eq!(resp.secret, "c2VjcmV0");
    assert_eq!(resp.passphrase, "pass");
}
```

- [ ] **Step 2: Run.** `cargo test -p polyoxide-clob builder_api_key_response_deserializes_key_field`. Expected: FAIL (no such type).
- [ ] **Step 3: Implement.** Add the response type and switch `create_builder_key` to **L2** auth (per `openapi.yaml:3025-3030` + `auth.md:63`):

```rust
/// Response from `POST /auth/builder-api-key` (note: `key`, not `apiKey`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderApiKeyResponse {
    pub key: String,
    pub secret: String,
    pub passphrase: String,
}
```

```rust
    /// Create a new builder API key (L2 auth)
    pub fn create_builder_key(&self) -> Request<BuilderApiKeyResponse> {
        Request::post(
            self.http_client.clone(),
            "/auth/builder-api-key".to_string(),
            self.l2_auth(),
            self.chain_id,
        )
    }
```

  > Signature change: drops the unused `nonce: u32` param (L2 needs no nonce). Update the re-export in `lib.rs` (`pub use api::auth::{… BuilderApiKeyResponse}`) and any `create_builder_key(` call sites.

- [ ] **Step 4: Run.** Expected: PASS. `cargo build -p polyoxide-clob --all-features`. Expected: compiles.
- [ ] **Step 5: Commit.**

```bash
git add polyoxide-clob/src/api/auth.rs polyoxide-clob/src/lib.rs
git commit -m "fix(clob): create_builder_key uses L2 auth and BuilderApiKeyResponse"
```

---

## Task 7: Workspace gate + clippy

- [ ] **Step 1: Full gate.** Run: `cargo clippy --all-targets --all-features --workspace -- -D warnings && cargo test --workspace --all-features`. Expected: green. Fix any fallout (e.g. `polyoxide-cli`/`polyoxide-py` references to removed `Order` fields or `create_builder_key(nonce)`).
- [ ] **Step 2: Doctests.** Run: `cargo test --doc -p polyoxide-clob`. Expected: PASS (update any rustdoc that constructs an `Order` with old fields).
- [ ] **Step 3: Commit any fixes.**

```bash
git add -A && git commit -m "fix: update call sites for V2 order types"
```

---

## Task 8: Live V2 re-validation (manual, `#[ignore]`)

**Files:** Modify `polyoxide-clob/tests/live_api.rs`

- [ ] **Step 1: Add an ignored live test** that places a tiny marketable order and cancels the resting remainder — proving V2 submission works. Reuse the `authenticated_client()`/`find_active_token_id()` helpers.

```rust
#[tokio::test]
#[ignore] // live; run with `-- --ignored`, needs funded proxy account
async fn live_v2_place_and_cancel() {
    let clob = authenticated_client();
    let token_id = find_active_token_id().await;
    let book = clob.markets().order_book(&token_id).send().await.unwrap();
    let ask = book.asks.iter().map(|l| l.price).min().expect("asks");
    let price: f64 = ask.to_string().parse().unwrap();
    let params = CreateOrderParams {
        token_id, price, size: 5.0, side: OrderSide::Buy,
        order_type: OrderKind::Gtc, post_only: false, expiration: None,
        funder: None, signature_type: Some(SignatureType::PolyProxy),
    };
    let order = clob.create_order(&params, None).await.unwrap();
    let signed = clob.sign_order(&order).await.unwrap();
    let resp = clob.post_order(&signed, OrderKind::Gtc, false).await.unwrap();
    assert!(resp.success, "V2 order rejected: {:?}", resp.error_msg);
    if let Some(id) = resp.order_id { let _ = clob.orders().unwrap().cancel(id).send().await; }
}
```

- [ ] **Step 2: Run it live** (requires `.env` with a funded proxy account): `cargo test -p polyoxide-clob --test live_api live_v2_place_and_cancel -- --ignored --nocapture`. Expected: `success=true` (no more "Invalid order payload"). **This is the gate that proves the migration.**
- [ ] **Step 3 (optional): builder attribution live check.** With a registered `POLYMARKET_BUILDER_CODE`, build the `Clob` via `ClobBuilder::new().with_account(..).builder_code(code).signature_type(PolyProxy)`, place one tiny order, then poll `account_api().builder_trades(code)` for the new fill. Record the result.
- [ ] **Step 4: Commit.**

```bash
git add polyoxide-clob/tests/live_api.rs
git commit -m "test(clob): live V2 place-and-cancel integration test"
```

---

## Task 9: Docs, changelog, release 0.18.0

- [ ] **Step 1: Builder-code rustdoc.** Add an example on `ClobBuilder::builder_code` showing attribution, and update `docs/specs/clob/auth.md` / `orders.md` to note V2 (signed `builder` field; fees off-order). Verify: `cargo test --doc -p polyoxide-clob`. Expected: PASS.
- [ ] **Step 2: CHANGELOG.** Add `## [0.18.0]`:

```markdown
### Changed (breaking)
- **CLOB V2 migration.** Orders are now signed with the Polymarket CLOB V2 EIP-712 scheme (domain version "2", V2 exchange contracts, 11-field signed struct). V1-shaped orders are rejected by the live exchange as of 2026-04-28. `Order`/`SignedOrder` gained `timestamp`/`metadata`/`builder` and dropped `taker`/`nonce`/`feeRateBps` from the signed struct; `feeRateBps` is no longer signed (fees collected on-chain at match).
- `SignatureType` gained `Poly1271` (EIP-1271; signing not yet implemented).
- `create_builder_key` now uses L2 auth and returns `BuilderApiKeyResponse { key, secret, passphrase }`.

### Added
- Builder-program attribution: `ClobBuilder::builder_code(B256)` stamps the signed `builder` field on every order.
```

- [ ] **Step 3: Version bump.** Set workspace `Cargo.toml` `version = "0.18.0"`.
- [ ] **Step 4: Final gate.** Run: `cargo clippy --all-targets --all-features --workspace -- -D warnings && cargo test --workspace --all-features && cargo fmt --all -- --check`. Expected: green.
- [ ] **Step 5: Release.** Tag + publish 0.18.0 via the repo's existing release flow (`.github/workflows/release.yml`); do not hand-publish if CI owns it.

---

## Self-review

- **Spec coverage:** V2 domain/version (Task 2 ✓), V2 contracts (Task 1 ✓), V2 signed struct + typehash (Task 2 ✓), V2 wire body incl. unsigned `expiration` (Task 3, 5 ✓), `timestamp`/`metadata`/`builder` (Tasks 3–4 ✓), fees off-signing (Task 4 ✓), builder attribution as signed field (Tasks 4–5 ✓), `Poly1271` enum + deferral (Task 3 ✓), `create_builder_key` bugs (Task 6 ✓), live re-validation (Task 8 ✓). Builder-code provenance (Builder Profile, off-API) is documented in the design spec, not code.
- **Placeholder scan:** none — every code step has concrete content. The one deliberate deferral (`Poly1271` signing) is explicit and guarded by a validation error.
- **Type consistency:** `ClobOrder`/`Order` fields (`timestamp: String`, `metadata: B256`, `builder: B256`, no `taker`/`nonce`/`fee_rate_bps`) are used identically across `eip712.rs` (Task 2), `types.rs` (Task 3), and `client.rs` (Tasks 4–5). `SignatureType` is `#[repr(u8)]` so `as u8` (Task 2) is valid. `build_order_v2`/`order_submit_payload`/`builder_code` names are consistent across tasks.
- **Open risk:** the deterministic signature vector in Task 2 Step 6 is a self-consistency regression guard, not a cross-checked golden value — Task 8's live `success=true` is the real proof the V2 signature is accepted. Amoy testnet V2 addresses are unverified (mainnet-only here).

---

## Post-execution note (2026-06-30) — salt masking, discovered during live re-validation

This plan assumed the existing `generate_salt()` (a raw `u64`) was V2-safe. **It is not.** Task 8's live re-validation rejected every order with `"Invalid order payload"` until the order `salt` was masked to the **JavaScript-safe-integer range** — `value & ((1 << 53) - 1)` — matching the official client's `to_ieee_754_int`. Polymarket parses `salt` as a JS-safe integer / signed int64; a raw `u64` (which exceeds 2^53 ~99.95% of the time) is corrupted on the wire, and because `salt` is part of the EIP-712 signed struct, that corruption invalidates the signature. None of the offline tests (typehash byte-match, deterministic signature, full-path body) caught this — only the live order did.

**Fix:** `polyoxide-clob/src/utils.rs::generate_salt` masks the nonce to 53 bits, with a regression test (`generate_salt_is_js_safe_integer`). After the fix a real V2 order was **accepted by the live exchange**. Any future re-execution (or a from-scratch port) MUST keep the salt mask.
