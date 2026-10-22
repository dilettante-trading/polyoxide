---
paths:
  - "polyoxide-polymarket/src/clob/account/**"
  - "polyoxide-polymarket/src/clob/core/eip712.rs"
  - "polyoxide-polymarket/src/clob/{client,mod}.rs"
  - "polyoxide-polymarket/src/clob/api/account.rs"
  - "polyoxide-polymarket/src/relay/{account,client,deposit_wallet,session_signers,wallet}.rs"
  - "polyoxide-polymarket/src/shared/session_signer.rs"
  - "polyoxide-polymarket/tests/fixtures/{clob,relay}/session_keys/**"
  - "polyoxide-polymarket/tests/live_clob_session_keys.rs"
  - "scripts/capture_polymarket_session_keys.py"
  - "docs/specs/session-keys/**"
---

# Deposit Wallets and session keys

- **Order signature type 3** is a Deposit Wallet signing an ERC-7739 `TypedDataSign` envelope: the
  exchange domain is the signing domain, and the wallet's `DepositWallet`/`1` domain rides inside
  the message. Getting that orientation backwards produces a different digest, so the signature
  cannot verify for the wallet. `v1_exchange.envelope_digest` in
  `polyoxide-polymarket/tests/fixtures/clob/session_keys/order_vectors.json` pins py-sdk's golden
  digest, and `polyoxide-polymarket/src/clob/core/eip712.rs` checks it. A session key adds a 6492
  envelope on top.
- The role (owner or session key) is never inferred from addresses:
  `SigningTarget::DepositWallet { wallet, role }` carries it.
- **No mirror.** The published OpenAPI omits almost all of it (the relay mirror documents only
  `GET /deployed?type=WALLET`). `docs/specs/session-keys/` holds the contract (`README.md`) and the
  SDK behaviours the pages omit (`OBSERVED.md`): signature type 3,
  `GET /v1/user/session-signers`, and the relayer's `type: WALLET` dialect with
  `/v1/session-signers/*` and `/v1/account/transactions/*`.
- The sources are the prose pages plus `github.com/Polymarket/py-sdk` and
  `github.com/Polymarket/ts-sdk` (0.11.0); where they disagree, the SDKs win. Per-route auth and
  timeouts live in py-sdk's `_require_*` guards and `httpx.Timeout` values, not in its payload
  builders (a plan got both wrong until a reviewer read `_internal/actions/session_keys.py`).
- `scripts/capture_polymarket_session_keys.py` regenerates the fixtures in
  `polyoxide-polymarket/tests/fixtures/{clob,relay}/session_keys/` from py-sdk; every signing,
  encoding and wire-body test is pinned to them, never to a self-computed value.
- Implemented by the `clob` module (`SigningTarget`, `Account::{with_signer,l2_only}`,
  `*_with_signature`, `list_session_signers`) and the `relay` module
  (`WalletType::DepositWallet`, `deposit_wallet`, `session_signers`, `resolve_wallet`,
  `with_auth`).
- The live round trip is `polyoxide-polymarket/tests/live_clob_session_keys.rs` (features `clob`,
  `relay` and `gamma`), `#[ignore]`d and gated on a Deposit Wallet fixture account that does not
  exist until prader-rs #125 is resolved.
