#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["polymarket-client==0.11.0"]
#
# [tool.uv]
# exclude-newer = "2026-09-25T00:00:00Z"
# ///
"""Generate ERC-7739 / session-key golden vectors from Polymarket's official py-sdk.

polyoxide pins its Deposit Wallet (signature type 3) signing against these bytes.
The fixture order is py-sdk's own golden fixture (tests/unit/test_order_typed_data_golden.py),
signed with Anvil key #0; the session signer is Anvil address #1.

Also generates `clob_auth.json`, pinning the L1 auth (`ClobAuth`) typed data and its
signature — used to verify `clob_auth_typed_data` and the signature-in auth path against
py-sdk's own `build_api_key_auth_typed_data`, rather than only against our own EIP-712
implementation of the same struct.

Also generates `relay_vectors.json`, pinning Deposit Wallet `Batch` signing (approve,
authorize/revoke session signer, CTF redeem), CREATE2 wallet derivations, and the relay
submit/authorization request bodies — used by `polyoxide-relay`.

This script imports py-sdk's private modules (`polymarket._internal...`), which are not a
stable public API, so it is expected to need import-path fixes after a py-sdk upgrade.

Usage:
    uv run scripts/capture_session_key_vectors.py \
        polyoxide-clob/tests/fixtures/session_keys polyoxide-relay/tests/fixtures/session_keys

Writes `order_vectors.json`, `clob_auth.json` and `PROVENANCE.md` to the first directory,
and `relay_vectors.json` and `PROVENANCE.md` to the second. Re-run after a py-sdk upgrade;
review the diff before committing.
"""
import importlib.metadata
import json
import re
import sys
import tomllib
from pathlib import Path

from eth_account import Account
from eth_account.messages import encode_typed_data
from eth_utils.crypto import keccak
from polymarket._internal.actions.orders.typed_data import (
    _app_domain_separator,
    _order_contents_hash,
    build_order_signature,
    build_order_typed_data,
)
from polymarket._internal.actions.orders.types import BYTES32_ZERO, UnsignedOrder
from polymarket._internal.actions.relayer.calls import (
    authorize_session_signer_call,
    ctf_redeem_positions_call,
    erc20_approval_call,
    revoke_session_signer_call,
)
from polymarket._internal.actions.relayer.gasless import build_deposit_wallet_payload
from polymarket._internal.actions.relayer.signing.deposit_wallet import (
    build_deposit_wallet_typed_data,
    sign_deposit_wallet_batch,
)
from polymarket._internal.environment import PRODUCTION_CONFIG
from polymarket._internal.l1_auth import build_api_key_auth_typed_data
from polymarket._internal.wallet import (
    derive_beacon_deposit_wallet_address,
    derive_proxy_wallet_address,
    derive_safe_wallet_address,
    derive_uups_deposit_wallet_address,
    wrap_deposit_wallet_session_signer_signature,
)
from polymarket.models.types import TokenId
from polymarket.types import EvmAddress, HexString

ANVIL_KEY_0 = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
ANVIL_ADDR_1 = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
DEPOSIT_WALLET = EvmAddress("0x57ffbc34de23124faeb8387fcd689d314e57accd")
EXCHANGES = {
    # py-sdk's golden fixture uses the V1 exchange address; keep it so the digest
    # cross-checks against their pinned value.
    "v1_exchange": "0x4bfb41d5b3570defd03c39a9a4d8de6bd8b8982e",
    # polyoxide's Polygon mainnet CTF Exchange V2, what the client signs against.
    "v2_exchange": "0xE111180000d2663C0091e4f400237545B87B996B",
}

# Fixed inputs for the L1 auth (`ClobAuth`) fixture. Same chain id, timestamp and nonce
# as the existing `REFERENCE_SIGNATURE` golden in eip712.rs's
# `clob_auth_reference_tests`, produced independently from py-clob-client +
# poly_eip712_structs — the two signatures should agree.
CLOB_AUTH_CHAIN_ID = 137
CLOB_AUTH_TIMESTAMP = 1700000000
CLOB_AUTH_NONCE = 42

# py-sdk's own pinned digest for this exact order, from its
# tests/unit/test_order_typed_data_golden.py. If this stops matching, either py-sdk's
# signing bytes changed upstream or our fixture no longer mirrors theirs — investigate
# before trusting anything else this script produces.
PY_SDK_GOLDEN_DIGEST = "0x1b9566eedd9589a73275df23a3a9d9e2e9897e76d31cd46d436f1b824d161b33"

# Fixed inputs for the relay (Deposit Wallet Batch / derivation) fixtures.
SIGNER_ONE = "0x0000000000000000000000000000000000000001"
PUSD = "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB"
CTF = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
EXCHANGE_V2 = "0xE111180000d2663C0091e4f400237545B87B996B"
MAX_UINT256 = (1 << 256) - 1
CONDITION_ID = "0x1171bfba0ad9386688133910593527fe77ce5406a7ac2c9a3552ab5471c1ac51"
# py-sdk's own golden: tests/unit/test_wallet_derivations.py EXPECTED_UUPS_DEPOSIT.
PY_SDK_GOLDEN_UUPS_FOR_SIGNER_ONE = "0x57ffbc34de23124faeb8387fcd689d314e57accd"


def fixture(exchange: str) -> UnsignedOrder:
    return UnsignedOrder(
        builder=BYTES32_ZERO,
        chain_id=137,
        exchange_address=EvmAddress(exchange),
        expiration=0,
        maker=DEPOSIT_WALLET,
        maker_amount=1_000_000,
        metadata=BYTES32_ZERO,
        order_type="GTC",
        salt=1,
        side="BUY",
        signature_type=3,
        signer=DEPOSIT_WALLET,
        taker_amount=500_000,
        timestamp=0,
        token_id=TokenId("1"),
    )


def digest(typed_data: dict) -> str:
    signable = encode_typed_data(full_message=typed_data)
    return "0x" + keccak(b"\x19\x01" + signable.header + signable.body).hex()


def clob_auth_fixture(signer: Account) -> dict:
    """L1 auth (`ClobAuth`) typed data and its signature, from py-sdk's own builder."""
    typed_data = build_api_key_auth_typed_data(
        address=signer.address,
        chain_id=CLOB_AUTH_CHAIN_ID,
        timestamp=CLOB_AUTH_TIMESTAMP,
        nonce=CLOB_AUTH_NONCE,
    )
    signature = "0x" + signer.sign_typed_data(full_message=typed_data).signature.hex()
    return {
        "address": signer.address,
        "chain_id": CLOB_AUTH_CHAIN_ID,
        "timestamp": CLOB_AUTH_TIMESTAMP,
        "nonce": CLOB_AUTH_NONCE,
        "typed_data": typed_data,
        "signature": signature,
    }


def _hexify(value):
    """Recursively turn `bytes` into `0x`-prefixed hex strings so a structure is JSON-safe.

    `build_deposit_wallet_typed_data`'s `message.calls[*].data` comes back as raw
    `bytes` (unlike the order-signing typed data, which is already JSON-safe); `digest()`
    is computed against that raw structure, and this is applied only to the copy that gets
    written out.
    """
    if isinstance(value, (bytes, bytearray)):
        return "0x" + value.hex()
    if isinstance(value, dict):
        return {k: _hexify(v) for k, v in value.items()}
    if isinstance(value, list):
        return [_hexify(v) for v in value]
    return value


def _derivations(signer: str, cfg) -> dict:
    return {
        "signer": signer,
        "uups": derive_uups_deposit_wallet_address(signer, cfg),
        "beacon": derive_beacon_deposit_wallet_address(signer, cfg),
        "safe": derive_safe_wallet_address(signer, cfg),
        "proxy": derive_proxy_wallet_address(signer, cfg),
    }


def _batch(signer, wallet: EvmAddress, calls, nonce: str, deadline: str, session: str) -> dict:
    typed_data = build_deposit_wallet_typed_data(
        wallet=wallet, calls=calls, nonce=nonce, deadline=deadline, chain_id=137
    )
    signature = sign_deposit_wallet_batch(
        signer, wallet=wallet, calls=calls, nonce=nonce, deadline=deadline, chain_id=137
    )
    return {
        "typed_data": _hexify(typed_data),
        "digest": digest(typed_data),
        "signature": signature,
        "session_signature": wrap_deposit_wallet_session_signer_signature(
            EvmAddress(session), HexString(signature)
        ),
        "calls": [{"target": str(c.to), "value": str(c.value), "data": c.data} for c in calls],
        "nonce": nonce,
        "deadline": deadline,
    }


def relay_vectors(signer) -> dict:
    """Deposit Wallet `Batch` signing, CREATE2 derivations, and relay request bodies.

    The owner is Anvil key #0 (same signer as the order-signing fixtures); the wallet
    is its beacon-generation Deposit Wallet. `derivations.signer_one` cross-checks against
    py-sdk's own `tests/unit/test_wallet_derivations.py` golden, asserted by the caller
    before anything here is written.
    """
    cfg = PRODUCTION_CONFIG.wallet_derivation
    owner = signer.address
    wallet = EvmAddress(derive_beacon_deposit_wallet_address(owner, cfg))
    approval = erc20_approval_call(
        token_address=EvmAddress(PUSD), spender=EvmAddress(EXCHANGE_V2), amount=MAX_UINT256
    )
    authorize = authorize_session_signer_call(
        wallet_address=wallet, session_signer=EvmAddress(ANVIL_ADDR_1), valid_until=1815534000
    )
    revoke = revoke_session_signer_call(wallet_address=wallet, session_signer=EvmAddress(ANVIL_ADDR_1))
    redeem = ctf_redeem_positions_call(
        ctf=EvmAddress(CTF), collateral=EvmAddress(PUSD), condition_id=CONDITION_ID
    )
    approval_batch = _batch(signer, wallet, [approval], "3", "1800000000", ANVIL_ADDR_1)
    authorize_batch = _batch(signer, wallet, [authorize], "4", "1800000600", ANVIL_ADDR_1)
    return {
        "owner": owner,
        "wallet": wallet,
        "session_signer": ANVIL_ADDR_1,
        "chain_id": 137,
        "config": {
            "deposit_wallet_factory": cfg.deposit_wallet_factory,
            "deposit_wallet_beacon": cfg.deposit_wallet_beacon,
            "deposit_wallet_implementation": cfg.deposit_wallet_implementation,
            "proxy_factory": cfg.proxy_factory,
            "proxy_implementation": cfg.proxy_implementation,
            "safe_factory": cfg.safe_factory,
            "safe_init_code_hash": cfg.safe_init_code_hash,
        },
        "derivations": {
            "signer_one": _derivations(SIGNER_ONE, cfg),
            "anvil0": _derivations(owner, cfg),
        },
        "approval_batch": approval_batch,
        "authorize_batch": authorize_batch,
        "revoke_batch": _batch(signer, wallet, [revoke], "5", "1800000600", ANVIL_ADDR_1),
        "redeem_batch": _batch(signer, wallet, [redeem], "6", "1800000600", ANVIL_ADDR_1),
        "submit_body": dict(build_deposit_wallet_payload(
            signer_address=owner,
            deposit_wallet_factory=cfg.deposit_wallet_factory,
            wallet=wallet,
            calls=[approval],
            nonce="3",
            deadline="1800000000",
            signature=approval_batch["signature"],
            metadata="",
        )),
        "authorization_body": {
            "deadline": "1800000600",
            "nonce": "4",
            "scopes": ["CLOB"],
            "sessionSignerAddress": ANVIL_ADDR_1,
            "signature": authorize_batch["signature"],
            "validUntil": "1815534000",
            "walletAddress": wallet,
        },
    }


# The canonical PEP 723 block-extraction regex, straight from the spec.
_PEP_723_BLOCK_RE = re.compile(
    r"(?m)^# /// (?P<type>[a-zA-Z0-9-]+)$\s(?P<content>(^#(| .*)$\s)+)^# ///$"
)


def read_dependency_cutoff() -> str:
    """Read `[tool.uv].exclude-newer` from this file's own PEP 723 metadata block.

    Keeps PROVENANCE.md's stated cutoff in sync with the pin that actually governs
    dependency resolution, instead of duplicating the date as a second literal.
    """
    match = _PEP_723_BLOCK_RE.search(Path(__file__).read_text())
    content = "".join(
        line[2:] if line.startswith("# ") else line[1:]
        for line in match.group("content").splitlines(keepends=True)
    )
    metadata = tomllib.loads(content)
    return metadata["tool"]["uv"]["exclude-newer"]


def order_provenance(order: UnsignedOrder) -> dict:
    return {
        "chain_id": order.chain_id,
        "salt": order.salt,
        "token_id": str(order.token_id),
        "maker_amount": order.maker_amount,
        "taker_amount": order.taker_amount,
        "side": order.side,
        "signature_type": order.signature_type,
        "timestamp": order.timestamp,
        "expiration": order.expiration,
        "metadata": order.metadata,
        "builder": order.builder,
    }


def write_provenance(out_dir: Path, sdk_version: str, dependency_cutoff: str) -> None:
    text = f"""# Deposit Wallet (session-key) order-signing vectors

Generated by `scripts/capture_session_key_vectors.py` (run:
`uv run scripts/capture_session_key_vectors.py polyoxide-clob/tests/fixtures/session_keys`)
against Polymarket's official Python SDK, `polymarket-client=={sdk_version}` — not a live
host. Dependency resolution is pinned to `exclude-newer = {dependency_cutoff}` (the
script's own PEP 723 metadata), so a rerun reproduces byte-identical output regardless of
the date it is run, until that cutoff or the pinned `polymarket-client` version changes.

The order is py-sdk's own golden fixture (`tests/unit/test_order_typed_data_golden.py`),
signed with Anvil key #0. Generation asserts the `v1_exchange` vector's `envelope_digest`
against py-sdk's pinned value before writing anything, so this fixture cannot drift from
theirs silently.

`session_signature` is a byte-pinning vector only, not a signature a real session key
would produce: the session-signer envelope names Anvil address #1 as the session signer,
but Anvil key #0 produced the inner order signature. The two identities are deliberately
mismatched so the fixture can pin bytes without coordinating two live signers.

`clob_auth.json` pins the L1 auth (`ClobAuth`) typed data — from py-sdk's own
`build_api_key_auth_typed_data`, not polyoxide's reimplementation of the same struct — and
its signature over Anvil key #0, at chain id 137, timestamp 1700000000, nonce 42.

| Fixture | Command |
|---|---|
| `order_vectors.json` | `uv run scripts/capture_session_key_vectors.py polyoxide-clob/tests/fixtures/session_keys` |
| `clob_auth.json` | `uv run scripts/capture_session_key_vectors.py polyoxide-clob/tests/fixtures/session_keys` |
"""
    (out_dir / "PROVENANCE.md").write_text(text)


def write_relay_provenance(out_dir: Path, sdk_version: str, dependency_cutoff: str) -> None:
    text = f"""# Deposit Wallet relay vectors

Generated by `scripts/capture_session_key_vectors.py` (run:
`uv run scripts/capture_session_key_vectors.py polyoxide-clob/tests/fixtures/session_keys
polyoxide-relay/tests/fixtures/session_keys`) against Polymarket's official Python SDK,
`polymarket-client=={sdk_version}` — not a live host. Dependency resolution is pinned to
`exclude-newer = {dependency_cutoff}` (the script's own PEP 723 metadata), so a rerun
reproduces byte-identical output regardless of the date it is run, until that cutoff or the
pinned `polymarket-client` version changes.

The owner is Anvil key #0 (the same signer as the clob fixtures); `wallet` is its
beacon-generation Deposit Wallet, `0x00000000000Fb5C9ADea0298D729A0CB3823Cc07`-factory
derived. `derivations.signer_one` reproduces py-sdk's own
`tests/unit/test_wallet_derivations.py` expected addresses for signer
`0x0000000000000000000000000000000000000001`; generation asserts
`derivations.signer_one.uups` against that golden before writing anything, so this
fixture cannot drift from theirs silently. `derivations.anvil0` is the same four
derivations for the owner itself, with no independent golden to check against.

`approval_batch`, `authorize_batch`, `revoke_batch` and `redeem_batch` are Deposit Wallet
`Batch` EIP-712 vectors (ERC-20 approve, authorize session signer, revoke session signer,
CTF redeem), each with its typed data, digest, raw signature and `session_signature`. As
in the clob fixtures, `session_signature` is a byte-pinning vector only: the session-signer
envelope names Anvil address #1 as the session signer, but Anvil key #0 produced the inner
batch signature. `submit_body` and `authorization_body` are the corresponding relay request
payloads (`build_deposit_wallet_payload`'s submit envelope and the session-signer
authorization body), built from `approval_batch` and `authorize_batch` respectively.

| Fixture | Command |
|---|---|
| `relay_vectors.json` | `uv run scripts/capture_session_key_vectors.py polyoxide-clob/tests/fixtures/session_keys polyoxide-relay/tests/fixtures/session_keys` |
"""
    (out_dir / "PROVENANCE.md").write_text(text)


def main(out_dir: Path, relay_dir: Path) -> None:
    signer = Account.from_key(ANVIL_KEY_0)
    sdk_version = importlib.metadata.version("polymarket-client")
    dependency_cutoff = read_dependency_cutoff()
    out = {}
    for name, exchange in EXCHANGES.items():
        order = fixture(exchange)
        typed_data = build_order_typed_data(order)
        inner = "0x" + signer.sign_typed_data(full_message=typed_data).signature.hex()
        wrapped = build_order_signature(order, HexString(inner))
        session = wrap_deposit_wallet_session_signer_signature(
            EvmAddress(ANVIL_ADDR_1), wrapped
        )
        envelope_digest = digest(typed_data)
        if name == "v1_exchange" and envelope_digest != PY_SDK_GOLDEN_DIGEST:
            print(
                "envelope_digest for v1_exchange does not match py-sdk's golden "
                f"fixture:\n  got:      {envelope_digest}\n"
                f"  expected: {PY_SDK_GOLDEN_DIGEST}",
                file=sys.stderr,
            )
            sys.exit(1)
        out[name] = {
            "exchange": exchange,
            "deposit_wallet": DEPOSIT_WALLET,
            "signer_address": signer.address,
            "session_signer": ANVIL_ADDR_1,
            "envelope_digest": envelope_digest,
            "app_domain_separator": _app_domain_separator(
                order, protocol_version=order.protocol_version
            ),
            "contents_hash": "0x" + _order_contents_hash(order).hex(),
            "inner_signature": inner,
            "wrapped_signature": wrapped,
            "session_signature": session,
            "order": order_provenance(order),
        }
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "order_vectors.json").write_text(json.dumps(out, indent=2) + "\n")
    clob_auth = clob_auth_fixture(signer)
    (out_dir / "clob_auth.json").write_text(json.dumps(clob_auth, indent=2) + "\n")
    write_provenance(out_dir, sdk_version, dependency_cutoff)
    print(f"wrote {out_dir / 'order_vectors.json'}")
    print(f"wrote {out_dir / 'clob_auth.json'}")

    vectors = relay_vectors(signer)
    got = vectors["derivations"]["signer_one"]["uups"].lower()
    if got != PY_SDK_GOLDEN_UUPS_FOR_SIGNER_ONE:
        print(
            "derivations.signer_one.uups does not match py-sdk's golden fixture:\n"
            f"  got:      {got}\n"
            f"  expected: {PY_SDK_GOLDEN_UUPS_FOR_SIGNER_ONE}",
            file=sys.stderr,
        )
        sys.exit(1)
    relay_dir.mkdir(parents=True, exist_ok=True)
    (relay_dir / "relay_vectors.json").write_text(json.dumps(vectors, indent=2) + "\n")
    write_relay_provenance(relay_dir, sdk_version, dependency_cutoff)
    print(f"wrote {relay_dir / 'relay_vectors.json'}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(Path(sys.argv[1]), Path(sys.argv[2]))
