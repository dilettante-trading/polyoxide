---
paths:
  - "polyoxide-polymarket/src/relay/**"
  - "polyoxide-polymarket/tests/*relay*.rs"
  - "polyoxide-polymarket/tests/fixtures/relay/**"
  - "polyoxide-polymarket/examples/redeem.rs"
  - "docs/specs/relay/**"
---

# Polymarket relay (gasless transactions)

- Built with `RelayClient::default_builder()`; default concurrency 2. Its own setters (`url`,
  `chain_id`, account, auth and wallet) are in `clients.md`: `url(&str) -> Result` checks the URL
  at once, and `build` keeps a path prefix.
- Its requests go through one private helper on the send loop, which signs with `RelayGetAuth` or
  `RelayPostAuth` and runs the auth checks and allow-lists before sending (`send-loop.md`). Its
  error is `RelayError::Api(ApiError)` (DRIFT R7): a refusal before sending is `Api(Validation)`
  through `RelayError::validation` (`errors.md`).
- Credentials: `BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE` (HMAC auth) **or**
  `RELAYER_API_KEY`, `RELAYER_API_KEY_ADDRESS` (static key auth); `RELAYER_URL` and `CHAIN_ID`
  optionally. With `keychain`, `BuilderAccount::from_keychain()` loads the `polyoxide-relay`
  service.
- Relay's credentialed live tests fail as `auth-gated` without their secrets, like clob's.
- Deposit Wallets and session signers (`WalletType::DepositWallet`, `deposit_wallet`,
  `session_signers`, `resolve_wallet`, `with_auth`): `session-keys.md`.
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
