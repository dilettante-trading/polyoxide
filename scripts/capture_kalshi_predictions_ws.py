#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["websockets>=13", "certifi", "cryptography"]
# ///
"""Capture the order-book socket's frames from Kalshi's demo host as test fixtures for
polyoxide-kalshi's `predictions-ws` feature, and record what the capture found in
docs/specs/kalshi/OBSERVED.md.

Usage: uv run scripts/capture_kalshi_predictions_ws.py [--ticker TICKER] [--seconds 60]

Needs the owner's demo key in KALSHI_DEMO_API_KEY_ID and KALSHI_DEMO_PRIVATE_KEY (the PEM
text): the handshake is signed through capture_kalshi_account.signed_headers. Without them
the run stops before it connects, and no fixture is ever written by hand. Keys do not cross
environments, so a production key does not work here.

It subscribes one open demo market with use_yes_price, reads its snapshot and up to
--seconds of deltas, adds a second market, unsubscribes, and sends a subscribe that names
no market. The markets come from the demo host's unsigned listing with status=open and
mve_filter=exclude: the open listing is dominated by multivariate combinations, which this
capture does not want, and the book of the one it subscribes is confirmed on the orderbook
route. Nothing is written until every step has succeeded, so a failed run leaves the
fixtures as they were. It writes subscribed, snapshot, deltas, ok, unsubscribed and error
under polyoxide-kalshi/tests/fixtures/predictions/ws/, with PROVENANCE.md, and writes one
section into docs/specs/kalshi/OBSERVED.md directly above its anchor line, replacing the
one an earlier run wrote. Review the diff before committing.
"""
from __future__ import annotations

import argparse
import asyncio
import json
import os
import time
import urllib.parse
from dataclasses import dataclass
from decimal import Decimal
from pathlib import Path
from typing import Any

from websockets.exceptions import ConnectionClosed, InvalidHandshake

import capture_common
import capture_kalshi_account

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "polyoxide-kalshi" / "tests" / "fixtures" / "predictions" / "ws"
OBSERVED = ROOT / "docs" / "specs" / "kalshi" / "OBSERVED.md"
REST = "https://external-api.demo.kalshi.co"
WS = "wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2"
SECRETS = ("KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY")
# How long a command may wait for its answer, and how long the capture waits for the
# snapshot of a market it added.
ANSWER_SECONDS = 10.0
ADDED_SECONDS = 3.0
# At most this many deltas are kept, and this many open markets are checked for a
# two-sided book.
MAX_DELTAS = 100
CANDIDATES = 20
ANSWERS = ("subscribed", "ok", "unsubscribed", "error")
ANCHOR = "<!-- capture findings go above this line -->"
BEGIN = "<!-- kalshi-ws-capture:begin -->"
END = "<!-- kalshi-ws-capture:end -->"


def require_key() -> None:
    missing = [name for name in SECRETS if not os.environ.get(name)]
    if missing:
        raise SystemExit(
            f"{' and '.join(missing)} not set. This capture signs its handshake with the owner's "
            "demo key: stop here and ask the owner for it. No fixture is ever written by hand."
        )


def require_anchor() -> None:
    if ANCHOR not in OBSERVED.read_text(encoding="utf-8"):
        raise SystemExit(f"{OBSERVED} has no anchor line {ANCHOR!r}: Story 1 writes it")


def signed(_method: str, url: str) -> dict[str, str]:
    """The handshake's headers, signed over the URL's path (`ws_session` calls this for
    each connection)."""
    return capture_kalshi_account.signed_headers("GET", urllib.parse.urlparse(url).path)


def fetch_ok(path: str, **params: Any) -> Any:
    reply = capture_common.get(f"{REST}{path}", params=params)
    return capture_common.require_ok(reply, f"GET {path}")


def two_sided(ticker: str) -> bool:
    book = fetch_ok(f"/trade-api/v2/markets/{ticker}/orderbook")["orderbook_fp"]
    return bool(book.get("yes_dollars")) and bool(book.get("no_dollars"))


def quoted(market: dict[str, Any]) -> bool:
    """Active, with a YES bid above 0 and a YES ask below 1."""
    return (
        market["status"] == "active"
        and Decimal(market["yes_bid_dollars"]) > 0
        and Decimal(market["yes_ask_dollars"]) < 1
    )


def choose(wanted: str | None) -> tuple[str, str]:
    """The market to subscribe (the first quoted open one whose book has both sides unless
    one is given), and another quoted one to add to the subscription.

    The listing excludes multivariate combinations (`mve_filter=exclude`), which dominate
    the open listing, and the candidates are its rows that quote both sides: a page of
    1000 held 28 of them on 2026-10-18, and none of its first 20 rows."""
    listing = fetch_ok("/trade-api/v2/markets", status="open", mve_filter="exclude", limit=1000)
    tickers = [market["ticker"] for market in listing["markets"] if quoted(market)]
    first = wanted or next((t for t in tickers[:CANDIDATES] if two_sided(t)), None)
    if first is None:
        raise SystemExit(
            f"none of the first {CANDIDATES} quoted open demo markets has a two-sided book: pass --ticker"
        )
    others = [t for t in tickers if t != first]
    if not others:
        raise SystemExit("the demo host lists no second quoted open market to add to the subscription")
    return first, others[0]


@dataclass
class Seen:
    """One frame read: its text, its parsed form, and the last `seq` its subscription had
    before it (`None` for a frame without `sid` and `seq`, and for the first of its
    subscription)."""

    text: str
    frame: dict[str, Any]
    previous: int | None


class Tape:
    """Every frame read from one connection, in order, with each subscription's `seq`."""

    def __init__(self, ws: Any) -> None:
        self.ws = ws
        self.log: list[Seen] = []
        self.last: dict[int, int] = {}
        self.first: dict[int, tuple[int, str]] = {}

    async def read(self, timeout: float) -> Seen | None:
        """The next frame, or None when none arrives within `timeout` seconds."""
        try:
            text = await asyncio.wait_for(self.ws.recv(), timeout=max(0.05, timeout))
        except asyncio.TimeoutError:
            return None
        frame = json.loads(text)
        previous = None
        sid, seq = frame.get("sid"), frame.get("seq")
        if isinstance(sid, int) and isinstance(seq, int):
            previous = self.last.get(sid)
            self.first.setdefault(sid, (seq, str(frame.get("type"))))
            self.last[sid] = seq
        seen = Seen(text, frame, previous)
        self.log.append(seen)
        return seen

    async def command(self, command: dict[str, Any]) -> Seen:
        """Sends `command` and reads until the frame answering its id arrives."""
        await self.ws.send(json.dumps(command))
        deadline = time.monotonic() + ANSWER_SECONDS
        while True:
            seen = await self.read(deadline - time.monotonic())
            if seen is None:
                raise SystemExit(
                    f"no answer to {command['cmd']} (id {command['id']}) in {ANSWER_SECONDS:.0f} s"
                )
            if seen.frame.get("id") == command["id"] and seen.frame.get("type") in ANSWERS:
                return seen

    async def listen(self, seconds: float) -> tuple[Seen | None, list[Seen]]:
        """The first snapshot, and up to MAX_DELTAS deltas, read in `seconds`."""
        snapshot: Seen | None = None
        deltas: list[Seen] = []
        deadline = time.monotonic() + seconds
        while len(deltas) < MAX_DELTAS and time.monotonic() < deadline:
            seen = await self.read(deadline - time.monotonic())
            if seen is None:
                break
            kind = seen.frame.get("type")
            if kind == "orderbook_snapshot" and snapshot is None:
                snapshot = seen
            elif kind == "orderbook_delta":
                deltas.append(seen)
        return snapshot, deltas

    async def read_for(self, seconds: float) -> None:
        """Reads whatever arrives in `seconds`."""
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if await self.read(deadline - time.monotonic()) is None:
                break


@dataclass
class Capture:
    first: str
    second: str
    subscribed: Seen
    snapshot: Seen
    deltas: list[Seen]
    added: Seen
    unsubscribed: Seen
    refused: Seen
    first_seq: tuple[int, str]
    snapshot_after_add: bool
    limit_hits: list[str]


def expect(seen: Seen, kind: str, what: str) -> None:
    if seen.frame.get("type") != kind:
        raise SystemExit(f"{what} was answered with {seen.text}, not a {kind}")


def book(snapshot: Seen) -> tuple[list[Decimal], list[Decimal]]:
    """The prices of the snapshot's YES side and of its NO side (in YES pricing)."""
    msg = snapshot.frame["msg"]
    return (
        [Decimal(price) for price, _count in msg.get("yes_dollars_fp", [])],
        [Decimal(price) for price, _count in msg.get("no_dollars_fp", [])],
    )


async def run(first: str, second: str, seconds: float) -> Capture:
    async with capture_common.ws_session(WS, headers=signed) as ws:
        tape = Tape(ws)
        params = {"channels": ["orderbook_delta"], "market_tickers": [first], "use_yes_price": True}
        subscribed = await tape.command({"id": 1, "cmd": "subscribe", "params": params})
        expect(subscribed, "subscribed", "subscribe")
        sid = subscribed.frame["msg"]["sid"]
        snapshot, deltas = await tape.listen(seconds)
        if snapshot is None:
            raise SystemExit(f"no orderbook_snapshot for {first} in {seconds:.0f} s")
        bids, asks = book(snapshot)
        if not bids or not asks:
            raise SystemExit(f"the snapshot of {first} is one-sided: pass --ticker of a market with both sides")
        if not deltas:
            raise SystemExit(
                f"no orderbook_delta for {first} in {seconds:.0f} s: rerun with --ticker of a busier market"
            )
        mark = len(tape.log)
        update = {"sid": sid, "action": "add_markets", "market_tickers": [second]}
        added = await tape.command({"id": 2, "cmd": "update_subscription", "params": update})
        expect(added, "ok", "update_subscription")
        await tape.read_for(ADDED_SECONDS)
        snapshot_after_add = any(
            seen.frame.get("type") == "orderbook_snapshot"
            and (seen.frame.get("msg") or {}).get("market_ticker") == second
            for seen in tape.log[mark:]
        )
        unsubscribed = await tape.command({"id": 3, "cmd": "unsubscribe", "params": {"sids": [sid]}})
        expect(unsubscribed, "unsubscribed", "unsubscribe")
        nothing = {"channels": ["orderbook_delta"]}
        refused = await tape.command({"id": 4, "cmd": "subscribe", "params": nothing})
        expect(refused, "error", "a subscribe that names no market")
        limit_hits = [
            seen.text
            for seen in tape.log
            if seen.frame.get("type") == "error" and (seen.frame.get("msg") or {}).get("code") == 26
        ]
        return Capture(
            first, second, subscribed, snapshot, deltas, added, unsubscribed, refused,
            tape.first[sid], snapshot_after_add, limit_hits,
        )


def advance(seen: Seen) -> str:
    """Whether the frame took the next number in its subscription's `seq`."""
    seq = seen.frame.get("seq")
    if not isinstance(seq, int):
        return "no: the frame carries no `seq`"
    if seen.previous is None:
        return f"unknown: `seq` {seq} was the first the capture saw on that subscription"
    verdict = "yes" if seq == seen.previous + 1 else "no"
    return f"{verdict}: `seq` {seen.previous} before it, {seq} on it"


def direction(prices: list[Decimal]) -> str:
    """Whether the prices, in wire order, ascend, descend or neither."""
    if prices == sorted(prices):
        return "ascending"
    if prices == sorted(prices, reverse=True):
        return "descending"
    return "unordered"


def reading(bids: list[Decimal], asks: list[Decimal]) -> str:
    """What the snapshot shows about its pricing. A book that is not crossed proves little
    for a market near 0.5, where the NO side read as raw NO prices is not crossed either,
    so the order of the levels is read as well."""
    raw_ask = 1 - max(asks)
    if max(bids) >= raw_ask:
        return (
            f"**The crossing settles it.** Read as raw NO prices the book would be crossed "
            f"(best bid {max(bids)}, best ask {raw_ask}), so the NO side is in YES pricing, "
            "as `use_yes_price` asks."
        )
    text = (
        "**An uncrossed book proves little for this market.** Read as raw NO prices it is "
        f"uncrossed too (best bid {max(bids)}, best ask {raw_ask}), so the crossing cannot "
        "tell the two pricings apart here. "
    )
    if direction(bids) == "ascending" and direction(asks) == "descending":
        return text + (
            "The order of the levels points to YES pricing. The mirror's own example "
            "(`docs/specs/kalshi/asyncapi.yaml`) lists both sides ascending, each ending at "
            "its best level in its own pricing. Here the YES side ascends to its best bid "
            "and the NO side descends to its lowest price, the best ask in YES pricing, so "
            "both sides run outside in to the touch; in raw NO pricing the best NO bid is "
            "the highest price and the side would ascend. That is an inference from the "
            "listing order, not a measurement, and a market priced well away from 0.5 would "
            "settle it by arithmetic. The client does not read the order: `event.rs` sorts "
            "both sides."
        )
    return text + "The order of the levels does not settle it either."


def section(capture: Capture, stamp: str) -> str:
    bids, asks = book(capture.snapshot)
    best_bid, best_ask = max(bids), min(asks)
    seq, kind = capture.first_seq
    uncrossed = "yes" if best_bid < best_ask else "NO, crossed"
    if capture.limit_hits:
        bound = f"met, code 26: `{capture.limit_hits[0]}`"
    else:
        bound = "none met: the capture subscribed one market, then added a second"
    rows = [
        ("First `seq` of a subscription", f"{seq}, on its `{kind}` frame"),
        ("`ok` to `update_subscription` advances `seq`", advance(capture.added)),
        ("`unsubscribed` advances `seq`", advance(capture.unsubscribed)),
        (
            "`add_markets` is followed by a snapshot of the added market",
            "yes" if capture.snapshot_after_add else "no",
        ),
        (
            "Snapshot book not crossed",
            f"{uncrossed}: best bid {best_bid}, best ask {best_ask}",
        ),
        (
            "Order of each side's levels on the wire",
            f"YES side {direction(bids)} ({bids[0]} to {bids[-1]}); "
            f"NO side {direction(asks)} ({asks[0]} to {asks[-1]})",
        ),
        ("Bound on markets per subscription", bound),
    ]
    lines = [
        "## The order-book socket, as the demo host answers it",
        "",
        f"Captured {stamp} by `scripts/capture_kalshi_predictions_ws.py` from `{WS}` on "
        f"`{capture.first}`, with `{capture.second}` added to the subscription. "
        "Fixtures: `polyoxide-kalshi/tests/fixtures/predictions/ws/`.",
        "",
        *capture_common.provenance_table(("Question", "Observed"), rows),
        "",
        reading(bids, asks),
    ]
    return "\n".join(lines)


def record_findings(text: str) -> None:
    """Writes `text` into OBSERVED.md between its markers, replacing an earlier run's, or
    directly above the anchor line when there is none."""
    current = OBSERVED.read_text(encoding="utf-8")
    block = f"{BEGIN}\n{text.rstrip()}\n{END}"
    if BEGIN in current and END in current:
        head, rest = current.split(BEGIN, 1)
        _, tail = rest.split(END, 1)
        current = head + block + tail
    else:
        current = current.replace(ANCHOR, f"{block}\n\n{ANCHOR}", 1)
    capture_common.write_raw(OBSERVED, current)


def write_fixtures(capture: Capture, stamp: str, seconds: float) -> None:
    files = [
        ("subscribed", capture.subscribed.frame, "the answer to the `subscribe`"),
        ("snapshot", capture.snapshot.frame, "the first `orderbook_snapshot` of the subscription"),
        (
            "deltas",
            [seen.frame for seen in capture.deltas],
            f"{len(capture.deltas)} `orderbook_delta` frames, as an array, in wire order",
        ),
        ("ok", capture.added.frame, "the `ok` answering `update_subscription` with `add_markets`"),
        ("unsubscribed", capture.unsubscribed.frame, "the `unsubscribed` answering `unsubscribe`"),
        ("error", capture.refused.frame, "the `error` answering a `subscribe` that names no market"),
    ]
    for name, value, _description in files:
        capture_common.write_json(OUT / f"{name}.json", value, ensure_ascii=True)
    capture_common.write_provenance(
        OUT,
        [
            "# Kalshi order-book socket fixtures",
            "",
            f"Captured {stamp} by `scripts/capture_kalshi_predictions_ws.py` from `{WS}` (the demo "
            "host), with the owner's demo key signing the handshake through "
            "`scripts/capture_kalshi_account.py`'s `signed_headers`. No key appears in a fixture: "
            "the handshake's headers are not frames.",
            "",
            f"Market `{capture.first}`; `{capture.second}` was added to the subscription. The "
            f"exchange: a `subscribe` (`orderbook_delta`, `use_yes_price: true`), up to "
            f"{seconds:.0f} s of frames (at most {MAX_DELTAS} deltas), an `update_subscription` "
            "`add_markets`, an `unsubscribe`, and a `subscribe` that names no market.",
            "",
            *capture_common.provenance_table(
                ("Fixture", "What it is"), [(f"`{name}.json`", what) for name, _value, what in files]
            ),
        ],
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Capture the order-book socket's demo frames.")
    parser.add_argument("--ticker", help="the demo market to subscribe (default: the first open one with a two-sided book)")
    parser.add_argument("--seconds", type=float, default=60.0, help="how long to read deltas (default 60)")
    args = parser.parse_args()
    require_key()
    require_anchor()
    first, second = choose(args.ticker)
    stamp = capture_common.stamp()
    try:
        capture = asyncio.run(run(first, second, args.seconds))
    except InvalidHandshake as err:
        raise SystemExit(
            f"the demo host refused the signed handshake ({err}): check the key id, and that the key is the demo environment's"
        ) from err
    except ConnectionClosed as err:
        raise SystemExit(f"the demo host closed the connection: {err}") from err
    write_fixtures(capture, stamp, args.seconds)
    record_findings(section(capture, stamp))
    print(f"captured 6 fixtures ({len(capture.deltas)} deltas) for {first} into {OUT}")
    print(f"recorded the findings in {OBSERVED}")


if __name__ == "__main__":
    main()
