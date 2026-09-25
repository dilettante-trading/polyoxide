# Deposit Wallet session keys: the offline surface

**Date:** 2026-09-25
**Branch:** `aidanb/non-custodial-keys`
**Supersedes the code items in:** `docs/handoff-deposit-wallet-session-keys.md` (items 1–11, 14–16)

## Goal

Give polyoxide everything a consumer needs to trade a Polymarket Deposit Wallet with a
scoped session key, without the SDK process ever holding the owner's key, and build all of
it against reference vectors so it can be verified before the venue enables our Builder key.

Non-custodial here means: the owner's key signs a one-time authorization and the L1 auth
message, and can do so in an external wallet. A session key, which cannot withdraw, does the
trading. The session key itself may be any `alloy` signer, so it can live in a KMS.

## Scope

In: `polyoxide-clob` and `polyoxide-relay`, plus docs. Everything runs offline and is
verified against vectors produced by Polymarket's official `py-sdk` and `ts-sdk`.

Out: Python bindings, CLI, `WALLET_CREATE` (deploying new Deposit Wallets), the live
round-trip test run, and the crates.io release. Those follow once builder@polymarket.com
enables session-key management on our Builder key.

## Decisions taken during brainstorming

| Question | Decision | Why |
|---|---|---|
| One spec or three | One spec, three ordered plans | The account model and the relay wallet types must agree |
| Where the deposit wallet address comes from | Explicit, at `Account` construction | A session key cannot be derived from anything; a wrong address fails on the first order instead of silently classifying as a session key |
| External signers | Typed-data-out / signature-in for every owner-signed payload, plus a generic `alloy` signer in both account types | Owner key is in a browser wallet; session key may be in a KMS |
| Surface | Rust crates only | The API will move after the live round trip; Python and CLI triple the rename cost |
| Relay breadth | Session-signer endpoints plus general `execute` for Deposit Wallets | Approvals and redemption sit on the critical path |
| Account shape | A `SigningTarget` on `Account`, per-call options kept as overrides | One place says how an account signs; the Deposit Wallet case needs an explicit role |

## Venue contract (from the prose pages and the official SDKs)

The published OpenAPI specs omit this entire surface: `signatureType` still enumerates
0–2, and neither `/v1/user/session-signers` nor `/v1/session-signers/*` nor
`/v1/account/transactions/params` appears. `nightly-schema.yml` cannot see it. The contract
is `docs.polymarket.com/trading/{session-keys,deposit-wallets,place-orders}.md` plus
`github.com/Polymarket/{py-sdk,ts-sdk}`.

### Order signing, signature type 3

`maker` = `signer` = the Deposit Wallet address. The signature is ERC-7739:

1. EIP-712 domain: `Polymarket CTF Exchange`, version `2`, chain 137, verifying contract =
   the exchange (standard or neg-risk, as today).
2. Primary type `TypedDataSign { contents: Order, name, version, chainId, verifyingContract, salt }`
   with `contents` = the V2 `Order`, `name = "DepositWallet"`, `version = "1"`,
   `chainId = 137`, `verifyingContract` = the Deposit Wallet, `salt = bytes32(0)`.
3. `inner = sign(digest)`.
4. `wrapped = inner ‖ appDomainSeparator ‖ contentsHash ‖ bytes(ORDER_TYPE) ‖ uint16(len(ORDER_TYPE))`
   where `appDomainSeparator` is the exchange domain separator, `contentsHash` is the
   `Order` struct hash, and `ORDER_TYPE` is the 186-byte V2 type string.
5. Session key only: `abi.encode(bytes32(leftPad(session_eoa)), bytes32(0), bytes(wrapped)) ‖ 0x6492…6492` (32 bytes of `6492`).

The owner signs steps 1–4 only. Both owner and session key are EOAs distinct from the
wallet, so the role cannot be inferred from addresses.

py-sdk pins the step-2 digest for a fixed fixture:
`0x1b9566eedd9589a73275df23a3a9d9e2e9897e76d31cd46d436f1b824d161b33`
(maker = signer = `0x57ffbc34de23124faeb8387fcd689d314e57accd`, exchange
`0x4bfb41d5b3570defd03c39a9a4d8de6bd8b8982e`, salt 1, tokenId 1, makerAmount 1_000_000,
takerAmount 500_000, BUY, timestamp 0, zero metadata and builder, chain 137).

### L1 auth

Plain `ClobAuth` under `ClobAuthDomain` v1, no verifying contract, exactly as today.
`POLY_ADDRESS` is the signing EOA: the owner for owner credentials, the session key for
session credentials. No ERC-7739 wrapping of `ClobAuth`. The owner's EOA-bound credentials
place owner-signed type-3 orders (ts-sdk live suite does this).

### Session-signers list

`GET /v1/user/session-signers` on the CLOB host under the owner's L2 credentials returns
`{ wallet, signers: [{ address, scopes, valid_until }] }`, `valid_until` in Unix seconds.

### Relayer, Deposit Wallet dialect (`https://relayer-v2.polymarket.com`)

- Nonce: `GET /v1/account/transactions/params?address=<owner>&type=WALLET` →
  `{ address, nonce }`.
- Typed data: domain `DepositWallet` v1, chain 137, verifying contract = the wallet;
  types `Call { target, value, data }`, `Batch { wallet, nonce, deadline, calls: Call[] }`.
  `deadline` must leave at least 10 s of validity when the relayer receives it.
- Execute: `POST /submit` with
  `{ type: "WALLET", from: <signer>, to: <factory>, nonce, signature, metadata?, depositWalletParams: { depositWallet, deadline, calls } }`.
  A session-key signer wraps `signature` in the 6492 envelope.
- Authorize: `POST /v1/session-signers/authorizations` with
  `{ walletAddress, sessionSignerAddress, scopes, validUntil, nonce, deadline, signature }`,
  Builder HMAC headers and `Idempotency-Key`. `validUntil` = now + 4 315 h (the venue
  rejects other lifetimes). `scopes` is `["CLOB"]`, `["COMBOSRFQ"]`, both, or `["ALL"]` alone.
  Response `{ operationId, status, transactionHash, transactionId }`; status ∈
  `SUBMITTED | REGISTRY_PENDING | REGISTERED | FAILED | SUPERSEDED | REPAIR_REQUIRED`.
- Revoke: `POST /v1/session-signers/revocations` with
  `{ walletAddress, sessionSignerAddress, nonce, deadline, signature }`, same headers.
  Status ∈ `PENDING | FENCED | SWEPT | CHAIN_SUBMITTED | CONFIRMED | FAILED`.
- Transaction poll: `GET /v1/account/transactions/<id>` → `state ∈ STATE_NEW | STATE_SUBMITTED | STATE_CONFIRMED | STATE_FAILED | STATE_INVALID`.
- The relayer may take up to five minutes to answer a session-signer submission.
  Revocation is complete for trading purposes when the key leaves the registry; the
  on-chain transaction and the cancel-all follow asynchronously.
- Calldata: `authorizeSessionSigner(address,uint256)` and `revokeSessionSigner(address)`
  on the Deposit Wallet.

### Deposit Wallet address derivation

Factory `0x00000000000Fb5C9ADea0298D729A0CB3823Cc07`. Wallets deployed before 2026-06-29 are
UUPS proxies (implementation `0x58CA52ebe0DadfdF531Cde7062e76746de4Db1eB`); later ones are
ERC-1967 beacon proxies (beacon `0x7A18EDfe055488A3128f01F563e5B479D92ffc3a`). Both are
CREATE2 from the factory with a salt derived from the owner. `GET /deployed?address=&type=WALLET`
(no auth) says whether a candidate exists. py-sdk's `wallet.py` is the readable reference.

## Design

### 1. CLOB signing core — `polyoxide-clob/src/core/eip712.rs`

New submodule `deposit_wallet`:

- `sol!` definition of `TypedDataSign` with the six fields in upstream order.
- `envelope_digest(order, chain_id, wallet) -> B256`: exchange domain separator (reusing
  the existing domain construction) over the `TypedDataSign` struct hash whose `contents`
  is the existing `Order` struct hash.
- `wrap_erc7739(inner: &[u8], app_domain_separator: B256, contents_hash: B256) -> Vec<u8>`.
- `wrap_session_signer(session_eoa: Address, wrapped: &[u8]) -> Vec<u8>`.
- `ORDER_TYPE_STRING` as a `const &str`, asserted at 186 bytes.

`sign_order` gains a `&SigningTarget` parameter. Types 0–2: unchanged path. Type 3: requires
`SigningTarget::DepositWallet`, signs `envelope_digest`, applies `wrap_erc7739`, and applies
`wrap_session_signer` when `role == SessionKey`.

Guards replace the three `Poly1271` rejections: `order_to_protocol` rejects a type-3 order
whose `maker != signer`; `sign_order` rejects a type-3 order against a non-Deposit-Wallet
target, and a non-type-3 order against a Deposit Wallet target.

### 2. Account model — `polyoxide-clob/src/account/`

- `Wallet` holds `Arc<dyn alloy::signers::Signer + Send + Sync>` and the cached address.
  `EthereumWallet` is removed (nothing reads it). `Account::new(hex_key, creds)` is unchanged
  and wraps a `PrivateKeySigner`; `Account::with_signer(signer, creds)` accepts any signer.
- `SigningTarget` enum: `Eoa`, `PolyProxy { funder: Address }`,
  `PolyGnosisSafe { funder: Address }`, `DepositWallet { wallet: Address, role: DepositWalletRole }`,
  with `DepositWalletRole::{Owner, SessionKey}`. `Account::with_target(self, target)`.
  Default `Eoa`. Accessors `maker()`, `order_signer()`, `signature_type()`.
- `create_order` and `create_market_order` take `maker`/`signer`/`signature_type` from the
  target. Per-call `funder` and `signature_type` in `PartialCreateOrderOptions` still
  override, so no existing caller changes. The Gamma proxy lookup stays for the proxy
  variants when no funder is known.
- `Account::l2_only(address, creds)`: no signer. Works for `post_order`, `orders()`,
  `account_api()`, `notifications()`, `session_signers()`. `create_order`, `sign_order`,
  `sign_clob_auth` and the L1 `auth()` calls return `ClobError::validation` naming the
  missing key. `AuthMode::L1` keeps carrying a signer, so an L2-only account cannot enter it.
- `auth()` namespace: `clob_auth_typed_data(address, chain_id, timestamp, nonce) -> serde_json::Value`
  (the exact JSON for `eth_signTypedData_v4`), `create_api_key_with_signature(address, timestamp, nonce, signature)`
  and `derive_api_key_with_signature(...)`. The existing signer-based calls become thin
  callers of the same request builder, so the headers are produced by one function.

### 3. Relay — `polyoxide-relay`

- `WalletType::DepositWallet` → wire `WALLET`.
- New `wallet.rs`: public pure derivations `derive_safe`, `derive_proxy`,
  `derive_deposit_wallet_uups`, `derive_deposit_wallet_beacon` (the first two move out of
  `client.rs`), with the constants above on `ContractConfig`.
  `RelayClient::resolve_wallet(owner) -> WalletKind { DepositWallet(Address) | Safe(Address) | Proxy(Address) | None }`
  derives all candidates, probes `/deployed` for the two Deposit Wallet candidates and the
  Safe, and errors if more than one is deployed.
- `RelayClientBuilder::with_auth(AuthConfig)` for header auth with no `BuilderAccount`.
  `BuilderAccount` takes the same generic signer as `Account`.
- `sol!` `Call` and `Batch`; `deposit_wallet_domain(wallet)`.
- `get_execute_params(owner) -> nonce` via `/v1/account/transactions/params?type=WALLET`.
- `deposit_wallet_batch_typed_data(wallet, calls, nonce, deadline) -> serde_json::Value`
  and `submit_deposit_wallet_batch(wallet, calls, nonce, deadline, signature, metadata)`.
  `execute` gains a `DepositWallet` arm that fetches the nonce, sets `deadline = now + 600 s` (the ts-sdk default),
  signs with the local signer, and applies the 6492 envelope when the account's role is
  session key. Trading approvals (the four calls from the deposit-wallets page) and
  redemption are call builders over this.
- `authorize_session_signer_typed_data(wallet, session, scopes, nonce, deadline) -> (serde_json::Value, SessionSignerAuthorization)`
  computes `valid_until` internally and encodes the calldata; the returned request struct
  carries every field the body needs except `signature`.
  `submit_session_signer_authorization(request, signature, idempotency_key) -> SessionSignerAuthorizationResponse`.
  `revoke_session_signer_typed_data` / `submit_session_signer_revocation` mirror it.
  Local-signer conveniences `authorize_session_signer(session, scopes)` and
  `revoke_session_signer(session)` run both halves. Builder HMAC only; a client without
  `AuthConfig::Builder` gets a validation error before any I/O.
- `SessionSignerScope { Clob, CombosRfq, All, Other(String) }` is defined in `polyoxide-core`
  and re-exported by both crates, serialised as the wire strings; `All` must appear alone. `SessionSignerAuthorizationStatus`,
  `SessionSignerRevocationStatus` and `TransactionState` are public enums with an `Other(String)`
  variant, and `is_terminal_failure()` on the two status enums.

### 4. CLOB session-signers endpoint and docs

- `account_api().session_signers() -> SessionSigners { wallet: Address, signers: Vec<SessionSigner> }`,
  `SessionSigner { address, scopes: Vec<SessionSignerScope>, valid_until: u64 }`. Works
  with an L2-only account. If the account's target is a Deposit Wallet and the response
  `wallet` differs, return `ClobError::validation`.
- Balance/allowance already sends the integer; doc comments listing 0–2 are corrected.
- `docs/specs/session-keys/README.md` (the contract above, transcribed),
  `docs/specs/session-keys/OBSERVED.md` (SDK behaviour the pages omit: the 4 315 h
  lifetime, Combos refused with a session key, the five-minute relayer wait, revocation's
  early return, the "unknown wallet defaults to session key" hack in ts-sdk that we do not
  copy), and an `INDEX.md` entry stating there is no machine-readable mirror and
  `nightly-schema.yml` excludes it. `CLAUDE.md` gets a paragraph pointing here and at
  the two SDK repos.
- The handoff doc's items get a one-line status each pointing at this spec.

### 5. Testing

- `scripts/capture_session_key_vectors.py` runs the installed `polymarket` package (py-sdk)
  against anvil key 0 and writes JSON fixtures to `polyoxide-clob/tests/fixtures/session_keys/`
  and `polyoxide-relay/tests/fixtures/session_keys/`: the type-3 envelope digest, the full
  7739-wrapped signature, the session envelope for a fixed session EOA, a signed Deposit
  Wallet `Batch`, and the four derived wallet addresses for a fixed owner. Rust tests compare
  byte for byte. The digest above is also hard-coded so the test is meaningful before the
  script has run.
- One test swaps the two domains and asserts the digest differs from the pinned value.
- `mockito` tests, following `tests/mock_api.rs`, asserting exact request bodies and headers:
  the params query, the `WALLET` submit body, Builder HMAC and `Idempotency-Key` on the
  session-signer routes, every status and state value, the session-signers response and the
  wallet-mismatch error, signature-in L1 headers byte-equal to the signer path, and an
  L2-only account refused by `create_order`.
- All existing `eip712.rs` vectors and client tests pass unchanged; `polyoxide-py` and the
  CLI still build.
- `RUSTDOCFLAGS="-D warnings" cargo doc` before every commit.
- A live round-trip test is added as `#[ignore]` with fixture wiring but is not run in this
  phase.

## Implementation order

Three plans, each ending in a green workspace:

1. **CLOB signing core and account model** (sections 1, 2, and the `session_signers()`
   call from 4). Vectors first.
2. **Relay Deposit Wallet dialect** (section 3), including the resolver and derivations.
3. **Docs and handoff status** (rest of section 4), plus the ignored live test skeleton.

## Open items carried to the blocked phase

- Whether balance/allowance under a session signer's credentials reports the wallet's balance.
- The exact tolerance on `validUntil`.
