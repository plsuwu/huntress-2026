#!/usr/bin/env python3
"""Per-leg timings of saved run dirs, relative to the server's started_at.  usage: run_legs.py RUN_DIR..."""
import json, sys
from datetime import datetime


def t(x):
    return datetime.strptime(x.replace("Z", "+0000"), "%Y-%m-%dT%H:%M:%S.%f%z").timestamp() * 1000


for d in sys.argv[1:]:
    m, s = json.load(open(d + "/meta.json")), json.load(open(d + "/start.json"))
    st, c = t(s["started_at"]), m["clock"]
    dl = m["downloads"]
    rel = lambda k: (t(c[k]) - st) if c.get(k) else float("nan")
    print(f"{d.rstrip('/').split('/')[-1][:24]:24} start_ttfb {m['start_ttfb_ms']:6.0f} | recv {rel('start_received'):6.0f}"
          f" | dl_ttfb {max(x['ttfb_ms'] for x in dl):5.0f} dl_total {max(x['total_ms'] for x in dl):5.0f}"
          f" analyse {max(x.get('analyse_ms') or 0 for x in dl):5.0f} | last_delivery {rel('last_delivery'):6.0f}"
          f" submit_recv {rel('submit_received'):6.0f}")
