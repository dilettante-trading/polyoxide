# Absorb the Data Spec Drift — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adopt the upstream `data-api` spec into `polyoxide-data` — fixing a live defect that makes two `ActivityType` filters silently return nothing, adding the `Position` fee-basis fields, and adding the `/v1/approvals` endpoint.

**Architecture:** Additive changes to `polyoxide-data`: one builder method on `ListActivity`, two `Option<f64>` fields plus `#[non_exhaustive]` on `Position`, a typed `Allowance` enum with a forward-compatible `Unknown` arm, and a new `approvals()` namespace. Python bindings and the type stub must be updated in lockstep because CI enforces their parity.

**Tech Stack:** Rust (MSRV 1.91), `serde`, `rust_decimal`, `mockito`, `tokio`; PyO3 bindings with a `pytest`-enforced stub.

**Spec:** [docs/superpowers/specs/2026-08-15-data-spec-adoption-design.md](../specs/2026-08-15-data-spec-adoption-design.md)

---

## Background an implementer needs

`polyoxide-data` is a read-only client for `data-api.polymarket.com`. Clients are built with `DataApi::builder().build()`, endpoints are grouped into namespaces (`data.trades()`, `data.misc()`, …), and query parameters are chained on a request builder before `.send().await?`.

**The defect this fixes.** `GET /activity` takes a query parameter `excludeDepositsWithdrawals`, a boolean the server defaults to `true`. Upstream states the default applies *even when `type` requests those records*. `polyoxide-data` exposes `ActivityType::Deposit` and `ActivityType::Withdrawal` as filter values but never sends the parameter, so both filters return an empty list with no error today.

**Conventions to follow.**

- Types that upstream may extend carry an `#[serde(other)] Unknown` variant so an unrecognized value never fails deserialization of a whole response. `polyoxide-data/src/types.rs` has five already.
- Unit tests for deserialization live inline in `polyoxide-data/src/types.rs` under `#[cfg(test)] mod tests` (starts at line 1048).
- Query-parameter tests live in `polyoxide-data/tests/mock_api.rs` using `mockito` with `Matcher::UrlEncoded`.
- Live tests live in `polyoxide-data/tests/live_api.rs`, gated `#[ignore]`.

**Two CI gates that will bite.**

1. `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo doc` under `RUSTDOCFLAGS="-D warnings"`. A doc comment on a `pub` item may not link to a `pub(crate)` item. A red doc build silently withholds the release tag.
2. `polyoxide-py/tests/test_stub_consistency.py` introspects the **compiled** extension and asserts `__init__.pyi` declares exactly the attributes each class exposes — no missing entries, no phantom ones. Adding a field to `Position` therefore requires updating **both** `py_type!` and the stub, or the Python Bindings job fails.

Commands:

```bash
cargo test -p polyoxide-data --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
```

## Scope note — a gap in the spec

The design spec does not mention the Python bindings. It should: `polyoxide-py/src/types/data.rs` enumerates all 26 `Position` fields in a `py_type!` macro, and `polyoxide-py/python/polyoxide/__init__.pyi` mirrors them, with a test enforcing parity. **Task 3 covers this.** Flagging it rather than silently absorbing it, because the spec is the reviewed artifact and this was not in it.

## File structure

| File | Responsibility | Change |
|---|---|---|
| `polyoxide-data/src/api/users.rs` | `ListActivity` builder | Modify |
| `polyoxide-data/src/types.rs` | Domain types + inline tests | Modify |
| `polyoxide-data/src/api/approvals.rs` | Approvals namespace | Create |
| `polyoxide-data/src/api/mod.rs` | Module registration | Modify |
| `polyoxide-data/src/client.rs` | `approvals()` accessor | Modify |
| `polyoxide-data/Cargo.toml` | `rust_decimal` dependency | Modify |
| `polyoxide-data/tests/mock_api.rs` | Query + response mock tests | Modify |
| `polyoxide-data/tests/live_api.rs` | Live `/v1/approvals` test | Modify |
| `polyoxide-py/src/types/data.rs` | `py_type!` field list | Modify |
| `polyoxide-py/python/polyoxide/__init__.pyi` | Type stub | Modify |
| `docs/specs/data/openapi.yaml` | Vendored mirror | Replace |
| `CLAUDE.md` | Namespace list | Modify |
| `Cargo.toml` | Workspace version | Modify |

---

## Task 1: Make deposit and withdrawal filters reachable

The live defect. Purely additive: the crate omits unspecified query parameters, so a caller who never invokes this method sees no behavioural change.

**Files:**
- Modify: `polyoxide-data/src/api/users.rs` — `ListActivity` impl (starts line 315)
- Modify: `polyoxide-data/src/types.rs` — `ActivityType::Deposit` / `::Withdrawal` doc comments (lines 266-269)
- Test: `polyoxide-data/tests/mock_api.rs`

- [ ] **Step 1: Write the failing tests**

Append to `polyoxide-data/tests/mock_api.rs`:

```rust
#[tokio::test]
async fn activity_sends_exclude_deposits_withdrawals_when_set() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/activity")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xabc123".into()),
            Matcher::UrlEncoded("excludeDepositsWithdrawals".into(), "false".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let data = test_data(&server);
    data.user("0xabc123")
        .activity()
        .exclude_deposits_withdrawals(false)
        .send()
        .await
        .expect("activity");

    mock.assert_async().await;
}

#[tokio::test]
async fn activity_omits_exclude_deposits_withdrawals_when_unset() {
    // Proves the fix is additive: an existing caller's request is unchanged.
    //
    // `Matcher::Exact` on `match_query` compares the WHOLE query string, so
    // this asserts absence — any extra parameter fails the match. Mockito's
    // `Matcher::Missing` is a unit variant for *headers* and cannot express
    // "this query key is absent".
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/activity")
        .match_query(Matcher::Exact("user=0xabc123".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let data = test_data(&server);
    data.user("0xabc123").activity().send().await.expect("activity");

    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p polyoxide-data --all-features --test mock_api activity_sends_exclude 2>&1 | tail -20
```

Expected: compile error — `no method named 'exclude_deposits_withdrawals' found for struct 'ListActivity'`.

- [ ] **Step 3: Add the builder method**

In `polyoxide-data/src/api/users.rs`, inside `impl ListActivity`, immediately after the `activity_type` method:

```rust
    /// Include deposit and withdrawal rows (`excludeDepositsWithdrawals`).
    ///
    /// Upstream defaults this to `true` and applies the default **even when
    /// [`activity_type`](Self::activity_type) explicitly requests**
    /// [`ActivityType::Deposit`] or [`ActivityType::Withdrawal`], so those two
    /// filters return an empty list unless this is called with `false`.
    ///
    /// Leaving it unset sends no parameter, preserving upstream's default.
    pub fn exclude_deposits_withdrawals(mut self, exclude: bool) -> Self {
        self.request = self.request.query("excludeDepositsWithdrawals", exclude);
        self
    }
```

- [ ] **Step 4: Point the enum variants at the fix**

In `polyoxide-data/src/types.rs`, replace these two doc comments:

```rust
    /// Collateral deposit
    Deposit,
    /// Collateral withdrawal
    Withdrawal,
```

with:

```rust
    /// Collateral deposit.
    ///
    /// Upstream excludes deposit rows by default, so filtering on this alone
    /// returns nothing. Pair it with
    /// [`ListActivity::exclude_deposits_withdrawals(false)`](crate::api::users::ListActivity::exclude_deposits_withdrawals).
    Deposit,
    /// Collateral withdrawal.
    ///
    /// Upstream excludes withdrawal rows by default, so filtering on this alone
    /// returns nothing. Pair it with
    /// [`ListActivity::exclude_deposits_withdrawals(false)`](crate::api::users::ListActivity::exclude_deposits_withdrawals).
    Withdrawal,
```

`ListActivity` is `pub` in a `pub` module, so this intra-doc link resolves and will not trip the rustdoc gate.

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cargo test -p polyoxide-data --all-features --test mock_api activity 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 6: Verify the doc gate**

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features -p polyoxide-data 2>&1 | tail -5
```

Expected: no warnings. A broken intra-doc link here fails CI and silently withholds the release tag.

- [ ] **Step 7: Commit**

```bash
git add polyoxide-data/src/api/users.rs polyoxide-data/src/types.rs polyoxide-data/tests/mock_api.rs
git commit -m "fix(data): make deposit and withdrawal activity filters reachable

Upstream defaults excludeDepositsWithdrawals to true and applies it even when
\`type\` requests those records, so ActivityType::Deposit and ::Withdrawal
returned an empty list with no error. Two of twelve variants were unreachable."
```

---

## Task 2: `Position` fee basis

**Files:**
- Modify: `polyoxide-data/src/types.rs` — `Position` (struct at line 386, ends line 437)
- Test: `polyoxide-data/src/types.rs` inline `mod tests`

- [ ] **Step 1: Write the failing test**

Append inside `#[cfg(test)] mod tests` in `polyoxide-data/src/types.rs`:

```rust
    #[test]
    fn deserialize_position_fee_basis() {
        // `entryFeesUsdc: 0` is a *measured* zero and must not collapse to None:
        // upstream returns an explicit 0 when the fee component is zero, and
        // omits the field entirely when the data is unavailable.
        let json = r#"{
            "proxyWallet": "0xabc123",
            "asset": "token123",
            "conditionId": "cond456",
            "size": 100.5,
            "avgPrice": 0.65,
            "initialValue": 65.0,
            "grossInitialValue": 65.5,
            "entryFeesUsdc": 0,
            "currentValue": 70.0,
            "cashPnl": 5.0,
            "percentPnl": 7.69,
            "totalBought": 100.5,
            "realizedPnl": 2.0,
            "percentRealizedPnl": 3.08,
            "curPrice": 0.70,
            "redeemable": false,
            "mergeable": true,
            "title": "Will X happen?",
            "slug": "will-x-happen",
            "outcome": "Yes",
            "outcomeIndex": 0,
            "oppositeOutcome": "No",
            "oppositeAsset": "token789",
            "negativeRisk": false
        }"#;

        let pos: Position = serde_json::from_str(json).unwrap();
        assert_eq!(pos.gross_initial_value, Some(65.5));
        assert_eq!(pos.entry_fees_usdc, Some(0.0));
        // initialValue keeps fee-exclusive semantics, so it is NOT the gross figure.
        assert!((pos.initial_value - 65.0).abs() < f64::EPSILON);
    }

    #[test]
    fn deserialize_position_without_fee_basis_is_none() {
        // Older payloads omit both fields; None means "unavailable", not zero.
        let json = r#"{
            "proxyWallet": "0xabc123",
            "asset": "token123",
            "conditionId": "cond456",
            "size": 100.5,
            "avgPrice": 0.65,
            "initialValue": 65.0,
            "currentValue": 70.0,
            "cashPnl": 5.0,
            "percentPnl": 7.69,
            "totalBought": 100.5,
            "realizedPnl": 2.0,
            "percentRealizedPnl": 3.08,
            "curPrice": 0.70,
            "redeemable": false,
            "mergeable": true,
            "title": "Will X happen?",
            "slug": "will-x-happen",
            "outcome": "Yes",
            "outcomeIndex": 0,
            "oppositeOutcome": "No",
            "oppositeAsset": "token789",
            "negativeRisk": false
        }"#;

        let pos: Position = serde_json::from_str(json).unwrap();
        assert_eq!(pos.gross_initial_value, None);
        assert_eq!(pos.entry_fees_usdc, None);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p polyoxide-data --all-features --lib position_fee 2>&1 | tail -15
```

Expected: compile error — `no field 'gross_initial_value' on type 'Position'`.

- [ ] **Step 3: Add the fields and mark the struct non-exhaustive**

In `polyoxide-data/src/types.rs`, change the `Position` attributes from:

```rust
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
```

to:

```rust
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Position {
```

Then add these two fields immediately after the existing `pub initial_value: f64,`:

```rust
    /// Remaining entry basis including attributed BUY fees.
    ///
    /// [`initial_value`](Self::initial_value) and [`avg_price`](Self::avg_price)
    /// keep their fee-**exclusive** semantics, so the fee-exclusive basis is
    /// `gross_initial_value - entry_fees_usdc`. `None` means upstream omitted
    /// the field — treat that as unavailable, not as zero.
    #[serde(default)]
    pub gross_initial_value: Option<f64>,
    /// Attributed BUY-fee component of [`gross_initial_value`](Self::gross_initial_value).
    ///
    /// SELL fees are exit costs and are never included. Upstream returns an
    /// explicit `0` when the component is zero, so `Some(0.0)` (a measured
    /// zero) and `None` (no data) are different answers.
    #[serde(default)]
    pub entry_fees_usdc: Option<f64>,
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p polyoxide-data --all-features --lib position 2>&1 | tail -10
```

Expected: PASS, including the pre-existing `deserialize_position_from_json`, which omits both fields and must still succeed.

- [ ] **Step 5: Confirm nothing else in the workspace broke**

```bash
cargo build --all-features --workspace 2>&1 | tail -10
```

Expected: success. `#[non_exhaustive]` only restricts construction from *other* crates, and nothing constructs a `Position` literal anywhere in this workspace.

- [ ] **Step 6: Commit**

```bash
git add polyoxide-data/src/types.rs
git commit -m "feat(data)!: add Position fee-basis fields, mark non_exhaustive

grossInitialValue includes attributed BUY fees while initialValue and avgPrice
keep fee-exclusive semantics, so the fee-exclusive basis is the difference.
Both are Option because upstream says to treat omission as unavailable rather
than zero — Some(0.0) is a measured zero, None is no data.

BREAKING CHANGE: Position gains public fields and is now non_exhaustive."
```

---

## Task 3: Python bindings and stub

`polyoxide-py/tests/test_stub_consistency.py` introspects the compiled extension and asserts the stub declares exactly the attributes each class exposes. Adding fields to `Position` without updating both files fails the Python Bindings CI job.

**Files:**
- Modify: `polyoxide-py/src/types/data.rs` — `py_type!` for `PyPosition` (lines 3-32)
- Modify: `polyoxide-py/python/polyoxide/__init__.pyi` — `Position` class stub

- [ ] **Step 1: Add the fields to the `py_type!` list**

In `polyoxide-py/src/types/data.rs`, in the `PyPosition` invocation, add the two identifiers immediately after `initial_value,`:

```rust
    initial_value,
    gross_initial_value,
    entry_fees_usdc,
    current_value,
```

- [ ] **Step 2: Add the properties to the stub**

In `polyoxide-py/python/polyoxide/__init__.pyi`, in the `Position` class, add immediately after the `initial_value` property:

```python
    @property
    def gross_initial_value(self) -> Any: ...
    @property
    def entry_fees_usdc(self) -> Any: ...
```

- [ ] **Step 3: Run the stub consistency test**

```bash
cd polyoxide-py && uv run pytest tests/test_stub_consistency.py -v 2>&1 | tail -15
```

Expected: PASS. A failure naming `gross_initial_value` or `entry_fees_usdc` means one of the two files was missed — the test reports missing and phantom attributes separately, so the message says which direction.

- [ ] **Step 4: Commit**

```bash
git add polyoxide-py/src/types/data.rs polyoxide-py/python/polyoxide/__init__.pyi
git commit -m "feat(py): expose Position fee-basis fields

py_type! and the .pyi stub enumerate fields independently and CI enforces
their parity, so both move together."
```

---

## Task 4: The `Allowance` type

`ApprovalContract.amount` is a string that is either the literal `"max"` or a decimal allowance in the token's base units, and is absent for `ERC1155` entries.

**Files:**
- Modify: `polyoxide-data/Cargo.toml`
- Modify: `polyoxide-data/src/types.rs`
- Test: `polyoxide-data/src/types.rs` inline `mod tests`

- [ ] **Step 1: Add the dependency**

In `polyoxide-data/Cargo.toml`, under `[dependencies]`, after the `serde_json` line:

```toml
rust_decimal = { workspace = true }
```

`rust_decimal` is already a workspace dependency at version 1.37; this adds no new third-party crate to the tree.

- [ ] **Step 2: Write the failing tests**

Append inside `#[cfg(test)] mod tests` in `polyoxide-data/src/types.rs`:

```rust
    #[test]
    fn deserialize_allowance_max_sentinel() {
        let v: Allowance = serde_json::from_str(r#""max""#).unwrap();
        assert_eq!(v, Allowance::Max);
    }

    #[test]
    fn deserialize_allowance_decimal_amount() {
        let v: Allowance = serde_json::from_str(r#""1000000""#).unwrap();
        assert_eq!(v, Allowance::Amount(rust_decimal::Decimal::new(1_000_000, 0)));
    }

    #[test]
    fn deserialize_allowance_beyond_decimal_range_is_unknown() {
        // rust_decimal tops out near 7.9e28; a uint256 allowance can reach
        // 1.2e77. Without the Unknown arm this would fail the whole response.
        let huge = "1".repeat(40);
        let json = format!(r#""{huge}""#);
        let v: Allowance = serde_json::from_str(&json).unwrap();
        assert_eq!(v, Allowance::Unknown(huge));
    }

    #[test]
    fn deserialize_allowance_unrecognized_sentinel_is_unknown() {
        let v: Allowance = serde_json::from_str(r#""unlimited""#).unwrap();
        assert_eq!(v, Allowance::Unknown("unlimited".to_string()));
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

```bash
cargo test -p polyoxide-data --all-features --lib allowance 2>&1 | tail -15
```

Expected: compile error — `cannot find type 'Allowance' in this scope`.

- [ ] **Step 4: Implement `Allowance`**

Add near the other enums in `polyoxide-data/src/types.rs` (after `ActivityType`'s `Display` impl):

```rust
/// An ERC20 allowance as reported by `/v1/approvals`.
///
/// Upstream sends a string that is either the sentinel `"max"` or a decimal
/// amount in the token's base units. `ERC1155` entries carry no amount at all,
/// which is represented by `Option::None` on the containing field rather than
/// by a variant here.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Allowance {
    /// The unlimited-allowance sentinel (`"max"`).
    Max,
    /// A concrete allowance, in the token's base units.
    Amount(rust_decimal::Decimal),
    /// A value that is neither `"max"` nor a decimal `rust_decimal` can hold,
    /// preserved verbatim.
    ///
    /// `rust_decimal` tops out near 7.9e28 while a uint256 allowance can reach
    /// 1.2e77, so an unusually large approval lands here instead of failing
    /// deserialization of the entire response.
    Unknown(String),
}

impl<'de> Deserialize<'de> for Allowance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        if raw == "max" {
            return Ok(Allowance::Max);
        }
        Ok(match raw.parse::<rust_decimal::Decimal>() {
            Ok(amount) => Allowance::Amount(amount),
            Err(_) => Allowance::Unknown(raw),
        })
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cargo test -p polyoxide-data --all-features --lib allowance 2>&1 | tail -10
```

Expected: PASS, all four.

- [ ] **Step 6: Run clippy**

```bash
cargo clippy -p polyoxide-data --all-targets --all-features -- -D warnings 2>&1 | tail -10
```

Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add polyoxide-data/Cargo.toml polyoxide-data/src/types.rs
git commit -m "feat(data): add typed Allowance for approval amounts

The wire format packs a sentinel and a number into one string. The Unknown arm
absorbs values beyond rust_decimal's ~7.9e28 ceiling — a uint256 allowance
reaches 1.2e77 — so an unusual approval degrades to one unparsed field instead
of failing the whole response."
```

---

## Task 5: Approval types

**Files:**
- Modify: `polyoxide-data/src/types.rs`
- Test: `polyoxide-data/src/types.rs` inline `mod tests`

- [ ] **Step 1: Write the failing tests**

Append inside `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn deserialize_approvals_response() {
        let json = r#"{
            "address": "0xabc123",
            "chainId": 137,
            "checkedAt": "2026-08-10T12:34:56Z",
            "contracts": [
                {
                    "id": "UsdcExchange",
                    "feature": "trading",
                    "token": "0xtoken",
                    "spender": "0xspender",
                    "standard": "ERC20",
                    "amount": "max",
                    "approved": true
                },
                {
                    "id": "CtfExchangeIsApprovedForAll",
                    "feature": "auto-redeem",
                    "token": "0xctf",
                    "spender": "0xspender2",
                    "standard": "ERC1155",
                    "approved": false
                }
            ]
        }"#;

        let resp: ApprovalsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.chain_id, 137);
        assert_eq!(resp.contracts.len(), 2);

        let erc20 = &resp.contracts[0];
        assert_eq!(erc20.feature, ApprovalFeature::Trading);
        assert_eq!(erc20.standard, ApprovalStandard::Erc20);
        assert_eq!(erc20.amount, Some(Allowance::Max));
        assert!(erc20.approved);

        // ERC1155 entries carry no amount at all.
        let erc1155 = &resp.contracts[1];
        assert_eq!(erc1155.feature, ApprovalFeature::AutoRedeem);
        assert_eq!(erc1155.standard, ApprovalStandard::Erc1155);
        assert_eq!(erc1155.amount, None);
        assert!(!erc1155.approved);
    }

    #[test]
    fn deserialize_approval_enums_tolerate_unknown_variants() {
        // Upstream adds features over time; an unrecognized value must not
        // fail the whole response.
        let json = r#"{
            "id": "SomethingNew",
            "feature": "staking",
            "token": "0xtoken",
            "spender": "0xspender",
            "standard": "ERC721",
            "approved": true
        }"#;

        let c: ApprovalContract = serde_json::from_str(json).unwrap();
        assert_eq!(c.feature, ApprovalFeature::Unknown);
        assert_eq!(c.standard, ApprovalStandard::Unknown);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p polyoxide-data --all-features --lib approval 2>&1 | tail -15
```

Expected: compile error — `cannot find type 'ApprovalsResponse' in this scope`.

- [ ] **Step 3: Implement the types**

Add to `polyoxide-data/src/types.rs`, after the `Allowance` impl:

```rust
/// What a tracked approval unlocks.
///
/// Upstream's values are lowercase and kebab-cased, unlike the UPPERCASE
/// enums elsewhere in this API, so each variant renames explicitly.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalFeature {
    /// Order placement and settlement.
    #[serde(rename = "trading")]
    Trading,
    /// Perpetual futures.
    #[serde(rename = "perps")]
    Perps,
    /// Liquidity reward accrual.
    #[serde(rename = "rewards")]
    Rewards,
    /// Automatic redemption of resolved positions.
    #[serde(rename = "auto-redeem")]
    AutoRedeem,
    /// A feature this client does not recognize (forward-compat).
    #[serde(other)]
    Unknown,
}

/// Token standard of a tracked approval.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStandard {
    /// Carries an allowance amount.
    #[serde(rename = "ERC20")]
    Erc20,
    /// An operator flag with no amount.
    #[serde(rename = "ERC1155")]
    Erc1155,
    /// A standard this client does not recognize (forward-compat).
    #[serde(other)]
    Unknown,
}

/// Approval state for one token and spender pair.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalContract {
    /// Stable identifier for the pair, such as `UsdcExchange`.
    pub id: String,
    /// What the approval unlocks.
    pub feature: ApprovalFeature,
    /// Token contract address.
    pub token: String,
    /// Spender contract address.
    pub spender: String,
    /// Token standard.
    pub standard: ApprovalStandard,
    /// Allowance for `ERC20` entries. Always `None` for `ERC1155`, which is an
    /// operator flag with no amount — read [`approved`](Self::approved) instead.
    #[serde(default)]
    pub amount: Option<Allowance>,
    /// Whether the approval is sufficient for its feature.
    pub approved: bool,
}

/// Token approval state for a wallet, from `GET /v1/approvals`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalsResponse {
    /// The wallet the approvals were read for.
    pub address: String,
    /// Chain the approvals were read on.
    pub chain_id: u64,
    /// RFC 3339 timestamp of when the response was generated.
    ///
    /// Left as a string deliberately: upstream tracks approval state from
    /// onchain events rather than reading fresh, so parsing this into a
    /// timestamp type would imply a freshness guarantee it does not carry.
    pub checked_at: String,
    /// Every approval Polymarket tracks, in a stable display order.
    ///
    /// Pairs the wallet has never approved are still present with `approved`
    /// false, so the length does not vary with wallet state. Upstream does not
    /// publish how many entries that is — do not depend on a count.
    pub contracts: Vec<ApprovalContract>,
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p polyoxide-data --all-features --lib approval 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-data/src/types.rs
git commit -m "feat(data): add approval types for /v1/approvals

Feature and standard both carry Unknown catch-alls: upstream adds features
over time and an unrecognized value must not fail a whole response. checkedAt
stays a string because upstream derives it from events, not a fresh read."
```

---

## Task 6: The `approvals()` namespace

**Files:**
- Create: `polyoxide-data/src/api/approvals.rs`
- Modify: `polyoxide-data/src/api/mod.rs`
- Modify: `polyoxide-data/src/client.rs`
- Test: `polyoxide-data/tests/mock_api.rs`, `polyoxide-data/tests/live_api.rs`

- [ ] **Step 1: Write the failing mock test**

Append to `polyoxide-data/tests/mock_api.rs`:

```rust
#[tokio::test]
async fn approvals_returns_contracts() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/approvals")
        .match_query(Matcher::UrlEncoded("user".into(), "0xabc123".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "address": "0xabc123",
                "chainId": 137,
                "checkedAt": "2026-08-10T12:34:56Z",
                "contracts": [{
                    "id": "UsdcExchange",
                    "feature": "trading",
                    "token": "0xtoken",
                    "spender": "0xspender",
                    "standard": "ERC20",
                    "amount": "max",
                    "approved": true
                }]
            }"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let resp = data.approvals().get("0xabc123").await.expect("approvals");

    assert_eq!(resp.address, "0xabc123");
    assert_eq!(resp.chain_id, 137);
    assert_eq!(resp.contracts[0].id, "UsdcExchange");
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cargo test -p polyoxide-data --all-features --test mock_api approvals 2>&1 | tail -15
```

Expected: compile error — `no method named 'approvals' found for struct 'DataApi'`.

- [ ] **Step 3: Create the namespace module**

Create `polyoxide-data/src/api/approvals.rs`:

```rust
use polyoxide_core::{HttpClient, Request};

use crate::{error::DataApiError, types::ApprovalsResponse};

/// Approvals namespace (`/v1/approvals`).
#[derive(Clone)]
pub struct ApprovalsApi {
    pub(crate) http_client: HttpClient,
}

impl ApprovalsApi {
    /// Get token approval state for a wallet (`GET /v1/approvals`).
    ///
    /// Reports whether the wallet has granted the approvals Polymarket needs,
    /// so a client can prompt for the missing ones instead of reading each
    /// allowance onchain. Every tracked token and spender pair is returned,
    /// including pairs the wallet has never approved.
    pub fn get(
        &self,
        user_address: impl Into<String>,
    ) -> Request<ApprovalsResponse, DataApiError> {
        Request::new(self.http_client.clone(), "/v1/approvals")
            .query("user", user_address.into())
    }
}
```

- [ ] **Step 4: Register the module**

In `polyoxide-data/src/api/mod.rs`, add in alphabetical order, before `pub mod builders;`:

```rust
pub mod approvals;
```

- [ ] **Step 5: Wire the client accessor**

In `polyoxide-data/src/client.rs`, add to the `use crate::{api::{...}}` block, in alphabetical order after `accounting::AccountingApi,`:

```rust
        approvals::ApprovalsApi,
```

Then add this accessor immediately before the existing `pub fn misc` method:

```rust
    /// Get approvals namespace (`/v1/approvals`)
    pub fn approvals(&self) -> ApprovalsApi {
        ApprovalsApi {
            http_client: self.http_client.clone(),
        }
    }
```

- [ ] **Step 6: Run the mock test to verify it passes**

```bash
cargo test -p polyoxide-data --all-features --test mock_api approvals 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 7: Add the live test**

Append to `polyoxide-data/tests/live_api.rs`:

```rust
#[tokio::test]
#[ignore]
async fn live_approvals() {
    let client = client();
    let resp = client
        .approvals()
        .get(TEST_USER)
        .await
        .expect("approvals");

    // Upstream returns every tracked pair regardless of wallet state, but does
    // not publish how many — assert non-empty, never a fixed count.
    assert!(
        !resp.contracts.is_empty(),
        "approvals should list tracked contracts even for an unused wallet"
    );
    assert_eq!(resp.chain_id, 137, "Polymarket settles on Polygon");
}
```

- [ ] **Step 8: Run the live test manually**

```bash
cargo test -p polyoxide-data --all-features --test live_api live_approvals -- --ignored --nocapture 2>&1 | tail -10
```

Expected: PASS against the real API. If it fails on `chain_id`, report the actual value rather than relaxing the assertion — a different chain would be genuine news.

- [ ] **Step 9: Run the full crate suite and clippy**

```bash
cargo test -p polyoxide-data --all-features 2>&1 | tail -5
cargo clippy -p polyoxide-data --all-targets --all-features -- -D warnings 2>&1 | tail -5
```

Expected: all pass, no warnings.

- [ ] **Step 10: Commit**

```bash
git add polyoxide-data/src/api/approvals.rs polyoxide-data/src/api/mod.rs polyoxide-data/src/client.rs polyoxide-data/tests/mock_api.rs polyoxide-data/tests/live_api.rs
git commit -m "feat(data): add approvals namespace for /v1/approvals

Upstream tags the route Misc, but it gets its own namespace for
discoverability and room to grow."
```

---

## Task 7: Spec sync, docs, and version bump

**Files:**
- Modify: `docs/specs/data/openapi.yaml`
- Modify: `CLAUDE.md`
- Modify: `Cargo.toml`

- [ ] **Step 1: Sync the vendored spec**

```bash
curl -fsSL https://docs.polymarket.com/api-spec/data-openapi.yaml -o docs/specs/data/openapi.yaml
```

- [ ] **Step 2: Confirm the drift is resolved**

```bash
cd .github/scripts && uv run python diff_openapi.py check \
  --crate data \
  --upstream-yaml <(curl -fsSL https://docs.polymarket.com/api-spec/data-openapi.yaml) \
  --vendored-yaml ../../docs/specs/data/openapi.yaml \
  --upstream-url https://docs.polymarket.com/api-spec/data-openapi.yaml \
  --vendored-label docs/specs/data/openapi.yaml \
  --output-dir /tmp/data-check
echo "exit=$?"
```

Expected: `exit=0` — no drift. This is what closes issue #22 on the next nightly.

- [ ] **Step 3: Add the namespace to CLAUDE.md**

In `CLAUDE.md`, find the Data namespace list beginning `- Data: \`data.user(addr)\`` and add `` `data.approvals()` `` immediately after `` `data.accounting()` ``, keeping the existing comma-separated style.

- [ ] **Step 4: Bump the workspace version**

In the root `Cargo.toml`, change:

```toml
version = "0.26.1"
```

to:

```toml
version = "0.27.0"
```

A minor bump rather than a patch: `Position` gained public fields and became `#[non_exhaustive]`, both breaking under semver for a pre-1.0 crate's minor position.

- [ ] **Step 5: Run the full workspace gate**

```bash
cargo build --all-features --workspace 2>&1 | tail -5
cargo test --all-features --workspace 2>&1 | tail -5
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace 2>&1 | tail -5
cargo fmt --all -- --check
cd polyoxide-py && uv run pytest tests/test_stub_consistency.py -q 2>&1 | tail -3
```

Expected: all green. The `cargo doc` step is the one that silently withholds release tags when it fails, so do not skip it.

- [ ] **Step 6: Commit**

```bash
git add docs/specs/data/openapi.yaml CLAUDE.md Cargo.toml
git commit -m "chore(specs): sync the data spec and release 0.27.0

Closes the last of the six drift findings, and the only one that touched
shipped code."
```

---

## Out of scope

The four remaining drift issues — `perps` (#23), `perps-ws` (#20), `bridge` (#24), `combos-rfq-ws` (#21) — are mirror-only syncs for APIs polyoxide does not implement. Each is a `curl` and a commit with no code to write, and they are deliberately not bundled here.
