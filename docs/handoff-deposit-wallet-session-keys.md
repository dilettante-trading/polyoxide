# Handoff: Deposit Wallet + Session Key support (for prader's non-custodial design)

Written 2026-09-13 from the prader `aidanb/custodial` brainstorm. Every claim below was
checked against polyoxide 0.31.0 source and the raw Mintlify markdown of the venue docs
(`docs.polymarket.com/trading/{session-keys,deposit-wallets,place-orders,manage-orders}.md`).
The consumer is prader; the shape prader needs is in "Interface prader consumes" at the end.

## Business prerequisite (not code)

The venue gates session-key management per Builder API key: *"During the initial rollout,
contact builder@polymarket.com to authorize your Builder API key for session-key
management."* Nothing below can be live-tested until DilettanteTrading's Builder key is
enabled. Do this first, in parallel with the code.

## Venue facts the work rests on

- Deposit Wallet = default for accounts created on/after 2026-05-04. Legacy Safe/Proxy
  accounts cannot use session keys; migration is "planned", not shipped.
- Session key = a fresh EOA the owner authorizes on the Deposit Wallet contract via
  `authorizeSessionSigner(address sessionSigner, uint256 validUntil)`. `validUntil` must be
  now + 180 days (the docs' example uses `4_315 * 60 * 60` seconds; "other values are
  rejected" — verify the exact tolerance live). Scopes: `"CLOB"`, `"COMBOSRFQ"`, or
  `["ALL"]`. Cannot withdraw. Revocation cancels its open orders.
- Visibility is scoped by submitting key in BOTH directions: a session key lists only its
  own orders/trades; *"A Deposit Wallet Owner cannot fetch orders submitted by its
  authorized Session Keys."* (session-keys.md §Considerations; repeated in manage-orders.md).
- The owner's L1 auth for session-signer management uses `POLY_ADDRESS: <deposit_wallet_owner_address>`
  with the plain `ClobAuth` typed data — i.e. an EOA-bound key, no ERC-7739 wrapping.
  Whether that EOA-bound owner key can list the deposit wallet's open orders is UNVERIFIED
  (open SDK issues #64/#65/#70/#71/#77 say owner-signed *orders* need a deposit-wallet-bound
  key, which the official clients cannot create). Probe item.

## polyoxide-clob

1. **Signature type 3 signing.** `core/eip712.rs:88` and `client.rs:241,322` reject
   `SignatureType::Poly1271`. Replace with the real flow:
   - Order fields: `maker` = `signer` = deposit wallet address, `signatureType` = 3.
     Today `build_order_v2` sets `signer` = `account.address()`; for type 3 it must be the
     deposit wallet, not the signing EOA.
   - Sign the ERC-7739 `TypedDataSign` envelope, NOT the bare `Order`:
     domain `{name:"DepositWallet", version:"1", chainId:137, verifyingContract:<deposit_wallet>}`,
     types `Order` (11 fields, V2) + `TypedDataSign{contents:Order, name, version, chainId,
     verifyingContract, salt}`, message `{contents:<order>, name:"Polymarket CTF Exchange",
     version:"2", chainId:137, verifyingContract:<exchange>, salt:bytes32(0)}`.
   - Wrap per `wrapDepositWalletSignature` (place-orders.md):
     `innerSig ‖ appDomainSeparator(exchange domain) ‖ contentsHash(Order struct hash) ‖
     bytes(ORDER_TYPE string) ‖ uint16(len(ORDER_TYPE))`.
     ORDER_TYPE = `Order(uint256 salt,address maker,address signer,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,uint256 timestamp,bytes32 metadata,bytes32 builder)`.
2. **Session-signer envelope** (only when the signer is a session key, not the owner):
   `abi.encode(bytes32 leftPad(session_signer_address), bytes32(0), bytes wrapped_sig)`
   ‖ `0x6492649264926492649264926492649264926492649264926492649264926492`.
   Request headers: `POLY_ADDRESS: <session_signer_address>`; body `owner` = the session
   signer's CLOB api key.
3. **Account without a wallet key.** `Account::new` requires a private key
   (`account/mod.rs:108`) but `post_order`, `orders()`, `account_api()`,
   `notifications()` only use `address()`, `credentials()`, `signer()` (the HMAC signer).
   Add an L2-only account (address + `Credentials`) so a holder of the owner's triplet can
   read and cancel without a key. `Account` is also missing a "signing target" notion:
   for a session key the signing EOA ≠ the maker/signer address. Model it explicitly
   (e.g. `SigningContext { eoa, deposit_wallet: Option<Address> }`) rather than threading
   `funder` through every call.
4. **L1 auth for external signers.** `sign_clob_auth<S: AlloySigner>` is fine for a
   local key (the session key on prader's server). The OWNER key in prader's design lives in
   an external wallet over WalletConnect, so add:
   - `clob_auth_typed_data(address, timestamp, nonce) -> serde_json::Value` (EIP-712 JSON
     for `eth_signTypedData_v4`), and
   - `create_api_key_with_signature(address, timestamp, nonce, signature)` /
     `derive_api_key_with_signature(...)` that take the signature instead of a signer.
   Optional, gated on the probe: a 7739-wrapped `ClobAuth` for a deposit-wallet-bound key
   (what the official SDKs fail to do). Only needed if the EOA-bound owner key cannot list
   deposit-wallet orders.
5. **`GET /v1/user/session-signers`** on the CLOB host, owner L2 auth: list session
   keys with scopes and expiry. Not in 0.31.0 (`grep session-signers` is empty).
6. **Balance/allowance with `signature_type=3`.** `api/account.rs` passes the enum as
   `u8`, so it already sends 3; the doc comment lists only 0/1/2. Probe what the venue
   returns for a session signer's key vs the owner's key, then document.

## polyoxide-relay

7. **`WalletType::DepositWallet`.** `types.rs:6` has only `Safe`/`Proxy`. The venue enum
   is `EOA=0, POLY_PROXY=1, GNOSIS_SAFE=2, DEPOSIT_WALLET=3`.
8. **Deposit Wallet `Batch` typed data.** Domain `{name:"DepositWallet", version:"1",
   chainId:137, verifyingContract:<deposit_wallet>}`; types `Call{target,value,data}`,
   `Batch{wallet,nonce,deadline,calls:Call[]}`. Nonce from
   `GET /v1/account/transactions/params?address=<owner>&type=WALLET` (Builder or Relayer
   API key headers). `deadline` must leave ≥ 10 s of validity.
9. **Session-signer endpoints** (base `https://relayer-v2.polymarket.com`, already the
   default in `client.rs:1044`):
   - `POST /v1/session-signers/authorizations` body
     `{walletAddress, sessionSignerAddress, scopes, validUntil, nonce, deadline, signature}`,
     headers `POLY_BUILDER_API_KEY / POLY_BUILDER_TIMESTAMP / POLY_BUILDER_PASSPHRASE /
     POLY_BUILDER_SIGNATURE` (HMAC-SHA256 over `timestamp + "POST" + path + body`, urlsafe
     base64 with padding) + `Idempotency-Key`. Response
     `{operationId, status, transactionHash, transactionId}`; poll the transaction.
   - `POST /v1/session-signers/revocations` with the `revokeSessionSigner(address)` calldata
     in the same Batch shape.
10. **Builder-only relay client.** `BuilderAccount` (`account.rs:18`) is
    `signer: PrivateKeySigner + config`. The session-signer endpoints need Builder HMAC
    headers with NO wallet key; the owner signature arrives from outside. Add a client mode
    that carries `BuilderConfig` only.
11. **Typed-data-out / signature-in** for every owner-signed batch (authorize, revoke,
    approvals, redemption): return the EIP-712 JSON, accept the signature. Redemption is
    not in any session scope (scopes are trading venues), so prader will redeem with the
    owner's external wallet through this path.

## Cross-cutting

12. **Live tests.** `polyoxide-clob/tests/live_api.rs` and `polyoxide-relay/tests/live_api.rs`
    exist; add a Deposit Wallet fixture account (fresh Polymarket account, funded a few USDC)
    and a session-key round trip: authorize → derive session creds → place GTC → list from
    session key → list from owner key (records the visibility answer) → cancel → revoke.
13. **Release** as 0.32.0 to crates.io. prader consumes crates.io only (path deps break
    prader's CI signal per `docs/claude/polyoxide-upgrades.md`).

## Interface prader consumes (the contract this design will cite)

- `clob`: build unsigned V2 order for a deposit wallet (maker=signer=DW, type 3);
  sign with a local session key including 7739 wrap + session envelope; post with the
  session signer's L2 creds; L2-only account for owner-triplet reads/cancels;
  `clob_auth_typed_data` + `derive_api_key_with_signature`; `list_session_signers`.
- `relay`: `authorize_session_signer_typed_data(dw, session_addr, scopes, valid_until, nonce, deadline)`,
  `submit_session_signer_authorization(body, signature)` under Builder HMAC auth, the
  revocation pair, and `redeem_typed_data` / `submit_with_signature` for deposit wallets.
