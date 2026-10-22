#!/usr/bin/env python3
"""Per-leg timings of saved run dirs, relative to the server's started_at: where the timed window goes.

  run_legs.py RUN_DIR...

Expects start.json "started_at" (ISO 8601, Z) and meta.json with "start_ttfb_ms", "clock" {start_received,
last_delivery, submit_received: ISO timestamps} and "downloads" [{ttfb_ms, total_ms, analyse_ms}] - adapt the field
names to what your solver persists.
"""
import json
import sys
from datetime import datetime


def t(x):
    return datetime.strptime(x.replace("Z", "+0000"), "%Y-%m-%dT%H:%M:%S.%f%z").timestamp() * 1000


for d in sys.argv[1:]:
    m, s = json.load(open(d + "/meta.json")), json.load(open(d + "/start.json"))
    st, c = t(s["started_at"]), m["clock"]
    dl = m.get("downloads") or [{}]
    rel = lambda k: (t(c[k]) - st) if c.get(k) else float("nan")
    mx = lambda k: max((x.get(k) or 0) for x in dl)
    print(f"{d.rstrip('/').split('/')[-1][:24]:24} start_ttfb {m.get('start_ttfb_ms', float('nan')):6.0f} "
          f"| recv {rel('start_received'):6.0f} | dl_ttfb {mx('ttfb_ms'):5.0f} dl_total {mx('total_ms'):5.0f} "
          f"analyse {mx('analyse_ms'):5.0f} | last_delivery {rel('last_delivery'):6.0f} submit_recv {rel('submit_received'):6.0f}")
