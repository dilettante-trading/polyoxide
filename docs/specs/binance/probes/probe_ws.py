"""Probe Binance market-stream socket behaviour.

Usage:
  python3 -I probe_ws.py <wsprobe_dir>                                  USD-M futures, as before
  python3 -I probe_ws.py <wsprobe_dir> --product spot                    spot: the four calm sections
  python3 -I probe_ws.py <wsprobe_dir> --product spot --section answers  spot: one section

The spot sections, on wss://stream.binance.com:9443/stream, one connection each, as Markdown
on stdout (progress on stderr):
  answers   how a SUBSCRIBE of an unknown symbol, an unknown stream type, an uppercase symbol
            and a lowercase one is answered and which of them delivers; an UNSUBSCRIBE of a
            name never subscribed; LIST_SUBSCRIPTIONS; a client ping
  request   names in one SUBSCRIBE: 200, 500, then 1024, each counted by LIST_SUBSCRIPTIONS
  pings     the server's pings over 65 s, each ponged as the client pongs it
  frames    20 s of every stream the design spec's section 5.9 names, and !ticker@arr: the
            envelope, each payload's keys and how often each stream sends
  pace      a client's schedule for 45 s: a request every --pace-ms (250 by default), a ping
            every 20 s, every server ping ponged; on a close, the messages sent just before it
  burst     20 requests back to back on a quiet connection, read until the server closes it

Binance disconnects a connection that sends it more than 5 messages a second (pings, pongs
and requests alike) and bans an address it disconnects repeatedly; the live suites and other
sessions on this host share the address. So the calm sections send at most one message a
second, a server close stops the whole run, and pace and burst run only when named. A request
every 250 ms was disconnected on 2026-11-02 (OBSERVED.md), so run pace only to confirm a
slower pace, and burst, the one deliberate excess, only once: never run it twice.
"""
import argparse
import base64
import json
import os
import select
import socket
import ssl
import statistics
import struct
import sys
import time
from collections import defaultdict
from datetime import datetime, timezone

sys.path.insert(0, sys.argv[1])
import wsprobe  # noqa: E402

HOST = "fstream.binance.com"


def open_ws(path):
    raw = socket.create_connection((HOST, 443), timeout=10)
    s = ssl.create_default_context().wrap_socket(raw, server_hostname=HOST)
    key = base64.b64encode(os.urandom(16)).decode()
    s.sendall((f"GET {path} HTTP/1.1\r\nHost: {HOST}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
               f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
    head = b""
    while b"\r\n\r\n" not in head:
        head += s.recv(1)
    assert b" 101 " in head.split(b"\r\n")[0], head
    s.settimeout(5)
    return s


def send(s, opcode, payload: bytes):
    mask = os.urandom(4)
    n = len(payload)
    header = bytes([0x80 | opcode])
    if n < 126:
        header += bytes([0x80 | n])
    elif n < 65536:
        header += bytes([0x80 | 126]) + struct.pack(">H", n)
    else:
        header += bytes([0x80 | 127]) + struct.pack(">Q", n)
    s.sendall(header + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))


def send_json(s, obj):
    send(s, 0x1, json.dumps(obj).encode())


def read_until(s, pred, limit=10.0):
    """Read frames until pred(op, data) is true; return (op, data, elapsed) or None."""
    t0 = time.time()
    while time.time() - t0 < limit:
        try:
            op, data = wsprobe.read_frame(s)
        except (socket.timeout, TimeoutError):
            continue
        except EOFError:
            return ("EOF", b"", time.time() - t0)
        if op == 0x8:
            code = struct.unpack(">H", data[:2])[0] if len(data) >= 2 else None
            return ("CLOSE", f"{code} {data[2:].decode('utf-8', 'replace')}".encode(), time.time() - t0)
        if pred(op, data):
            return (op, data, time.time() - t0)
    return None


def reply_with_id(i):
    def pred(op, data):
        if op != 0x1:
            return False
        try:
            msg = json.loads(data)
        except ValueError:
            return False
        return isinstance(msg, dict) and msg.get("id") == i
    return pred


def usdm_main():
    s = open_ws("/market/stream")
    send(s, 0x9, b"p1")
    r = read_until(s, lambda op, d: op == 0xA)
    print("client ping ->", "pong after %.3fs" % r[2] if r and r[0] == 0xA else r)

    for i, params in [(1, ["notasymbolusdt@aggTrade"]), (2, ["btcusdt@nonsense"]), (3, ["BTCUSDT@aggTrade"])]:
        send_json(s, {"method": "SUBSCRIBE", "params": params, "id": i})
        r = read_until(s, reply_with_id(i))
        print("SUBSCRIBE", params, "->", r[1].decode() if r and isinstance(r[1], bytes) else r)
    send_json(s, {"method": "LIST_SUBSCRIPTIONS", "id": 4})
    r = read_until(s, reply_with_id(4))
    print("LIST_SUBSCRIPTIONS ->", r[1].decode()[:300] if r else r)
    send_json(s, {"method": "UNSUBSCRIBE", "params": ["ethusdt@aggTrade"], "id": 5})
    r = read_until(s, reply_with_id(5))
    print("UNSUBSCRIBE never-subscribed ->", r[1].decode() if r else r)
    s.close()

    # Stream cap: fake symbols carry no data, so only the count matters.
    s = open_ws("/market/stream")
    total = 0
    for batch in range(6):
        params = [f"zz{total + k:04d}usdt@aggTrade" for k in range(200)]
        send_json(s, {"method": "SUBSCRIBE", "params": params, "id": 100 + batch})
        r = read_until(s, reply_with_id(100 + batch))
        total += 200
        print(f"cap batch {batch} (total {total}) ->", r[1].decode()[:200] if r and isinstance(r[1], bytes) else r)
        if not r or r[0] in ("CLOSE", "EOF") or b"error" in r[1]:
            break
        time.sleep(0.3)
    try:
        send_json(s, {"method": "LIST_SUBSCRIPTIONS", "id": 199})
        r = read_until(s, reply_with_id(199))
        if r and isinstance(r[1], bytes) and r[0] == 0x1:
            print("subscriptions held:", len(json.loads(r[1])["result"]))
        else:
            print("list after cap ->", r)
    except OSError as e:
        print("list after cap -> socket gone:", e)
    s.close()

    # Message rate: 15 control messages back to back.
    s = open_ws("/market/stream")
    for i in range(15):
        send_json(s, {"method": "LIST_SUBSCRIPTIONS", "id": 300 + i})
    got = []
    t0 = time.time()
    while time.time() - t0 < 8:
        r = read_until(s, lambda op, d: True, limit=1.0)
        if not r:
            continue
        if r[0] in ("CLOSE", "EOF"):
            got.append(r[0] + " " + (r[1].decode() if isinstance(r[1], bytes) else ""))
            break
        if r[0] == 0x1:
            got.append(json.loads(r[1]).get("id"))
    print("15 rapid messages ->", got)


# ---- spot -------------------------------------------------------------------------

SPOT_HOST = "stream.binance.com"
SPOT_PORT = 9443
SPOT_PATH = "/stream"
CALM = 1.0               # seconds between messages, outside pace and burst
PACE = 0.25              # the default interval between pace's requests (--pace-ms)
CLIENT_PING_EVERY = 20.0  # the client's planned wall-clock ping
BURST = 20
REQUEST_SIZES = (200, 500, 1024)
FRAME_STREAMS = [
    "btcusdt@aggTrade", "btcusdt@trade", "btcusdt@kline_1m", "btcusdt@miniTicker",
    "!miniTicker@arr", "btcusdt@ticker", "btcusdt@bookTicker",
    "btcusdt@depth5", "btcusdt@depth10", "btcusdt@depth20",
    "btcusdt@depth5@100ms", "btcusdt@depth10@100ms", "btcusdt@depth20@100ms",
    "btcusdt@avgPrice", "!ticker@arr",
]
RERUN = "`docs/specs/binance/probes/probe_ws.py <wsprobe_dir> --product spot --section {}`"


class Disconnected(Exception):
    """The server closed a connection: the run stops here."""


def log(text):
    print(text, file=sys.stderr, flush=True)


def utc_date():
    return datetime.now(timezone.utc).strftime("%Y-%m-%d")


def compact(obj, limit=160):
    text = json.dumps(obj, separators=(",", ":"), ensure_ascii=False)
    return text if len(text) <= limit else text[:limit] + "…"


def fake_names(count):
    return [f"zz{n:04d}usdt@aggTrade" for n in range(count)]


def is_answer(rid):
    return lambda f: f[1] == 0x1 and isinstance(f[2], dict) and f[2].get("id") == rid


def accepted(answer):
    return isinstance(answer, dict) and "result" in answer and "error" not in answer


class Conn:
    """One spot socket, read on this thread only. Frames are parsed from a buffer, so a read
    that times out loses nothing; a server ping is ponged as it is read, as the client does;
    every frame this side sends is timed, so a section can count its messages a second."""

    def __init__(self, label):
        self.label = label
        raw = socket.create_connection((SPOT_HOST, SPOT_PORT), timeout=10)
        self.sock = ssl.create_default_context().wrap_socket(raw, server_hostname=SPOT_HOST)
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall((f"GET {SPOT_PATH} HTTP/1.1\r\nHost: {SPOT_HOST}:{SPOT_PORT}\r\n"
                           "Upgrade: websocket\r\nConnection: Upgrade\r\n"
                           f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
        head = b""
        while b"\r\n\r\n" not in head:
            chunk = self.sock.recv(1)
            if not chunk:
                raise Disconnected(f"{label}: closed during the handshake")
            head += chunk
        status = head.split(b"\r\n")[0].decode("ascii", "replace")
        if " 101 " not in status:
            raise RuntimeError(f"{label}: the handshake was answered {status!r}")
        self.t0 = time.monotonic()
        self.buf = b""
        self.fragments = None    # [opcode, payload] of a message still arriving
        self.sent = []           # (t, kind) for every frame this side sent
        self.server_pings = []   # (t, payload)
        self.pongs = []          # (t, payload): pongs to this side's pings
        self.texts = []          # (t, the JSON, or the text when it is not JSON)
        self.closed = None       # (t, close code or None, reason)

    def now(self):
        return time.monotonic() - self.t0

    def _close_seen(self, code, reason):
        if self.closed is None:
            self.closed = (self.now(), code, reason)
            log(f"  {self.label}: closed at {self.closed[0]:.2f} s: {code} {reason!r}")

    def _send(self, opcode, payload, kind):
        mask = os.urandom(4)
        n = len(payload)
        header = bytes([0x80 | opcode])
        if n < 126:
            header += bytes([0x80 | n])
        elif n < 65536:
            header += bytes([0x80 | 126]) + struct.pack(">H", n)
        else:
            header += bytes([0x80 | 127]) + struct.pack(">Q", n)
        try:
            self.sock.sendall(header + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))
        except OSError as e:
            self._close_seen(None, f"send failed: {type(e).__name__}: {e}")
            return
        self.sent.append((self.now(), kind))

    def send_json(self, obj):
        self._send(0x1, json.dumps(obj).encode(), obj["method"])

    def ping(self, payload):
        self._send(0x9, payload, "PING")

    def request(self, rid, method, params=None, wait=10.0):
        """Sends one request and reads until its answer; None when none came."""
        msg = {"method": method, "id": rid} if params is None else \
            {"method": method, "params": params, "id": rid}
        self.send_json(msg)
        found = self.pump(wait, is_answer(rid))
        return None if found is None else found[2]

    def _fill(self, wait):
        """Bytes read into the buffer within `wait` seconds; -1 once the socket has ended."""
        if not self.sock.pending():
            ready, _, _ = select.select([self.sock], [], [], wait)
            if not ready:
                return 0
        try:
            chunk = self.sock.recv(65536)
        except OSError as e:
            self._close_seen(None, f"{type(e).__name__}: {e}")
            return -1
        if not chunk:
            self._close_seen(None, "the socket ended with no close frame")
            return -1
        self.buf += chunk
        return len(chunk)

    def _frame(self):
        """Takes one whole frame off the buffer, acting on a control frame; returns
        (t, opcode, payload), a text payload decoded, or None for want of bytes."""
        b = self.buf
        if len(b) < 2:
            return None
        n, at = b[1] & 0x7F, 2
        if n == 126:
            if len(b) < 4:
                return None
            n, at = struct.unpack(">H", b[2:4])[0], 4
        elif n == 127:
            if len(b) < 10:
                return None
            n, at = struct.unpack(">Q", b[2:10])[0], 10
        if b[1] & 0x80:
            raise RuntimeError("the server sent a masked frame")
        if len(b) < at + n:
            return None
        fin, opcode, payload = b[0] & 0x80, b[0] & 0x0F, b[at:at + n]
        self.buf = b[at + n:]
        t = self.now()
        if opcode == 0x9:
            self.server_pings.append((t, payload))
            self._send(0xA, payload, "PONG")
        elif opcode == 0xA:
            self.pongs.append((t, payload))
        elif opcode == 0x8:
            code = struct.unpack(">H", payload[:2])[0] if len(payload) >= 2 else None
            self._close_seen(code, payload[2:].decode("utf-8", "replace"))
        elif opcode == 0x0 or not fin:
            if opcode != 0x0:
                self.fragments = [opcode, payload]
            elif self.fragments is not None:
                self.fragments[1] += payload
            if not fin or self.fragments is None:
                return (t, 0x0, b"")
            opcode, payload = self.fragments
            self.fragments = None
        if opcode == 0x1:
            text = payload.decode("utf-8", "replace")
            try:
                decoded = json.loads(text)
            except ValueError:
                decoded = text
            self.texts.append((t, decoded))
            return (t, opcode, decoded)
        return (t, opcode, payload)

    def pump(self, limit, stop=None):
        """Reads frames for up to `limit` seconds; returns the first `stop` accepts, or None
        at the limit or a close."""
        deadline = time.monotonic() + limit
        while self.closed is None:
            frame = self._frame()
            if frame is not None:
                if stop is not None and stop(frame):
                    return frame
                continue
            left = deadline - time.monotonic()
            got = self._fill(max(0.0, left))
            if got < 0 or (got == 0 and left <= 0):
                break
        return None

    def check(self):
        if self.closed is not None:
            t, code, reason = self.closed
            raise Disconnected(f"{self.label}: the server closed the connection at {t:.2f} s "
                               f"with {code} {reason!r}")

    def close(self):
        if self.closed is None:
            self._send(0x8, struct.pack(">H", 1000), "CLOSE")
        try:
            self.sock.close()
        except OSError:
            pass

    def deliveries(self):
        """Data frames by the envelope's `stream`, as {stream: [(t, data)]}."""
        out = defaultdict(list)
        for t, obj in self.texts:
            if isinstance(obj, dict) and "stream" in obj:
                out[obj["stream"]].append((t, obj.get("data")))
        return out


def most_in_a_second(times):
    times = sorted(times)
    best, j = 0, 0
    for i, t in enumerate(times):
        while times[j] <= t - 1.0:
            j += 1
        best = max(best, i - j + 1)
    return best


def answers_section(out):
    log("answers: unknown, uppercase and lowercase names, an unknown UNSUBSCRIBE, a client ping")
    out += [
        "### Stream answers",
        "",
        f"Measured {utc_date()} on `stream.binance.com:9443/stream`: one connection, a message a "
        f"second. {RERUN.format('answers')} re-runs it.",
        "",
        "| Request | Answer | Frames delivered on the connection |",
        "|---|---|---|",
    ]
    steps = [
        (1, "an unknown symbol", "SUBSCRIBE", ["notasymbolusdt@aggTrade"]),
        (2, "an unknown stream type", "SUBSCRIBE", ["btcusdt@nonsense"]),
        (3, "an uppercase symbol", "SUBSCRIBE", ["BTCUSDT@trade"]),
        (4, "a lowercase symbol", "SUBSCRIBE", ["btcusdt@aggTrade"]),
        (5, "the subscriptions", "LIST_SUBSCRIPTIONS", None),
        (6, "a name never subscribed", "UNSUBSCRIBE", ["ethusdt@aggTrade"]),
    ]
    c = Conn("answers")
    try:
        sent_at = c.now()
        c.ping(b"probe")
        pong = c.pump(10, lambda f: f[1] == 0xA)
        c.check()
        answers = []
        for rid, what, method, params in steps:
            c.pump(CALM)
            answer = c.request(rid, method, params)
            c.check()
            log(f"  {method} {params} -> {compact(answer)}")
            answers.append((what, method, params, answer))
        c.pump(8)
        c.check()
        seen = c.deliveries()
        lived = c.now()
        for what, method, params, answer in answers:
            if params:
                wanted = {p.lower() for p in params}
                hits = {s: len(v) for s, v in seen.items() if s.lower() in wanted}
                delivered = ", ".join(f"{n} as `{s}`" for s, n in hits.items()) or "none"
                shown = f"`{method} {compact(params)}` ({what})"
            else:
                delivered = "-"
                shown = f"`{method}`"
            out.append(f"| {shown} | `{compact(answer)}` | {delivered} |")
        out.append("")
        out.append(f"The connection lived {lived:.0f} s. Every stream that delivered: "
                   + (", ".join(f"`{s}` ({len(v)})" for s, v in sorted(seen.items())) or "none") + ".")
        if pong is None:
            out.append("A client ping was not answered within 10 s.")
        else:
            echoed = "its payload echoed" if pong[2] == b"probe" else f"payload {pong[2]!r}"
            out.append(f"A client ping was answered with a pong in {pong[0] - sent_at:.3f} s, "
                       f"{echoed}.")
    finally:
        c.close()


def request_section(out):
    log("request: 200, 500, then 1024 names in one SUBSCRIBE")
    out += [
        "### Names per request",
        "",
        f"Measured {utc_date()} on `stream.binance.com:9443/stream`: one connection, a message a "
        "second, names of the form `zz0000usdt@aggTrade`; `Held` is the length of "
        f"`LIST_SUBSCRIPTIONS`' result after the request. {RERUN.format('request')} re-runs it.",
        "",
        "| Request | Frame | Answer | Held |",
        "|---|---|---|---|",
    ]
    c = Conn("request")
    rid = 100

    def held():
        nonlocal rid
        c.pump(CALM)
        listed = c.request(rid, "LIST_SUBSCRIPTIONS")
        rid += 1
        c.check()
        result = listed.get("result") if isinstance(listed, dict) else None
        return len(result) if isinstance(result, list) else f"`{compact(listed)}`"

    try:
        for size in REQUEST_SIZES:
            names = fake_names(size)
            for method in ("SUBSCRIBE", "UNSUBSCRIBE"):
                if method == "UNSUBSCRIBE" and size == REQUEST_SIZES[-1]:
                    break
                c.pump(CALM)
                frame = len(json.dumps({"method": method, "params": names, "id": rid}))
                answer = c.request(rid, method, names)
                rid += 1
                c.check()
                count = held()
                log(f"  {method} of {size}: {compact(answer)}, held {count}")
                out.append(f"| `{method}` of {size} names | {frame} bytes | `{compact(answer)}` "
                           f"| {count} |")
                if not accepted(answer):
                    return
    finally:
        c.close()


def pings_section(out):
    log("pings: the server's pings over 65 s on a connection carrying btcusdt@avgPrice")
    c = Conn("pings")
    try:
        c.request(1, "SUBSCRIBE", ["btcusdt@avgPrice"])
        c.check()
        c.pump(4)
        sent_at = c.now()
        c.ping(b"probe")
        pong = c.pump(10, lambda f: f[1] == 0xA)
        c.pump(max(0.0, 65 - c.now()))
        c.check()
    finally:
        c.close()
    times = [t for t, _ in c.server_pings]
    gaps = [b - a for a, b in zip(times, times[1:])]
    payloads = sorted({p.decode("utf-8", "replace") for _, p in c.server_pings})
    frames = sum(len(v) for v in c.deliveries().values())
    log(f"  server pings at {[round(t, 1) for t in times]}")
    out += [
        "### Pings",
        "",
        f"Measured {utc_date()} on `stream.binance.com:9443/stream`: one connection carrying "
        f"`btcusdt@avgPrice` for 65 s, every server ping ponged with its payload. "
        f"{RERUN.format('pings')} re-runs it.",
        "",
        f"- Server pings at {', '.join(f'{t:.1f} s' for t in times) or 'no time'} after the "
        "connection opened"
        + (f"; gaps {', '.join(f'{g:.1f} s' for g in gaps)}" if gaps else "") + ".",
        f"- Their payloads: {', '.join(repr(p) for p in payloads) or 'none'}.",
        "- A client ping " + (f"was answered in {pong[0] - sent_at:.3f} s." if pong
                              else "was not answered within 10 s."),
        f"- {frames} `avgPrice` frames arrived in 65 s.",
    ]


def keys_of(obj):
    def kind(v):
        return {str: "str", bool: "bool", int: "int", float: "float", list: "list",
                dict: "object", type(None): "null"}.get(type(v), type(v).__name__)
    return " ".join(f"`{k}`:{kind(v)}" for k, v in obj.items())


def describe(data):
    if isinstance(data, list):
        head = f"array of {len(data)} rows"
        return head + (f"; a row: {keys_of(data[0])}" if data and isinstance(data[0], dict) else "")
    if isinstance(data, dict):
        sides = [f"`{k}` {len(v)} levels" for k, v in data.items()
                 if k in ("bids", "asks", "b", "a") and isinstance(v, list)]
        return "object: " + keys_of(data) + (f" ({', '.join(sides)})" if sides else "")
    return type(data).__name__


def frames_section(out):
    log("frames: 20 s of every stream in the design spec's section 5.9, and !ticker@arr")
    c = Conn("frames")
    try:
        answer = c.request(1, "SUBSCRIBE", FRAME_STREAMS)
        c.check()
        c.pump(20)
        c.check()
    finally:
        c.close()
    seen = c.deliveries()
    envelopes = sorted({tuple(obj.keys()) for _, obj in c.texts
                        if isinstance(obj, dict) and "stream" in obj})
    out += [
        "### Frames",
        "",
        f"Measured {utc_date()} on `stream.binance.com:9443/stream`: one `SUBSCRIBE` of the "
        f"{len(FRAME_STREAMS)} names below, answered `{compact(answer)}`, then 20 s of frames. "
        "`Gap` is the median time between a stream's frames. Every envelope's keys: "
        + "; ".join(", ".join(f"`{k}`" for k in keys) for keys in envelopes)
        + f". {RERUN.format('frames')} re-runs it.",
        "",
        "| Name | Frames | Gap | `data` |",
        "|---|---|---|---|",
    ]
    for name in FRAME_STREAMS + sorted(set(seen) - set(FRAME_STREAMS)):
        got = seen.get(name, [])
        times = [t for t, _ in got]
        gap = f"{statistics.median(b - a for a, b in zip(times, times[1:])) * 1000:.0f} ms" \
            if len(times) > 2 else "-"
        what = describe(got[0][1]) if got else "nothing"
        log(f"  {name}: {len(got)} frames, gap {gap}")
        label = f"`{name}`" + ("" if name in FRAME_STREAMS else " (not subscribed)")
        out.append(f"| {label} | {len(got)} | {gap} | {what} |")


def pace_section(out):
    log(f"pace: a request every {PACE * 1000:.0f} ms and a ping every {CLIENT_PING_EVERY:.0f} s, "
        "for 45 s")
    c = Conn("pace")
    ids = []
    try:
        c.pump(2)
        c.check()
        start = c.now()
        next_request, next_ping, end = start, start + 10, start + 45
        while c.now() < end and c.closed is None:
            now = c.now()
            if now >= next_ping:
                c.ping(b"pace")
                next_ping += CLIENT_PING_EVERY
            if now >= next_request:
                method = "SUBSCRIBE" if len(ids) % 2 == 0 else "UNSUBSCRIBE"
                rid = 1000 + len(ids)
                c.send_json({"method": method, "params": ["btcusdt@avgPrice"], "id": rid})
                ids.append(rid)
                next_request += PACE
            c.pump(max(0.0, min(next_request, next_ping, end) - c.now()))
        c.pump(3)
    finally:
        c.close()
    answered = {obj["id"] for _, obj in c.texts if isinstance(obj, dict) and obj.get("id") in ids}
    refused = [obj for _, obj in c.texts if isinstance(obj, dict) and "error" in obj]
    sends = [t for t, kind in c.sent if kind != "CLOSE"]
    kinds = defaultdict(int)
    for _, kind in c.sent:
        kinds[kind] += 1
    ending = "The server did not close it." if c.closed is None else \
        (f"The server closed it at {c.closed[0]:.2f} s with {c.closed[1]} {c.closed[2]!r}.")
    out += [
        "### Pace",
        "",
        f"Measured {utc_date()} on `stream.binance.com:9443/stream`: one connection, alternating "
        f"`SUBSCRIBE` and `UNSUBSCRIBE` of `btcusdt@avgPrice` every {PACE * 1000:.0f} ms for 45 s, "
        f"a client ping every {CLIENT_PING_EVERY:.0f} s from 10 s in, every server ping ponged. "
        f"{RERUN.format('pace')} re-runs it.",
        "",
        f"- Sent: {len(ids)} requests, {kinds['PING']} client ping(s), {kinds['PONG']} pong(s); "
        f"at most {most_in_a_second(sends)} messages in any one second.",
        f"- Answered: {len(answered)} of {len(ids)} requests"
        + (f"; refused: {', '.join(f'`{compact(r)}`' for r in refused[:3])}" if refused else "")
        + f"; {len(c.pongs)} of {kinds['PING']} client ping(s).",
        f"- {ending}",
    ]
    if c.closed is not None:
        before = [f"{t:.3f} s {kind}" for t, kind in c.sent if c.closed[0] - 1.5 < t <= c.closed[0]]
        out.append(f"- Sent in the 1.5 s before the close: {', '.join(before) or 'nothing'}.")
    c.check()


def burst_section(out):
    log(f"burst: {BURST} requests back to back, once")
    c = Conn("burst")
    rids = []
    try:
        c.pump(2)
        c.check()
        first = c.now()
        for i in range(BURST):
            c.pump(0)
            if c.closed is not None:
                break
            rid = 2000 + i
            c.send_json({"method": "LIST_SUBSCRIPTIONS", "id": rid})
            if c.closed is None:
                rids.append(rid)
        took = c.now() - first
        c.pump(10)
    finally:
        c.close()
    answers = [(t - first, obj) for t, obj in c.texts if isinstance(obj, dict) and "id" in obj]
    ok = [obj["id"] for _, obj in answers if accepted(obj)]
    other = [f"`{compact(obj)}` at {t:.3f} s" for t, obj in answers if not accepted(obj)]
    ending = "The server did not close it within 10 s." if c.closed is None else \
        (f"The server closed it {c.closed[0] - first:.3f} s after the first request, with "
         f"{c.closed[1]} {c.closed[2]!r}.")
    log(f"  answered {ok}; {ending}")
    out += [
        "### One burst over the limit",
        "",
        f"Measured {utc_date()} on `stream.binance.com:9443/stream`, once: a quiet connection "
        f"sent {len(rids)} `LIST_SUBSCRIPTIONS` back to back in {took * 1000:.0f} ms, checking "
        "for a close before each, then read for 10 s. It is not re-run: an address Binance "
        "disconnects repeatedly is banned.",
        "",
        f"- Answered: {len(ok)} of {len(rids)}"
        + (f" (ids {ok[0]} to {ok[-1]})" if ok else "") + ".",
        f"- Other answers: {', '.join(other) or 'none'}.",
        f"- {ending}",
    ]


SPOT_SECTIONS = {
    "answers": answers_section,
    "request": request_section,
    "pings": pings_section,
    "frames": frames_section,
    "pace": pace_section,
    "burst": burst_section,
}
DEFAULT_RUN = ("answers", "request", "pings", "frames")


def spot_main(section, pace_ms):
    global PACE
    PACE = pace_ms / 1000
    sys.stdout.reconfigure(encoding="utf-8")
    out, stopped = [], None
    for name in [section] if section else DEFAULT_RUN:
        try:
            SPOT_SECTIONS[name](out)
        except Disconnected as e:
            stopped = str(e)
            break
        out.append("")
    print("\n".join(out))
    if stopped:
        log(f"STOPPED: {stopped}. No later section ran; find the cause before sending again.")
        sys.exit(2)


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("wsprobe_dir", help="the directory holding wsprobe.py")
    parser.add_argument("--product", choices=("usdm", "spot"), default="usdm",
                        help="the host to probe (default: usdm)")
    parser.add_argument("--section", choices=tuple(SPOT_SECTIONS),
                        help="with --product spot, run one section (default: the four calm ones)")
    parser.add_argument("--pace-ms", type=int, default=250,
                        help="with --section pace, the interval between requests (default: 250)")
    args = parser.parse_args(argv)
    if args.section and args.product != "spot":
        parser.error("--section needs --product spot")
    return args


def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    if args.product == "spot":
        spot_main(args.section, args.pace_ms)
    else:
        usdm_main()


if __name__ == "__main__":
    main()
