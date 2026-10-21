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

Epic 9 adds two options, each its own mode:

    uv run scripts/capture_kalshi_predictions_ws.py --epic9 [--ticker TICKER] [--seconds 60]
    uv run scripts/capture_kalshi_predictions_ws.py --check-epic9

`--epic9` signs one demo session and writes ten more files under the same ws/ directory, a block
of PROVENANCE.md between its epic9 markers, and a block of OBSERVED.md above its Epic 9 anchor
(`<!-- epic 9 findings go above this line -->`). --seconds is how long it waits for a `ticker`
frame and a `trade` frame. `--check-epic9` makes no connection: it re-reads the files and exits 1
naming what is wrong.
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

# The open listing is paged: quoted markets can sit thousands of rows in (9 pages on 2026-10-21).
MAX_PAGES = 30
EPIC9_ANCHOR = "<!-- epic 9 findings go above this line -->"
EPIC9_BEGIN = "<!-- kalshi-ws-epic9:begin -->"
EPIC9_END = "<!-- kalshi-ws-epic9:end -->"
PROV_BEGIN = "<!-- epic9:begin -->"
PROV_END = "<!-- epic9:end -->"
CHANNELS = ("orderbook_delta", "ticker", "trade")
EPIC9_FILES = (
    "subscribed_ticker.json",
    "subscribed_trade.json",
    "ticker.json",
    "trade.json",
    "snapshot_answer.json",
    "deltas_after_snapshot.json",
    "snapshot_outside.json",
    "error_snapshot_ticker_sid.json",
    "error_snapshot_trade_sid.json",
    "multi_sid_answer.json",
)
# A command's answer is complete when this long passes with no further frame carrying its id.
SETTLE_SECONDS = 2.0
# Pause before each command: Kalshi refuses a subscription that sends too fast (code 27).
PACE_SECONDS = 0.3
# How long the capture reads after get_snapshot, and after the subscribe with no market.
SNAPSHOT_SECONDS = 20.0
EMPTY_SECONDS = 15.0
EMPTY_FRAMES = 20
# `tickerPayload.msg` and `tradePayload.msg` required keys in docs/specs/kalshi/asyncapi.yaml.
TICKER_REQUIRED = (
    "market_id", "market_ticker", "price_dollars", "yes_bid_dollars", "yes_ask_dollars", "volume_fp",
    "open_interest_fp", "dollar_volume", "dollar_open_interest", "yes_bid_size_fp", "yes_ask_size_fp",
    "last_trade_size_fp", "ts", "ts_ms", "time",
)
TRADE_REQUIRED = (
    "trade_id", "market_ticker", "yes_price_dollars", "no_price_dollars", "count_fp", "taker_side",
    "taker_outcome_side", "taker_book_side", "is_block_trade", "ts", "ts_ms",
)


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


def choose_four(wanted: str | None) -> tuple[str, str, str, str]:
    """`first` (the market asked for, else the first quoted open one with a two-sided book), then
    three more quoted open markets: `second`, `third` and `outside`."""
    tickers: list[str] = []
    cursor = None
    for _page in range(MAX_PAGES):
        params: dict[str, Any] = {"status": "open", "mve_filter": "exclude", "limit": 1000}
        if cursor:
            params["cursor"] = cursor
        listing = fetch_ok("/trade-api/v2/markets", **params)
        tickers += [market["ticker"] for market in listing["markets"] if quoted(market)]
        cursor = listing.get("cursor")
        if not cursor or len(tickers) >= CANDIDATES + 3:
            break
    first = wanted or next((t for t in tickers[:CANDIDATES] if two_sided(t)), None)
    if first is None:
        raise SystemExit(
            f"none of the first {CANDIDATES} quoted open demo markets has a two-sided book: pass --ticker"
        )
    others = [t for t in tickers if t != first]
    if len(others) < 3:
        raise SystemExit("the demo host lists fewer than four quoted open markets")
    return first, others[0], others[1], others[2]


async def exchange(tape: Tape, command: dict[str, Any]) -> list[Seen]:
    """Sends `command` and returns every frame that carries its id, read until SETTLE_SECONDS pass
    after the last one. Exits when none arrives in ANSWER_SECONDS. Frames with other ids stay on the
    tape."""
    await asyncio.sleep(PACE_SECONDS)
    await tape.ws.send(json.dumps(command))
    got: list[Seen] = []
    give_up = time.monotonic() + ANSWER_SECONDS
    settle: float | None = None
    while True:
        remaining = (give_up if settle is None else settle) - time.monotonic()
        if remaining <= 0:
            break
        seen = await tape.read(remaining)
        if seen is None:
            break
        if seen.frame.get("id") == command["id"]:
            got.append(seen)
            settle = time.monotonic() + SETTLE_SECONDS
    if not got:
        raise SystemExit(f"no frame answered {command['cmd']} (id {command['id']}) in {ANSWER_SECONDS:.0f} s")
    return got


async def exchange_on_sid(tape: Tape, command: dict[str, Any], sid: int) -> list[Seen]:
    """Sends `command` and returns the frames that answer it: those carrying its id, and an `error` on
    `sid` without one (the host answers `get_snapshot` on the `trade` channel's `sid` with an `error`
    that has no `id`). Reads ANSWER_SECONDS / 2, and returns what it found, none included: an
    unanswered command is a finding, not an exit."""
    await asyncio.sleep(PACE_SECONDS)
    mark = len(tape.log)
    await tape.ws.send(json.dumps(command))
    await tape.read_for(ANSWER_SECONDS / 2)
    return [
        seen
        for seen in tape.log[mark:]
        if seen.frame.get("id") == command["id"]
        or (seen.frame.get("type") == "error" and seen.frame.get("sid") == sid)
    ]


def detail(frames: list[Seen]) -> str:
    """Each frame's type, error code and message, `id`, `sid` and `seq`, or that none arrived."""
    if not frames:
        return "no frame carrying the id or an `error` on the `sid` within the wait"
    parts = []
    for seen in frames:
        frame = seen.frame
        msg = frame.get("msg") or {}
        what = f"`{frame.get('type')}`"
        if frame.get("type") == "error":
            what += f" code {msg.get('code')} ({msg.get('msg')!r})"
        parts.append(
            f"{what}, `id` {frame.get('id', 'absent')}, `sid` {frame.get('sid', 'absent')}, `seq` {frame.get('seq', 'absent')}"
        )
    return "; ".join(parts)


def outcome(frames: list[Seen]) -> str:
    """The frames' types, and an error's code and message."""
    parts = []
    for seen in frames:
        msg = seen.frame.get("msg") or {}
        if seen.frame.get("type") == "error":
            parts.append(f"`error` code {msg.get('code')}: {msg.get('msg')}")
        else:
            parts.append(f"`{seen.frame.get('type')}`")
    return ", ".join(parts)


def is_snapshot_of(seen: Seen, ticker: str) -> bool:
    return seen.frame.get("type") == "orderbook_snapshot" and (seen.frame.get("msg") or {}).get("market_ticker") == ticker


@dataclass
class Epic9:
    tickers: tuple[str, str, str, str]
    sids: dict[str, int]
    subscribed: dict[str, Seen]
    ticker: Seen
    trade: Seen
    answer_frames: list[Seen]
    answer: Seen
    has_id: bool
    between: int
    deltas_after: list[Seen]
    multi: list[Seen]
    third_snapshot: bool
    outside: list[Seen]
    on_ticker_sid: list[Seen]
    on_trade_sid: list[Seen]
    empty_answers: list[Seen]
    empty_tickers: list[str]
    empty_count: int
    final: list[Seen]
    too_fast: int


async def run_epic9(tickers: tuple[str, str, str, str], seconds: float) -> Epic9:
    first, second, third, outside = tickers
    ids = iter(range(1, 1000))
    async with capture_common.ws_session(WS, headers=signed) as ws:
        tape = Tape(ws)

        subscribe = {
            "id": next(ids),
            "cmd": "subscribe",
            "params": {"channels": list(CHANNELS), "market_tickers": [first, second], "use_yes_price": True},
        }
        answers = await exchange(tape, subscribe)
        subscribed: dict[str, Seen] = {}
        for seen in answers:
            expect(seen, "subscribed", "the three-channel subscribe")
            channel = (seen.frame.get("msg") or {}).get("channel")
            if channel is None:
                raise SystemExit(f"a `subscribed` answer names no channel: {seen.text}")
            subscribed[channel] = seen
        if sorted(subscribed) != sorted(CHANNELS):
            raise SystemExit(f"the three-channel subscribe was answered for {sorted(subscribed)}: {[s.text for s in answers]}")
        sids = {channel: seen.frame["msg"]["sid"] for channel, seen in subscribed.items()}

        def logged(kind: str) -> Seen | None:
            return next((s for s in tape.log if s.frame.get("type") == kind), None)

        deadline = time.monotonic() + seconds
        while (logged("ticker") is None or logged("trade") is None) and time.monotonic() < deadline:
            if await tape.read(deadline - time.monotonic()) is None:
                break
        ticker, trade = logged("ticker"), logged("trade")
        if ticker is None or trade is None:
            missing = [name for name, seen in (("ticker", ticker), ("trade", trade)) if seen is None]
            raise SystemExit(
                f"no {' and no '.join(missing)} frame for {first} or {second} in {seconds:.0f} s: "
                "rerun with a larger --seconds, or --ticker of a busier market"
            )

        book = sids["orderbook_delta"]
        mark = len(tape.log)
        command = {
            "id": next(ids),
            "cmd": "update_subscription",
            "params": {"sid": book, "action": "get_snapshot", "market_tickers": [first]},
        }
        answer_frames = await exchange(tape, command)
        await tape.read_for(SNAPSHOT_SECONDS)
        window = tape.log[mark:]
        index = next((i for i, seen in enumerate(window) if is_snapshot_of(seen, first)), None)
        if index is None:
            raise SystemExit(
                f"get_snapshot for {first} was answered with {outcome(answer_frames)}, and no snapshot of it arrived"
            )
        answer = window[index]
        between = sum(
            1 for seen in window[:index] if seen.frame.get("type") == "orderbook_delta" and seen.frame.get("sid") == book
        )
        deltas_after = [
            seen for seen in window[index + 1 :] if seen.frame.get("type") == "orderbook_delta" and seen.frame.get("sid") == book
        ][:MAX_DELTAS]
        if not deltas_after:
            raise SystemExit(
                f"no orderbook_delta followed the snapshot in {SNAPSHOT_SECONDS:.0f} s: rerun with --ticker of a busier market"
            )

        mark = len(tape.log)
        command = {
            "id": next(ids),
            "cmd": "update_subscription",
            "params": {"sids": [book, sids["ticker"]], "action": "add_markets", "market_tickers": [third]},
        }
        multi = await exchange(tape, command)
        await tape.read_for(ADDED_SECONDS)
        third_snapshot = any(is_snapshot_of(seen, third) for seen in tape.log[mark:])

        command = {
            "id": next(ids),
            "cmd": "update_subscription",
            "params": {"sid": book, "action": "get_snapshot", "market_tickers": [outside]},
        }
        refused_frames = await exchange(tape, command)
        # The owner's decision of 2026-10-21 (spec 3): the demo host does not refuse an outside ticker, so
        # whatever it answers is written as snapshot_outside.json and recorded; it is not an exit.

        on_ticker_sid = await exchange_on_sid(
            tape,
            {
                "id": next(ids),
                "cmd": "update_subscription",
                "params": {"sid": sids["ticker"], "action": "get_snapshot", "market_tickers": [first]},
            },
            sids["ticker"],
        )
        on_trade_sid = await exchange_on_sid(
            tape,
            {
                "id": next(ids),
                "cmd": "update_subscription",
                "params": {"sid": sids["trade"], "action": "get_snapshot", "market_tickers": [first]},
            },
            sids["trade"],
        )
        for name, frames in (("ticker", on_ticker_sid), ("trade", on_trade_sid)):
            if not any(seen.frame.get("type") == "error" for seen in frames):
                raise SystemExit(
                    f"get_snapshot on the {name} channel's sid was answered with {detail(frames)}, not an error: "
                    "no error fixture can be captured from it, so report this to the owner"
                )

        mark = len(tape.log)
        empty_answers = await exchange(
            tape, {"id": next(ids), "cmd": "subscribe", "params": {"channels": ["ticker", "trade"]}}
        )
        fresh = [
            seen.frame["msg"]["sid"]
            for seen in empty_answers
            if seen.frame.get("type") == "subscribed" and isinstance((seen.frame.get("msg") or {}).get("sid"), int)
        ]

        def fresh_frames() -> list[Seen]:
            return [
                seen
                for seen in tape.log[mark:]
                if seen.frame.get("type") in ("ticker", "trade") and seen.frame.get("sid") in fresh
            ]

        deadline = time.monotonic() + EMPTY_SECONDS
        while fresh and len(fresh_frames()) < EMPTY_FRAMES and time.monotonic() < deadline:
            if await tape.read(deadline - time.monotonic()) is None:
                break
        seen_on_fresh = fresh_frames()
        empty_tickers = sorted({(seen.frame.get("msg") or {}).get("market_ticker") for seen in seen_on_fresh} - {None})

        final = await exchange(
            tape, {"id": next(ids), "cmd": "unsubscribe", "params": {"sids": [*sids.values(), *fresh]}}
        )
        too_fast = sum(
            1
            for seen in tape.log
            if seen.frame.get("type") == "error" and (seen.frame.get("msg") or {}).get("code") == 27
        )
        return Epic9(
            tickers, sids, subscribed, ticker, trade, answer_frames, answer,
            any(seen is answer for seen in answer_frames), between, deltas_after, multi,
            third_snapshot, refused_frames, on_ticker_sid, on_trade_sid, empty_answers, empty_tickers,
            len(seen_on_fresh), final, too_fast,
        )


def shape(frame: dict[str, Any], required: tuple[str, ...]) -> str:
    msg = frame.get("msg") or {}
    missing = [key for key in required if key not in msg]
    extra = sorted(key for key in msg if key not in required)
    absent = ", ".join(f"`{key}`" for key in missing) or "none"
    more = ", ".join(f"`{key}`" for key in extra) or "none"
    return f"required keys absent: {absent}; keys the mirror lacks: {more}"


def section9(c: Epic9, stamp: str) -> str:
    first, second, third, outside = c.tickers
    book = c.sids["orderbook_delta"]
    seq, previous = c.answer.frame.get("seq"), c.answer.previous
    first_delta = c.deltas_after[0].frame.get("seq")
    on_numbering = isinstance(seq, int) and previous is not None and seq == previous + 1
    continues = isinstance(seq, int) and first_delta == seq + 1
    elsewhere = sum(
        1 for seen in c.deltas_after if (seen.frame.get("msg") or {}).get("market_ticker") != first
    )
    d9 = "holds" if continues else "**DOES NOT HOLD. Report this to the owner before Story 5 runs.**"
    beyond = [t for t in c.empty_tickers if t not in c.tickers]
    rows = [
        (
            "One `subscribe` for the three channels",
            f"answered by {len(c.subscribed)} `subscribed` frames, one for each channel, each with its own `sid`: "
            + ", ".join(f"`{channel}` {sid}" for channel, sid in c.sids.items()),
        ),
        (
            "A `ticker` frame",
            f"on `sid` {c.ticker.frame.get('sid')} (the answered {c.sids['ticker']}); carries `seq`: "
            f"{'yes' if 'seq' in c.ticker.frame else 'no'}; {shape(c.ticker.frame, TICKER_REQUIRED)}",
        ),
        (
            "A `trade` frame",
            f"on `sid` {c.trade.frame.get('sid')} (the answered {c.sids['trade']}); carries `seq`: "
            f"{'yes' if 'seq' in c.trade.frame else 'no'}; {shape(c.trade.frame, TRADE_REQUIRED)}",
        ),
        ("`get_snapshot` for `first` on the order-book `sid`: the frames carrying its id", outcome(c.answer_frames)),
        (
            "The snapshot answering it",
            f"carries the command's `id`: {'yes' if c.has_id else 'no'}; `sid` {c.answer.frame.get('sid')} "
            f"(the order-book `sid` is {book}); `seq` {seq}; the last `seq` on that `sid` before it was {previous}; "
            f"{c.between} deltas on the `sid` came between the command and the snapshot",
        ),
        (
            "The snapshot's `seq` is the next number of the `sid`'s numbering",
            "yes" if on_numbering else f"no: {previous} before it, {seq} on it",
        ),
        (
            "The first delta after the snapshot continues from its `seq`",
            f"{'yes' if continues else 'no'}: `seq` {first_delta} after {seq}; {len(c.deltas_after)} deltas captured, "
            f"{elsewhere} of them for a market other than `first`",
        ),
        ("Spec D9's premise (the answer is on the `sid`'s numbering and the next delta follows it)", d9),
        (
            "One `update_subscription` naming two `sids` (`add_markets` for `third`)",
            f"{outcome(c.multi)}; a snapshot of `third` followed on the order-book `sid`: {'yes' if c.third_snapshot else 'no'}",
        ),
        (
            "`get_snapshot` for a ticker outside the subscription (the owner took the host's answer in place of an error fixture, spec 3)",
            f"{outcome(c.outside)}; the frame carrying the command's `id`: "
            + ", ".join(
                f"`{seen.frame.get('type')}` for `{(seen.frame.get('msg') or {}).get('market_ticker')}`, "
                f"`sid` {seen.frame.get('sid')}, `seq` {seen.frame.get('seq')} (the last before it {seen.previous})"
                for seen in c.outside
                if seen.frame.get("id") is not None
            ),
        ),
        ("`get_snapshot` on the `ticker` channel's `sid`", detail(c.on_ticker_sid)),
        ("`get_snapshot` on the `trade` channel's `sid`", detail(c.on_trade_sid)),
        (
            "A `subscribe` for `ticker` and `trade` naming no market",
            f"{outcome(c.empty_answers)}; {c.empty_count} frames in {EMPTY_SECONDS:.0f} s on its `sid`s from "
            f"{len(c.empty_tickers)} markets, {len(beyond)} of them outside the four chosen: "
            + ("it means every market" if beyond else "no market beyond the four was seen, so this does not show it means every market"),
        ),
        ("The final `unsubscribe` of every `sid`", outcome(c.final)),
        ("Code 27 (too many requests) met", str(c.too_fast)),
    ]
    lines = [
        "### The multi-channel socket, as the demo host answers it",
        "",
        f"Captured {stamp} by `scripts/capture_kalshi_predictions_ws.py --epic9` from `{WS}`. `first` `{first}` and "
        f"`second` `{second}` were subscribed, `third` `{third}` added by the two-`sid` update, and `outside` `{outside}` "
        "never subscribed. Fixtures: `polyoxide-kalshi/tests/fixtures/predictions/ws/`.",
        "",
        *capture_common.provenance_table(("Question", "Observed"), rows),
    ]
    return "\n".join(lines)


def record_block(path: Path, begin: str, end: str, anchor: str | None, text: str) -> None:
    """Writes `text` into `path` between `begin` and `end`, replacing an earlier run's block, or
    directly above `anchor` (the end of the file when `anchor` is None) when there is none."""
    current = path.read_text(encoding="utf-8")
    block = f"{begin}\n{text.rstrip()}\n{end}"
    if begin in current and end in current:
        head, rest = current.split(begin, 1)
        _, tail = rest.split(end, 1)
        current = head + block + tail
    elif anchor is None:
        current = current.rstrip("\n") + f"\n\n{block}\n"
    else:
        current = current.replace(anchor, f"{block}\n\n{anchor}", 1)
    capture_common.write_raw(path, current)


def write_epic9(c: Epic9, stamp: str, seconds: float) -> None:
    first, second, third, outside = c.tickers
    multi = c.multi[0].frame if len(c.multi) == 1 else [seen.frame for seen in c.multi]
    files = [
        ("subscribed_ticker.json", c.subscribed["ticker"].frame, "the `subscribed` answer for the `ticker` channel, from the three-channel `subscribe`"),
        ("subscribed_trade.json", c.subscribed["trade"].frame, "the `subscribed` answer for the `trade` channel, from the same `subscribe`"),
        ("ticker.json", c.ticker.frame, "the first `ticker` frame of the session"),
        ("trade.json", c.trade.frame, "the first `trade` frame of the session"),
        ("snapshot_answer.json", c.answer.frame, "the `orderbook_snapshot` answering `get_snapshot` for `first`, with its `id`, `sid` and `seq`"),
        (
            "deltas_after_snapshot.json",
            [seen.frame for seen in c.deltas_after],
            f"{len(c.deltas_after)} `orderbook_delta` frames on that `sid` after it, as an array, in wire order (every market of the `sid`, not only `first`)",
        ),
        ("snapshot_outside.json", next(s for s in c.outside if s.frame.get("id") is not None).frame, f"the answer to `get_snapshot` for `{outside}`, a ticker outside the subscription: an `orderbook_snapshot` carrying the command's `id`, as the host sent it"),
        ("error_snapshot_ticker_sid.json", next(s for s in c.on_ticker_sid if s.frame.get("type") == "error").frame, "the `error` answering `get_snapshot` on the `ticker` channel's `sid`"),
        ("error_snapshot_trade_sid.json", next(s for s in c.on_trade_sid if s.frame.get("type") == "error").frame, "the `error` answering `get_snapshot` on the `trade` channel's `sid`, which carries no `id`"),
        ("multi_sid_answer.json", multi, "the answer to one `update_subscription` naming two `sids` (an array when there is more than one frame)"),
    ]
    for name, value, _what in files:
        capture_common.write_json(OUT / name, value, ensure_ascii=True)
    lines = [
        "## Epic 9 captures",
        "",
        f"Captured {stamp} by `scripts/capture_kalshi_predictions_ws.py --epic9` from `{WS}` (the demo host), the "
        "handshake signed as above. A `subscribe` for `orderbook_delta`, `ticker` and `trade` on "
        f"`{first}` and `{second}` (`use_yes_price: true`), up to {seconds:.0f} s for a `ticker` and a `trade` "
        f"frame, then the commands of spec §3 in order. `{third}` was added by the two-`sid` update and `{outside}` "
        "was never subscribed.",
        "",
        *capture_common.provenance_table(
            ("Fixture", "What it is"), [(f"`{name}`", what) for name, _value, what in files]
        ),
    ]
    record_block(OUT / "PROVENANCE.md", PROV_BEGIN, PROV_END, None, "\n".join(lines))


def capture_epic9(args: argparse.Namespace) -> None:
    require_key()
    if EPIC9_ANCHOR not in OBSERVED.read_text(encoding="utf-8"):
        raise SystemExit(f"{OBSERVED} has no anchor line {EPIC9_ANCHOR!r}: Task 1.2 writes it")
    tickers = choose_four(args.ticker)
    stamp = capture_common.stamp()
    try:
        capture = asyncio.run(run_epic9(tickers, args.seconds))
    except InvalidHandshake as err:
        raise SystemExit(
            f"the demo host refused the signed handshake ({err}): check the key id, and that the key is the demo environment's"
        ) from err
    except ConnectionClosed as err:
        raise SystemExit(f"the demo host closed the connection: {err}") from err
    write_epic9(capture, stamp, args.seconds)
    record_block(OBSERVED, EPIC9_BEGIN, EPIC9_END, EPIC9_ANCHOR, section9(capture, stamp))
    print(f"captured {len(EPIC9_FILES)} fixtures for {tickers[0]} into {OUT}")
    print(f"recorded the findings in {OBSERVED}")


def check_epic9() -> None:
    """Re-reads Epic 9's ten fixtures, PROVENANCE.md and OBSERVED.md, and exits 1 naming each
    thing that is wrong."""
    problems: list[str] = []

    def load(name: str) -> Any:
        try:
            return json.loads((OUT / name).read_text(encoding="utf-8"))
        except (OSError, ValueError) as err:
            problems.append(f"{name}: {err}")
            return None

    def is_frame(value: Any, kind: str, name: str) -> bool:
        if not isinstance(value, dict) or value.get("type") != kind:
            problems.append(f"{name}: expected a `{kind}` frame")
            return False
        return True

    sids: dict[str, Any] = {}
    for channel in ("ticker", "trade"):
        name = f"subscribed_{channel}.json"
        frame = load(name)
        if frame is not None and is_frame(frame, "subscribed", name):
            msg = frame.get("msg") or {}
            if msg.get("channel") != channel or not isinstance(msg.get("sid"), int):
                problems.append(f"{name}: expected `msg.channel` {channel!r} and an integer `msg.sid`")
            if not isinstance(frame.get("id"), int):
                problems.append(f"{name}: expected an integer `id` on the frame")
            sids[channel] = msg.get("sid")
    if len(sids) == 2 and sids["ticker"] == sids["trade"]:
        problems.append("subscribed_ticker.json and subscribed_trade.json carry the same `sid`")

    for kind in ("ticker", "trade"):
        name = f"{kind}.json"
        frame = load(name)
        if frame is not None and is_frame(frame, kind, name):
            if not isinstance(frame.get("sid"), int) or not isinstance(frame.get("msg"), dict):
                problems.append(f"{name}: expected an integer `sid` and a `msg` object")
            elif sids.get(kind) is not None and frame["sid"] != sids[kind]:
                problems.append(f"{name}: `sid` {frame['sid']} is not the {kind} answer's {sids[kind]}")
            if kind == "ticker" and "seq" in frame:
                problems.append(f"{name}: the `ticker` channel carries no `seq`, but this frame has one")
            if kind == "trade" and not isinstance(frame.get("seq"), int):
                problems.append(f"{name}: expected an integer `seq` on the `trade` frame")

    answer = load("snapshot_answer.json")
    answer_sid = answer_seq = None
    if answer is not None and is_frame(answer, "orderbook_snapshot", "snapshot_answer.json"):
        answer_sid, answer_seq = answer.get("sid"), answer.get("seq")
        for key in ("id", "sid", "seq"):
            if not isinstance(answer.get(key), int):
                problems.append(f"snapshot_answer.json: expected an integer `{key}`")
    deltas = load("deltas_after_snapshot.json")
    if deltas is not None:
        if not isinstance(deltas, list) or not deltas:
            problems.append("deltas_after_snapshot.json: expected a non-empty array")
        else:
            seqs = [d.get("seq") if isinstance(d, dict) else None for d in deltas]
            if not all(isinstance(d, dict) and d.get("type") == "orderbook_delta" for d in deltas):
                problems.append("deltas_after_snapshot.json: every item must be an `orderbook_delta` frame")
            elif answer_sid is not None and any(d.get("sid") != answer_sid for d in deltas):
                problems.append("deltas_after_snapshot.json: a delta is not on the snapshot answer's `sid`")
            elif not all(isinstance(s, int) for s in seqs) or seqs != sorted(set(seqs)):
                problems.append("deltas_after_snapshot.json: `seq` must be integers in strictly increasing order")
            elif isinstance(answer_seq, int) and seqs[0] <= answer_seq:
                problems.append("deltas_after_snapshot.json: the first delta's `seq` does not come after the answer's")
    outside = load("snapshot_outside.json")
    if outside is not None and is_frame(outside, "orderbook_snapshot", "snapshot_outside.json"):
        for key in ("id", "sid", "seq"):
            if not isinstance(outside.get(key), int):
                problems.append(f"snapshot_outside.json: expected an integer `{key}`")
        if not (outside.get("msg") or {}).get("market_ticker"):
            problems.append("snapshot_outside.json: expected a `msg.market_ticker`")
    for channel in ("ticker", "trade"):
        name = f"error_snapshot_{channel}_sid.json"
        refusal = load(name)
        if refusal is not None and is_frame(refusal, "error", name):
            if not isinstance((refusal.get("msg") or {}).get("code"), int):
                problems.append(f"{name}: expected an integer `msg.code`")
            if sids.get(channel) is not None and refusal.get("sid") != sids[channel]:
                problems.append(f"{name}: `sid` is not the {channel} answer's {sids[channel]}")
    multi = load("multi_sid_answer.json")
    if multi is not None and not (
        (isinstance(multi, dict) and multi) or (isinstance(multi, list) and multi and all(isinstance(f, dict) for f in multi))
    ):
        problems.append("multi_sid_answer.json: expected a frame or a non-empty array of frames")

    try:
        provenance = (OUT / "PROVENANCE.md").read_text(encoding="utf-8")
    except OSError as err:
        problems.append(f"PROVENANCE.md: {err}")
    else:
        absent = [name for name in EPIC9_FILES if name not in provenance]
        if absent:
            problems.append("PROVENANCE.md does not mention " + ", ".join(absent))
    observed = OBSERVED.read_text(encoding="utf-8")
    if EPIC9_BEGIN not in observed or EPIC9_END not in observed:
        problems.append(f"{OBSERVED} holds no block between {EPIC9_BEGIN} and {EPIC9_END}")

    for problem in problems:
        print(f"  {problem}")
    if problems:
        raise SystemExit(f"{len(problems)} problem(s) in {OUT}")
    print(f"ok: {len(EPIC9_FILES)} Epic 9 socket fixtures in {OUT}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Capture the order-book socket's demo frames.")
    parser.add_argument("--ticker", help="the demo market to subscribe (default: the first open one with a two-sided book)")
    parser.add_argument("--seconds", type=float, default=60.0, help="how long to read deltas (default 60)")
    parser.add_argument("--check-epic9", action="store_true", help="re-read Epic 9's fixtures and exit 1 on a fault; no connection")
    parser.add_argument("--epic9", action="store_true", help="capture Epic 9's multi-channel frames (--seconds is the wait for a ticker and a trade frame)")
    args = parser.parse_args()
    if args.check_epic9:
        check_epic9()
        return
    if args.epic9:
        capture_epic9(args)
        return
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
