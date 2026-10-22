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
