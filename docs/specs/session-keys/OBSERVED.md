# Deposit Wallets and session keys: what the SDKs do that the pages do not say

`README.md` beside this file is the contract as the prose pages and the official SDKs
state it. This file records where the pages are silent, vague or contradicted by the
SDKs, with a citation into `polymarket-client==0.11.0` (py-sdk) or
`@polymarket/client` 0.11.0 (ts-sdk) for each. Apart from row 10's `/deployed` probe
(observed 2026-09-25 with unauthenticated GETs), nothing here has been observed against
the live host: session-key management is gated per Builder API key and ours is not yet
enabled (prader-rs #125). When the live round trip in
`polyoxide-clob/tests/live_session_keys.rs` runs, move each confirmed row into
`README.md` and each contradicted one into a new "Contradicted live" section here.

## Pages versus SDKs

| # | Page says | SDKs do | polyoxide |
|---|---|---|---|
| 1 | `validUntil` is "now + 180 days"; "other values are rejected" | Exactly `4_315 * 60 * 60` s (≈179.8 d), computed at call time and not a parameter (py-sdk `_internal/actions/session_keys.py:55`, `:350-353`; ts-sdk `packages/client/src/actions/session-keys.ts:135`, `:249-250`) | `SESSION_KEY_LIFETIME_SECS`; `authorize_session_signer_typed_data` computes it; a `_with_valid_until` variant exists for tests and for the day the tolerance is known |
| 2 | Nothing about request timeouts | Both session-signer POSTs wait 300 s, because the venue "synchronously validate[s], simulate[s], persist[s], and broadcast[s]" (py-sdk `session_keys.py:56-62`, used at `:506`; ts-sdk `session-keys.ts:137`, `:277`, `:412`) | `SESSION_SIGNER_REQUEST_TIMEOUT` = 300 s, per request, overriding the 30 s client default |
| 3 | Both routes use the Builder HMAC headers | Authorization requires a Builder key; revocation accepts a Builder key **or** a Relayer API key (py-sdk `session_keys.py:200` vs `:287`, definitions at `:432-442`) | Authorization refuses a Relayer API key before I/O; revocation accepts either |
| 4 | Transaction states are `STATE_NEW`, `STATE_SUBMITTED`, `STATE_CONFIRMED`, `STATE_FAILED`, `STATE_INVALID` | py-sdk models `STATE_NEW`, `STATE_EXECUTED`, `STATE_MINED`, `STATE_CONFIRMED`, `STATE_INVALID`, `STATE_FAILED` and no `STATE_SUBMITTED` (`models/clob/relayer.py:22-27`) | `TransactionState` carries py-sdk's six plus `Other(String)`, so an unlisted value is preserved, never an error |
| 5 | Four approvals make a wallet ready to trade | `_required_trading_approvals` yields 7 ERC-20 and 10 ERC-1155 approvals (py-sdk `_internal/actions/relayer/approvals.py:134-219`; addresses in `environments.py`) | `deposit_wallet_trading_approvals` returns all 17 in py-sdk's order, pinned to `relay_vectors.json` → `trading_approvals` |
| 6 | `metadata` is optional on `/submit` | Always sent, `""` by default, at most 500 characters (py-sdk `_internal/actions/relayer/gasless.py:54`, `:68`, `:82-83`, `:230`) | Deposit Wallet submits always carry it and refuse longer values before I/O; Safe and Proxy bodies are unchanged |
| 7 | Revocation "cancels open orders" | py-sdk returns as soon as the key has left the active-key registry, polling the registry until it has; the remaining revocation work (per the page, the on-chain revocation and the cancel-all) continues in the backend ("This eager return is intentional", py-sdk `session_keys.py:306-308`, poll at `:620-636`). The response carries `fenced: bool` (`:153`) | `SessionSignerRevocationResponse::fenced`. `revoke_session_signer` returns the venue's answer without polling the registry; a caller who needs py-sdk's guarantee checks `fenced` or lists the session signers |
| 8 | `COMBOSRFQ` is a valid scope | Both SDKs refuse Combos RFQ with a session key regardless of scope: "Combos is not supported with Session Keys" (py-sdk `_internal/actions/combo_rfq.py:823-824`; ts-sdk `packages/client/src/actions/rfq.ts:1438-1439`) | polyoxide has no Combos RFQ client, so nothing to refuse; recorded so a future one does |
| 9 | Nothing about how to tell an owner from a session key | When the wallet cannot be derived from the signer, both SDKs *assume* a session key on a Deposit Wallet ("TEMP: Default to the Deposit Wallet session-signature path…", py-sdk `_internal/wallet.py:161-163`; ts-sdk `packages/client/src/wallet.ts:367-372`) | Not copied. The role is explicit (`SigningTarget::DepositWallet { wallet, role }`, `RelayClientBuilder::deposit_wallet_role`), because a wrong guess signs with the wrong envelope, which cannot verify for the wallet |
| 10 | `type` on `GET /deployed` is `SAFE` (the default) or `WALLET` (`../relay/openapi.yaml:721-730`) | The SDKs disagree on a Proxy wallet. py-sdk sends `type=PROXY`, a value the spec does not list (`clients/async_secure.py:2885-2889`, mapping at `:3913-3922`). ts-sdk sends `WALLET` for a Deposit Wallet and omits `type` otherwise, so it asks about a Proxy as if it were a Safe (`packages/client/src/actions/gasless.ts:214-227`). Observed 2026-09-25: the relayer answers every `type` other than `WALLET` (including `PROXY` and an unknown value) identically, true for a deployed Safe or Proxy and false for a Deposit Wallet; `WALLET` answers true for all three. | `resolve_wallet` probes all four candidates, sending `type=PROXY` for the Proxy as py-sdk does |
| 11 | The deposit-wallets page names redemption only as a link ("Split, merge, or redeem tokens", to Manage Positions). The Manage Positions page's API tab does name the target: `CtfCollateralAdapter` when `negRisk` is false, `NegRiskCtfCollateralAdapter` when true, pUSD collateral, index sets `[1, 2]` (`trading/positions/manage.md`, read 2026-09-25). No disagreement; recorded because polyoxide got it wrong | Both SDKs agree with that page. py-sdk builds `ctf_redeem_positions_call(ctf=context.adapter_address, collateral=<pUSD>, …)` (`clients/secure.py:2868-2876`, async twin `clients/async_secure.py:3234-3242`), with `adapter_address` the neg-risk collateral adapter when `neg_risk` and the collateral adapter otherwise (`_internal/actions/relayer/positions.py:109`, addresses in `environments.py:94-95`), index sets `[1, 2]` (`_internal/actions/relayer/calls.py:47`). ts-sdk does the same (`packages/client/src/actions/positions.ts:1184-1186`, `:1273-1281`). A protocol-V2 market goes through the V2 router instead | `redeem_typed_data` / `submit_deposit_wallet_redemption` take `neg_risk` and pick the adapter; plan 2 targeted the CTF until the plan 3 review. `submit_deposit_wallet_redemption` sends `metadata: ""` where py-sdk's `redeem_positions` defaults to `Redeem positions for condition <id>`; the batch signature does not cover `metadata` |

## Behaviour the pages omit entirely

**Retries reuse the idempotency key.** py-sdk retries a session-signer POST up to twice
on 429, 5xx and transport errors with the *same* `Idempotency-Key`
(`session_keys.py:54`, `_SESSION_KEY_SUBMISSION_MAX_RETRIES`; loop at `:500-516`,
classifier at `:549-552`); ts-sdk generates a random UUID per call when none is given
(`session-keys.ts:273-274`, `:408-409`). polyoxide's conveniences generate a fresh
UUID v4 and retry only 429 and 425 (with that same key), so a caller who wants
py-sdk's retry shape uses the two-step API (`*_typed_data` then `submit_*` with its
own key) and resends with the same key.

**Session keys are managed only by the owner, only for a Deposit Wallet.** py-sdk
refuses before any request when the account is not a Deposit Wallet or the signer is
not its owner (`session_keys.py:425-429`, called at `:199` and `:286`). polyoxide's
`authorize_session_signer` and `revoke_session_signer` refuse under a session-key role
the same way.

**The nonce belongs to the signing EOA.** `GET /v1/account/transactions/params` is
queried with the address that will sign the batch: the owner for owner batches, the
session key for session-key batches. py-sdk's `build_signed_deposit_wallet_batch`
queries the params with `ctx.signer.address` and wraps the signature with the same
address (`_internal/actions/relayer/gasless.py:179-197`);
`build_signed_deposit_wallet_payload` puts that address in the envelope's `from` field
(`gasless.py:162`, `:226`). polyoxide's `execute` does the same with the account's
address.

**A batch `value` is an integer in the SDKs' typed data.** py-sdk emits `call.value`
as a Python int of any size (`_internal/actions/relayer/signing/deposit_wallet.py:63`;
the field is `value: int` at `_internal/actions/relayer/calls.py:54`). polyoxide emits
a JSON number up to `u64::MAX` wei and a decimal string above that, because
`serde_json` cannot hold a larger integer exactly and a rounded value would sign a
different batch than the digest hashes. alloy and eth_account both accept a decimal
string for `uint256`. No fixture reaches that range.

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
- Whether py-sdk's adapter-and-pUSD redemption is also what Safe and Proxy accounts
  should use now. polyoxide's legacy path for them is USDC on the Conditional Tokens
  contract.
