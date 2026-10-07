"""Pin the stream cap, uppercase delivery and the message-rate limit (needs <wsprobe_dir>)."""
import json, sys, time
sys.path.insert(0, sys.argv[1])
import probe_ws  # noqa: E402
from probe_ws import open_ws, send_json, read_until, reply_with_id

# Uppercase symbol: does it deliver anything?
s = open_ws("/market/stream")
send_json(s, {"method": "SUBSCRIBE", "params": ["BTCUSDT@aggTrade"], "id": 1})
read_until(s, reply_with_id(1))
r = read_until(s, lambda op, d: op == 0x1 and b'"stream"' in d, limit=6)
print("uppercase BTCUSDT@aggTrade data within 6s ->", r[1][:90].decode() if r and isinstance(r[1], bytes) else r)
send_json(s, {"method": "SUBSCRIBE", "params": ["btcusdt@aggTrade"], "id": 2})
read_until(s, reply_with_id(2))
r = read_until(s, lambda op, d: op == 0x1 and b'"stream"' in d, limit=6)
print("lowercase btcusdt@aggTrade data within 6s ->", r[1][:90].decode() if r and isinstance(r[1], bytes) else r)
s.close()

# Exact cap: 1024, then one more.
s = open_ws("/market/stream")
n = 0
for b in range(6):
    k = 200 if n + 200 <= 1024 else 1024 - n
    if k == 0:
        break
    send_json(s, {"method": "SUBSCRIBE", "params": [f"zz{n + i:04d}usdt@aggTrade" for i in range(k)], "id": 10 + b})
    r = read_until(s, reply_with_id(10 + b))
    n += k
    time.sleep(0.3)
print("after", n, "->", r[1].decode() if r and isinstance(r[1], bytes) else r)
send_json(s, {"method": "SUBSCRIBE", "params": ["zz9999usdt@aggTrade"], "id": 50})
r = read_until(s, reply_with_id(50))
print("stream 1025 ->", r[1].decode() if r and isinstance(r[1], bytes) else r)
r = read_until(s, lambda op, d: False, limit=3)
print("then ->", r)
s.close()

# Rate: 40 SUBSCRIBE messages back to back.
s = open_ws("/market/stream")
for i in range(40):
    send_json(s, {"method": "SUBSCRIBE", "params": [f"yy{i:04d}usdt@aggTrade"], "id": 100 + i})
ids, end = [], None
t0 = time.time()
while time.time() - t0 < 8:
    r = read_until(s, lambda op, d: True, limit=1.0)
    if not r:
        continue
    if r[0] in ("CLOSE", "EOF"):
        end = r; break
    if r[0] == 0x1:
        ids.append(json.loads(r[1]).get("id"))
print("40 rapid SUBSCRIBEs -> acks", len(ids), "last id", ids[-1] if ids else None, "end", end)
