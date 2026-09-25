# Deposit Wallets and session keys

**There is no machine-readable mirror for this surface.** Polymarket's published
OpenAPI documents (`../clob/openapi.yaml`, `../relay/openapi.yaml`) barely touch it.
The relay mirror documents only `GET /deployed?type=WALLET`, and its `/submit`
`type` enumerates `SAFE` and `PROXY` only. The CLOB mirror's `signatureType` still
enumerates 0–2. None of `/v1/user/session-signers`, `/v1/session-signers/*`,
`/v1/account/transactions/params` or `/v1/account/transactions/{id}` appears.
`nightly-schema.yml` therefore has nothing
to diff and deliberately excludes this directory (see the "Deliberately absent"
comment in the workflow). This file is the contract record; [OBSERVED.md](OBSERVED.md)
beside it records what the official SDKs do that the prose pages do not say.

## Sources

Read on 2026-09-25:

- Prose: `https://docs.polymarket.com/trading/session-keys.md`,
  `https://docs.polymarket.com/trading/deposit-wallets.md`,
  `https://docs.polymarket.com/trading/place-orders.md`,
  `https://docs.polymarket.com/trading/wallets-auth.md`,
  `https://docs.polymarket.com/trading/positions/manage.md`. The first two postdate the
  `../polymarket-llms.txt` snapshot, which does not list them.
- Code: `github.com/Polymarket/py-sdk` (`polymarket-client==0.11.0`) and
  `github.com/Polymarket/ts-sdk` (`@polymarket/client` 0.11.0). Where a page and an
  SDK disagree, polyoxide follows the SDK unless `OBSERVED.md`'s polyoxide column
  says otherwise; every such case is listed there.
- polyoxide's fixtures are generated from py-sdk by
  `scripts/capture_session_key_vectors.py` into
  `polyoxide-clob/tests/fixtures/session_keys/` and
  `polyoxide-relay/tests/fixtures/session_keys/`; each directory's `PROVENANCE.md`
  says exactly what was run.

## Vocabulary

- **Deposit Wallet.** Polymarket's smart account, the default for accounts created on
  or after 2026-05-04. It is the `maker` and `signer` of every order it places. Legacy
  Proxy and Safe accounts cannot use session keys.
- **Owner.** The EOA the wallet was derived from. It signs orders under signature type 3
  and relayer batches, and it is the only key that can sign session-key management
  batches.
- **Session key.** A fresh EOA the owner authorizes on the wallet contract. It can sign
  orders and relayer batches for the wallet within its scopes; it cannot withdraw and
  cannot manage other session keys.
- **Scopes.** `CLOB`, `COMBOSRFQ`, or `ALL` (alone). polyoxide models them as
  `polyoxide_core::SessionSignerScope`, an open enum.
- **Role.** Owner or session key. The two are EOAs distinct from the wallet, so the
  role cannot be inferred from addresses; polyoxide takes it explicitly
  (`SigningTarget::DepositWallet { wallet, role }` in clob,
  `RelayClientBuilder::deposit_wallet_role` in relay).

## Order signing, signature type 3

`maker` = `signer` = the Deposit Wallet address; `signatureType` = 3. The signature is
an ERC-7739 `TypedDataSign` envelope:

1. EIP-712 domain `Polymarket CTF Exchange`, version `2`, chain 137, verifying
   contract = the exchange (standard or neg-risk, as for any V2 order).
2. Primary type `TypedDataSign { contents: Order, name, version, chainId,
   verifyingContract, salt }` with `contents` = the V2 `Order`, `name = "DepositWallet"`,
   `version = "1"`, `chainId = 137`, `verifyingContract` = the Deposit Wallet,
   `salt = bytes32(0)`. The wallet's own domain rides *inside the message*; the
   exchange's is the signing domain. Swapping them produces a different digest, so
   the signature cannot verify for the wallet; `swapping_the_two_domains_changes_the_digest`
   in `eip712.rs` proves the fixture tells the two layouts apart.
3. `inner = sign(digest)`.
4. `wrapped = inner ‖ appDomainSeparator ‖ contentsHash ‖ bytes(ORDER_TYPE) ‖ uint16(len(ORDER_TYPE))`,
   where `appDomainSeparator` is the exchange domain separator, `contentsHash` the
   `Order` struct hash, and `ORDER_TYPE` the 186-byte V2 type string.
5. Session key only: `abi.encode(bytes32(leftPad(session_eoa)), bytes32(0), bytes(wrapped)) ‖ 0x6492…6492`
   (`0x6492` repeated 16 times, 32 bytes).

The owner signs steps 1–4. py-sdk pins the step-2 digest for a fixed fixture
(`0x1b9566eedd9589a73275df23a3a9d9e2e9897e76d31cd46d436f1b824d161b33`); polyoxide
pins the same digest as `v1_exchange.envelope_digest` in
`polyoxide-clob/tests/fixtures/session_keys/order_vectors.json`, alongside the
full wrapped bytes. `envelope_digest_matches_py_sdk_for_both_exchanges` in
`polyoxide-clob/src/core/eip712.rs` checks it, and the capture script refuses to
write a fixture whose digest differs.

## L1 auth (creating and deriving API credentials)

Plain `ClobAuth` under `ClobAuthDomain` v1 with no verifying contract, exactly as for
an EOA. `POLY_ADDRESS` is the signing EOA: the owner for owner credentials, the session
key for session credentials. Nothing is ERC-7739-wrapped here. The owner's EOA-bound
credentials place and cancel owner-signed type-3 orders (ts-sdk's live suite does this).

## L2 auth and visibility

L2 is unchanged (HMAC over `timestamp + method + path [+ body]`). Visibility is scoped
by the submitting key **in both directions**: a session key lists only its own orders
and trades, and *"A Deposit Wallet Owner cannot fetch orders submitted by its
authorized Session Keys"* (session-keys.md, Considerations). A caller who needs a
whole-wallet view must query with every key it holds.

## Session-signers list

`GET /v1/user/session-signers` on `clob.polymarket.com` under the owner's L2
credentials returns `{ wallet, signers: [{ address, scopes, valid_until }] }`,
`valid_until` in Unix seconds. Whether it answers under a session key's own
credentials, and with which `wallet`, is unverified (open item).

## Relayer, Deposit Wallet dialect (`https://relayer-v2.polymarket.com`)

| Route | Auth | Body / query | Response |
|---|---|---|---|
| `GET /v1/account/transactions/params?address=<signer>&type=WALLET` | none | | `{ address, nonce }` (`nonce` is a decimal string). The address is the EOA that will sign: owner or session key. |
| `POST /submit` | Builder HMAC or Relayer API key | `{ type: "WALLET", from: <signer>, to: <factory>, nonce, signature, metadata, depositWalletParams: { depositWallet, deadline, calls: [{ target, value, data }] } }` | `{ transactionID, state }` |
| `POST /v1/session-signers/authorizations` | Builder HMAC only, plus `Idempotency-Key` | `{ walletAddress, sessionSignerAddress, scopes, validUntil, nonce, deadline, signature }` | `{ operationId, status, transactionHash, transactionId }` |
| `POST /v1/session-signers/revocations` | Builder HMAC **or Relayer API key**, plus `Idempotency-Key` | `{ walletAddress, sessionSignerAddress, nonce, deadline, signature }` | `{ operationId, status, fenced, transactionId }` |
| `GET /v1/account/transactions/{id}` | none | | `{ transaction_id, transaction_hash, state, error_msg }` |
| `GET /deployed?address=<candidate>&type=WALLET` | none | | `{ deployed }`; the published spec enumerates `SAFE` and `WALLET`, py-sdk also sends `PROXY` |

Every `nonce`, `deadline`, `value` and `validUntil` is a decimal string on the wire.
`metadata` is always present, `""` by default, at most 500 characters.

**Batch typed data.** Domain `DepositWallet` v1, chain 137, verifying contract = the
wallet. Types `Call { address target; uint256 value; bytes data }` and
`Batch { address wallet; uint256 nonce; uint256 deadline; Call[] calls }`. Plain
EIP-712, no ERC-7739 layer. A session key's signature is wrapped in the same 6492
envelope as for orders. `deadline` must leave at least 10 s of validity on receipt; the
SDKs use now + 600 s.

**Session-key management.** `validUntil` = now + 4 315 h (the session-keys page says
other lifetimes are rejected; the tolerance is unverified). `scopes` is `["CLOB"]`,
`["COMBOSRFQ"]`, both, or `["ALL"]` alone. The
batch's one call targets the wallet itself with `authorizeSessionSigner(address,uint256)`
or `revokeSessionSigner(address)` calldata. Authorization status ∈
`SUBMITTED | REGISTRY_PENDING | REGISTERED | FAILED | SUPERSEDED | REPAIR_REQUIRED`;
revocation status ∈ `PENDING | FENCED | SWEPT | CHAIN_SUBMITTED | CONFIRMED | FAILED`.
The relayer may take up to five minutes to answer either. Revocation is complete for
trading purposes when the key leaves the registry (`fenced: true`); the on-chain
transaction and the cancel-all of its open orders follow asynchronously.

**Transaction states.** `STATE_NEW | STATE_EXECUTED | STATE_MINED | STATE_CONFIRMED |
STATE_INVALID | STATE_FAILED` (py-sdk's list; the prose page's list differs, see
`OBSERVED.md`). `CONFIRMED` is success; `INVALID` and `FAILED` are terminal failures.

**Trading approvals.** A fresh wallet is "fully approved" once it holds the 7 ERC-20
`approve` and 10 ERC-1155 `setApprovalForAll` calls py-sdk's
`_required_trading_approvals` lists (pUSD, the Conditional Tokens contract and the
position manager, towards the two V2 exchanges, exchange V3, the two collateral
adapters, the V2 router, perps deposit, the auto-redeem operator and the two modules).
The deposit-wallets page lists only four of them.

**Redemption.** py-sdk redeems in one batch call, and the target depends on the market.
For a CTF market it calls `redeemPositions(pUSD, 0x0, conditionId, [1, 2])` on the
collateral adapter (`0xAdA100Db00Ca00073811820692005400218FcE1f`); for a neg-risk market
the target is the neg-risk collateral adapter
(`0xadA2005600Dec949baf300f4C6120000bDB6eAab`). It never calls the Conditional Tokens
contract directly. A protocol-V2 market goes through the V2 router's `redeem` and is not
covered here. pUSD (`0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB`) is the Deposit Wallet
collateral; polyoxide's Safe and Proxy paths still redeem against USDC on the Conditional
Tokens contract, which is what they did before Deposit Wallets existed and has not been
re-checked against py-sdk. The Manage Positions page's API tab gives the same adapters,
collateral and index sets.

## Deposit Wallet address derivation

Factory `0x00000000000Fb5C9ADea0298D729A0CB3823Cc07`. Wallets deployed before 2026-06-29
are UUPS proxies (implementation `0x58CA52ebe0DadfdF531Cde7062e76746de4Db1eB`); later
ones are ERC-1967 beacon proxies (beacon `0x7A18EDfe055488A3128f01F563e5B479D92ffc3a`).
Both are CREATE2 from the factory with a salt derived from the owner. Resolving which
one an owner has means deriving all four candidates (both Deposit Wallet generations, the
Safe and the Proxy) and asking `GET /deployed` for each; `resolve_wallet` does so (four
requests).

## Where polyoxide implements it

| Contract item | polyoxide | Pinned by |
|---|---|---|
| Type-3 order signing, owner or session key | `polyoxide_clob::Account::with_target(SigningTarget::DepositWallet { wallet, role })`, then `Clob::create_order` / `sign_order` / `post_order` | `tests/fixtures/session_keys/order_vectors.json` (py-sdk golden digest and wrapped bytes), checked in `polyoxide-clob/src/core/eip712.rs` |
| Any alloy signer, or no key at all | `Account::with_signer`, `Account::l2_only` | unit and mock tests |
| L1 auth with an external signer | `clob_auth_typed_data`, `Clob::create_api_key_with_signature`, `Clob::derive_api_key_with_signature` (signer recovered locally before any request) | `tests/fixtures/session_keys/clob_auth.json` |
| Session-signers list | `AccountApi::list_session_signers` | mock tests |
| Wallet derivation and resolution | `polyoxide_relay::wallet::{derive_safe, derive_proxy, derive_deposit_wallet_uups, derive_deposit_wallet_beacon}`, `RelayClient::resolve_wallet -> Option<WalletKind>` over beacon, UUPS, Safe and Proxy | `polyoxide-relay/tests/fixtures/session_keys/relay_vectors.json` (`derivations`) |
| Nonce and transaction poll | `RelayClient::get_execute_params`, `RelayClient::get_gasless_transaction`, `TransactionState` | mock tests |
| Batch typed data, digest, session envelope, calldata | `polyoxide_relay::deposit_wallet::{batch_typed_data, batch_digest, wrap_session_signer, …_calldata}` | `relay_vectors.json` (five signed batches) |
| Execute as owner or session key | `RelayClient::execute` with `WalletType::DepositWallet` and `RelayClientBuilder::{deposit_wallet, deposit_wallet_role}` | `relay_vectors.json` (`submit_body`, `session_submit_body`) |
| Typed-data-out / signature-in | `RelayClient::deposit_wallet_batch_typed_data`, `submit_deposit_wallet_batch_from`, `redeem_typed_data(…, neg_risk, …)`, `submit_redemption_with_signature`; `RelayClientBuilder::with_auth` for a client with no key | `relay_vectors.json` (`redeem_adapter_batch`, `redeem_neg_risk_batch`) |
| Redemption as the client's own account | `RelayClient::submit_deposit_wallet_redemption(condition_id, neg_risk, estimate_gas)`; `submit_gasless_redemption` refuses a Deposit Wallet client before I/O | `relay_vectors.json` (`redeem_adapter_submit_body`) |
| Session-key management | `RelayClient::authorize_session_signer[_typed_data]`, `submit_session_signer_authorization`, `revoke_session_signer[_typed_data]`, `submit_session_signer_revocation`; `SESSION_KEY_LIFETIME_SECS`, `SESSION_SIGNER_REQUEST_TIMEOUT` | `relay_vectors.json` (`authorization_body`, `revocation_body`, the two constants) |
| Trading approvals | `RelayClient::deposit_wallet_trading_approvals` | `relay_vectors.json` (`trading_approvals`) |

Not covered: creating a Deposit Wallet (`WALLET_CREATE`), Combos with a session key
(both SDKs refuse it), the Python bindings and the CLI.

## Live verification

`polyoxide-clob/tests/live_session_keys.rs` holds the round trip (authorize → derive
session credentials → place → list from both keys → cancel → revoke) as an `#[ignore]`d
test gated on a Deposit Wallet fixture account. It has not been run: session-key
management is gated per Builder API key by the venue, and DilettanteTrading's key is
not yet enabled (prader-rs #125). Until it runs, every claim above rests on the SDKs
and the offline fixtures, not on the host.
