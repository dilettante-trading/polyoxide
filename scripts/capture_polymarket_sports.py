#!/usr/bin/env python3
"""Capture live frames from Polymarket's sports feed as test fixtures.

Usage:
    uv run --with websockets --with certifi python3 scripts/capture_polymarket_sports.py OUT_DIR [SECONDS]
    uv run --with websockets --with certifi python3 scripts/capture_polymarket_sports.py --cadence MINUTES [SECONDS]

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
worth keeping into polyoxide-polymarket/tests/fixtures/sports/ and list each one in
polyoxide-polymarket/src/sports/fixtures.rs; `every_fixture_file_is_listed` fails
otherwise. Run it in a busy window, such as a weekend afternoon UTC, to
see the most sports.

`--cadence MINUTES` is the browser-transport capture (spec D9), and writes
nothing to disk. Run it with no game live. It sends the text `ping` every
MINUTES minutes (SECONDS defaults to 1800, so `--cadence 10` pings at minutes
10 and 20) and nothing else, and prints, to stdout, the longest stretch with
no data frame (what a browser watches, since it sees no protocol ping),
whether each text ping was answered, and the rule that follows: A (the longest
silence is under the 45 s staleness limit: nothing changes), B (every ping was
answered: a text keepalive becomes the liveness) or C (otherwise: no staleness
check on a transport that hides the server's pings). A dropped connection is
reopened once and the capture runs on for the time lost, the pings with it, and
never more of them than an undropped run sends; a second drop ends it with
exit code 2, and the story stops and goes to the owner. Data frames in every
minute mean a game was live: the silence is then a live feed's, not a quiet
hour's, so the summary says "game live throughout" in place of a rule, the
script exits with code 3, and the story stops and goes to the owner there too.
Exit codes of `--cadence`: 0 a rule was reached, 2 a second drop, 3 a game live
throughout.
"""

import asyncio
import json
import math
import re
import sys
import time
from collections import Counter
from pathlib import Path

from websockets.asyncio.client import ClientConnection
from websockets.exceptions import ConnectionClosed, InvalidHandshake
from websockets.frames import Opcode

import capture_common

URL = "wss://sports-api.polymarket.com/ws"

# The staleness limit the supervised feed defaults to, and how long after a
# text ping a non-update frame counts as its answer.
STALE_AFTER = 45.0
ANSWER_WINDOW = 10.0

# The span in which a data frame means a game is live.
MINUTE = 60.0


class Capture:
    """Everything recorded, held outside the coroutine so that an interrupted
    capture still has it."""

    def __init__(self) -> None:
        self.started = time.monotonic()
        self.frames: list[tuple[float, str]] = []
        self.pings: list[float] = []
        self.binary = 0
        self.ended_by = "the time limit"
        # The cadence capture: when each text ping went out, each answer as
        # (ping time, arrival time, text), each drop as (time, why), and the
        # connected spans as (start, stop).
        self.sent: list[float] = []
        self.replies: list[tuple[float, float, str]] = []
        self.drops: list[tuple[float, str]] = []
        self.segments: list[tuple[float, float]] = []

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


def is_update(raw: str) -> bool:
    """Whether a text frame is a match update, which every one is keyed on `gameId`."""
    try:
        frame = json.loads(raw)
    except json.JSONDecodeError:
        return False
    return isinstance(frame, dict) and "gameId" in frame


def note_frame(message: str | bytes) -> None:
    """File one received frame: a binary one counted, a text one that follows a
    text ping within ANSWER_WINDOW and is no match update as that ping's answer,
    any other as a data frame."""
    if not isinstance(message, str):
        CAPTURE.binary += 1
        return
    now = CAPTURE.elapsed()
    if CAPTURE.sent and now - CAPTURE.sent[-1] <= ANSWER_WINDOW and not is_update(message):
        CAPTURE.replies.append((CAPTURE.sent[-1], now, message[:200]))
    else:
        CAPTURE.frames.append((now, message))


class PingPlan:
    """When the text ping goes out: every `cadence` seconds from the start, no
    more than an undropped run of `seconds` sends (D17 allows those and no
    other), and pushed out by the time a drop costs, since the cadence counts
    connected time. It reads no clock and opens no socket: the caller passes
    the time."""

    def __init__(self, started: float, seconds: float, cadence: float) -> None:
        self.cadence = cadence
        self.next_ping = started + cadence
        # The pings that fall before the end: at 10 and 20 minutes of 30, not 30.
        self.left = max(math.ceil(seconds / cadence) - 1, 0)

    def wait(self, now: float, remaining: float) -> float:
        """How long to read before the next ping is due, or to the end when no
        ping is left."""
        if self.left == 0:
            return remaining
        return min(remaining, max(self.next_ping - now, 0.0))

    def due(self, now: float) -> bool:
        return self.left > 0 and now >= self.next_ping

    def sent(self) -> None:
        self.next_ping += self.cadence
        self.left -= 1

    def lose(self, lost: float) -> None:
        """A drop cost `lost` seconds, which the end moves out by too."""
        self.next_ping += lost


async def record_cadence(seconds: float, cadence: float) -> None:
    """Receive for `seconds` of connected time, sending the text `ping` every
    `cadence` seconds and nothing else, into CAPTURE.

    A drop is recorded and the connection reopened once, and the end moves out
    by the time lost. A second drop ends the capture with `ended_by` saying so.
    """
    CAPTURE.started = time.monotonic()
    end = CAPTURE.started + seconds
    plan = PingPlan(CAPTURE.started, seconds, cadence)
    dropped_at: float | None = None
    while end - time.monotonic() > 0:
        segment_start = CAPTURE.elapsed()
        try:
            async with capture_common.ws_session(
                URL, ping_interval=None, create_connection=PingRecorder
            ) as ws:
                segment_start = CAPTURE.elapsed()
                if dropped_at is not None:
                    lost = time.monotonic() - dropped_at
                    end += lost
                    plan.lose(lost)
                    dropped_at = None
                try:
                    while (remaining := end - time.monotonic()) > 0:
                        wait = plan.wait(time.monotonic(), remaining)
                        try:
                            message = await asyncio.wait_for(ws.recv(), wait)
                        except asyncio.TimeoutError:
                            if plan.due(time.monotonic()) and end - time.monotonic() > 0:
                                await ws.send("ping")
                                CAPTURE.sent.append(CAPTURE.elapsed())
                                plan.sent()
                            continue
                        note_frame(message)
                finally:
                    CAPTURE.segments.append((segment_start, CAPTURE.elapsed()))
        except (ConnectionClosed, InvalidHandshake, OSError) as err:
            CAPTURE.drops.append((CAPTURE.elapsed(), str(err)))
            if len(CAPTURE.drops) > 1:
                CAPTURE.ended_by = f"a second drop at {CAPTURE.elapsed()} s: {err}"
                return
            dropped_at = time.monotonic()


def longest_silence(frame_times: list[float], segments: list[tuple[float, float]]) -> float:
    """The longest stretch inside one connected span with no data frame: the
    silence a browser watches, since it sees no protocol ping."""
    longest = 0.0
    for start, stop in segments:
        marks = [start, *[t for t in frame_times if start <= t <= stop], stop]
        longest = max(longest, *(b - a for a, b in zip(marks, marks[1:])))
    return longest


def live_throughout(frame_times: list[float], segments: list[tuple[float, float]]) -> bool:
    """Whether data frames arrived in every minute of connected time: a game was
    live, so the silence measured is a live feed's and says nothing of a quiet
    hour (D17 stops there). A minute counts when it lies whole inside one
    connected span, counted from the span's start, so a capture of less than a
    minute has none and is never live throughout."""
    minutes = 0
    for start, stop in segments:
        at = start
        while at + MINUTE <= stop:
            minutes += 1
            if not any(at <= t < at + MINUTE for t in frame_times):
                return False
            at += MINUTE
    return minutes > 0


def rule_for(
    silence: float, sent: list[float], replies: list[tuple[float, float, str]]
) -> str:
    """D9's sports rule for what the capture saw, as the letter and what it means."""
    if len(CAPTURE.drops) > 1:
        return "stopped: a second drop ended the capture; report to the owner"
    if silence < STALE_AFTER:
        return "A: the longest silence is under 45 s; nothing changes"
    answered = {ping for ping, _, _ in replies}
    if sent and all(ping in answered for ping in sent):
        return "B: every text ping was answered; a text keepalive is the liveness"
    return "C: a text ping went unanswered; no staleness check on such a transport"


def cadence_summary(seconds: float, cadence: float) -> list[str]:
    times = [t for t, _ in CAPTURE.frames]
    silence = longest_silence(times, CAPTURE.segments)
    live = live_throughout(times, CAPTURE.segments)
    lines = [
        f"cadence capture: {seconds} s of connected time, text ping every {cadence} s",
        f"ended by: {CAPTURE.ended_by}",
        f"drops: {CAPTURE.drops}",
        f"data frames: {len(CAPTURE.frames)} (a game live shows here); binary frames: {CAPTURE.binary}",
        f"protocol pings at (s): {CAPTURE.pings}",
        f"longest silence between data frames: {silence:.1f} s",
        f"game live throughout: {'YES' if live else 'no'} (data frames in every minute of connected time)",
    ]
    for ping in CAPTURE.sent:
        answers = [(at, raw) for sent, at, raw in CAPTURE.replies if sent == ping]
        if answers:
            at, raw = answers[0]
            lines.append(f"text ping at {ping} s: answered after {at - ping:.2f} s with {raw!r}")
        else:
            lines.append(f"text ping at {ping} s: unanswered within {ANSWER_WINDOW} s")
    if live and len(CAPTURE.drops) <= 1:
        lines.append(
            "D9 sports rule: none; a game was live throughout, so the silence and the "
            "unanswered pings are a live feed's. Stop and report to the owner (D17)"
        )
    else:
        lines.append(f"D9 sports rule: {rule_for(silence, CAPTURE.sent, CAPTURE.replies)}")
    return lines


def cadence_exit_code() -> int:
    """The exit code of a cadence capture: 2 when a second drop ended it, 3 when
    a game was live throughout (no rule follows; D17 stops there as it does at a
    second drop), else 0."""
    if len(CAPTURE.drops) > 1:
        return 2
    times = [t for t, _ in CAPTURE.frames]
    if live_throughout(times, CAPTURE.segments):
        return 3
    return 0


def cadence_main(args: list[str], cadence_minutes: float) -> int:
    """Run the D9 capture and print its summary; the exit code is
    `cadence_exit_code`'s."""
    seconds = int(args[0]) if args else 1800
    cadence = cadence_minutes * 60
    try:
        asyncio.run(record_cadence(seconds, cadence))
    except KeyboardInterrupt:
        CAPTURE.ended_by = f"Ctrl-C at {CAPTURE.elapsed()} s"
    print("\n".join(cadence_summary(seconds, cadence)))
    return cadence_exit_code()


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
    args = sys.argv[1:]
    if "--cadence" in args:
        at = args.index("--cadence")
        try:
            minutes = float(args[at + 1])
        except (IndexError, ValueError):
            sys.exit("--cadence takes a number of minutes")
        del args[at : at + 2]
        sys.exit(cadence_main(args, minutes))
    if not args:
        sys.exit(__doc__)
    out = Path(args[0])
    seconds = int(args[1]) if len(args) > 1 else 300
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
