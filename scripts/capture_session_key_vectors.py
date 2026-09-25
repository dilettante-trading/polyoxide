#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["polymarket-client==0.11.0"]
# ///
"""Generate ERC-7739 / session-key golden vectors from Polymarket's official py-sdk.

polyoxide pins its Deposit Wallet (signature type 3) signing against these bytes.
The fixture order is py-sdk's own golden fixture (tests/unit/test_order_typed_data_golden.py),
signed with Anvil key #0; the session signer is Anvil address #1.

Usage:
    uv run scripts/capture_session_key_vectors.py polyoxide-clob/tests/fixtures/session_keys

Writes `order_vectors.json`. Re-run after a py-sdk upgrade; review the diff before committing.
"""
import json
import sys
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
from polymarket._internal.wallet import wrap_deposit_wallet_session_signer_signature
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


def main(out_dir: Path) -> None:
    signer = Account.from_key(ANVIL_KEY_0)
    out = {}
    for name, exchange in EXCHANGES.items():
        order = fixture(exchange)
        typed_data = build_order_typed_data(order)
        inner = "0x" + signer.sign_typed_data(full_message=typed_data).signature.hex()
        wrapped = build_order_signature(order, HexString(inner))
        session = wrap_deposit_wallet_session_signer_signature(
            EvmAddress(ANVIL_ADDR_1), wrapped
        )
        out[name] = {
            "exchange": exchange,
            "deposit_wallet": DEPOSIT_WALLET,
            "signer_address": signer.address,
            "session_signer": ANVIL_ADDR_1,
            "envelope_digest": digest(typed_data),
            "app_domain_separator": _app_domain_separator(order, protocol_version="2"),
            "contents_hash": "0x" + _order_contents_hash(order).hex(),
            "inner_signature": inner,
            "wrapped_signature": wrapped,
            "session_signature": session,
        }
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "order_vectors.json").write_text(json.dumps(out, indent=2) + "\n")
    print(f"wrote {out_dir / 'order_vectors.json'}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(Path(sys.argv[1]))
