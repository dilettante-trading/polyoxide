# Data API Combos

Base URL: `https://data-api.polymarket.com`

Combinatorial (multi-market) positions and their lifecycle activity. A combo row on `/activity` (where `isCombo` is true) carries a `conditionId` that equals the combo's `combo_condition_id`; pass it as `market_id` here to fetch the combo's legs and detail.

## Get Combo Positions

`GET /v1/positions/combos`

Returns combinatorial (multi-market) positions held by a user, with per-leg breakdown. Also available at `/v1/data/user/{address}/positions/combos` (address taken from the path).

Open positions with `shares_balance` below 0.001 are omitted (dust floor — e.g. sub-0.001 remainders left by "sell all" cashouts); resolved positions are served regardless of balance.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| user | query | Address (`0x` + 40 hex) | yes | — | User wallet address |
| status | query | string[] (CSV, `explode=false`) | no | — | One or more of OPEN, PARTIAL, RESOLVED_PARTIAL, RESOLVED_WIN, RESOLVED_LOSS. Case-insensitive; any invalid member is a 400. Omit for the default listing (open plus resolved-with-recorded-resolution) |
| sort | query | string | no | current_value_desc | current_value_desc, first_entry_desc, entry_cost_desc, resolved_at_desc, updated_asc |
| market_id | query | ComboConditionId[] (`0x` + 62 hex) | no | — | Filter by combo condition ID(s); equal the `market_id` of `isCombo` rows on `/activity` |
| limit | query | integer (0-1000) | no | 20 | Results per page |
| offset | query | integer (0-100000) | no | 0 | Pagination offset. Ignored when `cursor` is set |
| updatedAfter | query | integer (epoch seconds) | no | — | Incremental-sync watermark, inclusive: only rows whose `updated_at` is at or after this time. Catches resolution/redemption changes a creation-time filter cannot. Pair with `sort=updated_asc` |
| updatedBefore | query | integer (epoch seconds) | no | — | Optional upper bound for `updated_at`, inclusive; clamped to the safety lag. Must be ≥ `updatedAfter` |
| cursor | query | string | no | — | Opaque continuation token from a previous response's `pagination.next_cursor`. Supersedes `offset`. Invalid, tampered, or cross-endpoint tokens return 400 |

**Response:** `CombosResponse`

```json
{
  "combos": [{
    "combo_condition_id": "string",
    "combo_position_id": "string",
    "module_id": 3,
    "user_address": "string",
    "shares_balance": "string",
    "entry_avg_price_usdc": "string",
    "entry_cost_usdc": "string",
    "realized_payout_usdc": "string",
    "total_cost_usdc": "string",
    "gross_entry_cost_usdc": "8999.997488",
    "entry_fees_usdc": "2.512000",
    "status": "OPEN",
    "first_entry_at": "string",
    "resolved_at": null,
    "updated_at": "string",
    "legs_total": 0,
    "legs_resolved": 0,
    "legs_pending": 0,
    "legs": [{
      "leg_index": 0,
      "leg_position_id": "string",
      "leg_condition_id": "string",
      "leg_outcome_index": 0,
      "leg_outcome_label": "string",
      "leg_status": "OPEN",
      "leg_resolved_at": null,
      "leg_current_price": "0",
      "market": {
        "market_id": "string",
        "slug": "string",
        "title": "string",
        "outcome": "string",
        "image_url": "string",
        "icon_url": "string",
        "category": "string",
        "subcategory": "string",
        "tags": ["string"],
        "end_date": "string",
        "event": {
          "event_id": "string",
          "event_slug": "string",
          "event_title": "string",
          "event_image": "string"
        }
      }
    }]
  }],
  "pagination": {
    "limit": 0,
    "offset": 0,
    "has_more": false,
    "next_cursor": null
  }
}
```

Note: `entry_cost_usdc` is the *remaining* cost basis and reads ~0 after a winning combo is redeemed; use `total_cost_usdc` for the original cost basis on closed positions. `realized_payout_usdc` is gross redemption proceeds (winning shares redeem 1:1 at $1), so net PnL = `realized_payout_usdc` − `total_cost_usdc`. `module_id` is `3` for the Combinatorial module.

`gross_entry_cost_usdc` and `entry_fees_usdc` are six-decimal precision-preserving strings — parse them as decimals, never through a float. Exact net basis = `gross_entry_cost_usdc` − `entry_fees_usdc`; `entry_fees_usdc` covers BUY fees only (SELL fees excluded) and is always ≤ `gross_entry_cost_usdc`.

`status` gained `RESOLVED_PARTIAL`: the combo resolved at a fractional payout (e.g. a leg voided 50/50) and redemption pays the fractional value per share. `leg_status` gained the same value.

`updated_at` (UTC, ISO 8601) is the incremental-sync watermark — it bumps on any recompute of the row (trade, redemption, resolution classification). It is omitted on responses served by the legacy backend. For sync, page with `updatedAfter` + `sort=updated_asc` + `cursor`.

## Get Combo Activity

`GET /v1/activity/combos`

Returns combo lifecycle and redeem events (split, merge, convert, compress, wrap, unwrap, redeem) for a user, with per-leg breakdown. This is the combo counterpart to `/activity` trade rows. Also available at `/v1/data/user/{address}/activity/combos` (address taken from the path).

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| user | query | Address (`0x` + 40 hex) | yes | — | User wallet address |
| market_id | query | ComboConditionId[] (`0x` + 62 hex) | no | — | Filter by combo condition ID(s); equal the `market_id` of `isCombo` rows on `/activity` |
| limit | query | integer (0-500) | no | 50 | Results per page |
| offset | query | integer (0-10000) | no | 0 | Pagination offset. Ignored when `cursor` is set |
| cursor | query | string | no | — | Opaque continuation token from a previous response's `pagination.next_cursor`. Supersedes `offset`. Invalid, tampered, or cross-endpoint tokens return 400 |

**Response:** `CombosActivityResponse`

```json
{
  "activity": [{
    "id": "string",
    "event_kind": "string",
    "side": "Split",
    "module_kind": "Combinatorial",
    "user_address": "string",
    "combo_condition_id": "string",
    "combo_position_id": "string",
    "module_id": 0,
    "amount_usdc": 0,
    "payout_usdc": null,
    "timestamp": 0,
    "tx_dttm": "string",
    "tx_hash": "string",
    "log_index": 0,
    "block_number": 0,
    "legs": [{
      "leg_index": 0,
      "leg_position_id": "string",
      "leg_condition_id": "string",
      "leg_outcome_index": 0,
      "leg_outcome_label": "string",
      "leg_status": "OPEN",
      "leg_resolved_at": null,
      "leg_current_price": "0",
      "market": {
        "market_id": "string",
        "slug": "string",
        "title": "string",
        "outcome": "string",
        "image_url": "string",
        "icon_url": "string",
        "category": "string",
        "subcategory": "string",
        "tags": ["string"],
        "end_date": "string",
        "event": {
          "event_id": "string",
          "event_slug": "string",
          "event_title": "string",
          "event_image": "string"
        }
      }
    }]
  }],
  "pagination": {
    "limit": 0,
    "offset": 0,
    "has_more": false,
    "next_cursor": null
  }
}
```

Note: `amount_usdc` is null on redeems; `payout_usdc` is null on lifecycle events.

`event_kind` (raw on-chain event, e.g. PositionsSplit) and `side` (normalized label: Split, Merge, Convert, Compress, Wrap, Unwrap, Redeem) are both **deprecated** upstream in favour of `type`. As of this snapshot the spec marks them `deprecated: true` and points at `type`, but does not yet declare `type` in the `ComboActivity` schema — treat it as optional until it appears there.

`pagination.next_cursor` is an opaque *signed* cursor. Pass it back verbatim as `?cursor=`, keeping the same sort; never parse or construct it. On cursor-enabled endpoints this makes deep pagination O(page) and stable against concurrent inserts.

## Errors

| Code | Description |
|------|-------------|
| 400 | Invalid parameters (missing required fields, out-of-range values) |
| 401 | Unauthorized |
| 500 | Internal server error |

**ErrorResponse:**

```json
{"error": "string"}
```

## Verification

```bash
# Get a user's open combo positions
curl -s 'https://data-api.polymarket.com/v1/positions/combos?user=0x...&status=OPEN&limit=1' | jq '.combos[0]'
```
