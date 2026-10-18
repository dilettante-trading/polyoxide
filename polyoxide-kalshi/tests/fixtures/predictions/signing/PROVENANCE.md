# Provenance

`vectors.json` is made by `scripts/capture_kalshi_account.py signing`. Its keys are throwaway ones that exist only for this fixture: no Kalshi account has ever held them. Every signature is over `1700000000000GET/trade-api/v2/portfolio/balance`.

The file never holds an armour line, so no push protection can refuse it. Each `*_body` is the base64 lines of a PEM, joined by newlines. The Rust suite puts the armour back before it reads a key, with the label `PRIVATE KEY` around a `pkcs8_body` and `RSA PRIVATE KEY` around the `pkcs1_body`.

| File | What | Generated |
|------|------|-----------|
| vectors.json | throwaway Ed25519 and RSA-2048 keys, and one signature by each | 2026-10-18T14:46:15Z |

Check it with `uv run scripts/capture_kalshi_account.py signing --check polyoxide-kalshi/tests/fixtures/predictions/signing`.
