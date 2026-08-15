# Absorb the Data Spec Drift — Design

**Status:** Approved (2026-08-15)
**Author:** aidanb
**Branch:** `aidanb/issues`
**Follows:** [2026-08-14-schema-drift-detection-design.md](2026-08-14-schema-drift-detection-design.md) (S1, PR #19) and [2026-08-15-drift-convergence-design.md](2026-08-15-drift-convergence-design.md) (S2, PR #26)
**Closes:** issue #22 (`Schema drift: data`)

## Goal

Adopt the upstream `data-api` spec into `polyoxide-data`, closing the only drift finding that touches shipped code.

## Context — this is S3 of three

S1 made drift findings true; S2 made them converge. Both were CI work. This one is the payoff: the finding that has consequences for users of the SDK.

Of the six issues the pipeline filed, five are mirror-only syncs for APIs polyoxide does not implement. `data` is the exception.

### The headline is a live defect, not new surface

`GET /activity` gained one query parameter, `excludeDepositsWithdrawals`, a boolean **defaulting to `true`**. Upstream's description is explicit:

> Excludes deposit and withdrawal records. The default `true` applies even when `type` requests those records, so to get deposits and withdrawals you must pass `false`.

`polyoxide-data` already exposes `ActivityType::Deposit` and `ActivityType::Withdrawal` as filter values on `ListActivity::activity_type`. Because the client never sends `excludeDepositsWithdrawals`, the server applies its default and returns nothing. **Two of the twelve `ActivityType` variants are unreachable, and the failure is silent** — an empty list, not an error.

### The raw diff overstated the /activity change

Issue #22's summary reports changes to `/activity` parameters 5 through 10, which reads like a rewrite. It is a positional artifact: upstream inserted one parameter at index 6, and `start`, `end`, `sortBy`, `sortDirection` all renumbered. Comparing by parameter *name* rather than index:

```
ADDED  : ['excludeDepositsWithdrawals']
REMOVED: []
CHANGED: type   (description only — schemas identical)
```

One addition, no removals, no semantic changes to existing parameters. Recorded here because the index-shift artifact is a trap the next reader of a drift issue will also fall into.

## Decisions

| # | Question | Decision |
|---|----------|----------|
| 1 | Where does the activity fix go? | Here, with the spec sync that documents why. |
| 2 | `Position`'s new fields and semver | `Option<f64>` plus `#[non_exhaustive]`; one breaking bump instead of two. |
| 3 | `ApprovalContract.amount` | A typed `Allowance` enum, not a raw string. |
| 4 | Where does `/v1/approvals` live? | A new `approvals()` namespace, despite upstream tagging it `Misc`. |
| 5 | Version | 0.26.1 → **0.27.0**. |

## Component 1 — `ListActivity::exclude_deposits_withdrawals`

**File:** `polyoxide-data/src/api/users.rs`

```rust
/// Include deposit and withdrawal rows (`excludeDepositsWithdrawals`).
///
/// Upstream defaults this to `true` and applies it even when
/// [`activity_type`](Self::activity_type) explicitly requests
/// [`ActivityType::Deposit`] or [`ActivityType::Withdrawal`], so those two
/// filters return nothing unless this is set to `false`.
pub fn exclude_deposits_withdrawals(mut self, exclude: bool) -> Self
```

The crate omits unspecified query parameters, so a caller who never touches this method sees no behavioural change. The fix is purely additive.

## Component 2 — `Position` fee basis

**File:** `polyoxide-data/src/types.rs`

Add to `Position`:

```rust
    /// Remaining entry basis including attributed BUY fees.
    ///
    /// `initial_value` and `avg_price` keep their fee-*exclusive* semantics,
    /// so the fee-exclusive basis is `gross_initial_value - entry_fees_usdc`.
    /// `None` means upstream omitted the field — treat that as unavailable,
    /// not as zero.
    pub gross_initial_value: Option<f64>,
    /// Attributed BUY-fee component of `gross_initial_value`. SELL fees are
    /// exit costs and are never included. Upstream returns an explicit `0`
    /// when the component is zero, so `Some(0.0)` and `None` differ: the
    /// former is a measured zero, the latter is no data.
    pub entry_fees_usdc: Option<f64>,
```

`Option` is upstream's own instruction ("treat an omitted field as unavailable rather than as zero"), and the `Some(0.0)` versus `None` distinction is load-bearing — collapsing it would report a fee-free entry as unmeasured, or worse, the reverse.

Mark the struct `#[non_exhaustive]`. It is a deserialization target, so real-world breakage from a struct literal is unlikely, but the change is technically breaking either way and doing it now makes every future field addition free.

## Component 3 — `Allowance`

**File:** `polyoxide-data/src/types.rs`

`ApprovalContract.amount` is a string that is either the literal `"max"` or a decimal allowance in the token's base units, and is absent for `ERC1155` entries.

```rust
pub enum Allowance {
    /// The unlimited-allowance sentinel (`"max"`).
    Max,
    /// A concrete allowance, in the token's base units.
    Amount(Decimal),
    /// A value that is neither `"max"` nor a representable decimal,
    /// preserved verbatim.
    Unknown(String),
}
```

Custom `Deserialize`: `"max"` → `Max`; parses as `Decimal` → `Amount`; otherwise `Unknown`.

**The `Unknown` arm is not decoration.** `rust_decimal::Decimal` tops out near 7.9 × 10²⁸ while a uint256 allowance can reach 1.2 × 10⁷⁷. Realistic allowances fit comfortably — a trillion USDC in base units is 10¹⁸ — and unlimited approvals arrive as the sentinel rather than a number, so the gap is narrow. But a wallet holding a large-but-not-max approval would otherwise fail deserialization of the entire response. `Unknown` turns that into one unparsed field, and it matches the five existing `#[serde(other)] Unknown` catch-alls in this crate.

`rust_decimal` is already a workspace dependency (used by `polyoxide-clob`); this adds `rust_decimal = { workspace = true }` to `polyoxide-data/Cargo.toml`.

## Component 4 — the `approvals()` namespace

**Files:** `polyoxide-data/src/api/approvals.rs` (new), `api/mod.rs`, `client.rs`, `types.rs`

```rust
data.approvals().get(address).await?
```

Upstream tags `/v1/approvals` as `Misc`, and `misc.rs` is where `/other` and `/revisions` live. A dedicated namespace is chosen anyway for discoverability and room to grow, at the cost of a new module and a client accessor.

Types:

```rust
pub struct ApprovalsResponse {
    pub address: String,
    pub chain_id: u64,
    pub checked_at: String,
    pub contracts: Vec<ApprovalContract>,
}

pub struct ApprovalContract {
    pub id: String,
    pub feature: ApprovalFeature,
    pub token: String,
    pub spender: String,
    pub standard: ApprovalStandard,
    pub amount: Option<Allowance>,
    pub approved: bool,
}
```

`ApprovalFeature` is `trading` / `perps` / `rewards` / `auto-redeem` — **kebab-case and lowercase**, unlike `ActivityType`'s `UPPERCASE`, so it needs explicit `#[serde(rename = "auto-redeem")]` rather than a blanket `rename_all`. `ApprovalStandard` is `ERC20` / `ERC1155`. Both get an `#[serde(other)] Unknown` arm.

`checked_at` stays a `String`: it is RFC 3339, but the crate has no datetime dependency and upstream warns the value is event-derived rather than a fresh onchain read, so parsing it would imply a freshness guarantee it does not carry.

Every tracked token/spender pair is present regardless of wallet state: upstream states the array holds "every approval Polymarket tracks, in a stable display order" and that pairs the wallet has never approved "are still present with `approved` false, so the array length does not change with wallet state". It does **not** publish how many entries that is, so nothing here may assume a count. `approved: false` is therefore the normal representation of "never approved", not an absence — and tests must not assert a fixed `contracts.len()`.

## Component 5 — vendored spec sync

```bash
curl -fsSL https://docs.polymarket.com/api-spec/data-openapi.yaml -o docs/specs/data/openapi.yaml
```

This is what closes issue #22. The nightly will confirm on its next run.

## Failure handling

| Condition | Behavior |
|---|---|
| `Position` omits both fee fields | Both `None`. Existing fields unaffected. |
| `entryFeesUsdc` is `0` | `Some(0.0)` — a measured zero, distinct from `None`. |
| `amount` absent (ERC1155) | `None`. |
| `amount` is an unrecognized string | `Allowance::Unknown(String)`, response still deserializes. |
| `amount` exceeds `Decimal` range | `Allowance::Unknown(String)`. |
| `feature` / `standard` unrecognized | `Unknown` variant, response still deserializes. |

Nothing upstream can add to these enums or fields breaks deserialization of a whole response. That is the crate's existing bar, and this meets it.

## Testing

Mock tests in `polyoxide-data/tests/mock_api.rs`, following the established `Matcher::UrlEncoded` style:

| Test | Pins |
|---|---|
| `activity_excludes_deposits_withdrawals_param` | `excludeDepositsWithdrawals=false` is actually sent when set. |
| `activity_omits_param_when_unset` | No `excludeDepositsWithdrawals` key when the builder method is not called — proves the fix is additive. |
| `position_deserializes_with_fee_fields` | `Some` values, and `Some(0.0)` for an explicit zero. |
| `position_deserializes_without_fee_fields` | Both `None`; no regression for older payloads. |
| `approvals_returns_contracts` | Full response shape, `ERC20` with `amount`, `ERC1155` without. |
| `allowance_parses_max_amount_and_unknown` | All three arms, including a value beyond `Decimal` range. |
| `approval_enums_tolerate_unknown_variants` | An invented `feature` and `standard` deserialize rather than error. |

A `#[ignore]` live test in `polyoxide-data/tests/live_api.rs` calls `/v1/approvals` against a real address, following the crate's existing live-test convention.

## Version

Two breaking changes — `#[non_exhaustive]` on `Position` and its new public fields — take the workspace from 0.26.1 to **0.27.0**.

## Documentation

`CLAUDE.md`'s Data namespace list gains `data.approvals()`. The `ActivityType` doc comment on `Deposit` and `Withdrawal` should note that they require `exclude_deposits_withdrawals(false)`, since that is where someone hits the problem.

## Out of scope

The other four open drift issues (`perps`, `perps-ws`, `bridge`, `combos-rfq-ws`) are mirror-only syncs for APIs polyoxide does not implement — each is a `curl` and a commit, with no code to write. They are deliberately not bundled here.
