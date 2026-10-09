#!/usr/bin/env python3
"""Capture one live frame per public perps WebSocket channel, plus the
subscribe, refusal and ping responses, as test fixtures.

Usage:
    uv run --with websockets --with certifi python3 scripts/capture_perps_ws_fixtures.py polyoxide-perps/tests/fixtures/ws [INSTRUMENT]

Subscribes to every channel kind for one instrument (default 1; pass the
busiest instrument from `GET /v1/info/statistics` when `trades` is quiet),
plus book at depth 50 and the two `::all` forms, sends a ping and a
deliberately malformed subscribe, and keeps the first frame seen per channel
label over 20 seconds. `klines` is the exception: a frame with a non-empty
`data` replaces an earlier empty one, so the fixture exercises the candle
type when the window sees a closed candle. Writes `<name>.json` per capture
and `PROVENANCE.md`.
"""
import asyncio
import json
import sys
import time
from pathlib import Path

import capture_common

URL = "wss://ws.perpetuals.polymarket.com/v1/ws"
SECONDS = 20


async def main():
    out = Path(sys.argv[1])
    iid = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    CHANNELS = [f"bbo::{iid}", f"book::{iid}", f"book::{iid}::50", f"trades::{iid}", f"klines::{iid}::1m",
                f"tickers::{iid}", "tickers::all", f"statistics::{iid}", "statistics::all"]
    WANTED = {f"bbo::{iid}": "bbo", f"book::{iid}": "book", f"book::{iid}::50": "book_50", f"trades::{iid}": "trades",
              f"klines::{iid}::1m": "klines", f"tickers::{iid}": "tickers", f"statistics::{iid}": "statistics"}
    out.mkdir(parents=True, exist_ok=True)
    captured = {}
    started = capture_common.stamp()
    async with capture_common.ws_session(URL, max_size=2**22) as ws:
        await ws.send(json.dumps({"id": 1, "req": "sub", "chs": CHANNELS}))
        await ws.send(json.dumps({"id": 2, "req": "post", "op": {"type": "ping"}}))
        await ws.send(json.dumps({"id": 3, "req": "sub", "chs": ["bbo::999999", "nonsense::1", f"book::{iid}::30"]}))
        deadline = time.time() + SECONDS
        fanout = set()
        while time.time() < deadline:
            try:
                text = await asyncio.wait_for(ws.recv(), timeout=max(0.1, deadline - time.time()))
            except asyncio.TimeoutError:
                break
            frame = json.loads(text)
            if "ch" in frame:
                label = frame["ch"]
                if label.startswith(("tickers::", "statistics::")) and label not in (f"tickers::{iid}", f"statistics::{iid}"):
                    fanout.add(label)
                name = WANTED.get(label)
            else:
                name = {1: "response_subscribe", 2: "response_ping", 3: "response_refused"}.get(frame.get("id"))
            if name and name not in captured:
                captured[name] = text
            elif name == "klines" and not json.loads(captured[name])["data"] and frame["data"]:
                captured[name] = text
        await ws.send(json.dumps({"id": 4, "req": "unsub", "chs": [f"bbo::{iid}"]}))
        deadline = time.time() + 3
        while time.time() < deadline and "response_unsubscribe" not in captured:
            try:
                text = await asyncio.wait_for(ws.recv(), timeout=max(0.1, deadline - time.time()))
            except asyncio.TimeoutError:
                break
            if json.loads(text).get("id") == 4:
                captured["response_unsubscribe"] = text
    missing = [n for n in list(WANTED.values()) + ["response_subscribe", "response_ping", "response_refused", "response_unsubscribe"] if n not in captured]
    for name, text in captured.items():
        capture_common.write_json(out / f"{name}.json", json.loads(text), ensure_ascii=True)
    lines = ["# Perps WebSocket fixtures", "",
             f"Captured {started} by `scripts/capture_perps_ws_fixtures.py` from `{URL}`:",
             f"one `sub` for {CHANNELS}, a ping, a malformed `sub`, {SECONDS} s of frames, then an `unsub`.",
             f"Instrument {iid}. Each file is the first frame seen for its label, pretty-printed",
             "(for `klines`, the first frame with a non-empty `data` when one arrived).", "",
             f"`tickers::all` and `statistics::all` fanned out as {len(fanout)} per-instrument labels; no frame carried an `::all` label.", "",
             *capture_common.provenance_table(("Fixture", "Label or request"), [(f"`{n}.json`", f"`{n}`") for n in captured])]
    if missing:
        lines += ["", f"Not captured this run (quiet channel or no reply): {missing}"]
    capture_common.write_provenance(out, lines)
    print(f"captured {len(captured)} fixtures; missing {missing}")


asyncio.run(main())
