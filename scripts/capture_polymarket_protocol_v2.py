#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["polymarket-client==0.12.0"]
#
# [tool.uv]
# exclude-newer = "2026-10-02T00:00:00Z"
# ///
"""Generate Protocol V2 golden vectors from Polymarket's official py-sdk 0.12.0.

polyoxide pins its Protocol V2 trading against these bytes: the ExchangeV3 order
signing (domain version "3", Deposit Wallet envelopes included), the asset-id
classification, the balance-allowance asset type, the Router calldata and the
outcome-pair selection. Every choice a vector records is the return value of py-sdk's
own function, given the case's inputs; this script never decides one.

The existing 0.11.0 script, `capture_polymarket_session_keys.py`, and its fixtures do
not change.

This script imports py-sdk's private modules (`polymarket._internal...`), which are not
a stable public API, so it is expected to need import-path fixes after a py-sdk upgrade.

Usage:
    uv run scripts/capture_polymarket_protocol_v2.py \
        polyoxide-polymarket/tests/fixtures/clob/protocol_v2 \
        polyoxide-polymarket/tests/fixtures/relay/protocol_v2 \
        polyoxide-polymarket/tests/fixtures/gamma/protocol_v2

Writes `order_vectors.json`, `id_vectors.json` and `PROVENANCE.md` to the first
directory, `router_vectors.json` and `PROVENANCE.md` to the second, and
`selector_vectors.json` and `PROVENANCE.md` to the third, creating the directories if
they are missing and writing nothing else. The output is byte-identical across runs on
one day: salts, timestamps and keys are fixed, and PROVENANCE.md carries only the day.
Re-run after a py-sdk upgrade; review the diff before committing.
"""
import copy
import dataclasses
import importlib.metadata
import json
import re
import sys
import tomllib
from pathlib import Path

from eth_account import Account
from eth_account.messages import encode_typed_data
from eth_utils.address import to_checksum_address
from eth_utils.crypto import keccak
from polymarket._internal.actions.orders.allowance import resolve_order_balance_allowance_target
from polymarket._internal.actions.orders.context import resolve_order_exchange_address
from polymarket._internal.actions.orders.orders import create_unsigned_order
from polymarket._internal.actions.orders.typed_data import (
    _app_domain_separator,
    _order_contents_hash,
    build_order_signature,
    build_order_typed_data,
)
from polymarket._internal.actions.orders.types import OrderDraft
from polymarket._internal.actions.relayer.calls import (
    _protocol_v2_condition_id_bytes,
    router_merge_call,
    router_redeem_call,
    router_split_call,
)
from polymarket._internal.actions.relayer.positions import (
    canonicalize_combo_legs,
    derive_combo_outcome_position_ids,
    derive_combo_position_context,
    normalize_market_position_context,
)
from polymarket._internal.actions.relayer.signing.deposit_wallet import (
    build_deposit_wallet_typed_data,
)
from polymarket._internal.environment import PRODUCTION_CONFIG
from polymarket._internal.protocol import decode_v2_outcome_position_id, is_v2_position_id
from polymarket._internal.wallet import wrap_deposit_wallet_session_signer_signature
from polymarket.errors import UnexpectedResponseError, UserInputError
from polymarket.models.gamma.market import Market
from polymarket.models.types import TokenId
from polymarket.types import EvmAddress
from pydantic import ValidationError

import capture_common

ANVIL_KEY_0 = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
ANVIL_ADDR_0 = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
ANVIL_ADDR_1 = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
DEPOSIT_WALLET = EvmAddress("0x57ffbc34de23124faeb8387fcd689d314e57accd")
CHAIN_ID = 137
# The addresses the design (and the Rust constants) name. They are only cross-checked
# against py-sdk's PRODUCTION_CONFIG before anything is written; every recorded address
# is read from the config.
EXCHANGE_V3 = "0xe3333700cA9d93003F00f0F71f8515005F6c00Aa"
ROUTER = "0x12121212006e4CD160D18e3f00711DA5c3372600"

# The ids' fixed layout: module byte, the first 18 bytes of keccak(ID_SEED), 8 zero
# bytes, four fixed bytes, outcome byte.
ID_SEED = b"polyoxide-v2-vector"
ID_FIXED_TAIL = bytes.fromhex("deadbeef")

# One order for every case: only the token, the wallet type and `neg_risk` vary.
SALT = 1
MAKER_AMOUNT = 1_000_000
TAKER_AMOUNT = 500_000
TIMESTAMP_MS = 1_700_000_000_000

MAX_UINT256 = (1 << 256) - 1
ROUTER_AMOUNTS = (0, 1_000_000, MAX_UINT256)
REDEEM_AMOUNT = 1_000_000
BATCH_NONCE = "0"
BATCH_DEADLINE = "1700000600"

GAMMA_MARKETS = (
    Path(__file__).resolve().parent.parent
    / "polyoxide-polymarket/tests/fixtures/clob/venue/gamma_markets.json"
)

# (case, id key, neg_risk passed to py-sdk, wallet type, wrapped in the session envelope)
ORDER_CASES = (
    ("v2_eoa", "v2_binary", False, "EOA", False),
    ("v2_neg_risk_eoa", "v2_neg_risk", True, "EOA", False),
    ("v2_deposit_wallet_owner", "v2_binary", False, "DEPOSIT_WALLET", False),
    ("v2_deposit_wallet_session", "v2_binary", False, "DEPOSIT_WALLET", True),
    ("v1_eoa", "v1", False, "EOA", False),
    ("v1_neg_risk_eoa", "v1", True, "EOA", False),
)

COMMAND = (
    "uv run scripts/capture_polymarket_protocol_v2.py "
    "polyoxide-polymarket/tests/fixtures/clob/protocol_v2 "
    "polyoxide-polymarket/tests/fixtures/relay/protocol_v2 "
    "polyoxide-polymarket/tests/fixtures/gamma/protocol_v2"
)


def digest(typed_data: dict) -> str:
    signable = encode_typed_data(full_message=typed_data)
    return "0x" + keccak(b"\x19\x01" + signable.header + signable.body).hex()


def check_environment(signer) -> None:
    """The inputs this script fixes must be the ones py-sdk's production config names."""
    if signer.address != ANVIL_ADDR_0:
        sys.exit(f"Anvil key #0 is {signer.address}, expected {ANVIL_ADDR_0}")
    for name, got, want in (
        ("chain_id", PRODUCTION_CONFIG.chain_id, CHAIN_ID),
        ("exchange_v3", PRODUCTION_CONFIG.exchange_v3, EXCHANGE_V3),
        ("protocol_v2_router", PRODUCTION_CONFIG.protocol_v2_router, ROUTER),
    ):
        if got != want:
            sys.exit(f"py-sdk's PRODUCTION_CONFIG.{name} is {got}, expected {want}")


# --- ids ---------------------------------------------------------------------------


def v2_id(module: int, outcome: int) -> str:
    """A decimal V2-shaped id: module byte, 18 hashed bytes, 8 zero bytes, `deadbeef`,
    outcome byte (32 bytes, big-endian)."""
    raw = bytes([module]) + keccak(ID_SEED)[:18] + bytes(8) + ID_FIXED_TAIL + bytes([outcome])
    if len(raw) != 32:
        sys.exit(f"v2_id layout is {len(raw)} bytes, expected 32")
    return str(int.from_bytes(raw, "big"))


def require_v2(name: str, position_id: str, outcome: int):
    """Keep an id only if py-sdk's `is_v2_position_id` and `decode_v2_outcome_position_id`
    both accept it, and the decode reads the outcome the layout put there."""
    if not is_v2_position_id(position_id):
        sys.exit(f"{name} is not a V2 position id to py-sdk: {position_id}")
    try:
        decoded = decode_v2_outcome_position_id(position_id)
    except UserInputError as error:
        sys.exit(f"{name} is refused by decode_v2_outcome_position_id: {error}")
    if decoded.outcome_index != outcome:
        sys.exit(f"{name} decodes to outcome {decoded.outcome_index}, expected {outcome}")
    return decoded


def first_gamma_market() -> dict:
    return json.loads(GAMMA_MARKETS.read_text())[0]


def build_ids() -> tuple[dict, str]:
    """The four ids of the vectors, and the sibling (outcome 1) of `v2_binary`."""
    v1 = json.loads(first_gamma_market()["clobTokenIds"])[0]
    if is_v2_position_id(v1):
        sys.exit(f"the V1 id is a V2 position id to py-sdk: {v1}")

    v2_binary = v2_id(1, 0)
    v2_neg_risk = v2_id(2, 0)
    require_v2("v2_binary", v2_binary, 0)
    require_v2("v2_neg_risk", v2_neg_risk, 0)
    sibling = v2_id(1, 1)
    require_v2("v2_binary sibling", sibling, 1)

    legs = canonicalize_combo_legs([v2_binary, v2_id(2, 1)])
    combo_context = derive_combo_position_context(legs)
    v2_combo = str(derive_combo_outcome_position_ids(combo_context.condition_id)[0])
    require_v2("v2_combo", v2_combo, 0)

    ids = {"v1": v1, "v2_binary": v2_binary, "v2_neg_risk": v2_neg_risk, "v2_combo": v2_combo}
    return ids, sibling


# --- id_vectors.json ---------------------------------------------------------------


def position_id_row(text: str) -> dict:
    answer = is_v2_position_id(text)
    try:
        decoded = decode_v2_outcome_position_id(text)
        recorded = {
            "condition_id": str(decoded.condition_id),
            "outcome_index": decoded.outcome_index,
        }
    except UserInputError:
        recorded = None
    return {"input": text, "is_v2_position_id": answer, "decoded": recorded}


def id_vectors(ids: dict) -> dict:
    binary = ids["v2_binary"]
    value = int(binary)
    inputs = [
        ids["v1"],
        ids["v2_binary"],
        ids["v2_neg_risk"],
        ids["v2_combo"],
        "1",
        "100",
        "123456",
        hex(value),
        bin(value),
        oct(value),
        f" {binary} ",
        "-1",
        str(1 << 256),
        "",
        "abc",
        v2_id(0, 0),
        v2_id(4, 0),
        v2_id(1, 2),
        # The mask's edges: bit 40 is the lowest reserved bit, bit 104 the lowest bit
        # above the reserved range. A mask shifted one bit either way changes an answer.
        str(value | (1 << 40)),
        str(value | (1 << 104)),
    ]
    targets = []
    for side in ("BUY", "SELL"):
        for key in ("v1", "v2_binary"):
            asset_type, asset_id_sent = resolve_order_balance_allowance_target(
                side=side, asset_id=ids[key]
            )
            targets.append(
                {
                    "side": side,
                    "asset_id": ids[key],
                    "asset_type": asset_type,
                    "asset_id_sent": None if asset_id_sent is None else str(asset_id_sent),
                }
            )
    return {
        "ids": ids,
        "position_ids": [position_id_row(text) for text in inputs],
        "balance_allowance_targets": targets,
    }


# --- order_vectors.json ------------------------------------------------------------


def order_case(signer, token_id: str, neg_risk: bool, wallet_type: str, session: bool) -> dict:
    """One order, through py-sdk's own exchange choice and `create_unsigned_order` (which
    sets `protocol_version`), with salt and timestamp fixed afterwards."""
    funder = DEPOSIT_WALLET if wallet_type == "DEPOSIT_WALLET" else EvmAddress(signer.address)
    exchange = resolve_order_exchange_address(
        PRODUCTION_CONFIG, asset_id=token_id, neg_risk=neg_risk
    )
    draft = OrderDraft(
        chain_id=CHAIN_ID,
        exchange_address=exchange,
        expiration=0,
        funder_address=funder,
        offered_amount=MAKER_AMOUNT,
        order_type="GTC",
        side="BUY",
        signer=EvmAddress(signer.address),
        requested_amount=TAKER_AMOUNT,
        asset_id=TokenId(token_id),
    )
    order = dataclasses.replace(
        create_unsigned_order(draft, wallet=DEPOSIT_WALLET, wallet_type=wallet_type),
        salt=SALT,
        timestamp=TIMESTAMP_MS,
    )
    if wallet_type == "EOA" and (order.maker != signer.address or order.signer != signer.address):
        sys.exit("a type-0 order's maker and signer must both be the signer's address")
    if wallet_type == "DEPOSIT_WALLET" and (
        order.maker != DEPOSIT_WALLET or order.signer != DEPOSIT_WALLET
    ):
        sys.exit("a type-3 order's maker and signer must both be the Deposit Wallet")

    typed_data = build_order_typed_data(order)
    inner = "0x" + signer.sign_typed_data(full_message=typed_data).signature.hex()
    signature = build_order_signature(order, inner)
    if session:
        signature = wrap_deposit_wallet_session_signer_signature(
            EvmAddress(to_checksum_address(ANVIL_ADDR_1)), signature
        )
    return {
        "neg_risk": neg_risk,
        "order": {
            "salt": str(order.salt),
            "maker": str(order.maker),
            "signer": str(order.signer),
            "token_id": str(order.token_id),
            "maker_amount": str(order.maker_amount),
            "taker_amount": str(order.taker_amount),
            "side": order.side,
            "signature_type": order.signature_type,
            "timestamp": str(order.timestamp),
            "expiration": str(order.expiration),
            "metadata": order.metadata,
            "builder": order.builder,
        },
        "exchange": to_checksum_address(order.exchange_address),
        "version": order.protocol_version,
        "domain_separator": _app_domain_separator(order, protocol_version=order.protocol_version),
        "struct_hash": "0x" + _order_contents_hash(order).hex(),
        "digest": digest(typed_data),
        "signature": str(signature),
    }


def order_vectors(signer, ids: dict) -> dict:
    cases = {}
    for name, key, neg_risk, wallet_type, session in ORDER_CASES:
        case = order_case(signer, ids[key], neg_risk, wallet_type, session)
        # Guards, never recorded: an id that py-sdk classified the other way would make
        # the whole case a control for the wrong protocol.
        want_version = "3" if key.startswith("v2") else "2"
        if case["version"] != want_version:
            sys.exit(f"{name}: py-sdk chose domain version {case['version']}, not {want_version}")
        if key.startswith("v2") and case["exchange"] != EXCHANGE_V3:
            sys.exit(f"{name}: py-sdk chose exchange {case['exchange']}, not ExchangeV3")
        cases[name] = case
    return {
        "chain_id": CHAIN_ID,
        "signer_address": signer.address,
        "session_signer": to_checksum_address(ANVIL_ADDR_1),
        "deposit_wallet": str(DEPOSIT_WALLET),
        "cases": cases,
    }


# --- router_vectors.json -----------------------------------------------------------


def bytes31_of(text: str):
    """`_protocol_v2_condition_id_bytes`'s result as `0x` hex, or None when it refuses."""
    try:
        return "0x" + _protocol_v2_condition_id_bytes(text).hex()
    except UserInputError:
        return None


def router_vectors(ids: dict, sibling: str) -> dict:
    router = EvmAddress(PRODUCTION_CONFIG.protocol_v2_router)
    binary = require_v2("v2_binary", ids["v2_binary"], 0)
    condition = str(binary.condition_id)

    def amount_rows(build) -> list[dict]:
        rows = []
        for amount in ROUTER_AMOUNTS:
            call = build(router=router, condition_id=condition, amount=amount)
            if call.to != router:
                sys.exit(f"a Router call targets {call.to}, not the Router {router}")
            rows.append({"condition_id": condition, "amount": str(amount), "calldata": call.data})
        return rows

    redeem = []
    for position_id, outcome in ((ids["v2_binary"], 0), (sibling, 1)):
        decoded = require_v2(f"redeem outcome {outcome}", position_id, outcome)
        call = router_redeem_call(
            router=router,
            condition_id=str(decoded.condition_id),
            outcome_index=decoded.outcome_index,
            amount=REDEEM_AMOUNT,
        )
        redeem.append(
            {
                "position_id": position_id,
                "condition_id": str(decoded.condition_id),
                "outcome_index": decoded.outcome_index,
                "amount": str(REDEEM_AMOUNT),
                "calldata": call.data,
            }
        )

    # py-sdk refuses an outcome index other than 0 or 1. The design records this refusal
    # through `id_vectors.json`'s outcome-byte-2 row, so the script only checks it.
    try:
        router_redeem_call(router=router, condition_id=condition, outcome_index=2, amount=1)
    except UserInputError as error:
        if not str(error).startswith("Protocol v2 outcome index must be 0 or 1"):
            sys.exit(f"router_redeem_call refused outcome index 2 for another reason: {error}")
    else:
        sys.exit("router_redeem_call accepted outcome index 2")

    hex62 = condition[2:]
    condition_inputs = [
        condition,
        condition + "00",
        condition + "01",
        "0X" + hex62.upper(),
        condition + "02",
        condition[:-2],
        hex62,
        "0x" + "zz" * 31,
    ]
    redeem_call = router_redeem_call(
        router=router,
        condition_id=redeem[0]["condition_id"],
        outcome_index=redeem[0]["outcome_index"],
        amount=REDEEM_AMOUNT,
    )
    batch = build_deposit_wallet_typed_data(
        wallet=DEPOSIT_WALLET,
        calls=[redeem_call],
        nonce=BATCH_NONCE,
        deadline=BATCH_DEADLINE,
        chain_id=CHAIN_ID,
    )
    return {
        "chain_id": CHAIN_ID,
        "router": str(router),
        "split": amount_rows(router_split_call),
        "merge": amount_rows(router_merge_call),
        "redeem": redeem,
        "condition_ids": [
            {"input": text, "bytes31": bytes31_of(text)} for text in condition_inputs
        ],
        "deposit_wallet_batch": {
            "wallet": str(DEPOSIT_WALLET),
            "nonce": BATCH_NONCE,
            "deadline": BATCH_DEADLINE,
            "calls": [
                {
                    "target": str(redeem_call.to),
                    "value": str(redeem_call.value),
                    "data": redeem_call.data,
                }
            ],
            "digest": digest(batch),
        },
    }


# --- selector_vectors.json ---------------------------------------------------------


def market_with(base: dict, *, version, clob_token_ids, position_ids) -> dict:
    """The first gamma market with only `version`, `clobTokenIds` and `positionIds`
    replaced; a `None` leaves the key out."""
    market = copy.deepcopy(base)
    for key, value in (
        ("version", version),
        ("clobTokenIds", clob_token_ids),
        ("positionIds", position_ids),
    ):
        if value is None:
            market.pop(key, None)
        else:
            market[key] = value
    return market


def select_pair(market: dict):
    """py-sdk's `(yes, no)` pair for a market, or the exception it refused with."""
    config = PRODUCTION_CONFIG
    try:
        context = normalize_market_position_context(
            Market.model_validate(market),
            context="selector vector",
            collateral_adapter=EvmAddress(config.collateral_adapter),
            neg_risk_collateral_adapter=EvmAddress(config.neg_risk_collateral_adapter),
            conditional_tokens=EvmAddress(config.conditional_tokens),
            neg_risk_adapter=EvmAddress(config.neg_risk_adapter),
            position_manager=EvmAddress(config.position_manager),
        )
    except (UnexpectedResponseError, ValidationError) as error:
        return None, error
    return [str(token_id) for token_id in context.token_ids], None


def selector_vectors(ids: dict, sibling: str) -> dict:
    base = first_gamma_market()
    clob = base["clobTokenIds"]
    v1_pair = json.loads(clob)
    # `positionIds` is a real JSON array, as gamma serves it; `clobTokenIds` stays text.
    positions = [ids["v2_binary"], sibling]
    one_position = [ids["v2_binary"]]
    # (case, version, clobTokenIds, positionIds, expected pair or the refusal), a refusal
    # being its type, or `(type, message prefix)` where py-sdk's message names the reason.
    cases = (
        ("v1_with_position_ids", "v1", clob, positions, v1_pair),
        ("v2_with_both", "v2", clob, positions, [ids["v2_binary"], sibling]),
        (
            "missing_version",
            None,
            clob,
            positions,
            (UnexpectedResponseError, "Missing market version"),
        ),
        ("unknown_version", "v3", clob, positions, ValidationError),
        (
            "v2_one_position_id",
            "v2",
            clob,
            one_position,
            (UnexpectedResponseError, "Incomplete market position IDs"),
        ),
        (
            "v1_no_clob_token_ids",
            "v1",
            None,
            positions,
            (UnexpectedResponseError, "Missing market token IDs"),
        ),
    )
    rows = []
    for name, version, clob_token_ids, position_ids, expected in cases:
        market = market_with(
            base, version=version, clob_token_ids=clob_token_ids, position_ids=position_ids
        )
        pair, error = select_pair(market)
        # Guards, never recorded: a case must be refused for the reason it exists, or an
        # unrelated validation failure would turn every case into a vacuous `null`.
        if isinstance(expected, list):
            if pair != expected:
                sys.exit(f"{name}: py-sdk chose {pair} ({error!r}), expected {expected}")
        else:
            kind, prefix = expected if isinstance(expected, tuple) else (expected, None)
            if not isinstance(error, kind):
                sys.exit(f"{name}: py-sdk gave {pair} / {error!r}, expected a {kind.__name__}")
            if prefix is not None and not str(error).startswith(prefix):
                sys.exit(f"{name}: refused with {str(error)!r}, expected it to start {prefix!r}")
            if isinstance(error, ValidationError) and not any(
                item["loc"] == ("version",) for item in error.errors()
            ):
                sys.exit(f"{name}: refused, but not on the `version` field: {error}")
        rows.append({"name": name, "market": market, "ids": pair})
    return {"cases": rows}


# --- PROVENANCE.md -----------------------------------------------------------------

# The canonical PEP 723 block-extraction regex, straight from the spec.
_PEP_723_BLOCK_RE = re.compile(
    r"(?m)^# /// (?P<type>[a-zA-Z0-9-]+)$\s(?P<content>(^#(| .*)$\s)+)^# ///$"
)


def read_dependency_cutoff() -> str:
    """Read `[tool.uv].exclude-newer` from this file's own PEP 723 metadata block, so
    PROVENANCE.md states the pin that governs resolution, not a second literal."""
    match = _PEP_723_BLOCK_RE.search(Path(__file__).read_text())
    content = "".join(
        line[2:] if line.startswith("# ") else line[1:]
        for line in match.group("content").splitlines(keepends=True)
    )
    return tomllib.loads(content)["tool"]["uv"]["exclude-newer"]


def commands_table(*fixtures: str) -> str:
    """The Fixture/Command table, built by `capture_common.provenance_table`."""
    rows = [(f"`{name}`", f"`{COMMAND}`") for name in fixtures]
    return "\n".join(
        capture_common.provenance_table(("Fixture", "Command"), rows, short_rule=True)
    )


def reproducibility(sdk_version: str, cutoff: str, day: str) -> str:
    return (
        "Generated by `scripts/capture_polymarket_protocol_v2.py` (run:\n"
        f"`{COMMAND}`)\n"
        f"on {day} against Polymarket's official Python SDK, "
        f"`polymarket-client=={sdk_version}`, not a live\n"
        f"host. Dependency resolution is pinned to `exclude-newer = {cutoff}` (the\n"
        "script's own PEP 723 metadata), so a rerun reproduces byte-identical output\n"
        "until that cutoff or the pinned `polymarket-client` version changes; only the\n"
        "date above moves. Every recorded choice is the return value of py-sdk's own\n"
        "function for the case's inputs; the script decides none. Anvil key #0 signs; the\n"
        "session signer is Anvil address #1; the Deposit Wallet is the one the\n"
        "`session_keys` fixtures use."
    )


def clob_provenance(sdk_version: str, cutoff: str, day: str) -> str:
    return f"""# Protocol V2 CLOB vectors

{reproducibility(sdk_version, cutoff, day)}

`id_vectors.json`: the four ids (`v1` is the first `clobTokenIds` entry of
`tests/fixtures/clob/venue/gamma_markets.json`'s first market; `v2_binary`, `v2_neg_risk`
and `v2_combo` are kept only if py-sdk's `is_v2_position_id` and
`decode_v2_outcome_position_id` accept them), then one row per probed input with both of
py-sdk's answers (`is_v2_position_id`, and the decode, or `null` when it refuses), then
`resolve_order_balance_allowance_target` for BUY and SELL against `v1` and `v2_binary`.
py-sdk's bare reserved-bit test is true for `"1"`, `"100"` and `"123456"`, which its
decode refuses: those rows are why polyoxide's rule is the conjunction of the two. The
last two rows are `v2_binary` with bit 40 and with bit 104 set, the edges of the reserved
bit range.

`order_vectors.json`: six orders. Each is an `OrderDraft` whose exchange is
`resolve_order_exchange_address`'s choice (`neg_risk` is what the script passed to it),
turned into an order by `create_unsigned_order` (which sets `protocol_version`), with salt
and timestamp fixed afterwards. `digest` is what the key signs; `signature` is what is
posted (`build_order_signature`'s output; for the session key, wrapped by
`wrap_deposit_wallet_session_signer_signature`). As in the `session_keys` fixtures, the
session-signer envelope names Anvil address #1 but Anvil key #0 produced the inner
signature: a byte-pinning vector, not a signature a real session key would produce.

{commands_table("id_vectors.json", "order_vectors.json")}
"""


def relay_provenance(sdk_version: str, cutoff: str, day: str) -> str:
    return f"""# Protocol V2 Router vectors

{reproducibility(sdk_version, cutoff, day)}

`router_vectors.json`: `split`, `merge` and `redeem` hold the `.data` of py-sdk's
`router_split_call`, `router_merge_call` and `router_redeem_call` called with
`router=` py-sdk's `protocol_v2_router`. `condition_ids` holds
`_protocol_v2_condition_id_bytes` over accepted and refused inputs (`bytes31` is `null`
when it raises). `deposit_wallet_batch` is `build_deposit_wallet_typed_data`'s EIP-712
digest for a batch of the one redeem call at outcome 0. Generation also checks that
`router_redeem_call` refuses outcome index 2; the refusal is not recorded here, because
the id table in the `clob` fixtures records the outcome-byte-2 position id.

{commands_table("router_vectors.json")}
"""


def gamma_provenance(sdk_version: str, cutoff: str, day: str) -> str:
    return f"""# Protocol V2 outcome-pair selector vectors

{reproducibility(sdk_version, cutoff, day)}

`selector_vectors.json`: each `market` is the first market of
`tests/fixtures/clob/venue/gamma_markets.json` with only `version`, `clobTokenIds` and
`positionIds` replaced (a missing key is a key left out). `ids` is the `(yes, no)` pair
that py-sdk's `normalize_market_position_context` returns for `Market.model_validate` of
that market, or `null` when either raises. Generation checks that each refusal is the one
the case exists for (an `UnexpectedResponseError`, or a validation error on `version`).

{commands_table("selector_vectors.json")}
"""


# --- main --------------------------------------------------------------------------


def emit_json(path: Path, value) -> None:
    capture_common.write_json(path, value, ensure_ascii=True)
    print(f"wrote {path}")


def emit_provenance(out_dir: Path, text: str) -> None:
    print(f"wrote {capture_common.write_provenance(out_dir, text)}")


def main(clob_dir: Path, relay_dir: Path, gamma_dir: Path) -> None:
    signer = Account.from_key(ANVIL_KEY_0)
    check_environment(signer)
    sdk_version = importlib.metadata.version("polymarket-client")
    if sdk_version != "0.12.0":
        sys.exit(f"polymarket-client is {sdk_version}, expected 0.12.0")
    cutoff = read_dependency_cutoff()
    day = capture_common.stamp("%Y-%m-%d")

    # --- compute everything first; nothing is written until every guard passed. ---
    ids, sibling = build_ids()
    ids_out = id_vectors(ids)
    orders_out = order_vectors(signer, ids)
    router_out = router_vectors(ids, sibling)
    selector_out = selector_vectors(ids, sibling)

    emit_json(clob_dir / "order_vectors.json", orders_out)
    emit_json(clob_dir / "id_vectors.json", ids_out)
    emit_provenance(clob_dir, clob_provenance(sdk_version, cutoff, day))
    emit_json(relay_dir / "router_vectors.json", router_out)
    emit_provenance(relay_dir, relay_provenance(sdk_version, cutoff, day))
    emit_json(gamma_dir / "selector_vectors.json", selector_out)
    emit_provenance(gamma_dir, gamma_provenance(sdk_version, cutoff, day))


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]))
