#!/usr/bin/env python3
"""Grouped oracle on an UNTIMED attempt: partial-submit a saved run's answers one question family at a time, then
split rejected families by field. Much faster than a blind bisection when one field kind is wrong.

  oracle_groups.py RUN_DIR [CHALLENGE]

Needs an open untimed attempt (single-solve, a regular dual-pool attempt, or a practice continuation): on timed
attempts a wrong partial submit just says "incorrect". Answers already accepted are banked by the server.
Question ids: an optional per-set prefix (`setN.`), then a family and a field. Families are the first two `-` parts
(`dNN-kind-001-field` -> `dNN-kind`) or the first `_` part (`dNN_field` -> `dNN`); adapt family()/field() to your
event's id scheme. Run directory layout: meta.json (with "challenge"), answers.json ({"answers": {...}} or a dict).
"""
import json
import os
import re
import sys
import time
import urllib.request
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import orchcfg  # noqa: E402

C = orchcfg.load()
CHUNK = 500        # answers per submit (keep each request well under the server's size limit)
PAUSE_S = 0.5      # between submits: never saturate the shared endpoint


def submit(challenge, answers):
    body = json.dumps({"op": "submit", "challenge": challenge, "answers": answers}).encode()
    req = urllib.request.Request(C["event"]["api"].rstrip("/") + C["api"]["submit"], body,
                                 {"Content-Type": "application/json", "Authorization": f"Bearer {C.token()}"})
    with urllib.request.urlopen(req, timeout=60) as r:
        out = json.load(r)
    time.sleep(PAUSE_S)
    return out


def chunks(items, n=CHUNK):
    items = list(items)
    for i in range(0, len(items), n):
        yield dict(items[i:i + n])


def strip_set(k):
    return re.sub(r"^set\d+\.", "", k)


def family(k):
    k = strip_set(k)
    return "-".join(k.split("-")[:2]) if "-" in k else k.split("_")[0]


def field(k):
    k = strip_set(k)
    return k.split("-")[-1] if "-" in k else k.split("_", 1)[-1]


def main():
    run = sys.argv[1]
    meta = json.load(open(os.path.join(run, "meta.json")))
    challenge = sys.argv[2] if len(sys.argv) > 2 else meta.get("challenge")
    if not C.in_scope(challenge):
        sys.exit(f"{challenge}: not a registered lane - refusing")
    answers = json.load(open(os.path.join(run, "answers.json")))
    answers = answers.get("answers", answers)
    fam = defaultdict(dict)
    for k, v in answers.items():
        fam[family(k)][k] = v
    bad = []
    for name, group in sorted(fam.items(), key=lambda x: -len(x[1])):
        ok = True
        for c in chunks(group.items()):
            r = submit(challenge, c)
            print(f"{name:32} {len(c):4} -> {r.get('result')} remaining={r.get('remaining')}", flush=True)
            if r.get("result") == "CORRECT":
                print(json.dumps(r))
                return
            ok &= r.get("result") == "accepted"
        if not ok:
            bad.append(name)
    for name in bad:  # split rejected families by field
        by_field = defaultdict(dict)
        for k, v in fam[name].items():
            by_field[field(k)][k] = v
        for fld, group in sorted(by_field.items()):
            for c in chunks(group.items()):
                r = submit(challenge, c)
                print(f"  {name}/{fld:20} {len(c):4} -> {r.get('result')} remaining={r.get('remaining')}", flush=True)
                if r.get("result") == "CORRECT":
                    print(json.dumps(r))
                    return


if __name__ == "__main__":
    main()
