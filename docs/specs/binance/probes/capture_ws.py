"""Capture one live combined-stream envelope per Binance stream as a fixture.

Usage: python3 -I capture_ws.py <out_dir> <wsprobe_dir>
Array payloads are trimmed to two rows and depth sides to three levels.
"""
import base64
import json
import os
import socket
import ssl
import sys
import time

sys.path.insert(0, sys.argv[2])
import wsprobe  # noqa: E402

OUT = sys.argv[1]
HOST = "fstream.binance.com"
STREAMS = {
    "market": ["!ticker@arr", "!markPrice@arr@1s", "btcusdt@aggTrade", "btcusdt@kline_1m",
               "btcusdt@markPrice@1s", "btcusdt@ticker"],
    "public": ["btcusdt@depth20@100ms", "btcusdt@bookTicker"],
}


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
    return s


def trim(stream, data):
    if isinstance(data, list):
        return data[:2]
    if stream.endswith("@depth20@100ms"):
        data = dict(data)
        data["b"] = data["b"][:3]
        data["a"] = data["a"][:3]
    return data


def main():
    os.makedirs(OUT, exist_ok=True)
    for path, streams in STREAMS.items():
        s = open_ws(f"/{path}/stream?streams=" + "/".join(streams))
        s.settimeout(15)
        want = set(streams)
        deadline = time.time() + 20
        while want and time.time() < deadline:
            op, data = wsprobe.read_frame(s)
            if op != 1:
                continue
            env = json.loads(data.decode("utf-8"))
            name = env.get("stream")
            if name in want:
                want.discard(name)
                env["data"] = trim(name, env["data"])
                fname = "stream_" + name.replace("!", "all_").replace("@", "_") + ".json"
                with open(os.path.join(OUT, fname), "w", encoding="utf-8") as f:
                    json.dump(env, f, ensure_ascii=False, indent=1)
                    f.write("\n")
        print(path, "missing:", sorted(want))


if __name__ == "__main__":
    main()
