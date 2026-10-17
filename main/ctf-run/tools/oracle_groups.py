#!/usr/bin/env python3
"""Untimed grouped oracle: partial-submit a saved run's answers by question family, then split
rejected families by field suffix. Much faster than blind bisection when one field kind is wrong.

usage: oracle_groups.py RUN_DIR [CHALLENGE]   (HUNTRESS_GM_TOKEN in env; external API)
"""
import json, os, re, sys, time, urllib.request
from collections import defaultdict

URL = os.environ.get("HUNTRESS_GM_URL", "https://2026.huntress.ctf.games")
TOKEN = os.environ["HUNTRESS_GM_TOKEN"]


def submit(challenge, answers):
    body = json.dumps({"op": "submit", "challenge": challenge, "answers": answers}).encode()
    req = urllib.request.Request(URL + "/", body, {"Content-Type": "application/json", "Authorization": f"Bearer {TOKEN}"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.load(r)


def chunks(items, n=500):
    items = list(items)
    for i in range(0, len(items), n):
        yield dict(items[i : i + n])


def strip_set(k):
    return re.sub(r"^set\d+\.", "", k)


def family(k):
    """`d17-audio-001-media-...` -> `d17-audio`; underscore ids (binary-RE roll-ups: `d01_analysis_marker`) -> `d01`,
    so a 204-question roll-up costs ~17 group submits instead of 204 singletons."""
    k = strip_set(k)
    return "-".join(k.split("-")[:2]) if "-" in k else k.split("_")[0]


def field(k):
    k = strip_set(k)
    return k.split("-")[-1] if "-" in k else k.split("_", 1)[-1]


def main():
    run = sys.argv[1]
    meta = json.load(open(os.path.join(run, "meta.json")))
    challenge = sys.argv[2] if len(sys.argv) > 2 else meta.get("challenge")
    answers = json.load(open(os.path.join(run, "answers.json")))["answers"]
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
                print(json.dumps(r)); return
            ok &= r.get("result") == "accepted"
        if not ok:
            bad.append(name)
    # Split rejected families by the field (last 1-2 id components).
    for name in bad:
        by_field = defaultdict(dict)
        for k, v in fam[name].items():
            by_field[field(k)][k] = v
        for fld, group in sorted(by_field.items()):
            for c in chunks(group.items()):
                r = submit(challenge, c)
                print(f"  {name}/{fld:20} {len(c):4} -> {r.get('result')} remaining={r.get('remaining')}", flush=True)
                if r.get("result") == "CORRECT":
                    print(json.dumps(r)); return


if __name__ == "__main__":
    main()
