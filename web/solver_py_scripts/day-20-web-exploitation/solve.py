#!/usr/bin/env python3
"""Day-20 web-exploitation solver.

Primary new bug this day: LFI / path traversal ("Stay in Your Directory",
web-exploitation-day-04-lfi) — a PHP viewer includes a file named by a `doc`
query param from its pages directory; traverse out to read the mounted proof
file (prompt says where, e.g. /run/huntress/objective.txt).

Works for all three modes. Mastery is cumulative across the lane's bug types
up to the day, so each target is sniffed from its landing page and routed to
the matching recoverer (lfi/sqli/idor/backup). One target per question; proofs
submitted in one batch under the exact answer_ids.

Usage:
  solve.py start <challenge-id> [--run-for-time]   # start/reconnect, show targets
  solve.py status <challenge-id>
  solve.py solve <challenge-id>                     # start (untimed) + recover + submit

Env: HUNTRESS_GM_URL, HUNTRESS_GM_TOKEN (falls back to AGENTS.md token).
"""
import argparse
import json
import re
import shutil
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))  # for `python3 -I`
import common

RUN = Path(__file__).resolve().parent / "run"


def start(cid, rft=False):
    payload = {"op": "start", "challenge": cid}
    if rft:
        payload["run_for_time"] = True
    data = common.gm(payload)
    RUN.mkdir(parents=True, exist_ok=True)
    (RUN / "start.json").write_text(json.dumps(data, indent=2))
    return data


def recover_target(url, prompt, outdir):
    if outdir.exists():
        shutil.rmtree(outdir)           # drop stale files from a prior variant
    get = common.client(outdir)
    kind, fn = common.sniff(url, get)
    print(f"  [{outdir.name}] sniffed bug = {kind}", file=sys.stderr)
    return fn(url, get, prompt)


def solve(cid):
    data = start(cid)
    # Timed/preparing targets may not be provisioned instantly.
    for _ in range(6):
        if data.get("result") == "CORRECT":
            print("already solved:", data.get("flag"))
            return
        if data.get("targets"):
            break
        print("  attempt preparing; polling status...", file=sys.stderr)
        time.sleep(2.5)
        data = common.gm(op="status", challenge=cid)
    targets = {t["id"]: t["url"] for t in data.get("targets", [])}
    if not targets:
        print("no targets:", json.dumps(data)[:400])
        sys.exit(1)
    prompt = data.get("prompt", "")
    answers = {}
    for q in data["questions"]:
        url = targets.get(q.get("artifact"))
        if not url:
            print("no target for", q["id"], q.get("artifact"), file=sys.stderr)
            continue
        outdir = RUN / re.sub(r"\W+", "_", q["artifact"])
        proof = recover_target(url, prompt, outdir)
        print(f"  {q['id']} [{q['artifact']}] -> {proof}", file=sys.stderr)
        if proof:
            answers[q["id"]] = proof
    if len(answers) != len(data["questions"]):
        print("missing proofs, not submitting:", json.dumps(answers))
        sys.exit(2)
    res = common.gm({"op": "submit", "challenge": cid, "answers": answers})
    (RUN / "submit.json").write_text(json.dumps(res, indent=2))
    print(json.dumps({k: res.get(k) for k in
                      ("result", "remaining", "accepted_ids", "flag")}, indent=2))


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cmd", choices=["start", "status", "solve"])
    ap.add_argument("challenge")
    ap.add_argument("--run-for-time", action="store_true")
    a = ap.parse_args()
    if a.cmd == "start":
        d = start(a.challenge, a.run_for_time)
        print(json.dumps({"result": d.get("result"), "deadline": d.get("deadline"),
                          "targets": d.get("targets")}, indent=2))
    elif a.cmd == "status":
        print(json.dumps(common.gm(op="status", challenge=a.challenge), indent=2))
    else:
        solve(a.challenge)


if __name__ == "__main__":
    main()
