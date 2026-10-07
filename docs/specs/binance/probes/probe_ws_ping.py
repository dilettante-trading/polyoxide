"""Record server ping frames on a quiet connection for 400 s (needs <wsprobe_dir> <probe_ws_dir>)."""
import sys, time, json
sys.path.insert(0, sys.argv[1]); sys.path.insert(0, sys.argv[2])
import wsprobe, probe_ws  # noqa: E402
s = probe_ws.open_ws("/market/stream")
probe_ws.send_json(s, {"method": "SUBSCRIBE", "params": ["zz0000usdt@aggTrade"], "id": 1})
t0 = time.time()
while time.time() - t0 < 400:
    try:
        op, data = wsprobe.read_frame(s)
    except Exception as e:  # noqa: BLE001 - a timeout just loops
        if "timed out" in str(e):
            continue
        print("%.1fs END %s" % (time.time() - t0, e)); break
    if op == 0x9:
        print("%.1fs server PING %r" % (time.time() - t0, data[:20]), flush=True)
        probe_ws.send(s, 0xA, data)
    elif op == 0x8:
        print("%.1fs CLOSE %r" % (time.time() - t0, data[:60])); break
    elif op == 0x1:
        print("%.1fs TEXT %s" % (time.time() - t0, data[:80].decode()), flush=True)
print("done after %.1fs" % (time.time() - t0))
