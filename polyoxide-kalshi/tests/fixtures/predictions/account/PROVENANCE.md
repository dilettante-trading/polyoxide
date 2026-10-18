# Provenance

`limits.json` is the body of `GET /trade-api/v2/account/limits` as the demo host (`https://external-api.demo.kalshi.co`) served it, signed with the owner's demo key (KALSHI_DEMO_API_KEY_ID and KALSHI_DEMO_PRIVATE_KEY). Only its indentation was changed.

| File | Route | Host | Signed | Captured |
|------|-------|------|--------|----------|
| limits.json | GET /trade-api/v2/account/limits | the demo host | yes | 2026-10-18T14:57:03Z |

Capture it again with `uv run scripts/capture_kalshi_account.py demo polyoxide-kalshi/tests/fixtures/predictions/account`.
