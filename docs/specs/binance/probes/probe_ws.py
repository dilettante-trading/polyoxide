"""Probe Binance USD-M market-stream socket behaviour.

Usage: python3 -I probe_ws.py <wsprobe_dir>
"""
import base64
import json
import os
import socket
import ssl
import struct
import sys
import time

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


def main():
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


if __name__ == "__main__":
    main()
