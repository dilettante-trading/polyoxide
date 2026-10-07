"""Minimal WebSocket probe: connect, optionally send one text frame, print the first frames.

Usage: python3 -I wsprobe.py <host> <path-with-query> [subscribe-json]
Prints the HTTP status line, then up to 3 text frames (first 300 chars each) within 6 s.
"""
import base64
import json
import os
import socket
import ssl
import struct
import sys
import time


def read_exact(s, n):
    buf = b""
    while len(buf) < n:
        chunk = s.recv(n - len(buf))
        if not chunk:
            raise EOFError("closed")
        buf += chunk
    return buf


def read_frame(s):
    b1, b2 = read_exact(s, 2)
    opcode = b1 & 0x0F
    length = b2 & 0x7F
    if length == 126:
        length = struct.unpack(">H", read_exact(s, 2))[0]
    elif length == 127:
        length = struct.unpack(">Q", read_exact(s, 8))[0]
    if b2 & 0x80:
        read_exact(s, 4)
    return opcode, read_exact(s, length)


def send_text(s, text):
    payload = text.encode("utf-8")
    mask = os.urandom(4)
    header = bytes([0x81])
    n = len(payload)
    if n < 126:
        header += bytes([0x80 | n])
    elif n < 65536:
        header += bytes([0x80 | 126]) + struct.pack(">H", n)
    else:
        header += bytes([0x80 | 127]) + struct.pack(">Q", n)
    masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
    s.sendall(header + mask + masked)


def main():
    host, path = sys.argv[1], sys.argv[2]
    sub = sys.argv[3] if len(sys.argv) > 3 else None
    raw = socket.create_connection((host, 443), timeout=8)
    s = ssl.create_default_context().wrap_socket(raw, server_hostname=host)
    key = base64.b64encode(os.urandom(16)).decode()
    req = (
        f"GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    )
    s.sendall(req.encode())
    head = b""
    while b"\r\n\r\n" not in head:
        head += s.recv(1)
    print("STATUS:", head.split(b"\r\n")[0].decode())
    if b" 101 " not in head.split(b"\r\n")[0]:
        return
    if sub:
        send_text(s, sub)
    deadline = time.time() + 6
    shown = 0
    s.settimeout(6)
    while time.time() < deadline and shown < 3:
        try:
            op, data = read_frame(s)
        except Exception as e:  # noqa: BLE001 - a probe reports whatever ended it
            print("END:", type(e).__name__, e)
            return
        if op == 1:
            text = data.decode("utf-8", "replace")
            try:
                obj = json.loads(text)
                if isinstance(obj, dict) and "stream" in obj:
                    d = obj["data"]
                    size = len(d) if isinstance(d, list) else len(d.keys())
                    print("FRAME stream=%r data=%s(%d)" % (obj["stream"], type(d).__name__, size))
                else:
                    print("FRAME", text[:300])
            except ValueError:
                print("FRAME(raw)", text[:300])
            shown += 1
        elif op == 8:
            print("CLOSE", data[:2], data[2:].decode("utf-8", "replace"))
            return


if __name__ == "__main__":
    main()
