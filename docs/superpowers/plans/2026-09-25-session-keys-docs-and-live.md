# Session Keys Plan 3: Docs, Handoff Status and the Live Round-Trip Skeleton

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record the Deposit Wallet / session-key contract and the SDK behaviours the venue's pages omit under `docs/specs/session-keys/`, wire that record into the spec index, the drift workflow and `CLAUDE.md`, mark every handoff item with its status, and add the `#[ignore]`d live round-trip test that the blocked phase will run once the Builder key is enabled (prader-rs #125).

**Architecture:** Two Markdown files carry the contract (`README.md`, transcribed from the approved spec) and the observations (`OBSERVED.md`, each with a file:line citation into py-sdk or ts-sdk 0.11.0). Three index files and `CLAUDE.md` point at them; `nightly-schema.yml` gains a comment saying why nothing is diffed. The live test lives in `polyoxide-clob/tests/live_session_keys.rs` with `polyoxide-relay` as a dev-dependency, gated on a distinct env-var set and panicking in the auth-gated wording the nightly classifier already recognises.

**Tech Stack:** Markdown; Rust 1.91 (`tokio`, `alloy` signers, `polyoxide-clob` + `polyoxide-relay` + `polyoxide-gamma`); GitHub Actions YAML (comment and one flag only).

**Spec:** `docs/superpowers/specs/2026-09-25-session-keys-offline-design.md` section 4 (the docs and handoff bullets), section 5's last bullet (the ignored live test), and "Implementation order" item 3. Plans 1 and 2 are merged to `main` (`02f011d`, `5932426`).

**Reference sources on this machine (all 0.11.0):**
- py-sdk: `/home/aidanb/.cache/uv/archive-v0/Wsx9G8NCgKLdKu1-/polymarket/` (`_internal/actions/session_keys.py`, `_internal/actions/relayer/{gasless,approvals,poll,deployed}.py`, `_internal/actions/combo_rfq.py`, `_internal/wallet.py`, `models/clob/relayer.py`, `environments.py`). Cite as `py-sdk <path>:<line>`.
- ts-sdk: `/tmp/claude-1000/-tb-Source-DilettanteTrading-polyoxide--loom-worktrees-aidanb-non-custodial-keys-18d880610dabaf96/292bd143-da8c-4ee0-94a9-63e036ac7458/scratchpad/ts-sdk/packages/client/src/` (`actions/session-keys.ts`, `actions/rfq.ts`, `wallet.ts`). Cite as `ts-sdk packages/client/src/<file>:<line>`. If that directory is gone, `git clone --depth 1 https://github.com/Polymarket/ts-sdk` into the scratchpad and check the version in `packages/client/package.json` is 0.11.0.
- Every line number in this plan was read on 2026-09-25 and must be re-checked with `sed -n` before it is written into a doc.

**Environment note:** builds on this machine can be killed by `earlyoom` (rustc exits with signal 15 / status 254). Rerun with `CARGO_BUILD_JOBS=4`. Never put a target dir under `/tmp`.

**Gates before every commit:** `cargo fmt --all -- --check` (Task 5 only touches Rust, but run it anyway; it is free). Before the final commit: `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace`, `cargo test --workspace --all-features`, and `cargo test -p polyoxide-clob --features ws,keychain --test live_session_keys` (compiles the ignored test and runs zero tests). Docs tasks have no compile gate, but every relative link in a Markdown file added or edited must resolve: check with `for f in <files>; do grep -o '](\([^)#]*\)' $f | ...; done` or by hand with `ls`.

---

## File map

| File | Responsibility |
|---|---|
| `docs/specs/session-keys/README.md` | Create. The contract: sources, vocabulary, signing, auth, relayer dialect, derivation, visibility rules, the implementation map. |
| `docs/specs/session-keys/OBSERVED.md` | Create. SDK behaviour the pages omit, each with a citation. |
| `docs/specs/INDEX.md` | Modify. A "Surfaces with no upstream spec" section pointing at `session-keys/`. |
| `docs/specs/clob/INDEX.md`, `docs/specs/relay/INDEX.md` | Modify. One note each listing what their `openapi.yaml` omits. |
| `.github/workflows/nightly-schema.yml` | Modify. One "Deliberately absent" comment bullet. No matrix change. |
| `CLAUDE.md` | Modify. A paragraph in "API Specs"; a sentence under the signing-schemes table. |
| `docs/handoff-deposit-wallet-session-keys.md` | Modify. A status header and a status line under each of items 1–16. |
| `docs/superpowers/specs/2026-09-25-session-keys-offline-design.md` | Modify. "Implementation order" items 2 and 3 marked done. |
| `polyoxide-clob/Cargo.toml` | Modify. `polyoxide-relay` as a dev-dependency. |
| `polyoxide-clob/tests/live_session_keys.rs` | Create. The `#[ignore]` round trip. |
| `.github/workflows/nightly-behavioral.yml` | Modify. `--test live_session_keys` added to the clob flags. |
| `polyoxide-relay/src/{wallet,client}.rs`, `polyoxide-relay/tests/mock_api.rs` | Modify (Task 6, review amendment). `WalletKind::Proxy`; `resolve_wallet` probes `type=PROXY`. |

Task 6 is the one library change (a new `#[non_exhaustive]` enum variant and a fourth probe); nothing else touches code outside tests.

---

### Task 1: `docs/specs/session-keys/README.md`

**Files:**
- Create: `docs/specs/session-keys/README.md`

- [ ] **Step 1: Verify the facts the file cites**

Run, and confirm each prints what the README below claims:

```bash
cd /tb/Source/DilettanteTrading/polyoxide/.loom/worktrees/aidanb/non-custodial-keys_18d880610dabaf96
# exported names the implementation map names
grep -n 'pub fn with_target\|pub fn with_signer\|pub fn l2_only' polyoxide-clob/src/account/mod.rs
grep -n 'pub async fn create_api_key_with_signature\|pub async fn derive_api_key_with_signature\|pub fn clob_auth_typed_data' polyoxide-clob/src/client.rs polyoxide-clob/src/core/eip712.rs
grep -n 'pub async fn list_session_signers' polyoxide-clob/src/api/account.rs
grep -n 'pub async fn resolve_wallet\|pub async fn get_execute_params\|pub async fn get_gasless_transaction\|pub fn authorize_session_signer_typed_data\|pub async fn submit_session_signer_authorization\|pub fn revoke_session_signer_typed_data\|pub async fn submit_session_signer_revocation\|pub async fn authorize_session_signer\|pub async fn revoke_session_signer\|pub fn redeem_typed_data\|pub async fn submit_redemption_with_signature\|pub fn deposit_wallet_batch_typed_data\|pub async fn submit_deposit_wallet_batch_from\|pub fn deposit_wallet_trading_approvals' polyoxide-relay/src/client.rs
grep -n 'pub fn batch_digest\|pub fn batch_typed_data\|pub fn wrap_session_signer\|pub const SESSION_KEY_LIFETIME_SECS\|pub const DEFAULT_BATCH_DEADLINE_SECS' polyoxide-relay/src/deposit_wallet.rs
grep -n 'pub const SESSION_SIGNER_REQUEST_TIMEOUT' polyoxide-relay/src/session_signers.rs
ls polyoxide-clob/tests/fixtures/session_keys polyoxide-relay/tests/fixtures/session_keys
```

Expected: every name exists; the fixture directories hold `order_vectors.json`, `clob_auth.json`, `PROVENANCE.md` (clob) and `relay_vectors.json`, `PROVENANCE.md` (relay).

- [ ] **Step 2: Write the file**

Create `docs/specs/session-keys/README.md`:

````markdown
# Deposit Wallets and session keys

**There is no machine-readable mirror for this surface.** Polymarket's published
OpenAPI documents (`../clob/openapi.yaml`, `../relay/openapi.yaml`) do not contain
it: `signatureType` still enumerates 0–2, and none of `/v1/user/session-signers`,
`/v1/session-signers/*`, `/v1/account/transactions/params` or
`/v1/account/transactions/{id}` appears. `nightly-schema.yml` therefore has nothing
to diff and deliberately excludes this directory (see the "Deliberately absent"
comment in the workflow). This file is the contract record; [OBSERVED.md](OBSERVED.md)
beside it records what the official SDKs do that the prose pages do not say.

## Sources

Read on 2026-09-25:

- Prose: `https://docs.polymarket.com/trading/session-keys.md`,
  `https://docs.polymarket.com/trading/deposit-wallets.md`,
  `https://docs.polymarket.com/trading/place-orders.md`,
  `https://docs.polymarket.com/trading/wallets-auth.md`. The first two postdate the
  `../polymarket-llms.txt` snapshot, which does not list them.
- Code: `github.com/Polymarket/py-sdk` (`polymarket-client==0.11.0`) and
  `github.com/Polymarket/ts-sdk` (`@polymarket/client` 0.11.0). Where a page and an
  SDK disagree, polyoxide follows the SDK, because the SDK is what the venue tests
  against; every such case is listed in `OBSERVED.md`.
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
  and signs every relayer batch (approvals, redemption, session-key management).
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
   exchange's is the signing domain. Swapping them produces a signature the venue
   rejects with no useful error.
3. `inner = sign(digest)`.
4. `wrapped = inner ‖ appDomainSeparator ‖ contentsHash ‖ bytes(ORDER_TYPE) ‖ uint16(len(ORDER_TYPE))`,
   where `appDomainSeparator` is the exchange domain separator, `contentsHash` the
   `Order` struct hash, and `ORDER_TYPE` the 186-byte V2 type string.
5. Session key only: `abi.encode(bytes32(leftPad(session_eoa)), bytes32(0), bytes(wrapped)) ‖ 0x6492…6492`
   (thirty-two bytes of `6492`).

The owner signs steps 1–4. py-sdk pins the step-2 digest for a fixed fixture
(`0x1b9566eedd9589a73275df23a3a9d9e2e9897e76d31cd46d436f1b824d161b33`); polyoxide
pins the same digest in `polyoxide-clob/src/core/eip712.rs` and the full wrapped
bytes in `polyoxide-clob/tests/fixtures/session_keys/order_vectors.json`.

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
| `GET /deployed?address=<candidate>&type=WALLET` | none | | `{ deployed }` (also answers `type=SAFE`; **not** Proxy) |

Every `nonce`, `deadline`, `value` and `validUntil` is a decimal string on the wire.
`metadata` is always present, `""` by default, at most 500 characters.

**Batch typed data.** Domain `DepositWallet` v1, chain 137, verifying contract = the
wallet. Types `Call { address target; uint256 value; bytes data }` and
`Batch { address wallet; uint256 nonce; uint256 deadline; Call[] calls }`. Plain
EIP-712, no ERC-7739 layer. A session key's signature is wrapped in the same 6492
envelope as for orders. `deadline` must leave at least 10 s of validity on receipt; the
SDKs use now + 600 s.

**Session-key management.** `validUntil` = now + 4 315 h (the venue rejects other
lifetimes). `scopes` is `["CLOB"]`, `["COMBOSRFQ"]`, both, or `["ALL"]` alone. The
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

**Redemption.** `redeemPositions(pUSD, 0x0, conditionId, indexSets)` on the Conditional
Tokens contract, as one batch call. pUSD (`0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB`)
is the Deposit Wallet collateral; Safe and Proxy redeem against USDC.

## Deposit Wallet address derivation

Factory `0x00000000000Fb5C9ADea0298D729A0CB3823Cc07`. Wallets deployed before 2026-06-29
are UUPS proxies (implementation `0x58CA52ebe0DadfdF531Cde7062e76746de4Db1eB`); later
ones are ERC-1967 beacon proxies (beacon `0x7A18EDfe055488A3128f01F563e5B479D92ffc3a`).
Both are CREATE2 from the factory with a salt derived from the owner. Resolving which
one an owner has means deriving both and asking `GET /deployed` for each; the relayer
answers for `WALLET` and `SAFE` only, so a Proxy is derived, never resolved.

## Where polyoxide implements it

| Contract item | polyoxide | Pinned by |
|---|---|---|
| Type-3 order signing, owner or session key | `polyoxide_clob::Account::with_target(SigningTarget::DepositWallet { wallet, role })`, then `Clob::create_order` / `sign_order` / `post_order` | `polyoxide-clob/src/core/eip712.rs` (py-sdk golden digest), `tests/fixtures/session_keys/order_vectors.json` |
| Any alloy signer, or no key at all | `Account::with_signer`, `Account::l2_only` | mock tests |
| L1 auth with an external signer | `clob_auth_typed_data`, `Clob::create_api_key_with_signature`, `Clob::derive_api_key_with_signature` (signer recovered locally before any request) | `tests/fixtures/session_keys/clob_auth.json` |
| Session-signers list | `AccountApi::list_session_signers` | mock tests |
| Wallet derivation and resolution | `polyoxide_relay::wallet::{derive_safe, derive_proxy, derive_deposit_wallet_uups, derive_deposit_wallet_beacon}`, `RelayClient::resolve_wallet -> Option<WalletKind>` | `polyoxide-relay/tests/fixtures/session_keys/relay_vectors.json` (`derivations`) |
| Nonce and transaction poll | `RelayClient::get_execute_params`, `RelayClient::get_gasless_transaction`, `TransactionState` | mock tests |
| Batch typed data, digest, session envelope, calldata | `polyoxide_relay::deposit_wallet::{batch_typed_data, batch_digest, wrap_session_signer, …_calldata}` | `relay_vectors.json` (five signed batches) |
| Execute as owner or session key | `RelayClient::execute` with `WalletType::DepositWallet` and `RelayClientBuilder::{deposit_wallet, deposit_wallet_role}` | `relay_vectors.json` (`submit_body`, `session_submit_body`) |
| Typed-data-out / signature-in | `RelayClient::deposit_wallet_batch_typed_data`, `submit_deposit_wallet_batch_from`, `redeem_typed_data`, `submit_redemption_with_signature`; `RelayClientBuilder::with_auth` for a client with no key | `relay_vectors.json` |
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
````

- [ ] **Step 3: Check links and commit**

```bash
ls docs/specs/clob/openapi.yaml docs/specs/relay/openapi.yaml docs/specs/polymarket-llms.txt docs/specs/session-keys/OBSERVED.md 2>&1
```
`OBSERVED.md` does not exist yet (Task 2 creates it); every other path must. Then:

```bash
git add docs/specs/session-keys/README.md
git commit -m "docs(specs): record the Deposit Wallet and session-key contract"
```

---

### Task 2: `docs/specs/session-keys/OBSERVED.md`

**Files:**
- Create: `docs/specs/session-keys/OBSERVED.md`

- [ ] **Step 1: Re-read every cited line**

```bash
P=/home/aidanb/.cache/uv/archive-v0/Wsx9G8NCgKLdKu1-/polymarket
T=/tmp/claude-1000/-tb-Source-DilettanteTrading-polyoxide--loom-worktrees-aidanb-non-custodial-keys-18d880610dabaf96/292bd143-da8c-4ee0-94a9-63e036ac7458/scratchpad/ts-sdk/packages/client/src
sed -n '54,60p;198,202p;285,290p;303,310p;425,444p' $P/_internal/actions/session_keys.py
sed -n '822,825p' $P/_internal/actions/combo_rfq.py
sed -n '156,164p' $P/_internal/wallet.py
sed -n '20,28p' $P/models/clob/relayer.py
grep -n '_METADATA_MAX_LENGTH' $P/_internal/actions/relayer/gasless.py
grep -n 'def _required_trading_approvals' -A 30 $P/_internal/actions/relayer/approvals.py | head -40
grep -n 'SAFE\|WALLET' $P/_internal/actions/relayer/deployed.py | head
sed -n '133,138p;247,251p;270,278p' $T/actions/session-keys.ts
sed -n '1436,1440p' $T/actions/rfq.ts
sed -n '365,372p' $T/wallet.ts
grep -n "type" docs/specs/relay/openapi.yaml | grep -n -i 'SAFE\|WALLET\|PROXY' | head
```

Adjust every `file:line` in the text below to what these print. If a line moved, fix the citation; if a claim is not supported by what you read, drop the claim and say so in the commit message.

- [ ] **Step 2: Write the file**

Create `docs/specs/session-keys/OBSERVED.md`:

````markdown
# Deposit Wallets and session keys: what the SDKs do that the pages do not say

`README.md` beside this file is the contract as the prose pages and the official SDKs
state it. This file records where the pages are silent, vague or contradicted by the
SDKs, with a citation into `polymarket-client==0.11.0` (py-sdk) or
`@polymarket/client` 0.11.0 (ts-sdk) for each. Nothing here has been observed against
the live host: session-key management is gated per Builder API key and ours is not yet
enabled (prader-rs #125). When the live round trip in
`polyoxide-clob/tests/live_session_keys.rs` runs, move each confirmed row into
`README.md` and each contradicted one into a new "Contradicted live" section here.

## Pages versus SDKs

| # | Page says | SDKs do | polyoxide |
|---|---|---|---|
| 1 | `validUntil` is "now + 180 days"; "other values are rejected" | Exactly `4_315 * 60 * 60` s (≈179.8 d), computed at call time and not a parameter (py-sdk `_internal/actions/session_keys.py:55`, `:351`; ts-sdk `packages/client/src/actions/session-keys.ts:135`, `:249-250`) | `SESSION_KEY_LIFETIME_SECS`; `authorize_session_signer_typed_data` computes it; a `_with_valid_until` variant exists for tests and for the day the tolerance is known |
| 2 | Nothing about request timeouts | Both session-signer POSTs wait 300 s, because the venue "synchronously validate[s], simulate[s], persist[s], and broadcast[s]" (py-sdk `session_keys.py:57-59`; ts-sdk `session-keys.ts:137`, `:277`) | `SESSION_SIGNER_REQUEST_TIMEOUT` = 300 s, per request, overriding the 30 s client default |
| 3 | Both routes use the Builder HMAC headers | Authorization requires a Builder key; revocation accepts a Builder key **or** a Relayer API key (py-sdk `session_keys.py:200` vs `:287`, definitions at `:432-444`) | Authorization refuses a Relayer API key before I/O; revocation accepts either |
| 4 | Transaction states are `STATE_NEW`, `STATE_SUBMITTED`, `STATE_CONFIRMED`, `STATE_FAILED`, `STATE_INVALID` | py-sdk models `STATE_NEW`, `STATE_EXECUTED`, `STATE_MINED`, `STATE_CONFIRMED`, `STATE_INVALID`, `STATE_FAILED` and no `STATE_SUBMITTED` (`models/clob/relayer.py:22-27`) | `TransactionState` carries py-sdk's six plus `Other(String)`, so an unlisted value is preserved, never an error |
| 5 | Four approvals make a wallet ready to trade | `_required_trading_approvals` yields 7 ERC-20 and 10 ERC-1155 approvals (py-sdk `_internal/actions/relayer/approvals.py`, function `_required_trading_approvals`; addresses in `environments.py`) | `deposit_wallet_trading_approvals` returns all 17 in py-sdk's order, pinned to `relay_vectors.json` → `trading_approvals` |
| 6 | `metadata` is optional on `/submit` | Always sent, `""` by default, at most 500 characters (py-sdk `_internal/actions/relayer/gasless.py`, `_METADATA_MAX_LENGTH`) | Deposit Wallet submits always carry it and refuse longer values before I/O; Safe and Proxy bodies are unchanged |
| 7 | Revocation "cancels open orders" | The SDK returns as soon as the key has left the active-key registry; the on-chain revocation and the cancel-all continue in the backend ("This eager return is intentional", py-sdk `session_keys.py:307-309`). The response carries `fenced: bool` | `SessionSignerRevocationResponse::fenced`; `revoke_session_signer` returns at the same point |
| 8 | `COMBOSRFQ` is a valid scope | Both SDKs refuse Combos RFQ with a session key regardless of scope: "Combos is not supported with Session Keys" (py-sdk `_internal/actions/combo_rfq.py:823-824`; ts-sdk `packages/client/src/actions/rfq.ts:1438-1439`) | polyoxide has no Combos RFQ client, so nothing to refuse; recorded so a future one does |
| 9 | Nothing about how to tell an owner from a session key | When the wallet cannot be derived from the signer, both SDKs *assume* a session key on a Deposit Wallet ("TEMP: Default to the Deposit Wallet session-signature path…", py-sdk `_internal/wallet.py:161-163`; ts-sdk `packages/client/src/wallet.ts:367-371`) | Not copied. The role is explicit (`SigningTarget::DepositWallet { role }`, `RelayClientBuilder::deposit_wallet_role`), because a wrong guess signs with the wrong envelope and fails at the venue with no useful error |
| 10 | `GET /deployed` takes a wallet type | The relayer answers for `SAFE` and `WALLET` only; the published `openapi.yaml` enumerates the same two (`../relay/openapi.yaml`, `type` on `/deployed`) | `resolve_wallet` probes the beacon and UUPS Deposit Wallets and the Safe, and never a Proxy; a Proxy address is derived (`derive_proxy`) but cannot be confirmed |

## Behaviour the pages omit entirely

**Retries reuse the idempotency key.** py-sdk retries a session-signer POST twice on
5xx and transport errors with the *same* `Idempotency-Key`
(`session_keys.py:54`, `_SESSION_KEY_SUBMISSION_MAX_RETRIES`); ts-sdk generates a
random UUID per call when none is given (`session-keys.ts:273-274`). polyoxide's
conveniences generate a fresh UUID v4 and retry only 429, so a caller who wants
py-sdk's retry shape uses the two-step API (`*_typed_data` then `submit_*` with its own
key) and resends with the same key.

**Session keys are managed only by the owner, only for a Deposit Wallet.** py-sdk
refuses before any request when the account is not a Deposit Wallet or the signer is
not its owner (`session_keys.py:425-430`). polyoxide's `authorize_session_signer` and
`revoke_session_signer` refuse under a session-key role the same way.

**The nonce belongs to the signing EOA.** `GET /v1/account/transactions/params` is
queried with the address that will sign the batch: the owner for owner batches, the
session key for session-key batches. py-sdk's `build_signed_deposit_wallet_batch`
uses `ctx.signer.address` for both the query and the `from` field, and wraps the
signature with the same address. polyoxide's `execute` does the same with the
account's address.

**A batch `value` is an integer in the SDKs' typed data.** py-sdk emits `call.value`
as a Python int of any size. polyoxide emits a JSON number up to `u64::MAX` wei and a
decimal string above that, because `serde_json` cannot hold a larger integer exactly
and a rounded value would sign a different batch than the digest hashes. Every
EIP-712 signer accepts a decimal string for `uint256`. No fixture reaches that range.

**Object key order in typed data is not preserved.** py-sdk emits fields in insertion
order; polyoxide's `serde_json::Value` serialises object keys sorted. EIP-712 hashes
the `types` arrays, whose order both preserve, so signatures agree; only a byte
comparison of the JSON text would differ.

## Open items (need the live host)

- The tolerance, if any, on `validUntil` around 4 315 h.
- Whether `GET /v1/user/session-signers` answers under a session key's own
  credentials, and with which `wallet`.
- Whether balance/allowance under a session key's credentials reports the wallet's
  balance.
- Whether the owner's `list` really omits session-key orders (the page says so; the
  round trip records the answer).
````

- [ ] **Step 3: Check links and commit**

```bash
ls docs/specs/relay/openapi.yaml polyoxide-clob/tests/live_session_keys.rs 2>&1
```
The test file does not exist yet (Task 5); `openapi.yaml` must. Then:

```bash
git add docs/specs/session-keys/OBSERVED.md
git commit -m "docs(specs): record what py-sdk and ts-sdk do for session keys that the pages omit"
```

---

### Task 3: Index entries, the drift-workflow comment and `CLAUDE.md`

**Files:**
- Modify: `docs/specs/INDEX.md` (after the "Hosts with no upstream spec" section)
- Modify: `docs/specs/clob/INDEX.md` (after the "Machine-readable schema" line)
- Modify: `docs/specs/relay/INDEX.md` (after the "Machine-readable schema" line)
- Modify: `.github/workflows/nightly-schema.yml` (the "Deliberately absent" comment)
- Modify: `CLAUDE.md` (two places)

- [ ] **Step 1: `docs/specs/INDEX.md`**

After the paragraph that ends "derived from live responses rather than a vendor contract." insert:

```markdown

## Surfaces with no upstream spec

Some surfaces *are* on documented hosts but are absent from the host's published
OpenAPI. [session-keys/README.md](session-keys/README.md) records Polymarket's
Deposit Wallet and session-key contract — order signature type 3 (ERC-7739),
`GET /v1/user/session-signers` on the CLOB host, and the relayer's `type: WALLET`
dialect with `/v1/session-signers/*` and `/v1/account/transactions/*` — which
`clob/openapi.yaml` and `relay/openapi.yaml` do not mention. The contract comes from
the prose pages and the official `py-sdk` / `ts-sdk` (0.11.0);
[session-keys/OBSERVED.md](session-keys/OBSERVED.md) lists where the SDKs and the
pages part ways. There is no mirror, so `nightly-schema.yml` excludes it; the offline
fixtures under `polyoxide-{clob,relay}/tests/fixtures/session_keys/` are the drift
detector until the live round trip can run.
```

- [ ] **Step 2: `docs/specs/clob/INDEX.md` and `docs/specs/relay/INDEX.md`**

In `docs/specs/clob/INDEX.md`, directly after the line beginning `Machine-readable schema:` add:

```markdown

Not in the schema: order `signatureType` 3 (a Deposit Wallet signing an ERC-7739
envelope, as owner or session key) and `GET /v1/user/session-signers`. See
[../session-keys/README.md](../session-keys/README.md).
```

In `docs/specs/relay/INDEX.md`, directly after the line beginning `Machine-readable schema:` add:

```markdown

Not in the schema: `type: WALLET` (Deposit Wallet batches) on `/submit` and
`/deployed`, `GET /v1/account/transactions/params`, `GET /v1/account/transactions/{id}`,
`POST /v1/session-signers/authorizations` and `POST /v1/session-signers/revocations`.
See [../session-keys/README.md](../session-keys/README.md).
```

Also in `docs/specs/relay/INDEX.md`, change the description line
`Gasless transaction relay for Polymarket. Submits transactions to Polygon via Safe or Proxy wallets without requiring users to hold MATIC for gas.`
to end `via Safe, Proxy or Deposit Wallets without requiring users to hold MATIC for gas.`

- [ ] **Step 3: `.github/workflows/nightly-schema.yml`**

In the `# Deliberately absent:` comment block, after the RTDS bullet (the one ending "is nothing to diff it against either."), add:

```yaml
        #   - Deposit Wallets / session keys (docs/specs/session-keys/): the
        #     surface is absent from the published CLOB and relayer OpenAPI
        #     entirely, so the record there is prose plus SDK-generated
        #     fixtures. Nothing to diff; the fixtures are the drift check.
```

Keep the indentation identical to the surrounding bullets (eight spaces, `#`, three spaces, `-`). Run `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/nightly-schema.yml'))"` (or `uv run --with pyyaml python -c ...`) to prove the file still parses.

- [ ] **Step 4: `CLAUDE.md`**

(a) In "Key Patterns", directly after the paragraph that begins `The two EIP-712 domains are unrelated` (the one ending with "pins them against golden vectors from `py-clob-client`."), add:

```markdown

A fourth scheme, **order signature type 3**, is a Deposit Wallet signing an ERC-7739
`TypedDataSign` envelope: the exchange domain is the signing domain and the wallet's
`DepositWallet`/`1` domain rides inside the message. Getting that orientation backwards
yields a signature the venue rejects with no useful error, which is why
`polyoxide-clob/src/core/eip712.rs` pins py-sdk's golden digest. A session key adds a
6492 envelope on top. The role (owner or session key) is never inferred from
addresses; `SigningTarget::DepositWallet { wallet, role }` carries it. The whole
surface is absent from the published OpenAPI: see `docs/specs/session-keys/`.
```

(b) In "API Specs", directly after the paragraph that begins `**A mirror can match upstream and still be wrong.**`, add:

```markdown

**Deposit Wallets and session keys have no mirror at all.** `docs/specs/session-keys/`
holds the contract (`README.md`) and the SDK behaviours the pages omit (`OBSERVED.md`):
signature type 3, `GET /v1/user/session-signers`, and the relayer's `type: WALLET`
dialect with `/v1/session-signers/*` and `/v1/account/transactions/*`. The sources are
the prose pages plus `github.com/Polymarket/py-sdk` and `github.com/Polymarket/ts-sdk`
(0.11.0), and where they disagree the SDKs win. Per-route auth and timeouts live in
py-sdk's `_require_*` guards and `httpx.Timeout` values, not in its payload builders;
plan 2 got both wrong until a reviewer read `_internal/actions/session_keys.py`.
`scripts/capture_session_key_vectors.py` regenerates the fixtures from py-sdk; every
signing, encoding and wire-body test is pinned to them, never to a self-computed value.
Implemented by `polyoxide-clob` (`SigningTarget`, `Account::{with_signer,l2_only}`,
`*_with_signature`, `list_session_signers`) and `polyoxide-relay` (`WalletType::DepositWallet`,
`deposit_wallet`, `session_signers`, `resolve_wallet`, `with_auth`). The live round trip
is `polyoxide-clob/tests/live_session_keys.rs`, `#[ignore]`d and gated on a Deposit
Wallet fixture account that does not exist until prader-rs #125 is resolved.
```

- [ ] **Step 5: Check links and commit**

```bash
ls docs/specs/session-keys/README.md docs/specs/session-keys/OBSERVED.md
git add docs/specs/INDEX.md docs/specs/clob/INDEX.md docs/specs/relay/INDEX.md .github/workflows/nightly-schema.yml CLAUDE.md
git commit -m "docs: point the spec index, the drift workflow and CLAUDE.md at docs/specs/session-keys"
```

---

### Task 6 (executed after Task 3, before Task 4): `resolve_wallet` probes the Proxy too, as py-sdk does

**Why this task exists (review amendment, 2026-09-25):** plan 2 left the Proxy out of
`resolve_wallet` because `docs/specs/relay/openapi.yaml` enumerates only `SAFE` and
`WALLET` for `/deployed?type=`. Task 2's citation check found that py-sdk 0.11.0 sends
`type=PROXY` for a Proxy account (`clients/async_secure.py`, `_ensure_wallet_ready` →
`_relayer_transaction_type_for_wallet`, which maps `POLY_PROXY` → `RelayerTransactionType.PROXY`;
the enum in `models/clob/relayer.py`). py-sdk is the contract; the spec is incomplete
here as it is everywhere on this surface. `WalletKind` is `#[non_exhaustive]`, so adding
a variant is not a breaking change.

**Files:**
- Modify: `polyoxide-relay/src/wallet.rs` (`WalletKind::Proxy`, doc)
- Modify: `polyoxide-relay/src/client.rs` (`resolve_wallet` fourth candidate, doc)
- Modify: `polyoxide-relay/tests/mock_api.rs` (resolver tests)
- Modify: `docs/specs/session-keys/README.md`, `docs/specs/session-keys/OBSERVED.md`
- Modify: `docs/superpowers/specs/2026-09-25-session-keys-offline-design.md` (the `resolve_wallet` sentence in section 3 and the "Deposit Wallet address derivation" paragraph)
- Modify: `docs/superpowers/plans/2026-09-25-session-keys-relay.md` (one "Review amendment" line under Task 4, so the historical plan says why the code differs)

- [ ] **Step 1: Failing tests**

In `polyoxide-relay/tests/mock_api.rs`, in `resolve_wallet_probes_both_deposit_wallet_generations_and_the_safe`, add a fourth asserted mock on `/deployed` matching `address` = the fixture's `derivations.anvil0.proxy` (read it from `relay_vectors.json`; the test already reads the other three the same way) and `type` = `PROXY`, answering `{"deployed": false}`; assert it once like the others. In `resolve_wallet_refuses_two_deployed_wallets` and `resolve_wallet_reports_none_when_nothing_is_deployed`, change `expect(3)` to `expect(4)`. Add:

```rust
#[tokio::test]
async fn resolve_wallet_reports_a_deployed_proxy() {
    let v = relay_vectors();
    let owner: alloy::primitives::Address = v["owner"].as_str().unwrap().parse().unwrap();
    let proxy = v["derivations"]["anvil0"]["proxy"].as_str().unwrap();
    let mut server = Server::new_async().await;
    // Everything but the Proxy says "not deployed".
    let others = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![Matcher::Regex("type=(WALLET|SAFE)".into())]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":false}"#)
        .expect(3)
        .create_async()
        .await;
    let proxy_mock = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), proxy.into()),
            Matcher::UrlEncoded("type".into(), "PROXY".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":true}"#)
        .expect(1)
        .create_async()
        .await;
    let client = RelayClient::builder().unwrap().url(&server.url()).unwrap().build().unwrap();
    let kind = client.resolve_wallet(owner).await.unwrap();
    assert_eq!(kind, Some(polyoxide_relay::WalletKind::Proxy(proxy.parse().unwrap())));
    others.assert_async().await;
    proxy_mock.assert_async().await;
}
```

Run `cargo test -p polyoxide-relay --test mock_api resolve_wallet` — the new test fails to compile (`WalletKind::Proxy` missing) and the `expect(4)` tests fail once it does.

- [ ] **Step 2: Implement**

`polyoxide-relay/src/wallet.rs`: add the variant and replace the doc paragraph that begins "A Proxy wallet is not represented here":

```rust
/// A Polymarket proxy wallet. The published relayer spec enumerates only `SAFE`
/// and `WALLET` for `/deployed?type=`, but py-sdk 0.11.0 asks with `type=PROXY`
/// and this crate follows py-sdk; whether the host answers is recorded in
/// `docs/specs/session-keys/OBSERVED.md`. A Proxy auto-deploys on first use, so a
/// fresh Proxy account resolves to `None` until then; use [`derive_proxy`] when
/// you already know the account is a Proxy.
```
```rust
    /// A deployed Polymarket proxy wallet.
    Proxy(Address),
```
and `Self::DepositWallet(a) | Self::Safe(a) | Self::Proxy(a) => *a`.

`polyoxide-relay/src/client.rs`, in `resolve_wallet`, after the Safe push:

```rust
        if cfg.proxy_factory.is_some() && cfg.proxy_implementation.is_some() {
            candidates.push((
                WalletKind::Proxy(crate::wallet::derive_proxy(owner, cfg)?),
                WalletType::Proxy,
            ));
        }
```
(check that `derive_proxy` reads exactly `proxy_factory` and `proxy_implementation` and gate on what it reads). Update the doc comment: "Derives the beacon and UUPS Deposit Wallets, the Safe and the Proxy, asks `/deployed` for each … Costs up to four requests …", and drop the sentence "A Proxy wallet cannot be observed this way".

- [ ] **Step 3: Docs that said otherwise**

- `docs/specs/session-keys/README.md`: the `/deployed` row's response cell → "`{ deployed }`; the published spec enumerates `SAFE` and `WALLET`, py-sdk also sends `PROXY`"; the derivation paragraph's last sentence → "Resolving which one an owner has means deriving all four and asking `GET /deployed` for each; `resolve_wallet` does so (four requests)."; the implementation-map row for derivation → "`RelayClient::resolve_wallet -> Option<WalletKind>` over beacon, UUPS, Safe and Proxy".
- `docs/specs/session-keys/OBSERVED.md` row 10's polyoxide cell → "`resolve_wallet` probes all four candidates, sending `type=PROXY` for the Proxy as py-sdk does; whether the host honours `PROXY` (the spec omits it) is an open item"; keep the open item the implementer added.
- Design spec: find the sentences in section 3 and in "Deposit Wallet address derivation" that say a Proxy is not probed / `WalletKind { DepositWallet | Safe }` and correct them to four candidates.
- Plan 2 doc: under Task 4 append `**Review amendment (2026-09-25, plan 3 Task 6):** py-sdk queries \`/deployed?type=PROXY\`, so \`resolve_wallet\` gained a Proxy candidate and \`WalletKind::Proxy\`; the "Proxy is not probed" decision above was based on the spec's enum alone.`
- `polyoxide-relay/README.md` and `lib.rs` crate docs: grep for "Proxy" near `resolve_wallet` and fix any sentence that says it is not resolved.

- [ ] **Step 4: Verify and commit**

`cargo test -p polyoxide-relay --all-features` (all resolver tests pass), `cargo clippy -p polyoxide-relay --all-targets --all-features -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features -p polyoxide-relay`, `cargo fmt --all -- --check`. Then:

```bash
git add polyoxide-relay/src/wallet.rs polyoxide-relay/src/client.rs polyoxide-relay/tests/mock_api.rs polyoxide-relay/README.md polyoxide-relay/src/lib.rs docs/specs/session-keys/README.md docs/specs/session-keys/OBSERVED.md docs/superpowers/specs/2026-09-25-session-keys-offline-design.md docs/superpowers/plans/2026-09-25-session-keys-relay.md
git commit -m "feat(relay): resolve_wallet probes the Proxy with type=PROXY, as py-sdk does"
```

---

### Task 4: Handoff status lines and the spec's implementation order

**Files:**
- Modify: `docs/handoff-deposit-wallet-session-keys.md`
- Modify: `docs/superpowers/specs/2026-09-25-session-keys-offline-design.md`

- [ ] **Step 1: Status header**

In `docs/handoff-deposit-wallet-session-keys.md`, after the first paragraph (the one ending `is in "Interface prader consumes" at the end.`) insert:

```markdown

## Status (2026-09-25)

Every code item below is implemented on polyoxide `main` (plan 1 `02f011d`, plan 2
`5932426`; docs plan 3 follows) and pinned offline to fixtures generated from
`py-sdk` 0.11.0. The contract record is `docs/specs/session-keys/README.md`; the
design and its two breaking-changes lists are
`docs/superpowers/specs/2026-09-25-session-keys-offline-design.md`. Nothing has run
against the live host: item 12's round trip exists as an `#[ignore]`d test and waits
on the business prerequisite (prader-rs #125). Each item carries a *Status* line.
```

- [ ] **Step 2: One status line per item**

Append the following line as a new paragraph directly under the last line of each numbered item (keep the item's indentation: three spaces for items 1–9, four for 10–16). Do not edit the items' existing text.

- Item 1: `*Status:* Done, plan 1. `SigningTarget::DepositWallet { wallet, role }` on `Account`; `Clob::create_order` sets maker = signer = wallet and type 3; `sign_order` produces the 7739 wrap, pinned to py-sdk's digest and full bytes (`polyoxide-clob/tests/fixtures/session_keys/order_vectors.json`).`
- Item 2: `*Status:* Done, plan 1. Applied when `role == DepositWalletRole::SessionKey`; bytes pinned to py-sdk. The relay carries an identical copy (`polyoxide_relay::deposit_wallet::wrap_session_signer`) for batches.`
- Item 3: `*Status:* Done, plan 1. `Account::l2_only(address, credentials)` and `Account::with_signer(any alloy signer, credentials)`; the "signing target" is `SigningTarget`, set with `Account::with_target`.`
- Item 4: `*Status:* Done, plan 1. `clob_auth_typed_data`, `Clob::create_api_key_with_signature`, `Clob::derive_api_key_with_signature`; the signer is recovered locally and checked against `address` before any request. No 7739-wrapped `ClobAuth`, as this item already concluded.`
- Item 5: `*Status:* Done, plan 1. `AccountApi::list_session_signers() -> SessionSigners { wallet, signers }`; errors if the account's Deposit Wallet target disagrees with `wallet`. Answering under a session key's own credentials is an open item (`docs/specs/session-keys/OBSERVED.md`).`
- Item 6: `*Status:* Done, plan 1 (doc comments corrected; `signature_type` defaults from the account's target). The owner-vs-session probe is an open item until the live round trip runs.`
- Item 7: `*Status:* Done, plan 2. `WalletType::DepositWallet` (wire `"WALLET"`); the enum is now `#[non_exhaustive]`.`
- Item 8: `*Status:* Done, plan 2. `polyoxide_relay::deposit_wallet::{batch_typed_data, batch_digest}`, `RelayClient::get_execute_params(signer, WalletType::DepositWallet)`; five signed batches pinned to py-sdk (`polyoxide-relay/tests/fixtures/session_keys/relay_vectors.json`). The nonce is queried for the EOA that signs, owner or session key.`
- Item 9: `*Status:* Done, plan 2. `authorize_session_signer[_typed_data]` / `submit_session_signer_authorization`, `revoke_session_signer[_typed_data]` / `submit_session_signer_revocation`; bodies pinned to py-sdk's builders. Correction: revocation also accepts a Relayer API key, and both routes use a 300 s request timeout (`docs/specs/session-keys/OBSERVED.md` rows 2–3).`
- Item 10: `*Status:* Done, plan 2. `RelayClientBuilder::with_auth(AuthConfig)` builds a client with auth and no key; `BuilderAccount::with_signer` takes any alloy signer with `sign_hash` (so `BuilderAccount::signer()` now returns `&DynSigner`, a breaking change listed in the spec).`
- Item 11: `*Status:* Done, plan 2. `deposit_wallet_batch_typed_data` / `submit_deposit_wallet_batch_from` for any batch, `redeem_typed_data` / `submit_redemption_with_signature` for redemption (pUSD collateral), `deposit_wallet_trading_approvals` for the 17 approvals py-sdk requires (the page's four are a subset).`
- Item 12: `*Status:* Skeleton only, plan 3. `polyoxide-clob/tests/live_session_keys.rs` runs the round trip as an `#[ignore]`d test gated on `POLYMARKET_DW_*` and `BUILDER_*` env vars; it panics with the nightly's auth-gated wording when they are unset. No fixture account exists yet (prader-rs #125).`
- Item 13: `*Status:* Pending. Plans 1–3 are on `main`, unpushed; 0.33.0 follows plan 3 with the breaking changes listed at the end of the design spec.`
- Item 14: `*Status:* Done, plan 2 (Proxy added in plan 3). Pure derivations `derive_{safe,proxy,deposit_wallet_uups,deposit_wallet_beacon}` (pinned to py-sdk's own derivation tests) and `RelayClient::resolve_wallet(owner) -> Result<Option<WalletKind>>` over beacon, UUPS, Safe and Proxy (`type=PROXY`, as py-sdk sends; the published spec lists only `SAFE`/`WALLET`). `None` means nothing deployed; two deployed is an error.`
- Item 15: `*Status:* Done, plan 2. `AuthConfig::{Builder, RelayerApiKey}` already existed; `with_auth` accepts either without an account. Correction: only *authorization* is Builder-only; revocation takes either, as py-sdk does.`
- Item 16: `*Status:* Done, plan 2. `TransactionState` with `is_terminal` / `is_success` and `GaslessTransaction` from `GET /v1/account/transactions/{id}`. Correction: py-sdk's list is `NEW | EXECUTED | MINED | CONFIRMED | INVALID | FAILED` (no `SUBMITTED`); unknown values land in `Other(String)`.`

Also change the heading `## Interface prader consumes (the contract this design will cite)` to `## Interface prader consumes (the contract this design cites)` and, under its `relay` bullet, replace `authorize_session_signer_typed_data(dw, session_addr, scopes, valid_until, nonce, deadline)` with `authorize_session_signer_typed_data(dw, session_addr, scopes, nonce, deadline)` followed by ` (`valid_until` is computed; a `_with_valid_until` variant takes it)`, and replace `submit_session_signer_authorization(body, signature)` with `submit_session_signer_authorization(request, signature, idempotency_key)`.

- [ ] **Step 3: The design spec's implementation order**

In `docs/superpowers/specs/2026-09-25-session-keys-offline-design.md`, under `## Implementation order`, change item 2 to end `**Done 2026-09-25**, merged to \`main\` at \`5932426\` (plan \`docs/superpowers/plans/2026-09-25-session-keys-relay.md\`).` and item 3 to end `**Done 2026-09-25** (plan \`docs/superpowers/plans/2026-09-25-session-keys-docs-and-live.md\`); the live test is written, not run.` Keep the items' existing text before those additions.

- [ ] **Step 4: Commit**

```bash
git add docs/handoff-deposit-wallet-session-keys.md docs/superpowers/specs/2026-09-25-session-keys-offline-design.md
git commit -m "docs: status of every handoff item; plans 2 and 3 marked done in the design"
```

---

### Task 5: The `#[ignore]`d live round trip

**Files:**
- Modify: `polyoxide-clob/Cargo.toml` (`[dev-dependencies]`)
- Create: `polyoxide-clob/tests/live_session_keys.rs`
- Modify: `.github/workflows/nightly-behavioral.yml` (clob flags)

- [ ] **Step 1: Dev-dependency**

In `polyoxide-clob/Cargo.toml` under `[dev-dependencies]` add:

```toml
polyoxide-relay = { workspace = true }
```

`polyoxide-relay` depends on `polyoxide-core` only, so this creates no cycle. Run `cargo check -p polyoxide-clob --tests` to confirm it resolves (expect a compile of relay and a clean check).

- [ ] **Step 2: Write the test**

Create `polyoxide-clob/tests/live_session_keys.rs`:

```rust
//! Live round trip for a Deposit Wallet and one session key.
//!
//! authorize (relay, owner) → derive session credentials (clob L1, session key)
//! → place a resting GTC (clob, session key) → list from the session key and from
//! the owner → cancel (session key) → revoke (relay, owner) → confirm the key is
//! gone from `GET /v1/user/session-signers`.
//!
//! Gated behind `#[ignore]` and a Deposit Wallet fixture account that does not
//! exist yet: session-key management is enabled per Builder API key by the venue
//! (prader-rs #125). Until it exists this file only has to compile. Run with
//!
//! ```sh
//! cargo test -p polyoxide-clob --test live_session_keys -- --ignored
//! ```
//!
//! Environment (a `.env` is picked up by dotenvy):
//!
//! | Variable | Meaning |
//! |---|---|
//! | `POLYMARKET_DW_OWNER_PRIVATE_KEY` | The owner EOA's hex key. |
//! | `POLYMARKET_DW_WALLET` | The Deposit Wallet address (resolve it with `RelayClient::resolve_wallet`). |
//! | `POLYMARKET_DW_SESSION_PRIVATE_KEY` | A fresh EOA the test authorizes and then revokes. |
//! | `BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE` | Builder HMAC credentials, enabled for session-key management. |
//!
//! When any is missing the test panics with the wording the nightly classifier
//! treats as auth-gated (`AUTH_GATED_RE` in `.github/scripts/classify_failures.py`),
//! so the nightly logs and skips it instead of filing an issue.

use std::time::{Duration, Instant};

use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer as _;
use polyoxide_clob::{
    Account, Clob, ClobBuilder, CreateOrderParams, Credentials, DepositWalletRole, OrderKind,
    OrderSide, SessionSignerScope, SigningTarget,
};
use polyoxide_gamma::Gamma;
use polyoxide_relay::{BuilderAccount, BuilderConfig, RelayClient, WalletType};
use rust_decimal::Decimal;

/// How long to wait for the relayer to report a session-signer batch terminal
/// and for the registry to reflect it. The venue allows five minutes.
const REGISTRY_TIMEOUT: Duration = Duration::from_secs(6 * 60);
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const MAX_BOOK_PROBES: usize = 10;

struct Fixture {
    owner: PrivateKeySigner,
    wallet: Address,
    session: PrivateKeySigner,
    builder: BuilderConfig,
}

/// Load the fixture account, or panic in the auth-gated wording. Never
/// soft-skip: a test that asserted nothing must not report `ok`.
fn load_fixture() -> Fixture {
    dotenvy::dotenv().ok();
    let var = |name: &str| {
        std::env::var(name).unwrap_or_else(|_| {
            panic!(
                "POLYMARKET_* env vars required for the Deposit Wallet round trip \
                 ({name} unset; see docs/specs/session-keys/README.md, Live verification)"
            )
        })
    };
    let owner: PrivateKeySigner = var("POLYMARKET_DW_OWNER_PRIVATE_KEY")
        .parse()
        .expect("POLYMARKET_DW_OWNER_PRIVATE_KEY is a hex private key");
    let wallet: Address = var("POLYMARKET_DW_WALLET")
        .parse()
        .expect("POLYMARKET_DW_WALLET is an address");
    let session: PrivateKeySigner = var("POLYMARKET_DW_SESSION_PRIVATE_KEY")
        .parse()
        .expect("POLYMARKET_DW_SESSION_PRIVATE_KEY is a hex private key");
    let builder = BuilderConfig::new(
        var("BUILDER_API_KEY"),
        var("BUILDER_SECRET"),
        std::env::var("BUILDER_PASS_PHRASE").ok(),
    );
    Fixture {
        owner,
        wallet,
        session,
        builder,
    }
}

/// Derive (or create) L2 credentials for `signer` under plain L1 auth, then build
/// a clob client whose orders are signed for the Deposit Wallet in `role`.
async fn clob_for(signer: &PrivateKeySigner, wallet: Address, role: DepositWalletRole) -> Clob {
    // L1 needs a signer and no credentials; the placeholder is never sent.
    let placeholder = Credentials {
        key: String::new(),
        secret: String::new(),
        passphrase: String::new(),
    };
    let l1 = ClobBuilder::new()
        .with_account(Account::with_signer(signer.clone(), placeholder))
        .build()
        .expect("L1 client");
    let derived = match l1.auth().expect("auth").derive_api_key(0).send().await {
        Ok(creds) => creds,
        Err(_) => l1
            .auth()
            .expect("auth")
            .create_api_key(0)
            .send()
            .await
            .expect("create api key"),
    };
    let credentials = Credentials {
        key: derived.api_key,
        secret: derived.secret,
        passphrase: derived.passphrase,
    };
    ClobBuilder::new()
        .with_account(
            Account::with_signer(signer.clone(), credentials)
                .with_target(SigningTarget::DepositWallet { wallet, role }),
        )
        .build()
        .expect("deposit wallet clob client")
}

fn relay_for_owner(fx: &Fixture) -> RelayClient {
    let account = BuilderAccount::with_signer(fx.owner.clone(), Some(polyoxide_relay::AuthConfig::Builder(fx.builder.clone())));
    RelayClient::builder()
        .expect("relay builder")
        .with_account(account)
        .wallet_type(WalletType::DepositWallet)
        .deposit_wallet(fx.wallet)
        .build()
        .expect("relay client")
}

/// Wait until the owner's session-signer list does (`present == true`) or does
/// not contain `session`, or fail after `REGISTRY_TIMEOUT`.
async fn wait_for_registry(owner_clob: &Clob, session: Address, present: bool) {
    let started = Instant::now();
    loop {
        let listed = owner_clob
            .account_api()
            .expect("account api")
            .list_session_signers()
            .await
            .expect("list session signers");
        let found = listed.signers.iter().any(|s| s.address == session);
        if found == present {
            return;
        }
        assert!(
            started.elapsed() < REGISTRY_TIMEOUT,
            "session signer {session} {} in the registry after {:?}: {listed:?}",
            if present { "never appeared" } else { "is still" },
            started.elapsed()
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Wait for a relayer transaction to reach a terminal state and assert success.
async fn wait_for_transaction(relay: &RelayClient, id: &str) {
    let started = Instant::now();
    loop {
        let tx = relay
            .get_gasless_transaction(id)
            .await
            .expect("get gasless transaction");
        if tx.state.is_terminal() {
            assert!(tx.state.is_success(), "transaction {id} failed: {tx:?}");
            return;
        }
        assert!(
            started.elapsed() < REGISTRY_TIMEOUT,
            "transaction {id} not terminal after {:?}: {tx:?}",
            started.elapsed()
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// An open market whose CLOB best ask sits strictly above `min_ask`, so a bid at
/// `min_ask` rests. Same selection as `live_api.rs`; selected on the URL it is
/// asserted on, never on gamma's cached figure alone.
async fn find_token_id_with_min_ask(min_ask: Decimal) -> String {
    let gamma = Gamma::builder().build().expect("gamma client");
    let markets = gamma
        .markets()
        .list()
        .closed(false)
        .limit(100)
        .send()
        .await
        .expect("gamma list markets");
    let clob = Clob::public();
    let mut probed = 0usize;
    for market in markets.iter() {
        let gamma_ask = market.best_ask.and_then(|ask| Decimal::try_from(ask).ok());
        if gamma_ask.is_none_or(|ask| ask <= min_ask) {
            continue;
        }
        let token_id = market.clob_token_ids.as_ref().and_then(|ids| {
            serde_json::from_str::<Vec<String>>(ids)
                .ok()
                .and_then(|v| v.into_iter().next())
        });
        let Some(token_id) = token_id else {
            continue;
        };
        probed += 1;
        if let Ok(book) = clob.markets().order_book(&token_id).send().await {
            let best_ask = book.asks.iter().map(|level| level.price).min();
            if best_ask.is_some_and(|ask| ask > min_ask) {
                return token_id;
            }
        }
        if probed >= MAX_BOOK_PROBES {
            break;
        }
    }
    panic!("no suitable market: no open market with a best ask above {min_ask} in {probed} books");
}

#[tokio::test]
#[ignore] // live; needs a Deposit Wallet fixture account and an enabled Builder key
async fn live_session_key_round_trip() {
    let fx = load_fixture();
    let session_address = fx.session.address();
    let relay = relay_for_owner(&fx);
    let owner_clob = clob_for(&fx.owner, fx.wallet, DepositWalletRole::Owner).await;

    // 1. Authorize the session key (owner signs the batch; Builder HMAC submits it).
    let submitted = relay
        .authorize_session_signer(session_address, vec![SessionSignerScope::Clob])
        .await
        .expect("authorize session signer");
    assert!(
        !submitted.status.is_terminal_failure(),
        "authorization refused: {submitted:?}"
    );
    wait_for_transaction(&relay, &submitted.transaction_id).await;
    wait_for_registry(&owner_clob, session_address, true).await;

    // 2. The session key derives its own credentials and places a resting order.
    let session_clob = clob_for(&fx.session, fx.wallet, DepositWalletRole::SessionKey).await;
    let min_ask = Decimal::new(1, 2);
    let token_id = find_token_id_with_min_ask(min_ask).await;
    const RESTING_PRICE: f64 = 0.01;
    let params = CreateOrderParams {
        token_id,
        price: RESTING_PRICE,
        size: 5.0,
        side: OrderSide::Buy,
        order_type: OrderKind::Gtc,
        post_only: true,
        expiration: None,
        funder: None,
        // Defaults to type 3 from the account's target.
        signature_type: None,
    };
    let resp = session_clob
        .place_order(&params, None)
        .await
        .expect("place order as session key");
    assert!(resp.success, "session-key order rejected: {:?}", resp.error_msg);
    let order_id = resp.order_id.expect("accepted order must return an id");

    // 3. Visibility, both directions. The page says the owner cannot see it; the
    //    assertion is one-directional and the owner's answer is only recorded.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let from_session = session_clob
        .orders()
        .expect("orders")
        .list()
        .send()
        .await
        .expect("list as session key");
    assert!(
        from_session.data.iter().any(|o| o.id == order_id),
        "session key cannot see its own order {order_id}: {from_session:?}"
    );
    let from_owner = owner_clob
        .orders()
        .expect("orders")
        .list()
        .send()
        .await
        .expect("list as owner");
    eprintln!(
        "visibility: owner {} order {order_id} placed by the session key",
        if from_owner.data.iter().any(|o| o.id == order_id) {
            "SEES"
        } else {
            "does not see"
        }
    );

    // 4. Cancel with the key that placed it.
    let cancelled = session_clob
        .orders()
        .expect("orders")
        .cancel(order_id.clone())
        .send()
        .await
        .expect("cancel as session key");
    assert!(
        cancelled.canceled.contains(&order_id),
        "cancel did not report {order_id}: {cancelled:?}"
    );

    // 5. Revoke and confirm the key leaves the registry.
    let revoked = relay
        .revoke_session_signer(session_address)
        .await
        .expect("revoke session signer");
    assert!(
        !revoked.status.is_terminal_failure(),
        "revocation refused: {revoked:?}"
    );
    wait_for_registry(&owner_clob, session_address, false).await;
}
```

Notes for the implementer:
- `BuilderAccount::with_signer` takes `(signer, Option<AuthConfig>)`; check its exact signature in `polyoxide-relay/src/account.rs` and adjust the `relay_for_owner` line. `AuthConfig::Builder(BuilderConfig)` is the variant name used in the relay mock tests.
- If `Account::with_signer` requires `S: 'static` and `PrivateKeySigner` is `Clone`, `signer.clone()` is right. If `derive_api_key(0)` / `create_api_key(0)` are not `Request<ApiKeyResponse>` with `.send()`, match what `polyoxide-clob/src/api/auth.rs` exposes.
- `market.best_ask` and `market.clob_token_ids` are what `polyoxide-clob/tests/live_api.rs` uses; copy its exact field access if these differ.
- `SessionSignerAuthorizationStatus::is_terminal_failure` and `SessionSignerRevocationStatus::is_terminal_failure` exist (plan 2 Task 7).
- Do NOT add a soft-skip. Do NOT run the test against the live host.

- [ ] **Step 3: Compile it**

```bash
cargo test -p polyoxide-clob --features ws,keychain --test live_session_keys
```
Expected: builds; `running 0 tests` … `1 ignored`. Then `cargo clippy -p polyoxide-clob --all-targets --all-features -- -D warnings` must be clean (the test binary is a target).

- [ ] **Step 4: Nightly matrix**

In `.github/workflows/nightly-behavioral.yml`, change the clob matrix row from
`- { crate: polyoxide-clob,  flags: "--features ws --test live_api --test live_ws" }`
to
`- { crate: polyoxide-clob,  flags: "--features ws --test live_api --test live_ws --test live_session_keys" }`.
The new test panics in the auth-gated wording, so the nightly logs and skips it. Confirm the YAML still parses.

- [ ] **Step 5: Full gates and commit**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
cargo check --workspace --all-features
cargo test --workspace --all-features
```
Expected: each exits 0; the workspace totals are unchanged from `5932426` apart from one more ignored test. Then:

```bash
git add polyoxide-clob/Cargo.toml Cargo.lock polyoxide-clob/tests/live_session_keys.rs .github/workflows/nightly-behavioral.yml
git commit -m "test(clob): ignored live round trip for a Deposit Wallet session key"
```

---

## Not in this plan

- Running the live test (blocked on prader-rs #125).
- Python bindings and the CLI.
- The 0.33.0 release (push `main`, check origin's latest tag first, bump the workspace version, CHANGELOG from the two breaking-changes lists in the spec).
