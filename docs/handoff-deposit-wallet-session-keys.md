# Handoff: Deposit Wallet + Session Key support (for prader's non-custodial design)

Written 2026-09-13 from the prader `aidanb/custodial` brainstorm. Every claim below was
checked against polyoxide 0.31.0 source and the raw Mintlify markdown of the venue docs
(`docs.polymarket.com/trading/{session-keys,deposit-wallets,place-orders,manage-orders}.md`).
The consumer is prader; the shape prader needs is in "Interface prader consumes" at the end.

## Status (2026-09-25)

Every code item below is implemented on polyoxide `main` (plan 1 `02f011d`, plan 2
`5932426`; docs plan 3 on the branch) and pinned offline to fixtures generated from
`py-sdk` 0.11.0. The contract record is `docs/specs/session-keys/README.md`; the
design and its two breaking-changes lists are
`docs/superpowers/specs/2026-09-25-session-keys-offline-design.md`. Nothing has run
against the live host: item 12's round trip exists as an `#[ignore]`d test and waits
on the business prerequisite (prader-rs #125). Each item carries a *Status* line.

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
  The same EOA-bound owner key also places and cancels owner-signed type-3 orders:
  `ts-sdk`'s live suite (`packages/client/tests/integration/orders.test.ts`) does exactly
  that with `secureClientWithDepositWallet`, and its L1 header is `POLY_ADDRESS: account.signer`
  (`clients.ts`). Resolved 2026-09-25; the earlier "probe item" citing SDK issues
  #64/#65/#70/#71/#77 predates the official `ts-sdk`/`py-sdk` and is moot.

## polyoxide-clob

1. **Signature type 3 signing.** `core/eip712.rs:88` and `client.rs:241,322` reject
   `SignatureType::Poly1271`. Replace with the real flow:
   - Order fields: `maker` = `signer` = deposit wallet address, `signatureType` = 3.
     Today `build_order_v2` sets `signer` = `account.address()`; for type 3 it must be the
     deposit wallet, not the signing EOA.
   - Sign the ERC-7739 `TypedDataSign` envelope, NOT the bare `Order`:
     domain `{name:"Polymarket CTF Exchange", version:"2", chainId:137, verifyingContract:<exchange>}`
     (the *app* domain — the same one a plain V2 order signs under),
     types `Order` (11 fields, V2) + `TypedDataSign{contents:Order, name, version, chainId,
     verifyingContract, salt}`, message `{contents:<order>, name:"DepositWallet",
     version:"1", chainId:137, verifyingContract:<deposit_wallet>, salt:bytes32(0)}`
     (the *account* domain rides inside the message, per ERC-7739).
     **Corrected 2026-09-25**: an earlier revision had the two domains swapped. The
     orientation above matches `session-keys.md`, `py-sdk`
     (`src/polymarket/_internal/actions/orders/typed_data.py`) and `ts-sdk`; py-sdk pins
     the digest for a fixed type-3 fixture in `tests/unit/test_order_typed_data_golden.py`
     (`0x1b9566eedd9589a73275df23a3a9d9e2e9897e76d31cd46d436f1b824d161b33`) — pin
     polyoxide against it before any live test.
   - Wrap per `wrapDepositWalletSignature` (place-orders.md):
     `innerSig ‖ appDomainSeparator(exchange domain) ‖ contentsHash(Order struct hash) ‖
     bytes(ORDER_TYPE string) ‖ uint16(len(ORDER_TYPE))`.
     ORDER_TYPE = `Order(uint256 salt,address maker,address signer,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,uint256 timestamp,bytes32 metadata,bytes32 builder)`.

   *Status:* Done, plan 1. `SigningTarget::DepositWallet { wallet, role }` on `Account`;
   `Clob::create_order` sets maker = signer = wallet and type 3; `sign_order` produces the
   7739 wrap, pinned to py-sdk's digest and full bytes
   (`polyoxide-clob/tests/fixtures/session_keys/order_vectors.json`).
2. **Session-signer envelope** (only when the signer is a session key, not the owner):
   `abi.encode(bytes32 leftPad(session_signer_address), bytes32(0), bytes wrapped_sig)`
   ‖ `0x6492649264926492649264926492649264926492649264926492649264926492`.
   Request headers: `POLY_ADDRESS: <session_signer_address>`; body `owner` = the session
   signer's CLOB api key.

   *Status:* Done, plan 1. Applied when `role == DepositWalletRole::SessionKey`; bytes
   pinned to py-sdk. The relay carries an identical copy
   (`polyoxide_relay::deposit_wallet::wrap_session_signer`) for batches.
3. **Account without a wallet key.** `Account::new` requires a private key
   (`account/mod.rs:108`) but `post_order`, `orders()`, `account_api()`,
   `notifications()` only use `address()`, `credentials()`, `signer()` (the HMAC signer).
   Add an L2-only account (address + `Credentials`) so a holder of the owner's triplet can
   read and cancel without a key. `Account` is also missing a "signing target" notion:
   for a session key the signing EOA ≠ the maker/signer address. Model it explicitly
   (e.g. `SigningContext { eoa, deposit_wallet: Option<Address> }`) rather than threading
   `funder` through every call.

   *Status:* Done, plan 1. `Account::l2_only(address, credentials)` and
   `Account::with_signer(any alloy signer, credentials)`; the "signing target" is
   `SigningTarget`, set with `Account::with_target`.
4. **L1 auth for external signers.** `sign_clob_auth<S: AlloySigner>` is fine for a
   local key (the session key on prader's server). The OWNER key in prader's design lives in
   an external wallet over WalletConnect, so add:
   - `clob_auth_typed_data(address, timestamp, nonce) -> serde_json::Value` (EIP-712 JSON
     for `eth_signTypedData_v4`), and
   - `create_api_key_with_signature(address, timestamp, nonce, signature)` /
     `derive_api_key_with_signature(...)` that take the signature instead of a signer.
   No 7739-wrapped `ClobAuth` is needed: neither official SDK wraps it, and the EOA-bound
   owner key already covers deposit-wallet orders (see "Venue facts"). Dropped 2026-09-25.

   *Status:* Done, plan 1. `clob_auth_typed_data`, `Clob::create_api_key_with_signature`,
   `Clob::derive_api_key_with_signature`; the signer is recovered locally and checked
   against `address` before any request. No 7739-wrapped `ClobAuth`, as this item already
   concluded.
5. **`GET /v1/user/session-signers`** on the CLOB host, owner L2 auth: list session
   keys with scopes and expiry. Not in 0.31.0 (`grep session-signers` is empty).

   *Status:* Done, plan 1. `AccountApi::list_session_signers() -> SessionSigners {
   wallet, signers }`; errors if the account's Deposit Wallet target disagrees with
   `wallet`. Answering under a session key's own credentials is an open item
   (`docs/specs/session-keys/OBSERVED.md`).
6. **Balance/allowance with `signature_type=3`.** `api/account.rs` passes the enum as
   `u8`, so it already sends 3; the doc comment lists only 0/1/2. Probe what the venue
   returns for a session signer's key vs the owner's key, then document.

   *Status:* Done, plan 1 (doc comments corrected; `signature_type` defaults from the
   account's target). The owner-vs-session probe is an open item until the live round
   trip runs.

## polyoxide-relay

7. **`WalletType::DepositWallet`.** `types.rs:6` has only `Safe`/`Proxy`. The venue enum
   is `EOA=0, POLY_PROXY=1, GNOSIS_SAFE=2, DEPOSIT_WALLET=3`.

   *Status:* Done, plan 2. `WalletType::DepositWallet` (wire `"WALLET"`); the enum is
   now `#[non_exhaustive]`.
8. **Deposit Wallet `Batch` typed data.** Domain `{name:"DepositWallet", version:"1",
   chainId:137, verifyingContract:<deposit_wallet>}`; types `Call{target,value,data}`,
   `Batch{wallet,nonce,deadline,calls:Call[]}`. Nonce from
   `GET /v1/account/transactions/params?address=<owner>&type=WALLET` (Builder or Relayer
   API key headers). `deadline` must leave ≥ 10 s of validity.

   *Status:* Done, plan 2. `polyoxide_relay::deposit_wallet::{batch_typed_data,
   batch_digest}`, `RelayClient::get_execute_params(signer, WalletType::DepositWallet)`;
   seven signed batches pinned to py-sdk
   (`polyoxide-relay/tests/fixtures/session_keys/relay_vectors.json`). The nonce is
   queried for the EOA that signs, owner or session key.
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

   *Status:* Done, plan 2. `authorize_session_signer[_typed_data]` /
   `submit_session_signer_authorization`, `revoke_session_signer[_typed_data]` /
   `submit_session_signer_revocation`; bodies pinned to py-sdk's builders. Correction:
   revocation also accepts a Relayer API key, and both routes use a 300 s request timeout
   (`docs/specs/session-keys/OBSERVED.md` rows 2–3).
10. **Builder-only relay client.** `BuilderAccount` (`account.rs:18`) is
    `signer: PrivateKeySigner + config`. The session-signer endpoints need Builder HMAC
    headers with NO wallet key; the owner signature arrives from outside. Add a client mode
    that carries `BuilderConfig` only.

    *Status:* Done, plan 2. `RelayClientBuilder::with_auth(AuthConfig)` builds a client
    with auth and no key; `BuilderAccount::with_signer` takes any alloy signer with
    `sign_hash` (so `BuilderAccount::signer()` now returns `&DynSigner`, a breaking
    change listed in the spec).
11. **Typed-data-out / signature-in** for every owner-signed batch (authorize, revoke,
    approvals, redemption): return the EIP-712 JSON, accept the signature. Redemption is
    not in any session scope (scopes are trading venues), so prader will redeem with the
    owner's external wallet through this path.

    *Status:* Done, plan 2 (redemption target corrected in plan 3).
    `deposit_wallet_batch_typed_data` / `submit_deposit_wallet_batch_from` for any batch;
    `redeem_typed_data(…, neg_risk, …)` / `submit_redemption_with_signature` and
    `submit_deposit_wallet_redemption` for redemption through the collateral adapter or
    the neg-risk collateral adapter with pUSD, as py-sdk does;
    `deposit_wallet_trading_approvals` for the 17 approvals py-sdk requires (the page's
    four are a subset).

## Cross-cutting

12. **Live tests.** `polyoxide-clob/tests/live_api.rs` and `polyoxide-relay/tests/live_api.rs`
    exist; add a Deposit Wallet fixture account (fresh Polymarket account, funded a few USDC)
    and a session-key round trip: authorize → derive session creds → place GTC → list from
    session key → list from owner key (records the visibility answer) → cancel → revoke.

    *Status:* Skeleton only, plan 3. `polyoxide-clob/tests/live_session_keys.rs` runs the
    round trip as an `#[ignore]`d test gated on `POLYMARKET_DW_*` and `BUILDER_*` env
    vars; it panics with the nightly's auth-gated wording when they are unset. No fixture
    account exists yet (prader-rs #125).
13. **Release** as the next minor (0.33.0; 0.32.x shipped without this work) to
    crates.io. prader consumes crates.io only (path deps break prader's CI signal per
    `docs/claude/polyoxide-upgrades.md`).

    *Status:* Pending. Plans 1–3 are on `main`, unpushed; 0.33.0 follows plan 3 with the
    breaking changes listed at the end of the design spec.

## Amendments 2026-09-25 (prader's ADR-0021 accepted; raw docs re-read)

14. **Two Deposit Wallet generations.** Wallets deployed before 2026-06-29 are UUPS
    proxies (factory `0x00000000000Fb5C9ADea0298D729A0CB3823Cc07`, implementation
    `0x58CA52ebe0DadfdF531Cde7062e76746de4Db1eB`); later ones are ERC-1967 beacon proxies
    (same factory, beacon `0x7A18EDfe055488A3128f01F563e5B479D92ffc3a`). The docs publish
    only the beacon recipe (`trading/wallets-auth` → "Derive a Deposit Wallet Address");
    the UUPS recipe is in `@polymarket/client` 0.11.0's bundle (`function Gn`, next to
    `$n` for beacon, `jc` for Safe; constants under `walletDerivation`). Expose
    `resolve_wallet(owner, &RelayClient) -> WalletKind { DepositWallet | Safe | Proxy |
    None }`: derive all four, ask `GET /deployed?address=&type=WALLET` (no auth) per
    Deposit Wallet candidate, error on two deployed. Keep the pure derivations `pub` for
    fixture tests.

    *Status:* Done, plan 2 (Proxy added in plan 3). Pure derivations
    `derive_{safe,proxy,deposit_wallet_uups,deposit_wallet_beacon}` (pinned to py-sdk's
    own derivation tests) and `RelayClient::resolve_wallet(owner) -> Result<Option<WalletKind>>`
    over beacon, UUPS, Safe and Proxy (`type=PROXY`, as py-sdk sends; the published spec
    lists only `SAFE`/`WALLET`). Observed 2026-09-25: the relayer answers every non-`WALLET`
    type alike, so the probe works. `None` means nothing deployed; two deployed is an
    error.
15. **Relayer auth is an enum.** `/submit` and the transaction poll accept Builder HMAC
    or the user's Relayer API key (`RELAYER_API_KEY` + `RELAYER_API_KEY_ADDRESS`, from
    polymarket.com → Settings → API Keys). Model `RelayerAuth::Builder(triplet) |
    UserKey { key, address }`; the session-signer endpoints take Builder only. Note the
    published relayer OpenAPI does not list `/v1/session-signers/*` or
    `/v1/account/transactions/params`; the docs page is their only contract.

    *Status:* Done, plan 2. `AuthConfig::{Builder, RelayerApiKey}` already existed;
    `with_auth` accepts either without an account. Correction: only *authorization* is
    Builder-only; revocation takes either, as py-sdk does.
16. **Transaction state is public.** Expose the poll's `STATE_NEW | STATE_SUBMITTED |
    STATE_CONFIRMED | STATE_FAILED | STATE_INVALID` as an enum prader can persist and
    resume on: the venue allows five minutes for an authorization to broadcast and runs a
    revocation's cancel-all asynchronously afterwards, so prader's completion is a job,
    not a request.

    *Status:* Done, plan 2. `TransactionState` with `is_terminal` / `is_success` and
    `GaslessTransaction` from `GET /v1/account/transactions/{id}`. Correction: py-sdk's
    list is `NEW | EXECUTED | MINED | CONFIRMED | INVALID | FAILED` (no `SUBMITTED`);
    unknown values land in `Other(String)`.

## Interface prader consumes (the contract this design cites)

- `clob`: build unsigned V2 order for a deposit wallet (maker=signer=DW, type 3);
  sign with a local session key including 7739 wrap + session envelope; post with the
  session signer's L2 creds; L2-only account for owner-triplet reads/cancels;
  `clob_auth_typed_data` + `derive_api_key_with_signature`; `list_session_signers`.
- `relay`: `authorize_session_signer_typed_data(dw, session_addr, scopes, nonce, deadline)`
  (`valid_until` is computed; a `_with_valid_until` variant takes it),
  `submit_session_signer_authorization(request, signature, idempotency_key)` under Builder
  HMAC auth, the revocation pair, and `redeem_typed_data` / `submit_with_signature` for
  deposit wallets.
