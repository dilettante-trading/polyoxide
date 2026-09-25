# Deposit Wallet (session-key) order-signing vectors

Captured 2026-09-25 by `scripts/capture_session_key_vectors.py` (run:
`uv run scripts/capture_session_key_vectors.py polyoxide-clob/tests/fixtures/session_keys`)
against Polymarket's official Python SDK, `polymarket-client==0.11.0` — not a live
host.

The order is py-sdk's own golden fixture (`tests/unit/test_order_typed_data_golden.py`),
signed with Anvil key #0. Generation asserts the `v1_exchange` vector's `envelope_digest`
against py-sdk's pinned value before writing anything, so this fixture cannot drift from
theirs silently.

`session_signature` is a byte-pinning vector only, not a signature a real session key
would produce: the ERC-7739 envelope names Anvil address #1 as the session signer, but
Anvil key #0 produced the inner order signature. The two identities are deliberately
mismatched so the fixture can pin bytes without coordinating two live signers.

| Fixture | Command |
|---|---|
| `order_vectors.json` | `uv run scripts/capture_session_key_vectors.py polyoxide-clob/tests/fixtures/session_keys` |
