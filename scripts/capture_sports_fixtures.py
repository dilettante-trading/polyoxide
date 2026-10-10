#!/usr/bin/env python3
"""Capture live frames from Polymarket's sports feed as test fixtures.

Usage:
    uv run --with websockets --with certifi python3 scripts/capture_sports_fixtures.py OUT_DIR [SECONDS]

Records wss://sports-api.polymarket.com/ws for SECONDS (default 300) and
keeps, byte for byte as `<league>-<n>.json`, the first frame of each shape
not seen before. A shape is any of: the top-level keys with each value's JSON
type, the `eventState.type`, the `status` value, the `(live, ended)` pair, or
the league, since score formats differ by sport. Writes PROVENANCE.md with
the date, what ended the capture, frame counts per league, binary frames,
protocol ping times and the longest gap between data frames.

A dropped connection or Ctrl-C ends the capture early, and everything
received until then is still written.

Write to a scratch directory, not the fixtures directory. Copy the frames
worth keeping into polyoxide-sports/tests/fixtures/ and list each one in
polyoxide-sports/src/fixtures.rs; `every_fixture_file_is_listed` fails
otherwise. Run it in a busy window, such as a weekend afternoon UTC, to
see the most sports.
"""

import asyncio
import json
import re
import sys
import time
from collections import Counter
from pathlib import Path

from websockets.asyncio.client import ClientConnection
from websockets.exceptions import ConnectionClosed
from websockets.frames import Opcode

import capture_common

URL = "wss://sports-api.polymarket.com/ws"


class Capture:
    """Everything recorded, held outside the coroutine so that an interrupted
    capture still has it."""

    def __init__(self) -> None:
        self.started = time.monotonic()
        self.frames: list[tuple[float, str]] = []
        self.pings: list[float] = []
        self.binary = 0
        self.ended_by = "the time limit"

    def elapsed(self) -> float:
        return round(time.monotonic() - self.started, 2)


CAPTURE = Capture()


class PingRecorder(ClientConnection):
    """Notes when each protocol ping arrives.

    The handshake response passes through `process_event` too, and it has
    no opcode, so the check must not assume one.
    """

    def process_event(self, event):
        if getattr(event, "opcode", None) is Opcode.PING:
            CAPTURE.pings.append(CAPTURE.elapsed())
        return super().process_event(event)


async def record(seconds: int) -> None:
    """Receive for `seconds`, into CAPTURE."""
    CAPTURE.started = time.monotonic()
    async with capture_common.ws_session(
        URL, ping_interval=None, create_connection=PingRecorder
    ) as ws:
        end = CAPTURE.started + seconds
        while (remaining := end - time.monotonic()) > 0:
            try:
                message = await asyncio.wait_for(ws.recv(), remaining)
            except asyncio.TimeoutError:
                return
            except ConnectionClosed as closed:
                CAPTURE.ended_by = f"the connection closing at {CAPTURE.elapsed()} s: {closed}"
                return
            if isinstance(message, str):
                CAPTURE.frames.append((CAPTURE.elapsed(), message))
            else:
                CAPTURE.binary += 1


def json_type(value: object) -> str:
    """The JSON type, with integers told apart from other numbers: a `gameId`
    arriving as `1.0` would break a `u64` field."""
    return {
        dict: "object",
        list: "array",
        str: "string",
        bool: "boolean",
        int: "integer",
        type(None): "null",
    }.get(type(value), "number")


def hashable(value: object) -> object:
    """A scalar as itself; an object or array as its canonical JSON. A value
    that turns into an object is exactly the drift worth keeping, and must
    not crash the set it is checked against."""
    if isinstance(value, (str, int, float, bool, type(None))):
        return value
    return json.dumps(value, sort_keys=True)


def shape_marks(raw: str) -> list[tuple[str, object]]:
    """Every shape a frame has; it is kept if any of them is new."""
    try:
        frame = json.loads(raw)
    except json.JSONDecodeError:
        return [("unparsed", raw[:40])]
    if not isinstance(frame, dict):
        return [("not-an-object", type(frame).__name__)]
    marks: list[tuple[str, object]] = [
        ("keys", tuple(sorted((key, json_type(value)) for key, value in frame.items()))),
        ("status", hashable(frame.get("status"))),
        ("live-ended", (hashable(frame.get("live")), hashable(frame.get("ended")))),
        ("league", hashable(frame.get("leagueAbbreviation"))),
    ]
    state = frame.get("eventState")
    if isinstance(state, dict):
        marks.append(("eventState", hashable(state.get("type"))))
    return marks


def league_of(raw: str) -> str:
    try:
        return str(json.loads(raw).get("leagueAbbreviation", "unknown"))
    except (json.JSONDecodeError, AttributeError):
        return "unparsed"


def slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", text.lower()).strip("_") or "unknown"


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    out = Path(sys.argv[1])
    seconds = int(sys.argv[2]) if len(sys.argv) > 2 else 300
    out.mkdir(parents=True, exist_ok=True)

    captured_at = capture_common.stamp("%Y-%m-%d %H:%M UTC")
    try:
        asyncio.run(record(seconds))
    except KeyboardInterrupt:
        CAPTURE.ended_by = f"Ctrl-C at {CAPTURE.elapsed()} s"
    frames = CAPTURE.frames

    seen: set = set()
    kept = []
    for _, raw in frames:
        new = [mark for mark in shape_marks(raw) if mark not in seen]
        if new:
            seen.update(new)
            name = f"{slug(league_of(raw))}-{len(kept)}.json"
            # Bytes, not text: no newline translation on any platform, and the
            # received UTF-8 re-encodes exactly. No trailing newline.
            capture_common.write_raw(out / name, raw)
            kept.append((name, new))

    times = [t for t, _ in frames]
    if len(times) > 1:
        longest_gap = f"{max(b - a for a, b in zip(times, times[1:])):.1f} s"
    else:
        longest_gap = "n/a, fewer than two frames"
    leagues = Counter(league_of(raw) for _, raw in frames)
    lines = [
        "# Sports capture provenance",
        "",
        f"- Captured: {captured_at}, for up to {seconds} s, from `{URL}`",
        f"- Ended by: {CAPTURE.ended_by}",
        f"- Text frames: {len(frames)}; binary frames: {CAPTURE.binary}",
        f"- Protocol pings at (s): {CAPTURE.pings}",
        f"- Longest gap between data frames: {longest_gap}",
        "",
        *capture_common.provenance_table(
            ("League", "Frames"),
            [(league, str(count)) for league, count in leagues.most_common()],
            short_rule=True,
        ),
        "",
        *capture_common.provenance_table(
            ("File", "New shapes"), [(f"`{name}`", str(new)) for name, new in kept], short_rule=True
        ),
    ]
    capture_common.write_provenance(out, lines)
    print(f"{len(frames)} frames, {len(kept)} kept, written to {out}")


if __name__ == "__main__":
    main()
