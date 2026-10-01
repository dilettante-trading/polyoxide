#!/usr/bin/env python3
"""Capture live frames from Polymarket's sports feed as test fixtures.

Usage:
    uv run --with websockets --with certifi python3 scripts/capture_sports_fixtures.py OUT_DIR [SECONDS]

Records wss://sports-api.polymarket.com/ws for SECONDS (default 300). Keeps
the first frame of each distinct top-level key-set, and of each
`eventState.type`, byte for byte as `<league>-<n>.json`. Writes
PROVENANCE.md with the date, the frame count per league, the protocol ping
times and the longest gap between data frames.

Write to a scratch directory, not the fixtures directory. Copy the frames
worth keeping into polyoxide-sports/tests/fixtures/ and list each one in
polyoxide-sports/src/fixtures.rs; `every_fixture_file_is_listed` fails
otherwise. Run it in a busy window, such as a weekend afternoon UTC, to
see the most sports.
"""

import asyncio
import json
import re
import ssl
import sys
import time
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path

import certifi
from websockets.asyncio.client import ClientConnection, connect
from websockets.frames import Opcode

URL = "wss://sports-api.polymarket.com/ws"


class PingRecorder(ClientConnection):
    """Notes when each protocol ping arrives.

    The handshake response passes through `process_event` too, and it has
    no opcode, so the check must not assume one.
    """

    started = 0.0
    pings: list[float] = []

    def process_event(self, event):
        if getattr(event, "opcode", None) is Opcode.PING:
            PingRecorder.pings.append(round(time.monotonic() - PingRecorder.started, 2))
        return super().process_event(event)


async def record(seconds: int) -> list[tuple[float, str]]:
    """Every text frame received in `seconds`, with its arrival time."""
    ctx = ssl.create_default_context(cafile=certifi.where())
    frames = []
    PingRecorder.started = time.monotonic()
    async with connect(URL, ssl=ctx, ping_interval=None, create_connection=PingRecorder) as ws:
        end = PingRecorder.started + seconds
        while (remaining := end - time.monotonic()) > 0:
            try:
                message = await asyncio.wait_for(ws.recv(), remaining)
            except asyncio.TimeoutError:
                break
            if isinstance(message, str):
                frames.append((round(time.monotonic() - PingRecorder.started, 2), message))
    return frames


def shape_marks(raw: str) -> list[tuple[str, object]]:
    """What makes a frame's shape distinct: its key-set and eventState type."""
    try:
        frame = json.loads(raw)
    except json.JSONDecodeError:
        return [("unparsed", raw[:40])]
    if not isinstance(frame, dict):
        return [("not-an-object", type(frame).__name__)]
    marks: list[tuple[str, object]] = [("keys", tuple(sorted(frame)))]
    state = frame.get("eventState")
    if isinstance(state, dict):
        marks.append(("eventState", state.get("type")))
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

    captured_at = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    frames = asyncio.run(record(seconds))

    seen: set = set()
    kept = []
    for _, raw in frames:
        marks = shape_marks(raw)
        if any(mark not in seen for mark in marks):
            seen.update(marks)
            name = f"{slug(league_of(raw))}-{len(kept)}.json"
            # No trailing newline: the file is the frame, byte for byte.
            (out / name).write_text(raw, encoding="utf-8")
            kept.append((name, marks))

    times = [t for t, _ in frames]
    longest_gap = max((b - a for a, b in zip(times, times[1:])), default=0.0)
    leagues = Counter(league_of(raw) for _, raw in frames)
    lines = [
        "# Sports capture provenance",
        "",
        f"- Captured: {captured_at}, for {seconds} s, from `{URL}`",
        f"- Frames: {len(frames)}",
        f"- Protocol pings at (s): {PingRecorder.pings}",
        f"- Longest gap between data frames: {longest_gap:.1f} s",
        "",
        "| League | Frames |",
        "|---|---|",
        *[f"| {league} | {count} |" for league, count in leagues.most_common()],
        "",
        "| File | Shape |",
        "|---|---|",
        *[f"| `{name}` | {marks} |" for name, marks in kept],
    ]
    (out / "PROVENANCE.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{len(frames)} frames, {len(kept)} distinct shapes, written to {out}")


if __name__ == "__main__":
    main()
